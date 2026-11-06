use std::{ffi::OsString, fs::{FileType as StdFileType, Metadata}, io, os::unix::fs::{FileTypeExt, MetadataExt}, time::{Duration, SystemTime, UNIX_EPOCH}};

use fuser::{FileAttr, FileType, INodeNo, TimeOrNow};

pub fn sec_nsec_to_systime(secs: i64, nsec: i64) -> SystemTime {
    let nsec_dur = Duration::new(0, nsec as u32);
    if secs >= 0 {
        UNIX_EPOCH + Duration::from_secs(secs as u64) + nsec_dur
    } else {
        UNIX_EPOCH - Duration::from_secs(secs.unsigned_abs()) + nsec_dur
    }
}


pub fn type_from_std_type(ft: StdFileType) -> FileType {
    if ft.is_dir() { FileType::Directory }
    else if ft.is_symlink() { FileType::Symlink }
    else if ft.is_fifo() { FileType::NamedPipe }
    else if ft.is_socket() { FileType::Socket }
    else if ft.is_char_device() { FileType::CharDevice }
    else if ft.is_block_device() { FileType::BlockDevice }
    else { FileType::RegularFile }
}


fn resolve_meta_kind(meta: &Metadata) -> FileType {
    type_from_std_type(meta.file_type())
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
        perm: (metadata.mode() & 0o7777) as u16,
        nlink: metadata.nlink() as u32,
        uid: metadata.uid(),
        gid: metadata.gid(),
        rdev: metadata.rdev() as u32,
        blksize: metadata.blksize() as u32,
        flags: 0
    })
}

pub fn new_dir_entries(cur_ino: INodeNo, parent_ino: INodeNo) -> Vec<(INodeNo, FileType, OsString)> {
    vec![
        (cur_ino, FileType::Directory, OsString::from(".")),
        (parent_ino, FileType::Directory, OsString::from("..")),
    ]
}

pub fn c_ret_to_io_res(ret: i32) -> io::Result<()> {
    if ret == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}


pub fn to_timespec(t: Option<TimeOrNow>) -> libc::timespec {
    match t {
        None => libc::timespec { tv_sec: 0, tv_nsec: libc::UTIME_OMIT },
        Some(TimeOrNow::Now) => libc::timespec { tv_sec: 0, tv_nsec: libc::UTIME_NOW },
        Some(TimeOrNow::SpecificTime(st)) => {
            let d = st.duration_since(UNIX_EPOCH).unwrap_or_default();
            libc::timespec { tv_sec: d.as_secs() as i64, tv_nsec: d.subsec_nanos() as _ }
        }
    }
}
