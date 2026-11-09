use std::io;

use fuser::{Errno, FileHandle};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MapperError {
    #[error("Provided file was not registered in mapper")]
    FileNotFound,
    #[error("Invalid flags provided")]
    InvalidFlags,
    #[error("INodeNo not found")]
    INodeNoFetchError,
    #[error("Io error: {0}")]
    Io(std::io::Error),
    #[error("Error when interfacing with HandleManager: {0}")]
    ManagerError(ManagerError),
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
    pub fn errno(&self) -> Errno {
        match self {
            Self::InvalidFlags => Errno::EINVAL,
            Self::INodeNoFetchError => Errno::ENOENT,
            Self::UnregisteredINodeNo
            | Self::FileNotFound => Errno::EBADF,
            Self::PoisonError => Errno::EIO,
            Self::ManagerError(e) => e.errno(),
            Self::MetadataFetchError(e)
            | Self::MetadataParseError(e)
            | Self::OpenError(e)
            | Self::Io(e)
            | Self::ReadDirError(e) => io_errno(e),
        }
    }
}


#[derive(Debug, Error)]
pub enum ManagerError {
    #[error("Error on fsync call: {0}")]
    FsyncError(std::io::Error),
}

pub type ManagerResult<T> = Result<T, ManagerError>;

impl ManagerError {
    pub fn errno(&self) -> Errno {
        match self {
            Self::FsyncError(e) => io_errno(e)
        }
    }
}
