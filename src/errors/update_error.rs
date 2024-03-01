use crate::errors::builder_not_initialized::BuilderNotInitialized;
use reqwest::header::ToStrError;
use std::num::ParseIntError;

#[derive(Debug, Clone)]
pub struct UpdateError(pub String);

impl std::fmt::Display for UpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<reqwest::Error> for UpdateError {
    fn from(error: reqwest::Error) -> Self {
        UpdateError(format!("A reqwest error has occurred: {}", error))
    }
}

impl From<std::io::Error> for UpdateError {
    fn from(error: std::io::Error) -> Self {
        UpdateError(format!("A std io error has occurred: {}", error))
    }
}

impl From<ToStrError> for UpdateError {
    fn from(error: ToStrError) -> Self {
        UpdateError(format!(
            "A error has occurred when converting header to str: {}",
            error
        ))
    }
}

impl From<ParseIntError> for UpdateError {
    fn from(error: ParseIntError) -> Self {
        UpdateError(format!(
            "A error has occurred when converting string to integer: {}",
            error
        ))
    }
}

impl From<BuilderNotInitialized> for UpdateError {
    fn from(_: BuilderNotInitialized) -> Self {
        UpdateError("You must call the build method on the builder to use it.".to_owned())
    }
}
