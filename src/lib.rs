use std::path::{Path, PathBuf};

use futures_util::TryStreamExt;
use reqwest::{Client, Response, header, header::HeaderValue};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

mod error;

pub use error::GithubUpdaterError;

/// Download information struct.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged, rename_all = "camelCase")]
pub enum UpdateResult {
    Updated {
        from: Option<String>,
        to: String,
        forced: bool,
        checksum_verified: bool,
    },
    AlreadyUpToDate {
        version: String,
    },
}

#[derive(Debug, Deserialize)]
struct Release {
    assets: Vec<Asset>,
    name: String,
}

#[derive(Debug, Deserialize)]
struct Asset {
    digest: Option<String>,
    name: String,
    url: String,
}

#[derive(Default)]
#[must_use]
pub struct GithubUpdaterBuilder {
    reqwest_client: Option<Client>,
    github_token: Option<String>,
    pattern: Option<String>,
    app_name: Option<String>,
    rust_target: Option<String>,
    repository_info: Option<(String, String)>,
    download_path: Option<PathBuf>,
    file_extension: Option<String>,
    erase_previous_file: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct GithubUpdater {
    reqwest_client: Client,
    github_token: Option<HeaderValue>,
    pattern: String,
    app_name: String,
    rust_target: Option<String>,
    repository_info: (String, String),
    download_path: PathBuf,
    file_name: String,
    erase_previous_file: bool,
}

impl GithubUpdaterBuilder {
    /// Sets a Reqwest client that has already been initialized.
    ///
    /// # Arguments
    ///
    /// * `reqwest_client` - The already initialized Reqwest client.
    ///
    /// # Returns
    ///
    /// The modified `GithubUpdaterBuilder` builder instance.
    ///
    /// # Example
    ///
    /// ```rust
    /// use github_updater::GithubUpdater;
    /// use reqwest::Client;
    ///
    /// let updater_builder = GithubUpdater::builder()
    ///     .reqwest_client(Client::new())
    ///     .build()?;
    /// ```
    pub fn reqwest_client(mut self, reqwest_client: Client) -> Self {
        self.reqwest_client = Some(reqwest_client);

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
    /// The modified `GithubUpdaterBuilder` builder instance.
    ///
    /// # Example
    ///
    /// ```rust
    /// use github_updater::GithubUpdater;
    ///
    /// let updater_builder = GithubUpdater::builder()
    ///     .release_file_name_pattern("{app_name}-{app_version}-{rust_target}")
    ///     .build()?;
    /// ```
    pub fn release_file_name_pattern<S: Into<String>>(mut self, pattern: S) -> Self {
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
    /// The modified `GithubUpdaterBuilder` builder instance.
    ///
    /// # Example
    ///
    /// ```rust
    /// use github_updater::GithubUpdater;
    ///
    /// let updater_builder = GithubUpdater::builder()
    ///     .app_name("afetch")
    ///     .build()?;
    /// ```
    pub fn app_name<S: Into<String>>(mut self, app_name: S) -> Self {
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
    /// The modified `GithubUpdaterBuilder` builder instance.
    ///
    /// # Example
    ///
    /// ```rust
    /// use github_updater::GithubUpdater;
    ///
    /// let updater_builder = GithubUpdater::builder()
    ///     .github_token("some")
    ///     .build()?;
    /// ```
    pub fn github_token<S: Into<String>>(mut self, github_token: S) -> Self {
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
    /// The modified `GithubUpdaterBuilder` builder instance.
    ///
    /// # Example
    ///
    /// ```rust
    /// use github_updater::GithubUpdater;
    ///
    /// let updater_builder = GithubUpdater::builder()
    ///     .rust_target("i686-unknown-freebsd")
    ///     .build()?;
    /// ```
    pub fn rust_target<S: Into<String>>(mut self, rust_target: S) -> Self {
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
    /// The modified `GithubUpdaterBuilder` builder instance.
    ///
    /// # Example
    ///
    /// ```rust
    /// use github_updater::GithubUpdater;
    ///
    /// let updater_builder = GithubUpdater::builder()
    ///     .repository_info("Asthowen", "AFetch")
    ///     .build()?;
    /// ```
    pub fn repository_info<S1: Into<String>, S2: Into<String>>(
        mut self,
        repository_owner: S1,
        repository_name: S2,
    ) -> Self {
        self.repository_info = Some((repository_owner.into(), repository_name.into()));

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
    /// The modified `GithubUpdaterBuilder` builder instance.
    ///
    /// # Example
    ///
    /// ```rust
    /// use github_updater::GithubUpdater;
    /// use std::path::Path;
    ///
    /// let updater_builder = GithubUpdater::builder()
    ///     .download_path("~/downloads/")
    ///     .build()?;
    /// ```
    pub fn download_path(mut self, path: impl AsRef<Path>) -> Self {
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
    /// The modified `GithubUpdaterBuilder` builder instance.
    ///
    /// # Example
    ///
    /// ```rust
    /// use github_updater::GithubUpdater;
    ///
    /// let updater_builder = GithubUpdater::builder()
    ///     .file_extension("so")
    ///     .build()?;
    /// ```
    pub fn file_extension<S: Into<String>>(mut self, extension: S) -> Self {
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
    /// The modified `GithubUpdaterBuilder` builder instance.
    ///
    /// # Example
    ///
    /// ```rust
    /// use github_updater::GithubUpdater;
    ///
    /// let updater_builder = GithubUpdater::builder()
    ///     .preserve_previous_file()
    ///     .build()?;
    /// ```
    pub fn preserve_previous_file(mut self) -> Self {
        self.erase_previous_file = Some(false);

        self
    }

    pub fn build(self) -> Result<GithubUpdater, GithubUpdaterError> {
        let app_name = self
            .app_name
            .ok_or_else(|| GithubUpdaterError::MissingBuilderField("app_name"))?;

        let pattern = self
            .pattern
            .as_ref()
            .ok_or(GithubUpdaterError::MissingBuilderField("pattern"))?;
        if pattern.contains("rust_target") && self.rust_target.is_none() {
            return Err(GithubUpdaterError::MissingBuilderField("rust_target"));
        }

        Ok(GithubUpdater {
            reqwest_client: self.reqwest_client.unwrap_or_default(),
            github_token: self
                .github_token
                .as_deref()
                .map(|token| HeaderValue::from_str(&format!("Bearer {token}")))
                .transpose()
                .expect("GitHub token should always be valid ASCII"),
            pattern: self.pattern.unwrap_or_default(),
            file_name: match self.file_extension {
                Some(extension) => format!("{app_name}.{extension}"),
                None => app_name.clone(),
            },
            app_name,
            rust_target: self.rust_target,
            repository_info: self
                .repository_info
                .ok_or_else(|| GithubUpdaterError::MissingBuilderField("repository_info"))?,
            download_path: self
                .download_path
                .ok_or_else(|| GithubUpdaterError::MissingBuilderField("download_path"))?,
            erase_previous_file: self.erase_previous_file.unwrap_or(true),
        })
    }
}
impl GithubUpdater {
    pub fn builder() -> GithubUpdaterBuilder {
        GithubUpdaterBuilder::default()
    }

    /// Force download the latest GitHub release.
    ///
    /// # Errors
    ///
    /// Return (`UpdateError` error) if an error occurs while fetching the latest release, if an error occurs while retrieving the release URL, if no version of the application is found, if an error occurs during file operations, or if an error occurs while downloading the file.
    ///
    /// # Returns
    ///
    /// A `Result` containing the update information (`UpdateResult`) if the update is successful.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let update_result = updater_builder.force_update().await?;
    /// ```
    pub async fn force_update(&self) -> Result<UpdateResult, GithubUpdaterError> {
        let (asset, latest_version) = self.fetch_latest_release().await?;
        let target_path = self.download_path.join(&self.file_name);
        self.update_internal(target_path, &asset, latest_version, true)
            .await
    }

    /// Check and download, if necessary, the latest version of the release on GitHub.
    ///
    /// # Errors
    ///
    /// Returns (`UpdateError` error) if an error occurs while fetching the latest release, if an error occurs while retrieving the release URL, if no version of the application is found, if an error occurs during file operations, or if an error occurs while downloading the file.
    ///
    /// # Returns
    ///
    /// A `Result` containing the update information (`UpdateResult`) if the update is successful.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let update_result = updater_builder.update().await?;
    /// ```
    pub async fn update(&self) -> Result<UpdateResult, GithubUpdaterError> {
        let (asset, latest_version) = self.fetch_latest_release().await?;

        let downloaded_path = self.download_path.join(&self.file_name);
        let downloaded_digest = if downloaded_path.exists() {
            Some(self.file_digest(&downloaded_path).await?)
        } else {
            None
        };

        if (asset.digest.is_none() || downloaded_digest == asset.digest)
            && let Some(current_version) = self.downloaded_version().await?
            && current_version == latest_version
        {
            Ok(UpdateResult::AlreadyUpToDate {
                version: current_version,
            })
        } else {
            self.update_internal(downloaded_path, &asset, latest_version, false)
                .await
        }
    }

    async fn downloaded_version(&self) -> Result<Option<String>, GithubUpdaterError> {
        let version_file: PathBuf = self.download_path.join(format!(".{}", self.app_name));
        if version_file.exists() {
            Ok(Some(
                tokio::fs::read_to_string(&version_file)
                    .await?
                    .trim()
                    .to_owned(),
            ))
        } else {
            Ok(None)
        }
    }

    async fn send_request(&self, url: &str, accept: &str) -> Result<Response, GithubUpdaterError> {
        let mut request_builder = self
            .reqwest_client
            .get(url)
            .header(header::ACCEPT, accept)
            .header(header::USER_AGENT, "GitHub-Updater");
        if let Some(github_token) = &self.github_token {
            request_builder = request_builder.header(header::AUTHORIZATION, github_token);
        }

        let response = request_builder.send().await?;
        let status = response.status();
        if !status.is_success() {
            return Err(GithubUpdaterError::UnexpectedStatus {
                url: url.to_owned(),
                status,
            });
        }
        Ok(response)
    }

    async fn fetch_latest_release(&self) -> Result<(Asset, String), GithubUpdaterError> {
        let url: String = format!(
            "https://api.github.com/repos/{}/{}/releases/latest",
            self.repository_info.0, self.repository_info.1
        );
        let response = self
            .send_request(&url, "application/vnd.github+json")
            .await?
            .json::<Release>()
            .await?;

        let mut pattern: String = self
            .pattern
            .replace("{app_version}", &response.name)
            .replace("{app_name}", &self.app_name);
        if let Some(rust_target) = &self.rust_target {
            pattern = pattern.replace("{rust_target}", rust_target);
        }

        response
            .assets
            .into_iter()
            .find(|asset| asset.name.contains(&pattern))
            .ok_or_else(|| {
                GithubUpdaterError::FetchFailed("Unable to find the URL of the requested release.")
            })
            .map(|mut asset| {
                asset.digest = asset
                    .digest
                    .map(|digest| digest.trim_start_matches("sha256:").to_owned());
                (asset, response.name)
            })
    }

    async fn update_internal(
        &self,
        target_path: impl AsRef<Path>,
        asset: &Asset,
        version: String,
        force: bool,
    ) -> Result<UpdateResult, GithubUpdaterError> {
        let target_path = target_path.as_ref();
        let download_path: PathBuf = if target_path.exists() {
            self.download_path.join(format!("new_{}", self.file_name))
        } else {
            if !self.download_path.exists() {
                tokio::fs::create_dir_all(&self.download_path).await?;
            }

            target_path.to_owned()
        };

        let downloaded_version: Option<String> = self.downloaded_version().await?;

        if download_path.exists() {
            tokio::fs::remove_file(&download_path).await?;
        }

        let response = self
            .send_request(&asset.url, "application/octet-stream")
            .await?;
        let content_length = response
            .headers()
            .get(header::CONTENT_LENGTH)
            .ok_or_else(|| GithubUpdaterError::FetchFailed("The content-length header is absent."))?
            .to_str()
            .map_err(|_| {
                GithubUpdaterError::FetchFailed("The content-length header is not valid UTF-8.")
            })?
            .parse::<usize>()
            .map_err(|_| {
                GithubUpdaterError::FetchFailed("The content-length header is invalid.")
            })?;

        let mut file: File = File::create(&download_path).await?;
        let mut file_size = 0;
        let mut file_hasher = if asset.digest.is_some() {
            Some(Sha256::new())
        } else {
            None
        };

        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.try_next().await? {
            file.write_all(&chunk).await?;
            if let Some(file_hasher) = file_hasher.as_mut() {
                file_hasher.update(&chunk);
            }
            file_size += chunk.len();
        }

        let file_digest = file_hasher.map(|file_hasher| format!("{:x}", file_hasher.finalize()));
        if (asset.digest.is_some() && file_digest != asset.digest) || content_length != file_size {
            tokio::fs::remove_file(&download_path).await?;

            return Err(GithubUpdaterError::FetchFailed(
                if content_length == file_size {
                    "File corrupted: SHA-256 checksum does not match."
                } else {
                    "File corrupted: Incorrect file size detected."
                },
            ));
        }

        if self.erase_previous_file && target_path != download_path {
            tokio::fs::remove_file(&target_path).await?;
            tokio::fs::rename(&download_path, &target_path).await?;
        }

        // Write version in file
        tokio::fs::write(
            self.download_path.join(format!(".{}", self.app_name)),
            version.as_bytes(),
        )
        .await?;

        Ok(UpdateResult::Updated {
            from: downloaded_version,
            to: version,
            forced: force,
            checksum_verified: asset.digest.is_some(),
        })
    }

    async fn file_digest(&self, path: impl AsRef<Path>) -> Result<String, GithubUpdaterError> {
        let mut file = File::open(path.as_ref()).await?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 8 * 1024];

        loop {
            let size = file.read(&mut buffer).await?;
            if size == 0 {
                break;
            }
            hasher.update(&buffer[..size]);
        }

        Ok(format!("{:x}", hasher.finalize()))
    }
}
