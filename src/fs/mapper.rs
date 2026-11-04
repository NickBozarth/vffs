use std::{ffi::{CString, OsStr, OsString}, fs::{self, Metadata, OpenOptions}, os::unix::{ffi::OsStrExt, fs::{DirEntryExt, PermissionsExt}}, path::{Path, PathBuf}, sync::{RwLock, RwLockReadGuard, RwLockWriteGuard}};

use fuser::{FileAttr, FileHandle, FileType, INodeNo, TimeOrNow};

use crate::fs::{errors::{MapperError, MapperResult}, handle_manager::HandleManager, inode_table::INodeTable, inode_table::ROOT, utils::{c_ret_to_io_res, metadata_to_attr, new_dir_entries, to_timespec, type_from_std_type}};


pub struct SetAttrArgs {
    pub mode: Option<u32>,
    pub uid: Option<u32>,
    pub gid: Option<u32>,
    pub size: Option<u64>,
    pub atime: Option<TimeOrNow>,
    pub mtime: Option<TimeOrNow>,
    pub fh: Option<FileHandle>
}


pub struct Mapper {
    table: RwLock<INodeTable>,
    handles: RwLock<HandleManager>
}

impl Mapper {
    pub fn new<P: AsRef<Path>>(source_dir: P) -> Self {
        Self {
            table: RwLock::new(INodeTable::new(source_dir.as_ref().to_path_buf())),
            handles: RwLock::new(HandleManager::new())
        }
    }

    fn table_read(&self) -> MapperResult<RwLockReadGuard<'_, INodeTable>> {
        self.table
            .read()
            .map_err(|_| MapperError::PoisonError)
    }

    fn table_write(&self) -> MapperResult<RwLockWriteGuard<'_, INodeTable>> {
        self.table
            .write()
            .map_err(|_| MapperError::PoisonError)
    }

    fn handles_read(&self) -> MapperResult<RwLockReadGuard<'_, HandleManager>> {
        self.handles
            .read()
            .map_err(|_| MapperError::PoisonError)
    }

    fn handles_write(&self) -> MapperResult<RwLockWriteGuard<'_, HandleManager>> {
        self.handles
            .write()
            .map_err(|_| MapperError::PoisonError)
    }

    // Wrapper for accessing path_of on self.table
    fn path_of(&self, ino: INodeNo) -> MapperResult<PathBuf> {
        self.table_read()?
            .path_of(ino)
            .map(Path::to_path_buf)
            .ok_or(MapperError::UnregisteredINodeNo)
    }

    // Retrieve attr from meta
    // We ensure that the requested ino is always returned
    fn attr(&self, ino: INodeNo, meta: &Metadata) -> MapperResult<FileAttr> {
        let mut attr = metadata_to_attr(meta)
            .map_err(MapperError::MetadataParseError)?;
        attr.ino = ino;
        Ok(attr)
    }

    // Retrieves attr from a given parent INodeNo and name
    // NOTE self.table registers ino when table.lookup is called
    pub fn lookup(&self, parent: INodeNo, name: &OsStr) -> MapperResult<FileAttr> {
        let path = self.path_of(parent)?.join(name);
        let meta = fs::symlink_metadata(&path)
            .map_err(MapperError::MetadataFetchError)?;
        let ino = self.table_write()?
            .lookup(path);
        self.attr(ino, &meta)
    }

    pub fn forget(&self, ino: INodeNo, nlookup: u64) -> MapperResult<()> {
        self.table_write()
            .map_err(|_| MapperError::PoisonError)?
            .forget(ino, nlookup);

        Ok(())
    }

    pub fn getattr(&self, ino: INodeNo, fh: Option<FileHandle>) -> MapperResult<FileAttr> {
        let file_opt = fh.and_then(|fh| {
            self.handles_read()
                .ok()?
                .get_file(fh)
        });

        let meta = match file_opt {
            Some(file) => file.metadata(),
            None => fs::symlink_metadata(self.path_of(ino)?)
        }
        .map_err(MapperError::MetadataFetchError)?;

        self.attr(ino, &meta)
    }

    fn list_dir(&self, ino: INodeNo) -> MapperResult<Vec<(INodeNo, FileType, OsString)>> {
        let path = self.path_of(ino)?;
        let table = self.table_read()?;

        let parent_ino = if ino == ROOT {
            ROOT
        } else {
            path.parent()
                .and_then(|p| {
                    self.table_read()
                        .ok()?
                        .by_path(p)
                })
                .unwrap_or(ROOT)
        };

        let mut entries = new_dir_entries(ino, parent_ino);
        for entry in fs::read_dir(path).map_err(MapperError::ReadDirError)? {
            let entry = entry.map_err(MapperError::ReadDirError)?;
            let Ok(ft) = entry.file_type() else { continue };

            let child_ino = table.by_path(entry.path())
                .unwrap_or(INodeNo(entry.ino()));

            entries.push((
                child_ino,
                type_from_std_type(ft),
                entry.file_name()
            ));
        }

        Ok(entries)
    }

    pub fn setattr(&self, ino: INodeNo, args: SetAttrArgs) -> MapperResult<FileAttr> {
        let path = self.path_of(ino)?;
        let open_file = args.fh
            .and_then(|fh| {
                self.handles_read().ok()?.get_file(fh)
            });

        if let Some(mode) = args.mode {
            fs::set_permissions(&path, fs::Permissions::from_mode(mode))
                .map_err(MapperError::Io)?;
        }

        if args.uid.is_some() || args.gid.is_some() {
            std::os::unix::fs::lchown(&path, args.uid, args.gid).map_err(MapperError::Io)?;
        }
        if let Some(size) = args.size {
            match &open_file {
                Some(f) => f.set_len(size),
                None => {
                    OpenOptions::new()
                        .write(true)
                        .open(&path)
                        .and_then(|f| f.set_len(size))
                },
            }
            .map_err(MapperError::Io)?;
        }

        if args.atime.is_some() || args.mtime.is_some() {
            let c = CString::new(path.as_os_str().as_bytes())
                .map_err(|_| MapperError::InvalidFlags)?;
            let times = [to_timespec(args.atime), to_timespec(args.mtime)];
            c_ret_to_io_res(unsafe {
                libc::utimensat(
                    libc::AT_FDCWD,
                    c.as_ptr(),
                    times.as_ptr(),
                    libc::AT_SYMLINK_NOFOLLOW
                )
            })
            .map_err(MapperError::Io)?;
        }

        self.getattr(ino, args.fh)
    }
}
