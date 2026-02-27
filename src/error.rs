use thiserror::Error;

#[derive(Debug, Error)]
pub enum GithubUpdaterError {
    #[error("Missing builder field attribute: {0}")]
    MissingBuilderField(&'static str),
    #[error("Failed to fetch resource: {0}")]
    FetchFailed(&'static str),
    #[error("Unexpected HTTP status {status} for {url}")]
    UnexpectedStatus {
        status: reqwest::StatusCode,
        url: String,
    },
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
