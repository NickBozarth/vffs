use std::{ffi::OsStr, io, path::{Path, PathBuf}, time::Duration};

use fuser::{BackgroundSession, Config, Errno, FileHandle, Filesystem, Generation, INodeNo, ReplyAttr, ReplyDirectory, ReplyEntry, Request};

use crate::fs::mapper::Mapper;

pub struct FaultInjectionFileSystem {
    mountpoint: PathBuf,
    ttl: Duration,
    fs_mapper: Mapper
}

impl FaultInjectionFileSystem {
    pub fn new<P: AsRef<Path>>(source_dir: P, mountpoint: P, timeout: Duration) -> Self {
        Self {
            mountpoint: mountpoint.as_ref().to_path_buf(),
            ttl: timeout,
            fs_mapper: Mapper::new(source_dir.as_ref().to_path_buf())
        }
    }

    pub fn spawn_mount(self) -> io::Result<BackgroundSession> {
        let mut options = Config::default();
        options.mount_options.extend_from_slice(&vec![
            fuser::MountOption::RW,
            fuser::MountOption::FSName("vffs".to_string())
        ]);

        let mountpoint = self.mountpoint.clone();
        fuser::spawn_mount(self, &mountpoint, &options)
    }
}

impl Filesystem for FaultInjectionFileSystem {
    fn getattr(
        &self,
        _req: &Request,
        ino: INodeNo,
        _fh: Option<FileHandle>,
        reply: ReplyAttr
    ) {
        match self.fs_mapper.getattr(ino) {
            Ok(attr) => reply.attr(&self.ttl, &attr),
            Err(_) => reply.error(Errno::ENOENT),
        }
    }

    fn lookup(
        &self,
        _req: &Request,
        parent: INodeNo,
        name: &OsStr,
        reply: ReplyEntry
    ) {
        match self.fs_mapper.lookup(parent, name) {
            Ok(attr) => reply.entry(&self.ttl, &attr, Generation(0)),
            Err(_) => reply.error(Errno::ENOENT),
        }
    }

    fn readdir(
        &self,
        _req: &Request,
        ino: INodeNo,
        _fh: FileHandle,
        offset: u64,
        mut reply: ReplyDirectory,
    )
    {
        match self.fs_mapper.readdir(ino, offset, &mut reply) {
            Ok(()) => reply.ok(),
            Err(_) => reply.error(Errno::ENOENT),
        }
    }
}
