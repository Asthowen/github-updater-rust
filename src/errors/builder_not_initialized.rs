#[derive(Debug, Clone)]
pub struct BuilderNotInitialized;

impl std::fmt::Display for BuilderNotInitialized {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "You must call the build method on the builder to use it."
        )
    }
}

impl From<reqwest::Error> for BuilderNotInitialized {
    fn from(error: reqwest::Error) -> Self {
        print!("reqwest Error: {}", error);
        BuilderNotInitialized
    }
}
