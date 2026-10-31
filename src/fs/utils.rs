use std::{ffi::OsString, fs::Metadata, io, os::unix::fs::MetadataExt, time::{Duration, SystemTime, UNIX_EPOCH}};

use fuser::{FileAttr, FileType, INodeNo};


fn sec_nsec_to_systime(secs: i64, nsec: i64) -> SystemTime {
    if secs >= 0 {
        UNIX_EPOCH + Duration::new(secs as u64, nsec as u32)
    } else {
        UNIX_EPOCH - Duration::new((-secs) as u64, nsec as u32)
    }
}

pub fn resolve_meta_kind(metadata: &Metadata) -> FileType {
    match metadata.file_type() {
        f if f.is_dir() => FileType::Directory,
        f if f.is_symlink() => FileType::Symlink,
        _ => FileType::RegularFile
    }
}


pub fn metadata_to_attr(metadata: &Metadata) -> io::Result<FileAttr> {
    let crtime = metadata.created().unwrap_or_else(|_| {
        sec_nsec_to_systime(metadata.mtime(), metadata.mtime_nsec())
    });

    Ok(FileAttr {
        ino: INodeNo(metadata.ino()),
        size: metadata.size(),
        blocks: metadata.blocks(),
        atime: sec_nsec_to_systime(metadata.atime(), metadata.atime_nsec()),
        mtime: sec_nsec_to_systime(metadata.mtime(), metadata.mtime_nsec()),
        ctime: sec_nsec_to_systime(metadata.ctime(), metadata.ctime_nsec()),
        crtime,
        kind: resolve_meta_kind(metadata),
        perm: metadata.mode() as u16,
        nlink: metadata.nlink() as u32,
        uid: metadata.uid(),
        gid: metadata.gid(),
        rdev: metadata.rdev() as u32,
        blksize: metadata.blksize() as u32,
        flags: 0
    })
}

pub fn new_dir_entries(cur_ino: INodeNo) -> Vec<(INodeNo, FileType, OsString)> {
    vec![
        (cur_ino, FileType::Directory, OsString::from(".")),
        (cur_ino, FileType::Directory, OsString::from("..")),
    ]
}
