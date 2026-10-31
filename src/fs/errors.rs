use thiserror::Error;

#[derive(Error, Debug)]
pub enum MapperError {
    #[error("Error fetching metadata: {0}")]
    MetadataFetchError(std::io::Error),
    #[error("Error parsing metadata: {0}")]
    MetadataParseError(std::io::Error),
    #[error("Attempted to access paths with poisoned mutex")]
    PoisonedMutex,
    #[error("Failed to read dir: {0}")]
    ReadDirError(std::io::Error),
    #[error("Attempted to access path of unregistered INodeNo")]
    UnregisteredINodeNo,
}

pub type MapperResult<T> = Result<T, MapperError>;
