use base64::{Engine as _, engine::general_purpose::STANDARD};
use md5::Digest;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::{Client, Response};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

mod error;
pub use error::GithubUpdaterError;

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
    reqwest_client: Option<Client>,
    built: bool,
    pattern: Option<String>,
    app_name: Option<String>,
    github_token: Option<String>,
    rust_target: Option<String>,
    repository_infos: Option<(String, String)>,
    download_path: Option<PathBuf>,
    file_extension: Option<String>,
    erase_previous_file: bool,
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
            erase_previous_file: true,
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
    pub fn with_reqwest_client(mut self, reqwest_client: Client) -> Self {
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
        self.reqwest_client = Some(
            Client::builder()
                .default_headers({
                    let mut headers = HeaderMap::new();
                    headers.insert(
                        reqwest::header::ACCEPT_ENCODING,
                        HeaderValue::from_static("identity"),
                    );
                    headers
                })
                .build()
                .unwrap(),
        );

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
    /// use std::path::Path;
    ///
    /// let updater_builder = GithubUpdater::builder()
    ///     .with_download_path(&Path::new("~/Downloads/"))
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

    /// Disables the erasure of the previous file before downloading a new one.
    ///
    /// When this option is enabled, the original file is preserved, and the new file is saved
    /// with the prefix `new_` added to its filename. Note that the version information
    /// in the text file will still be updated accordingly.
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
    ///     .without_erase_previous_file()
    ///     .build();
    /// ```
    pub fn without_erase_previous_file(mut self) -> Self {
        self.erase_previous_file = false;

        self
    }

    pub fn build(mut self) -> Result<Self, GithubUpdaterError> {
        if self.reqwest_client.is_none() {
            return Err(GithubUpdaterError::BuilderMissingField("reqwest_client"));
        }
        if self.app_name.is_none() {
            return Err(GithubUpdaterError::BuilderMissingField("app_name"));
        }
        if let Some(pattern) = &self.pattern {
            if pattern.contains("rust_target") && self.rust_target.is_none() {
                return Err(GithubUpdaterError::BuilderMissingField("rust_target"));
            }
        } else {
            return Err(GithubUpdaterError::BuilderMissingField("pattern"));
        }
        if self.repository_infos.is_none() {
            return Err(GithubUpdaterError::BuilderMissingField("repository_infos"));
        }
        if self.download_path.is_none() {
            return Err(GithubUpdaterError::BuilderMissingField("download_path"));
        }

        self.built = true;

        Ok(self)
    }

    fn generate_file_name(&self, app_name: &str) -> String {
        let extension: String = self
            .file_extension
            .as_ref()
            .map_or_else(String::default, |ext| format!(".{ext}"));
        format!("{app_name}{extension}")
    }

    async fn get_current_version(
        &self,
        app_name: &str,
        path: &Path,
    ) -> Result<Option<String>, GithubUpdaterError> {
        let path_version_file: PathBuf = path.join(format!("binary-version-{app_name}.txt"));
        if path_version_file.exists() {
            Ok(Some(tokio::fs::read_to_string(&path_version_file).await?))
        } else {
            Ok(None)
        }
    }

    async fn send_request(&self, url: &str, accept: &str) -> Result<Response, GithubUpdaterError> {
        let mut build_request = self
            .reqwest_client
            .as_ref()
            .ok_or(GithubUpdaterError::BuilderNotInitialized)?
            .get(url)
            .header("User-Agent", "GitHub-Updater")
            .header("Accept", accept);
        if let Some(token) = &self.github_token {
            build_request = build_request.header("Authorization", format!("token {token}"));
        }
        let response = build_request.send().await?;
        if !response.status().is_success() {
            return Err(GithubUpdaterError::FetchError(format!(
                "An error occurred while downloading the file, HTTP code: {}",
                response.status()
            )));
        }
        Ok(response)
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
    /// ```rust,ignore
    /// updater_builder.fetch_last_release().await;
    /// ```
    pub async fn fetch_last_release(&mut self) -> Result<(), GithubUpdaterError> {
        if !self.built {
            return Err(GithubUpdaterError::BuilderNotInitialized);
        }

        let repository_infos: &(String, String) = self
            .repository_infos
            .as_ref()
            .ok_or(GithubUpdaterError::BuilderNotInitialized)?;
        let url: String = format!(
            "https://api.github.com/repos/{}/{}/releases/latest",
            repository_infos.0, repository_infos.1
        );
        let response = self
            .send_request(&url, "application/vnd.github.v3+json")
            .await?
            .json::<Release>()
            .await?;
        let asset_urls: Vec<String> = response
            .assets
            .iter()
            .map(|asset| asset.browser_download_url.to_owned())
            .collect();

        let mut pattern: String = self
            .pattern
            .as_ref()
            .ok_or(GithubUpdaterError::BuilderNotInitialized)?
            .replace("{app_version}", &response.name);
        if let Some(app_name) = &self.app_name {
            pattern = pattern.replace("{app_name}", app_name);
        }
        if let Some(rust_target) = &self.rust_target {
            pattern = pattern.replace("{rust_target}", rust_target);
        }
        self.app_version = Some(response.name);

        let matching_value: String = asset_urls
            .into_iter()
            .find(|value| value.contains(&pattern))
            .ok_or_else(|| {
                GithubUpdaterError::FetchError(
                    "No URL matching the pattern entered was found.".to_owned(),
                )
            })?;
        let api_url: String = response
            .assets
            .into_iter()
            .find(|asset| asset.browser_download_url == matching_value)
            .map(|asset| asset.url)
            .ok_or_else(|| {
                GithubUpdaterError::FetchError(
                    "Unable to find the URL of the requested release.".to_owned(),
                )
            })?;

        self.release_url = Some(api_url);

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
    /// ```rust,ignore
    /// let update_is_needed = updater_builder.check_if_update_is_needed().await;
    /// ```
    async fn check_if_update_is_needed(&mut self) -> Result<bool, GithubUpdaterError> {
        if !self.built {
            return Err(GithubUpdaterError::BuilderNotInitialized);
        }

        let path: &PathBuf = self
            .download_path
            .as_ref()
            .ok_or(GithubUpdaterError::BuilderNotInitialized)?;
        let current_version: &String = self
            .app_version
            .as_ref()
            .ok_or(GithubUpdaterError::BuilderNotInitialized)?;
        let app_name: &String = self
            .app_name
            .as_ref()
            .ok_or(GithubUpdaterError::BuilderNotInitialized)?;
        let path_version_file: PathBuf = path.join(format!("binary-version-{app_name}.txt"));

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
    /// ```rust,ignore
    /// let download_infos = updater_builder.force_update().await?;
    /// ```
    pub async fn force_update(&mut self) -> Result<DownloadInfos, GithubUpdaterError> {
        if !self.built {
            return Err(GithubUpdaterError::BuilderNotInitialized);
        }

        if self.need_refresh {
            self.fetch_last_release().await?;
        }

        let app_name: &String = self
            .app_name
            .as_ref()
            .ok_or(GithubUpdaterError::BuilderNotInitialized)?;
        let path: &PathBuf = self
            .download_path
            .as_ref()
            .ok_or(GithubUpdaterError::BuilderNotInitialized)?;
        let file_name = self.generate_file_name(app_name);
        let previous_file: PathBuf = path.join(&file_name);
        let new_file: PathBuf = if previous_file.exists() {
            path.join(format!("new_{file_name}"))
        } else {
            previous_file.clone()
        };
        let release_url: &String =
            self.release_url
                .as_ref()
                .ok_or(GithubUpdaterError::FetchError(
                    "Unable to find the URL of the requested release.".to_owned(),
                ))?;
        let previous_version: Option<String> = self.get_current_version(app_name, path).await?;
        let new_version: String = self
            .app_version
            .as_ref()
            .ok_or(GithubUpdaterError::BuilderNotInitialized)?
            .to_owned();

        if !path.exists() {
            tokio::fs::create_dir_all(path).await?;
        }
        if new_file.exists() {
            tokio::fs::remove_file(&new_file).await?;
        }

        let response = self
            .send_request(release_url, "application/octet-stream")
            .await?;
        let github_md5: Option<String> = response
            .headers()
            .get("content-md5")
            .map(HeaderValue::to_str)
            .transpose()?
            .map(str::to_owned);
        let content_length: usize = response
            .headers()
            .get("content-length")
            .ok_or_else(|| {
                GithubUpdaterError::FetchError("The content-length header is absent.".to_owned())
            })?
            .to_str()?
            .parse::<usize>()?;

        let mut file: File = File::create(&new_file).await?;
        let body = response.bytes().await?;
        file.write_all(&body).await?;

        // Verify file integrity with md5 and content-size
        let file_md5: Option<String> = if github_md5.is_some() {
            let mut hasher = md5::Md5::new();
            let mut file: File = File::open(&new_file).await?;
            let mut content: Vec<u8> = Vec::new();
            file.read_to_end(&mut content).await?;
            hasher.update(&content);

            Some(STANDARD.encode(hasher.finalize()))
        } else {
            None
        };

        if github_md5 != file_md5 || content_length != body.len() {
            tokio::fs::remove_file(&new_file).await?;

            return Err(GithubUpdaterError::FetchError(if github_md5 == file_md5 {
                "File corrupted: Incorrect file size detected.".to_owned()
            } else {
                "File corrupted: MD5 checksum does not match.".to_owned()
            }));
        }

        if self.erase_previous_file && previous_file != new_file {
            tokio::fs::remove_file(&previous_file).await?;
            tokio::fs::rename(&new_file, &previous_file).await?;
        }

        // Write version in file
        let mut file: File =
            File::create(path.join(format!("binary-version-{app_name}.txt"))).await?;
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
    /// ```rust,ignore
    /// let download_infos = updater_builder.force_update().await?;
    /// ```
    pub async fn update_if_needed(&mut self) -> Result<DownloadInfos, GithubUpdaterError> {
        if !self.built {
            return Err(GithubUpdaterError::BuilderNotInitialized);
        }

        self.fetch_last_release().await?;
        self.need_refresh = false;

        if self.check_if_update_is_needed().await.unwrap_or(false) {
            self.forced_update = false;
            return self.force_update().await;
        }

        let path: &PathBuf = self
            .download_path
            .as_ref()
            .ok_or(GithubUpdaterError::BuilderNotInitialized)?;
        let app_name: &String = self
            .app_name
            .as_ref()
            .ok_or(GithubUpdaterError::BuilderNotInitialized)?;
        let current_version: Option<String> = self.get_current_version(app_name, path).await?;

        Ok(DownloadInfos {
            previous_version: current_version.clone(),
            new_version: current_version.unwrap_or_default(),
            has_been_updated: false,
            forced_update: false,
        })
    }
}
