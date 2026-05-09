use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Release {
    pub assets: Vec<Asset>,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct Asset {
    pub digest: Option<String>,
    pub name: String,
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct Tag {
    pub name: String,
}
