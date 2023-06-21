use crate::errors::builder_not_initialized::BuilderNotInitialized;
use crate::errors::update_error::UpdateError;
use errors::builder_missing_element::BuilderMissingElement;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

pub mod errors;

#[derive(Debug, Deserialize)]
struct Release {
    assets: Vec<Asset>,
    name: String,
}

#[derive(Debug, Deserialize)]
struct Asset {
    url: String,
    browser_download_url: String,
}

pub struct GithubUpdater {
    reqwest_client: Option<reqwest::Client>,
    builded: bool,
    pattern: Option<String>,
    app_name: Option<String>,
    github_token: Option<String>,
    rust_target: Option<String>,
    repository_infos: Option<(String, String)>,
    download_path: Option<PathBuf>,
    file_extension: Option<String>,
    release_url: Option<String>,
    app_version: Option<String>,
    need_refresh: bool,
}

impl GithubUpdater {
    pub fn builder() -> Self {
        Self {
            reqwest_client: None,
            builded: false,
            pattern: None,
            app_name: None,
            github_token: None,
            rust_target: None,
            repository_infos: None,
            download_path: None,
            file_extension: None,
            release_url: None,
            app_version: None,
            need_refresh: true,
        }
    }

    pub fn with_reqwest_client(mut self, reqwest_client: reqwest::Client) -> Self {
        self.reqwest_client = Some(reqwest_client);

        self
    }

    pub fn with_initialized_reqwest_client(mut self) -> Self {
        self.reqwest_client = Some(reqwest::Client::new());

        self
    }

    pub fn with_release_file_name_pattern<S: Into<String>>(mut self, pattern: S) -> Self {
        self.pattern = Some(pattern.into());

        self
    }

    pub fn with_app_name<S: Into<String>>(mut self, app_name: S) -> Self {
        self.app_name = Some(app_name.into());

        self
    }

    pub fn with_github_token<S: Into<String>>(mut self, github_token: S) -> Self {
        self.github_token = Some(github_token.into());

        self
    }

    pub fn with_rust_target<S: Into<String>>(mut self, rust_taget: S) -> Self {
        self.rust_target = Some(rust_taget.into());

        self
    }

    pub fn with_repository_infos<S: Into<String>>(
        mut self,
        repository_owner: S,
        repository_name: S,
    ) -> Self {
        self.repository_infos = Some((repository_owner.into(), repository_name.into()));

        self
    }

    pub fn with_download_path<P: AsRef<Path>>(mut self, path: &P) -> Self {
        self.download_path = Some(path.as_ref().to_owned());

        self
    }

    pub fn with_file_extension<S: Into<String>>(mut self, extension: S) -> Self {
        self.file_extension = Some(extension.into());

        self
    }

    pub fn build(mut self) -> Result<Self, BuilderMissingElement> {
        if self.reqwest_client.is_none() {
            return Err(BuilderMissingElement("reqwest_client".to_owned()));
        }
        if self.app_name.is_none() {
            return Err(BuilderMissingElement("app_name".to_owned()));
        }
        if let Some(pattern) = &self.pattern {
            if pattern.contains("rust_target") && self.rust_target.is_none() {
                return Err(BuilderMissingElement("rust_target".to_owned()));
            }
        } else {
            return Err(BuilderMissingElement("pattern".to_owned()));
        }
        if self.repository_infos.is_none() {
            return Err(BuilderMissingElement("repository_infos".to_owned()));
        }
        if self.download_path.is_none() {
            return Err(BuilderMissingElement("download_path".to_owned()));
        }

        self.builded = true;

        Ok(self)
    }

    fn generate_file_name(&self, app_name: &String) -> String {
        let extension: String = self
            .file_extension
            .as_ref()
            .map_or_else(String::default, |ext| format!(".{}", ext));
        format!("{}{}", app_name, extension)
    }

    pub async fn fetch_last_release(&mut self) -> Result<(), UpdateError> {
        if !self.builded {
            return Err(BuilderNotInitialized.into());
        }

        let repository_infos = self.repository_infos.as_ref().unwrap();
        let url = format!(
            "https://api.github.com/repos/{}/{}/releases/latest",
            repository_infos.0, repository_infos.1
        );

        let mut build_request = self
            .reqwest_client
            .clone()
            .unwrap()
            .get(&url)
            .header("User-Agent", "GitHub-Updater")
            .header("Accept", "application/vnd.github.v3+json");
        if let Some(token) = &self.github_token {
            build_request = build_request.header("Authorization", format!("token {}", token));
        }

        let response = build_request.send().await?.json::<Release>().await?;
        let asset_urls: Vec<String> = response
            .assets
            .iter()
            .map(|asset| asset.browser_download_url.to_owned())
            .collect();

        let mut pattern = self.pattern.clone().unwrap();
        if let Some(app_name) = &self.app_name {
            pattern = pattern.replace("{app_name}", app_name);
        }
        if let Some(rust_target) = &self.rust_target {
            pattern = pattern.replace("{rust_target}", rust_target);
        }
        pattern = pattern.replace("{app_version}", &response.name);
        self.app_version = Some(response.name);

        let matching_value = asset_urls.iter().find(|&value| value.contains(&pattern));
        if let Some(value) = matching_value {
            let api_url: &String = match response
                .assets
                .iter()
                .find(|asset| &asset.browser_download_url == value)
            {
                Some(asset) => &asset.url,
                None => {
                    return Err(UpdateError(
                        "An error occurred while retrieving the release URL.".to_owned(),
                    ));
                }
            };

            self.release_url = Some(api_url).cloned();
        } else {
            return Err(UpdateError(
                "No URL matching the pattern entered was found.".to_owned(),
            ));
        }

        Ok(())
    }

    async fn check_if_update_is_needed(&mut self) -> Result<bool, UpdateError> {
        if !self.builded {
            return Err(BuilderNotInitialized.into());
        }

        let path = self.download_path.as_ref().ok_or(BuilderNotInitialized)?;
        let current_version = self.app_version.as_ref().ok_or(BuilderNotInitialized)?;
        let app_name = self.app_name.as_ref().ok_or(BuilderNotInitialized)?;
        let path_version_file = path.join(format!("binary-version-{}.txt", app_name));

        if !path_version_file.exists() || !path.join(self.generate_file_name(app_name)).exists() {
            return Ok(true);
        }

        let previous_version: String = tokio::fs::read_to_string(&path_version_file).await?;

        Ok(previous_version.trim() != current_version)
    }

    pub async fn force_update(&mut self) -> Result<(), UpdateError> {
        if !self.builded {
            return Err(BuilderNotInitialized.into());
        }

        if self.need_refresh {
            self.fetch_last_release().await?;
        }

        let app_name = self.app_name.as_ref().ok_or(BuilderNotInitialized)?;
        let path = self.download_path.as_ref().ok_or(BuilderNotInitialized)?;
        let binary_path = path.join(app_name);
        let release_url = self.release_url.as_ref().ok_or(UpdateError(
            "An error occurred while retrieving the release URL.".to_owned(),
        ))?;

        if path.exists() {
            if binary_path.exists() {
                tokio::fs::remove_file(binary_path).await?;
            }
        } else {
            tokio::fs::create_dir_all(path).await?;
        }

        let mut build_request = self
            .reqwest_client
            .clone()
            .unwrap()
            .get(release_url)
            .header("User-Agent", "GitHub-Updater")
            .header("Accept", "application/octet-stream");
        if let Some(token) = &self.github_token {
            build_request = build_request.header("Authorization", format!("token {}", token));
        }

        let response = build_request.send().await?;
        if response.status().is_success() {
            let mut file: File =
                File::create(&path.join(self.generate_file_name(app_name))).await?;
            let body = response.bytes().await?;
            file.write_all(&body).await?;

            // Write version in file
            if let Some(app_version) = &self.app_version {
                let mut file =
                    File::create(path.join(format!("binary-version-{}.txt", app_name))).await?;
                file.write_all(app_version.as_bytes()).await?;
            }
        } else {
            return Err(UpdateError(format!(
                "An error occurred while downloading the file, HTTP code: {}",
                response.status()
            )));
        }

        Ok(())
    }

    pub async fn update_if_needed(&mut self) -> Result<(), UpdateError> {
        if !self.builded {
            return Err(BuilderNotInitialized.into());
        }

        self.fetch_last_release().await?;
        self.need_refresh = false;

        if self.check_if_update_is_needed().await.unwrap_or(false) {
            return self.force_update().await;
        }

        Ok(())
    }
}
