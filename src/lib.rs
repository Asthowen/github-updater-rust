use crate::errors::builder_not_initialized::BuilderNotInitialized;
use crate::errors::update_error::UpdateError;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use errors::builder_missing_element::BuilderMissingElement;
use md5::Digest;
use serde::{Deserialize, Serialize};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub mod errors;

/// Download information struct.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DownloadInfos {
    /// The previous version installed. The value is null if the file was not present before.
    pub previous_version: Option<String>,
    /// The new downloaded version.
    pub new_version: String,
    /// To find out whether or not an update has been download.
    pub has_been_updated: bool,
    /// This shows whether or not the update has been forced.
    pub forced_update: bool,
}

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

#[derive(Debug, Clone)]
pub struct GithubUpdater {
    reqwest_client: Option<reqwest::Client>,
    built: bool,
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
    forced_update: bool,
}

impl GithubUpdater {
    pub fn builder() -> Self {
        Self {
            reqwest_client: None,
            built: false,
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
            forced_update: true,
        }
    }

    /// Sets a Reqwest client that has already been initialized.
    ///
    /// # Arguments
    ///
    /// * `reqwest_client` - The already initialized Reqwest client.
    ///
    /// # Returns
    ///
    /// The modified `GithubUpdater` builder instance.
    ///
    /// # Example
    ///
    /// ```rust
    /// use github_updater::GithubUpdater;
    ///
    /// let updater_builder = GithubUpdater::builder()
    ///     .with_app_name("afetch")
    ///     .build();
    /// ```
    pub fn with_reqwest_client(mut self, reqwest_client: reqwest::Client) -> Self {
        self.reqwest_client = Some(reqwest_client);

        self
    }

    /// Creation of a new Reqwest customer, without option activated.
    ///
    /// # Returns
    ///
    /// The modified `GithubUpdater` builder instance.
    ///
    /// # Example
    ///
    /// ```rust
    /// use github_updater::GithubUpdater;
    ///
    /// let updater_builder = GithubUpdater::builder()
    ///     .with_initialized_reqwest_client()
    ///     .build();
    /// ```
    pub fn with_initialized_reqwest_client(mut self) -> Self {
        self.reqwest_client = Some(reqwest::Client::new());

        self
    }

    /// Sets the filename pattern in GitHub releases.
    ///
    /// # Arguments
    ///
    /// * `pattern` - The file pattern, which can contain:
    ///    * `app_name`: The name of the application.
    ///    * `rust_target`: The Rust target, e.g.: i686-unknown-freebsd.
    ///    * `app_version`: The version of the application.
    ///
    /// # Returns
    ///
    /// The modified `GithubUpdater` builder instance.
    ///
    /// # Example
    ///
    /// ```rust
    /// use github_updater::GithubUpdater;
    ///
    /// let updater_builder = GithubUpdater::builder()
    ///     .with_release_file_name_pattern("{app_name}-{app_version}-{rust_target}")
    ///     .build();
    /// ```
    pub fn with_release_file_name_pattern<S: Into<String>>(mut self, pattern: S) -> Self {
        self.pattern = Some(pattern.into());

        self
    }

    /// Sets the application name which is used to define the name of the downloaded executable.
    ///
    /// # Arguments
    ///
    /// * `app_name` - The name of the application.
    ///
    /// # Returns
    ///
    /// The modified `GithubUpdater` builder instance.
    ///
    /// # Example
    ///
    /// ```rust
    /// use github_updater::GithubUpdater;
    ///
    /// let updater_builder = GithubUpdater::builder()
    ///     .with_app_name("afetch")
    ///     .build();
    /// ```
    pub fn with_app_name<S: Into<String>>(mut self, app_name: S) -> Self {
        self.app_name = Some(app_name.into());
        self
    }

    /// Sets the GitHub token which will be used to make requests to the GitHub API.
    ///
    /// # Arguments
    ///
    /// * `github_token` - The GitHub token to use for authentication.
    ///
    /// # Returns
    ///
    /// The modified `GithubUpdater` builder instance.
    ///
    /// # Example
    ///
    /// ```rust
    /// use github_updater::GithubUpdater;
    ///
    /// let updater_builder = GithubUpdater::builder()
    ///     .with_github_token("some")
    ///     .build();
    /// ```
    pub fn with_github_token<S: Into<String>>(mut self, github_token: S) -> Self {
        self.github_token = Some(github_token.into());

        self
    }

    /// Sets the rust target that will be searched for in GitHub releases.
    ///
    /// # Arguments
    ///
    /// * `rust_target` - The Rust target, e.g.: i686-unknown-freebsd.
    ///
    /// # Returns
    ///
    /// The modified `GithubUpdater` builder instance.
    ///
    /// # Example
    ///
    /// ```rust
    /// use github_updater::GithubUpdater;
    ///
    /// let updater_builder = GithubUpdater::builder()
    ///     .with_rust_target("i686-unknown-freebsd")
    ///     .build();
    /// ```
    pub fn with_rust_target<S: Into<String>>(mut self, rust_target: S) -> Self {
        self.rust_target = Some(rust_target.into());

        self
    }

    /// Sets information about the GitHub repository on which the releases are located.
    ///
    /// # Arguments
    ///
    /// * `repository_owner` - The GitHub repository owner, e.g.: `Asthowen`.
    /// * `repository_name` - The GitHub repository name, e.g.: `AFetch`.
    ///
    /// # Returns
    ///
    /// The modified `GithubUpdater` builder instance.
    ///
    /// # Example
    ///
    /// ```rust
    /// use github_updater::GithubUpdater;
    ///
    /// let updater_builder = GithubUpdater::builder()
    ///     .with_repository_infos("Asthowen", "AFetch")
    ///     .build();
    /// ```
    pub fn with_repository_infos<S: Into<String>>(
        mut self,
        repository_owner: S,
        repository_name: S,
    ) -> Self {
        self.repository_infos = Some((repository_owner.into(), repository_name.into()));

        self
    }

    /// Sets the file download folder path.
    ///
    /// # Arguments
    ///
    /// * `path` - The download folder path, e.g.: `~/Downloads/`.
    ///
    /// # Returns
    ///
    /// The modified `GithubUpdater` builder instance.
    ///
    /// # Example
    ///
    /// ```rust
    /// use github_updater::GithubUpdater;
    ///
    /// let updater_builder = GithubUpdater::builder()
    ///     .with_download_path("~/Downloads/")
    ///     .build();
    /// ```
    pub fn with_download_path<P: AsRef<Path>>(mut self, path: &P) -> Self {
        self.download_path = Some(path.as_ref().to_owned());

        self
    }

    /// Sets the extension of the downloaded file.
    ///
    /// # Arguments
    ///
    /// * `path` - The extension, e.g.: `exe`, `so`, `dll`.
    ///
    /// # Returns
    ///
    /// The modified `GithubUpdater` builder instance.
    ///
    /// # Example
    ///
    /// ```rust
    /// use github_updater::GithubUpdater;
    ///
    /// let updater_builder = GithubUpdater::builder()
    ///     .with_file_extension("so")
    ///     .build();
    /// ```
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

        self.built = true;

        Ok(self)
    }

    fn generate_file_name(&self, app_name: &String) -> String {
        let extension: String = self
            .file_extension
            .as_ref()
            .map_or_else(String::default, |ext| format!(".{}", ext));
        format!("{}{}", app_name, extension)
    }

    async fn get_current_version(
        &self,
        app_name: &str,
        path: &Path,
    ) -> Result<Option<String>, UpdateError> {
        let path_version_file: PathBuf = path.join(format!("binary-version-{}.txt", app_name));
        if path_version_file.exists() {
            Ok(Some(tokio::fs::read_to_string(&path_version_file).await?))
        } else {
            Ok(None)
        }
    }

    /// Retrieve the latest version of the release from GitHub.
    ///
    /// # Errors
    ///
    /// Returns an `Err` if the builder is not initialized (`BuilderNotInitialized` error).
    ///
    /// But return (`UpdateError` error) if an error occurs while making the API request, if an error occurs while parsing the response JSON, if an error occurs while retrieving the release URL, or if no URL matching the pattern is found.
    ///
    /// # Returns
    ///
    /// `Ok(())` if the fetch is successful.
    ///
    /// # Example
    ///
    /// ```rust
    /// updater_builder.fetch_last_release().await;
    /// ```
    pub async fn fetch_last_release(&mut self) -> Result<(), UpdateError> {
        if !self.built {
            return Err(BuilderNotInitialized.into());
        }

        let repository_infos: &(String, String) = self
            .repository_infos
            .as_ref()
            .ok_or(BuilderNotInitialized)?;
        let url: String = format!(
            "https://api.github.com/repos/{}/{}/releases/latest",
            repository_infos.0, repository_infos.1
        );

        let mut build_request = self
            .reqwest_client
            .as_ref()
            .ok_or(BuilderNotInitialized)?
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

        let mut pattern: String = self
            .pattern
            .as_ref()
            .ok_or(BuilderNotInitialized)?
            .replace("{app_version}", &response.name);
        if let Some(app_name) = &self.app_name {
            pattern = pattern.replace("{app_name}", app_name);
        }
        if let Some(rust_target) = &self.rust_target {
            pattern = pattern.replace("{rust_target}", rust_target);
        }
        self.app_version = Some(response.name);

        let matching_value: Option<&String> =
            asset_urls.iter().find(|&value| value.contains(&pattern));
        if let Some(value) = matching_value {
            let api_url: String = response
                .assets
                .iter()
                .find(|asset| &asset.browser_download_url == value)
                .map(|asset| &asset.url)
                .ok_or_else(|| {
                    UpdateError("An error occurred while retrieving the release URL.".to_owned())
                })?
                .clone();

            self.release_url = Some(api_url);
        } else {
            return Err(UpdateError(
                "No URL matching the pattern entered was found.".to_owned(),
            ));
        }

        Ok(())
    }

    /// Checks if an update is needed for the GitHub release.
    ///
    /// # Errors
    ///
    /// Returns an `Err` if the builder is not initialized (`BuilderNotInitialized` error).
    ///
    /// # Returns
    ///
    /// - `Ok(true)` if an update is needed.
    /// - `Ok(false)` if no update is needed.
    ///
    /// # Example
    ///
    /// ```rust
    /// let update_is_needed = updater_builder.check_if_update_is_needed().await;
    /// ```
    async fn check_if_update_is_needed(&mut self) -> Result<bool, UpdateError> {
        if !self.built {
            return Err(BuilderNotInitialized.into());
        }

        let path: &PathBuf = self.download_path.as_ref().ok_or(BuilderNotInitialized)?;
        let current_version: &String = self.app_version.as_ref().ok_or(BuilderNotInitialized)?;
        let app_name: &String = self.app_name.as_ref().ok_or(BuilderNotInitialized)?;
        let path_version_file: PathBuf = path.join(format!("binary-version-{}.txt", app_name));

        if !path_version_file.exists() || !path.join(self.generate_file_name(app_name)).exists() {
            return Ok(true);
        }

        let previous_version: String = tokio::fs::read_to_string(&path_version_file).await?;

        Ok(previous_version.trim() != current_version)
    }

    /// Force download the latest GitHub release.
    ///
    /// # Errors
    ///
    /// Returns an `Err` if the builder is not initialized (`BuilderNotInitialized` error).
    ///
    /// But return (`UpdateError` error) if an error occurs while fetching the last release, if an error occurs while retrieving the release URL, if no version of the application is found, if an error occurs during file operations, or if an error occurs while downloading the file.
    ///
    /// # Returns
    ///
    /// A `Result` containing the download information (`DownloadInfos`) if the update is successful.
    ///
    /// # Example
    ///
    /// ```rust
    /// let download_infos = updater_builder.force_update().await?;
    /// ```
    pub async fn force_update(&mut self) -> Result<DownloadInfos, UpdateError> {
        if !self.built {
            return Err(BuilderNotInitialized.into());
        }

        if self.need_refresh {
            self.fetch_last_release().await?;
        }

        let app_name: &String = self.app_name.as_ref().ok_or(BuilderNotInitialized)?;
        let path: &PathBuf = self.download_path.as_ref().ok_or(BuilderNotInitialized)?;
        let binary_path: PathBuf = path.join(app_name);
        let old_file: PathBuf = path.join(format!("old_{}", app_name));
        let release_url: &String = self.release_url.as_ref().ok_or(UpdateError(
            "An error occurred while retrieving the release URL.".to_owned(),
        ))?;
        let previous_version: Option<String> = self.get_current_version(app_name, path).await?;
        let new_version: String = self
            .app_version
            .as_ref()
            .ok_or_else(|| UpdateError("No version of the application found.".to_owned()))?
            .to_owned();

        if path.exists() && binary_path.exists() {
            tokio::fs::rename(&binary_path, &old_file).await?;
        } else {
            tokio::fs::create_dir_all(path).await?;
        }

        let mut build_request = self
            .reqwest_client
            .as_ref()
            .ok_or(BuilderNotInitialized)?
            .get(release_url)
            .header("User-Agent", "GitHub-Updater")
            .header("Accept", "application/octet-stream");
        if let Some(token) = &self.github_token {
            build_request = build_request.header("Authorization", format!("token {}", token));
        }

        let response = build_request.send().await?;
        if !response.status().is_success() {
            return Err(UpdateError(format!(
                "An error occurred while downloading the file, HTTP code: {}",
                response.status()
            )));
        }

        let file_path = path.join(self.generate_file_name(app_name));

        let github_md5: String = response
            .headers()
            .get("content-md5")
            .ok_or_else(|| UpdateError("The content-md5 header is absent.".to_owned()))?
            .to_str()?
            .to_owned();
        let content_length: u64 = response
            .headers()
            .get("content-length")
            .ok_or_else(|| UpdateError("The content-length header is absent.".to_owned()))?
            .to_str()?
            .parse::<u64>()?;

        let mut file: File = File::create(&file_path).await?;
        let body = response.bytes().await?;
        file.write_all(&body).await?;

        // Verify file integrity with md5 and content-size
        let mut hasher = md5::Md5::new();
        let mut file: File = File::open(&file_path).await?;
        let mut content: Vec<u8> = Vec::new();
        file.read_to_end(&mut content).await?;
        hasher.update(&content);

        let file_md5: String = STANDARD.encode(hasher.finalize());
        let file_size = file.metadata().await?.size();

        if github_md5 != file_md5 || content_length != file_size {
            tokio::fs::remove_file(&file_path).await?;
            if old_file.exists() {
                tokio::fs::rename(&old_file, &file_path).await?;
            }
            if content_length != file_size {
                return Err(UpdateError(
                    "File corrupted: Incorrect file size detected.".to_owned(),
                ));
            }
            return Err(UpdateError(
                "File corrupted: MD5 checksum does not match.".to_owned(),
            ));
        }

        if old_file.exists() {
            tokio::fs::remove_file(&old_file).await?;
        }

        // Write version in file
        let mut file: File =
            File::create(path.join(format!("binary-version-{}.txt", app_name))).await?;
        file.write_all(new_version.as_bytes()).await?;

        let forced_update: bool = self.forced_update;
        self.forced_update = true;

        Ok(DownloadInfos {
            previous_version,
            new_version,
            has_been_updated: true,
            forced_update,
        })
    }

    /// Check and download, if necessary, the latest version of the release on GitHub.
    ///
    /// # Errors
    ///
    /// Returns an `Err` if the builder is not initialized (`BuilderNotInitialized` error).
    ///
    /// But return (`UpdateError` error) if an error occurs while fetching the last release, if an error occurs while retrieving the release URL, if no version of the application is found, if an error occurs during file operations, or if an error occurs while downloading the file.
    ///
    /// # Returns
    ///
    /// A `Result` containing the download information (`DownloadInfos`) if the update is successful.
    ///
    /// # Example
    ///
    /// ```rust
    /// let download_infos = updater_builder.force_update().await?;
    /// ```
    pub async fn update_if_needed(&mut self) -> Result<DownloadInfos, UpdateError> {
        if !self.built {
            return Err(BuilderNotInitialized.into());
        }

        self.fetch_last_release().await?;
        self.need_refresh = false;

        if self.check_if_update_is_needed().await.unwrap_or(false) {
            self.forced_update = false;
            return self.force_update().await;
        }

        let path: &PathBuf = self.download_path.as_ref().ok_or(BuilderNotInitialized)?;
        let app_name: &String = self.app_name.as_ref().ok_or(BuilderNotInitialized)?;
        let current_version: Option<String> = self.get_current_version(app_name, path).await?;

        Ok(DownloadInfos {
            previous_version: current_version.clone(),
            new_version: current_version.unwrap_or_default(),
            has_been_updated: false,
            forced_update: false,
        })
    }
}
