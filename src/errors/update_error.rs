use crate::errors::builder_not_initialized::BuilderNotInitialized;

#[derive(Debug, Clone)]
pub struct UpdateError(pub String);

impl std::fmt::Display for UpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<reqwest::Error> for UpdateError {
    fn from(error: reqwest::Error) -> Self {
        UpdateError(format!("reqwest Error: {}", error))
    }
}

impl From<std::io::Error> for UpdateError {
    fn from(error: std::io::Error) -> Self {
        UpdateError(format!("Error: {}", error))
    }
}

impl From<BuilderNotInitialized> for UpdateError {
    fn from(_: BuilderNotInitialized) -> Self {
        UpdateError("You must call the build method on the builder to use it.".to_string())
    }
}
