use std::{collections::HashMap, ffi::OsStr, fs::{self, Metadata}, os::unix::fs::{DirEntryExt, MetadataExt}, path::{Path, PathBuf}, sync::RwLock};

use fuser::{FileAttr, INodeNo, ReplyDirectory};

use crate::fs::{errors::{MapperError, MapperResult}, utils::{metadata_to_attr, new_dir_entries, resolve_meta_kind}};

pub struct Mapper {
    ino_to_source: RwLock<HashMap<INodeNo, PathBuf>>
}

impl Mapper {
    pub fn new<P: AsRef<Path>>(source_dir: P) -> Self {
        let mut ino_to_source_map = HashMap::new();
        ino_to_source_map.insert(
            INodeNo(1),
            source_dir.as_ref().to_path_buf()
        );

        Self {
            ino_to_source: RwLock::new(ino_to_source_map)
        }
    }

    fn ino_to_source_path(&self, ino: INodeNo) -> MapperResult<PathBuf> {
        self.ino_to_source
            .read()
            .map_err(|_| MapperError::PoisonedMutex)?
            .get(&ino)
            .map(|path| path.clone())
            .ok_or(MapperError::UnregisteredINodeNo)
    }

    fn path_to_meta<P: AsRef<Path>>(&self, path: P) -> MapperResult<Metadata> {
        fs::symlink_metadata(path)
            .map_err(MapperError::MetadataFetchError)
    }

    fn meta_from_name(&self, parent: INodeNo, name: &OsStr) -> MapperResult<Metadata> {
        let parent_path = self.ino_to_source_path(parent)?;
        let path = parent_path.join(name);
        let meta = self.path_to_meta(&path)?;

        self.ino_to_source
            .write()
            .map_err(|_| MapperError::PoisonedMutex)?
            .entry(INodeNo(meta.ino()))
            .or_insert(path);

        Ok(meta)
    }




    pub fn getattr(&self, ino: INodeNo) -> MapperResult<FileAttr> {
        let path = self.ino_to_source_path(ino)?;
        let meta = self.path_to_meta(&path)?;
        let mut attr = metadata_to_attr(&meta)
            .map_err(MapperError::MetadataParseError)?;

        // Guarantee that attr has the ino that was requested
        attr.ino = ino;
        Ok(attr)
    }

    pub fn lookup(&self, parent: INodeNo, name: &OsStr) -> MapperResult<FileAttr> {
        let meta = self.meta_from_name(parent, name)?;
        metadata_to_attr(&meta)
            .map_err(MapperError::MetadataParseError)
    }

    // TODO this can be improved
    pub fn readdir(
        &self,
        ino: INodeNo,
        offset: u64,
        reply: &mut ReplyDirectory
    ) -> MapperResult<()> {
        let path = self.ino_to_source_path(ino)?;
        let dir = fs::read_dir(path)
            .map_err(MapperError::ReadDirError)?;
        let mut dir_entries = new_dir_entries(ino);

        for entry in dir.flatten() {
            if let Ok(meta) = self.path_to_meta(entry.path()) {
                dir_entries.push((
                    INodeNo(entry.ino()),
                    resolve_meta_kind(&meta),
                    entry.file_name()
                ))
            };
        }

        // Return only the requested entries
        for (i, (ino, kind, name)) in dir_entries.into_iter().enumerate().skip(offset as usize) {
            let cur_offset = (i + 1) as u64;
            if reply.add(ino, cur_offset, kind, name) {
                break;
            }
        }

        Ok(())
    }
}
