use reqwest::header::ToStrError;
use std::num::ParseIntError;

#[derive(Debug)]
pub enum GithubUpdaterError {
    BuilderNotInitialized,
    BuilderMissingField(&'static str),
    FetchError(String),
    IoError(std::io::Error),
    ToStrError(ToStrError),
    ParseIntError(ParseIntError),
    ReqwestError(reqwest::Error),
}

impl std::fmt::Display for GithubUpdaterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BuilderNotInitialized => write!(f, "Builder not initialized"),
            Self::BuilderMissingField(field) => write!(f, "Missing required field: {field}"),
            Self::FetchError(message) => write!(f, "Fetch error: {message}"),
            Self::IoError(error) => write!(f, "IO error: {error}"),
            Self::ToStrError(error) => write!(f, "ToStr error: {error}"),
            Self::ParseIntError(error) => write!(f, "Parse int error: {error}"),
            Self::ReqwestError(error) => write!(f, "Reqwest error: {error}"),
        }
    }
}

impl std::error::Error for GithubUpdaterError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::IoError(error) => Some(error),
            Self::ToStrError(error) => Some(error),
            Self::ParseIntError(error) => Some(error),
            Self::ReqwestError(error) => Some(error),
            _ => None,
        }
    }
}

impl From<reqwest::Error> for GithubUpdaterError {
    fn from(error: reqwest::Error) -> Self {
        Self::ReqwestError(error)
    }
}

impl From<std::io::Error> for GithubUpdaterError {
    fn from(error: std::io::Error) -> Self {
        Self::IoError(error)
    }
}

impl From<ToStrError> for GithubUpdaterError {
    fn from(error: ToStrError) -> Self {
        Self::ToStrError(error)
    }
}

impl From<ParseIntError> for GithubUpdaterError {
    fn from(error: ParseIntError) -> Self {
        Self::ParseIntError(error)
    }
}
