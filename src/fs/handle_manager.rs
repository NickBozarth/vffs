use std::{collections::HashMap, fs::File, path::{Path, PathBuf}, sync::{Arc}};

use fuser::FileHandle;

use crate::fs::errors::{ManagerError, ManagerResult};


pub struct OpenFile {
    file: Arc<File>,
    path: PathBuf
}

pub struct HandleManager {
    open_handles: HashMap<FileHandle, OpenFile>,
    next_fh: u64,
}

impl HandleManager {
    pub fn new() -> Self {
        Self {
            open_handles: HashMap::new(),
            next_fh: 1
        }
    }

    pub fn register_handle<P: AsRef<Path>>(&mut self, file: File, path: P) -> FileHandle {
        let fh = FileHandle(self.next_fh);
        self.next_fh += 1;

        self.open_handles
            .insert(fh, OpenFile {
                file: Arc::new(file),
                path: path.as_ref().to_path_buf()
            });
        fh
    }

    pub fn get_file(&self, fh: FileHandle) -> Option<Arc<File>> {
        self.open_handles
            .get(&fh)
            .map(|open_file| open_file.file.clone())
    }

    pub fn path_of(&self, fh: FileHandle) -> Option<PathBuf> {
        self.open_handles
            .get(&fh)
            .map(|open_file| open_file.path.clone())
    }

    pub fn release(&mut self, fh: FileHandle) {
        self.open_handles
            .remove(&fh);
    }

    pub fn fsync(&mut self, fh: FileHandle, datasync: bool) -> ManagerResult<()> {
        if let Some(ofile) = self.open_handles.get(&fh) {
            if datasync {
                ofile.file.sync_data().map_err(ManagerError::FsyncError)
            } else {
                ofile.file.sync_all().map_err(ManagerError::FsyncError)
            }
        } else {
            Ok(())
        }
    }
}
