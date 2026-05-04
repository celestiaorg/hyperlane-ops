use std::path::PathBuf;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, IgpOracleError>;

#[derive(Debug, Error)]
pub enum IgpOracleError {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse YAML at {path}: {source}")]
    Yaml {
        path: PathBuf,
        #[source]
        source: serde_yaml::Error,
    },

    #[error("failed to write JSON artifact at {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("invalid config: {0}")]
    InvalidConfig(String),

    #[error("registry error: {0}")]
    Registry(String),

    #[error("invalid target selection: {0}")]
    InvalidTarget(String),

    #[error("data source error: {0}")]
    DataSource(String),

    #[error("policy error: {0}")]
    Policy(String),

    #[error("unsupported protocol: {0}")]
    UnsupportedProtocol(String),

    #[error("live on-chain reads are not implemented in stage one: {0}")]
    UnsupportedLiveRead(String),

    #[error("write mode is not implemented in stage one")]
    UnsupportedWrite,
}

impl IgpOracleError {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::InvalidConfig(_)
            | Self::Registry(_)
            | Self::Io { .. }
            | Self::Yaml { .. }
            | Self::Json { .. }
            | Self::DataSource(_) => 20,
            Self::InvalidTarget(_) | Self::UnsupportedProtocol(_) | Self::Policy(_) => 30,
            Self::UnsupportedWrite => 40,
            Self::UnsupportedLiveRead(_) => 30,
        }
    }
}

pub fn read_to_string(path: impl Into<PathBuf>) -> Result<String> {
    let path = path.into();
    std::fs::read_to_string(&path).map_err(|source| IgpOracleError::Io { path, source })
}

pub fn create_dir_all(path: impl Into<PathBuf>) -> Result<()> {
    let path = path.into();
    std::fs::create_dir_all(&path).map_err(|source| IgpOracleError::Io { path, source })
}

pub fn write(path: impl Into<PathBuf>, contents: impl AsRef<[u8]>) -> Result<()> {
    let path = path.into();
    std::fs::write(&path, contents).map_err(|source| IgpOracleError::Io { path, source })
}
