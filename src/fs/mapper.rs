use std::{cell::LazyCell, ffi::{CString, OsStr, OsString}, fs::{self, Metadata, OpenOptions}, io, os::unix::{ffi::OsStrExt, fs::{DirEntryExt, FileExt, OpenOptionsExt, PermissionsExt}}, path::{Path, PathBuf}, sync::{RwLock, RwLockReadGuard, RwLockWriteGuard}};

use fuser::{FileAttr, FileHandle, FileType, INodeNo, OpenAccMode, OpenFlags, TimeOrNow};

use crate::fs::{errors::{MapperError, MapperResult}, handle_manager::HandleManager, inode_table::{INodeTable, ROOT}, utils::{c_ret_to_io_res, metadata_to_attr, new_dir_entries, to_timespec, type_from_std_type}};


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

    // -----------------------------------------------------
    // RwLock accessors; These will lock if RwLock is in use
    // -----------------------------------------------------
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

    // Retrieves ino of parent of a given ino or path
    // Defaults to ROOT if not found
    fn get_parent_ino<P: AsRef<Path>>(&self, ino: INodeNo, path_opt: Option<P>) -> MapperResult<INodeNo> {
        let path = match path_opt {
            Some(path) => path.as_ref().to_path_buf(),
            None => self.path_of(ino)?
        };

        if ino == ROOT {
            Ok(ROOT)
        } else {
            let parent_ino = path.parent()
                .and_then(|p| {
                    self.table_read()
                        .ok()?
                        .by_path(p)
                })
                .unwrap_or(ROOT);

            Ok(parent_ino)
        }
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


    // Subtracts reference count on a given ino and cleans up table if count <= 0
    pub fn forget(&self, ino: INodeNo, nlookup: u64) -> MapperResult<()> {
        self.table_write()
            .map_err(|_| MapperError::PoisonError)?
            .forget(ino, nlookup);

        Ok(())
    }

    // Retrieves FileAttr from a given FileHandle (if provided) or ino
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

    // Retrieves a vec of identifiers within a directory
    pub fn list_dir(&self, ino: INodeNo) -> MapperResult<Vec<(INodeNo, FileType, OsString)>> {
        let path = self.path_of(ino)?;
        let table = self.table_read()?;

        let parent_ino = self.get_parent_ino(ino, Some(&path))?;

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

    // Update certain attributes of a file
    // Arguments given with a None variant are not updated
    pub fn setattr(&self, ino: INodeNo, args: SetAttrArgs) -> MapperResult<FileAttr> {
        let path = self.path_of(ino)?;
        let open_file = LazyCell::new(|| {
            args.fh
                .and_then(|fh| {
                    self.handles_read().ok()?.get_file(fh)
                })
        });

        if let Some(mode) = args.mode {
            fs::set_permissions(&path, fs::Permissions::from_mode(mode))
                .map_err(MapperError::Io)?;
        }

        if args.uid.is_some() || args.gid.is_some() {
            std::os::unix::fs::lchown(&path, args.uid, args.gid).map_err(MapperError::Io)?;
        }
        if let Some(size) = args.size {
            match &*open_file {
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

    // Opens a given file with given flags
    pub fn open(&self, ino: INodeNo, flags: OpenFlags) -> MapperResult<FileHandle> {
        let raw = flags.0;
        let mut options = OpenOptions::new();
        match flags.acc_mode() {
            OpenAccMode::O_RDONLY => { options.read(true); }
            OpenAccMode::O_WRONLY => { options.write(true); }
            OpenAccMode::O_RDWR => { options.read(true).write(true); }
        }
        if raw & libc::O_APPEND != 0 { options.append(true); }
        if raw & libc::O_TRUNC != 0 { options.truncate(true); }

        // Extract custom flags from raw flags by masking against pre-defined flags
        options.custom_flags(
            raw & !(
                libc::O_ACCMODE | libc::O_APPEND | libc::O_TRUNC | libc::O_CREAT
            ));

        let source = self.path_of(ino)?;
        let file = options.open(&source).map_err(MapperError::OpenError)?;
        Ok(self.handles_write()?.register_handle(file, source))
    }

    // Retrieves an owned slice of a requested size from a file
    pub fn read(&self, fh: FileHandle, offset: u64, size: u32) -> MapperResult<Box<[u8]>> {
        let file = self.handles_read()?.get_file(fh)
            .ok_or(MapperError::FileNotFound)?;

        let mut buf = vec![0u8; size as usize];
        let mut filled = 0;
        while filled < buf.len() {
            match file.read_at(&mut buf[filled..], offset + filled as u64) {
                Ok(0) => break,
                Ok(n) => filled += n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(err) => return Err(MapperError::Io(err))
            }
        }

        buf.truncate(filled);
        Ok(buf.into_boxed_slice())
    }

    // Writes a captured slice to a file
    pub fn write(&self, fh: FileHandle, offset: u64, data: &[u8]) -> MapperResult<u32> {
        let file = self.handles_read()?
            .get_file(fh)
            .ok_or(MapperError::FileNotFound)?;
        file.write_all_at(data, offset)
            .map_err(MapperError::Io);
        Ok(data.len() as u32)
    }

    // Release a given FileHandle
    // Usually called on close()
    pub fn release(&self, fh: FileHandle) -> MapperResult<()> {
        Ok(self.handles_write()?.release(fh))
    }

    // Flush modified data to disk
    pub fn fsync(&self, fh: FileHandle, datasync: bool) -> MapperResult<()> {
        self.handles_write()?
            .fsync(fh, datasync)
            .map_err(MapperError::ManagerError)
    }
}
