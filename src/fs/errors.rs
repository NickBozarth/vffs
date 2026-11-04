use std::io;

use fuser::Errno;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MapperError {
    #[error("Provided file was not registered in mapper")]
    FileNotFound,
    #[error("Invalid flags provided")]
    InvalidFlags,
    #[error("Io error: {0}")]
    Io(std::io::Error),
    #[error("Error fetching metadata: {0}")]
    MetadataFetchError(std::io::Error),
    #[error("Error parsing metadata: {0}")]
    MetadataParseError(std::io::Error),
    #[error("Error opening file: {0}")]
    OpenError(std::io::Error),
    #[error("Attempted to access a poisoned mutex")]
    PoisonError,
    #[error("Failed to read dir: {0}")]
    ReadDirError(std::io::Error),
    #[error("Attempted to access path of unregistered INodeNo")]
    UnregisteredINodeNo,
}

pub type MapperResult<T> = Result<T, MapperError>;

fn io_errno(e: &io::Error) -> Errno {
    Errno::from_i32(e.raw_os_error().unwrap_or(libc::EIO))
}

impl MapperError {
    /// The errno the kernel should see. Keeps EACCES, ENOTDIR, ENOSPC, etc. intact
    /// instead of everything becoming ENOENT.
    pub fn errno(&self) -> Errno {
        match self {
            Self::InvalidFlags => Errno::EINVAL,
            Self::UnregisteredINodeNo
            | Self::FileNotFound => Errno::EBADF,
            Self::PoisonError => Errno::EIO,
            Self::MetadataFetchError(e)
            | Self::MetadataParseError(e)
            | Self::OpenError(e)
            | Self::Io(e)
            | Self::ReadDirError(e) => io_errno(e),
        }
    }
}
