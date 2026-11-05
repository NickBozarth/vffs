use std::{ffi::OsStr, io, path::{Path, PathBuf}, time::{Duration, SystemTime}};

use fuser::{BackgroundSession, BsdFileFlags, Config, FileHandle, Filesystem, FopenFlags, Generation, INodeNo, LockOwner, OpenFlags, ReplyAttr, ReplyData, ReplyDirectory, ReplyEmpty, ReplyEntry, ReplyOpen, Request, TimeOrNow};

use crate::fs::{mapper::{Mapper, SetAttrArgs}};

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
    fn lookup(&self,
        _req: &Request,
        parent: INodeNo,
        name: &OsStr,
        reply: ReplyEntry
    ) {
        match self.fs_mapper.lookup(parent, name) {
            Ok(attr) => reply.entry(&self.ttl, &attr, Generation(0)),
            Err(err) => reply.error(err.errno()),
        }
    }

    fn forget(&self, _req: &Request, ino: INodeNo, nlookup: u64) {
        let _ = self.fs_mapper.forget(ino, nlookup);
    }

    fn getattr(
        &self,
        _req: &Request,
        ino: INodeNo,
        fh: Option<FileHandle>,
        reply: ReplyAttr
    ) {
        match self.fs_mapper.getattr(ino, fh) {
            Ok(attr) => reply.attr(&self.ttl, &attr),
            Err(err) => reply.error(err.errno()),
        }
    }

    fn setattr(
        &self,
        _req: &Request,
        ino: INodeNo,
        mode: Option<u32>,
        uid: Option<u32>,
        gid: Option<u32>,
        size: Option<u64>,
        atime: Option<TimeOrNow>,
        mtime: Option<TimeOrNow>,
        _ctime: Option<SystemTime>,
        fh: Option<FileHandle>,
        _crtime: Option<SystemTime>,
        _chgtime: Option<SystemTime>,
        _bkuptime: Option<SystemTime>,
        _flags: Option<BsdFileFlags>,
        reply: ReplyAttr,
    )
    {
        let args = SetAttrArgs { mode, uid, gid, size, atime, mtime, fh };
        match self.fs_mapper.setattr(ino, args) {
            Ok(attr) => reply.attr(&self.ttl, &attr),
            Err(err) => reply.error(err.errno()),
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
        match self.fs_mapper.list_dir(ino) {
            Ok(entries) => {
                let entries_iter = entries
                    .into_iter()
                    .enumerate()
                    .skip(offset as usize);
                for (i, (ino, kind, name)) in entries_iter {
                    if reply.add(ino, (i + 1) as u64, kind, name) {
                        break;
                    }
                }

                reply.ok();
            }
            Err(err) => reply.error(err.errno()),
        }
    }

    fn open(
        &self,
        _req: &Request,
        ino: INodeNo,
        flags: OpenFlags,
        reply: ReplyOpen
    ) {
        match self.fs_mapper.open(ino, flags) {
            Ok(fh) => reply.opened(fh, FopenFlags::empty()),
            Err(err) => reply.error(err.errno()),
        }
    }

    fn read(
        &self,
        _req: &Request,
        _ino: INodeNo,
        fh: FileHandle,
        offset: u64,
        size: u32,
        _flags: OpenFlags,
        _lock_owner: Option<LockOwner>,
        reply: ReplyData,
    )
    {
        match self.fs_mapper.read(fh, offset, size) {
            Ok(data) => reply.data(&data),
            Err(err) => reply.error(err.errno()),
        }
    }

    fn write(
        &self,
        _req: &Request,
        _ino: INodeNo,
        fh: FileHandle,
        offset: u64,
        data: &[u8],
        _write_flags: fuser::WriteFlags,
        _flags: OpenFlags,
        _lock_owner: Option<fuser::LockOwner>,
        reply: fuser::ReplyWrite,
    )
    {
        match self.fs_mapper.write(fh, offset, data) {
            Ok(n) => reply.written(n),
            Err(err) => reply.error(err.errno()),
        }
    }

    fn release(
        &self,
        _req: &Request,
        _ino: INodeNo,
        fh: FileHandle,
        _flags: OpenFlags,
        _lock_owner: Option<fuser::LockOwner>,
        _flush: bool,
        reply: fuser::ReplyEmpty,
    )
    {
        match self.fs_mapper.release(fh) {
            Ok(()) => reply.ok(),
            Err(err) => reply.error(err.errno()),
        }
    }

    fn fsync(
        &self,
        _req: &Request,
        _ino: INodeNo,
        fh: FileHandle,
        datasync: bool,
        reply: ReplyEmpty,
    )
    {
        match self.fs_mapper.fsync(fh, datasync) {
            Ok(()) => reply.ok(),
            Err(err) => reply.error(err.errno()),
        }
    }
}
