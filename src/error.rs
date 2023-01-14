use globset::Error as GlobError;
use std::ffi::OsString;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Error: {0}")]
    String(String),

    #[error("Warning: {0}")]
    Warning(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("OsString error: {0}")]
    OsString(String),

    #[error("Glob error: {0}")]
    Glob(#[from] GlobError),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),

    #[error("{0}")]
    Tera(#[from] tera::Error),

    #[error("StripPrefixError: {0}")]
    StripPrefixError(#[from] std::path::StripPrefixError),

    #[error("Toml Deserialize: {0}")]
    TomlDeserialize(#[from] toml::de::Error),
}

impl From<&str> for Error {
    fn from(s: &str) -> Self {
        Error::String(s.to_string())
    }
}

impl From<String> for Error {
    fn from(s: String) -> Self {
        Error::String(s)
    }
}

impl From<OsString> for Error {
    fn from(os_str: OsString) -> Error {
        Error::OsString(format!("{:?}", os_str))
    }
}
