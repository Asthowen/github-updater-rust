#[derive(Debug, Clone)]
pub struct BuilderMissingElement(pub String);

impl std::fmt::Display for BuilderMissingElement {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "You have not filled in all the required elements in the builder: {}",
            self.0
        )
    }
}
