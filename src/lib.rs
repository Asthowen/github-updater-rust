#![deny(clippy::all, clippy::pedantic)]

mod error;
mod github_api;
mod utils;

pub use error::GithubUpdaterError;

use std::path::{Path, PathBuf};

use base16ct::HexDisplay;
use futures_util::TryStreamExt;
use reqwest::header::{self, HeaderValue};
use reqwest::{Client, Response};
use semver::{Prerelease, Version};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use self::github_api::{Asset, Release, Tag};
use self::utils::next_link_header;

/// The result of an update operation.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum UpdateResult {
    /// If a new version of the file has been downloaded.
    #[serde(rename_all = "camelCase")]
    Updated {
        /// Version of the file present before the update.
        /// May not be available if this is the first download,
        /// or if the file storing the version has been deleted.
        from: Option<String>,
        /// Version of the downloaded file.
        to: String,
        /// If the update was forced (using the [`GithubUpdater::force_update`] method).
        forced: bool,
        /// If the checksum of the downloaded file have been verified,
        /// it will still be false for releases prior to GitHub adding the `digest` field.
        checksum_verified: bool,
    },
    /// If the file was already in the latest version.
    AlreadyUpToDate {
        /// Version of the file already downloaded.
        version: String,
    },
}

/// Specifies which versions are eligible for updates.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub enum VersionStrategy {
    /// Selects the highest available version.
    Latest,
    /// Selects the highest available stable version.
    ///
    /// Pre-release such as release candidates are ignored.
    #[default]
    LatestStable,
}

/// A `GithubUpdaterBuilder` can be used to create a `GithubUpdater`.
#[derive(Default)]
#[must_use]
pub struct GithubUpdaterBuilder {
    reqwest_client: Option<Client>,
    github_token: Option<String>,
    pattern: Option<String>,
    tag_prefix: Option<String>,
    tag_version_strategy: Option<VersionStrategy>,
    tag_pages_limit: Option<u8>,
    app_name: Option<String>,
    rust_target: Option<String>,
    repository_info: Option<(String, String)>,
    download_path: Option<PathBuf>,
    erase_previous_file: Option<bool>,
}

/// A `GithubUpdater` client for checking and downloading updates from GitHub releases.
#[derive(Clone, Debug)]
pub struct GithubUpdater {
    reqwest_client: Client,
    github_token: Option<HeaderValue>,
    pattern: String,
    tag_prefix: Option<String>,
    tag_version_strategy: Option<VersionStrategy>,
    tag_pages_limit: Option<u8>,
    app_name: String,
    rust_target: Option<String>,
    repository_info: (String, String),
    download_path: PathBuf,
    erase_previous_file: bool,
}

impl GithubUpdaterBuilder {
    /// Sets a reqwest client that has already been initialized.
    ///
    /// # Arguments
    ///
    /// * `reqwest_client` - The already initialized reqwest client.
    ///
    /// # Example
    ///
    /// ```rust
    /// use github_updater::GithubUpdater;
    /// use reqwest::Client;
    ///
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let updater_builder = GithubUpdater::builder()
    ///     .reqwest_client(Client::new())
    ///     .build()?;
    /// #
    /// # Ok(())
    /// # }
    /// ```
    pub fn reqwest_client(mut self, reqwest_client: Client) -> Self {
        self.reqwest_client = Some(reqwest_client);

        self
    }

    /// Sets the filename pattern in GitHub releases.
    ///
    /// # Arguments
    ///
    /// * `pattern` - A file pattern, the following placeholders are supported:
    ///    * `app_name`: The name of the application, e.g. `afetch`.
    ///    * `rust_target`: The Rust target, e.g. `i686-unknown-freebsd`.
    ///    * `app_version`: The version of the application.
    ///
    /// # Example
    ///
    /// ```rust
    /// use github_updater::GithubUpdater;
    ///
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let updater_builder = GithubUpdater::builder()
    ///     .release_file_name_pattern("{app_name}-{app_version}-{rust_target}.so")
    ///     .build()?;
    /// #
    /// # Ok(())
    /// # }
    /// ```
    pub fn release_file_name_pattern<S: Into<String>>(mut self, pattern: S) -> Self {
        self.pattern = Some(pattern.into());

        self
    }

    /// Sets the tag prefix used to extract versions from Git tags.
    ///
    /// **The version must always appear at the end of the tag name (after the prefix).**
    ///
    /// This allows to retrieve the latest version using Git tags and semantic versioning
    /// rather than using the GitHub release marked as `latest`.
    ///
    /// # Arguments
    ///
    /// * `prefix` - A prefix used to match tag names, the following placeholders are supported:
    ///    * `app_name`: The name of the application, e.g. `afetch`.
    ///    * `rust_target`: The Rust target, e.g. `i686-unknown-freebsd`.
    ///
    /// # Example
    ///
    /// - Match tags like `1.0.0`:
    /// ```rust
    /// use github_updater::GithubUpdater;
    ///
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let updater_builder = GithubUpdater::builder()
    ///     .tag_prefix("")
    ///     .build()?;
    /// #
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// - Match tags like `afetch-1.0.0`:
    /// ```rust
    /// use github_updater::GithubUpdater;
    ///
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let updater_builder = GithubUpdater::builder()
    ///     .tag_prefix("{app_name}-")
    ///     .build()?;
    /// #
    /// # Ok(())
    /// # }
    /// ```
    pub fn tag_prefix<S: Into<String>>(mut self, prefix: S) -> Self {
        self.tag_prefix = Some(prefix.into());

        self
    }

    /// Sets the tag version strategy.
    ///
    /// # Arguments
    ///
    /// * `version_strategy` - The version strategy.
    ///
    /// # Example
    ///
    /// ```rust
    /// use github_updater::{GithubUpdater, VersionStrategy};
    ///
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let updater_builder = GithubUpdater::builder()
    ///     .tag_version_strategy(VersionStrategy::LatestStable)
    ///     .build()?;
    /// #
    /// # Ok(())
    /// # }
    /// ```
    pub fn tag_version_strategy(mut self, version_strategy: VersionStrategy) -> Self {
        self.tag_version_strategy = Some(version_strategy);

        self
    }

    /// Sets the maximum number of pages to scan when searching for the latest tag version (a page can contain up to 100 items).
    /// The default value is `2`.
    ///
    /// # Arguments
    ///
    /// * `pages` - The maximum number of pages to process.
    ///
    /// # Example
    ///
    /// ```rust
    /// use github_updater::{GithubUpdater, VersionStrategy};
    ///
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let updater_builder = GithubUpdater::builder()
    ///     .tag_pages_limit(2)
    ///     .build()?;
    /// #
    /// # Ok(())
    /// # }
    /// ```
    pub fn tag_pages_limit(mut self, pages: u8) -> Self {
        self.tag_pages_limit = Some(pages);

        self
    }

    /// Sets the application name which is used to define the name of the downloaded executable.
    ///
    /// # Arguments
    ///
    /// * `app_name` - The name of the application.
    ///
    /// # Example
    ///
    /// ```rust
    /// use github_updater::GithubUpdater;
    ///
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let updater_builder = GithubUpdater::builder()
    ///     .app_name("afetch")
    ///     .build()?;
    /// #
    /// # Ok(())
    /// # }
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
    /// # Example
    ///
    /// ```rust
    /// use github_updater::GithubUpdater;
    ///
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let updater_builder = GithubUpdater::builder()
    ///     .github_token("some")
    ///     .build()?;
    /// #
    /// # Ok(())
    /// # }
    /// ```
    pub fn github_token<S: Into<String>>(mut self, github_token: S) -> Self {
        self.github_token = Some(github_token.into());

        self
    }

    /// Sets the rust target that will be searched for in GitHub releases.
    ///
    /// # Arguments
    ///
    /// * `rust_target` - The Rust target, e.g. `i686-unknown-freebsd`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use github_updater::GithubUpdater;
    ///
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let updater_builder = GithubUpdater::builder()
    ///     .rust_target("i686-unknown-freebsd")
    ///     .build()?;
    /// #
    /// # Ok(())
    /// # }
    /// ```
    pub fn rust_target<S: Into<String>>(mut self, rust_target: S) -> Self {
        self.rust_target = Some(rust_target.into());

        self
    }

    /// Sets information about the GitHub repository on which the releases are located.
    ///
    /// # Arguments
    ///
    /// * `repository_owner` - The GitHub repository owner, e.g. `Asthowen`.
    /// * `repository_name` - The GitHub repository name, e.g. `AFetch`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use github_updater::GithubUpdater;
    ///
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let updater_builder = GithubUpdater::builder()
    ///     .repository_info("Asthowen", "AFetch")
    ///     .build()?;
    /// #
    /// # Ok(())
    /// # }
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
    /// * `path` - The download folder path, e.g. `~/downloads`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use github_updater::GithubUpdater;
    ///
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let updater_builder = GithubUpdater::builder()
    ///     .download_path("~/downloads")
    ///     .build()?;
    /// #
    /// # Ok(())
    /// # }
    /// ```
    pub fn download_path(mut self, path: impl AsRef<Path>) -> Self {
        self.download_path = Some(path.as_ref().to_owned());

        self
    }

    /// Disables the erasure of the previous file before downloading a new one.
    ///
    /// When this option is enabled, the original file is preserved, and the new file is saved
    /// with the prefix `new_` added to its filename. Note that the version in the text file
    /// will still be updated accordingly.
    ///
    /// # Example
    ///
    /// ```rust
    /// use github_updater::GithubUpdater;
    ///
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let updater_builder = GithubUpdater::builder()
    ///     .preserve_previous_file()
    ///     .build()?;
    /// #
    /// # Ok(())
    /// # }
    /// ```
    pub fn preserve_previous_file(mut self) -> Self {
        self.erase_previous_file = Some(false);

        self
    }

    /// Returns a `GithubUpdater` that uses this `GithubUpdaterBuilder` configuration.
    ///
    /// # Errors
    ///
    /// This method returns an error if `app_name` or `download_path` or `repository_info` or `pattern`
    /// is not set or if `pattern` or `tag_prefix` contains `{rust_target}`, but `rust_target` is not set.
    ///
    /// # Panics
    ///
    /// This method fails if the GitHub token provided is not valid ASCII.
    pub fn build(self) -> Result<GithubUpdater, GithubUpdaterError> {
        let app_name = self
            .app_name
            .ok_or_else(|| GithubUpdaterError::MissingBuilderField("app_name"))?;

        let pattern = self
            .pattern
            .clone()
            .ok_or(GithubUpdaterError::MissingBuilderField("pattern"))?;
        if (pattern.contains("{rust_target}") && self.rust_target.is_none())
            || self.tag_prefix.as_ref().is_some_and(|tag_prefix| {
                self.rust_target.is_none() && tag_prefix.contains("{rust_target}")
            })
        {
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
            pattern,
            tag_prefix: self.tag_prefix,
            tag_version_strategy: self.tag_version_strategy,
            tag_pages_limit: self.tag_pages_limit,
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
    /// Returns [`GithubUpdaterError`] if an error occurs while fetching the latest file.
    ///
    /// # Returns
    ///
    /// A [`UpdateResult`] containing the update information if the update is successful.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use github_updater::GithubUpdaterBuilder;
    ///
    /// # async fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let updater = GithubUpdaterBuilder::default().build()?;
    /// let update_result = updater.force_update().await?;
    /// #
    /// # Ok(())
    /// # }
    /// ```
    pub async fn force_update(&self) -> Result<UpdateResult, GithubUpdaterError> {
        let (asset, latest_version) = self.fetch_latest_asset().await?;
        let target_path = self.download_path.join(&asset.name);
        self.update_internal(target_path, &asset, latest_version, true)
            .await
    }

    /// Download, if necessary, the latest version of the release on GitHub.
    ///
    /// # Errors
    ///
    /// Returns [`GithubUpdaterError`] if an error occurs while fetching the latest file.
    ///
    /// # Returns
    ///
    /// A [`UpdateResult`] containing the update information if the update is successful.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use github_updater::GithubUpdaterBuilder;
    ///
    /// # async fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let updater = GithubUpdaterBuilder::default().build()?;
    /// let update_result = updater.update().await?;
    /// #
    /// # Ok(())
    /// # }
    /// ```
    pub async fn update(&self) -> Result<UpdateResult, GithubUpdaterError> {
        let (asset, latest_version) = self.fetch_latest_asset().await?;

        let downloaded_path = self.download_path.join(&asset.name);
        let downloaded_digest = if tokio::fs::try_exists(&downloaded_path).await? {
            Some(self.file_digest(&downloaded_path).await?)
        } else {
            None
        };

        if (asset.digest.is_none() || downloaded_digest == asset.digest)
            && let Some(current_version) = self.downloaded_version(&asset.name).await?
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

    async fn downloaded_version(
        &self,
        file_name: &str,
    ) -> Result<Option<String>, GithubUpdaterError> {
        let version_file: PathBuf = self.download_path.join(format!(".{file_name}.txt"));
        if tokio::fs::try_exists(&version_file).await? {
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

    async fn send_json_request<T: DeserializeOwned>(
        &self,
        url: &str,
    ) -> Result<(T, Option<String>), GithubUpdaterError> {
        let response = self
            .send_request(url, "application/vnd.github+json")
            .await?;

        let link_header = response
            .headers()
            .get(header::LINK)
            .and_then(|header| header.to_str().ok())
            .map(str::to_owned);

        Ok((response.json().await?, link_header))
    }

    async fn fetch_latest_asset(&self) -> Result<(Asset, String), GithubUpdaterError> {
        let (release_url_part, version) = if let Some(tag_prefix) = &self.tag_prefix {
            let (tag_name, version) = self.fetch_latest_tag(tag_prefix).await?;
            (format!("tags/{tag_name}"), Some(version.to_string()))
        } else {
            ("latest".to_owned(), None)
        };
        let (release, _) = self
            .send_json_request::<Release>(&format!(
                "https://api.github.com/repos/{}/{}/releases/{release_url_part}",
                self.repository_info.0, self.repository_info.1
            ))
            .await?;

        let version = version.unwrap_or(release.name);
        let pattern: String = self.resolve_pattern(&self.pattern, Some(&version));

        release
            .assets
            .into_iter()
            .find(|asset| asset.name == pattern)
            .ok_or_else(|| {
                GithubUpdaterError::FetchFailed("Unable to find the URL of the requested release.")
            })
            .map(|mut asset| {
                asset.digest = asset
                    .digest
                    .map(|digest| digest.trim_start_matches("sha256:").to_owned());
                (asset, version)
            })
    }

    async fn fetch_latest_tag(
        &self,
        tag_name_pattern: &str,
    ) -> Result<(String, Version), GithubUpdaterError> {
        let pattern: String = self.resolve_pattern(tag_name_pattern, None);
        let tag_version_strategy = self.tag_version_strategy.unwrap_or_default();

        let mut latest_version = None;
        let mut next_url = Some(format!(
            "https://api.github.com/repos/{}/{}/tags?per_page=100",
            self.repository_info.0, self.repository_info.1
        ));
        let mut tag_pages_limit = self.tag_pages_limit.unwrap_or(2);
        while let Some(url) = next_url
            && tag_pages_limit > 0
        {
            let (tags, link_header) = self.send_json_request::<Vec<Tag>>(&url).await?;

            let max_version = tags
                .into_iter()
                .filter_map(|tag| {
                    let version = Version::parse(if pattern.is_empty() {
                        tag.name.as_str()
                    } else {
                        tag.name.split_once(&pattern)?.1
                    })
                    .ok()?;

                    match tag_version_strategy {
                        VersionStrategy::Latest => Some((tag.name, version)),
                        VersionStrategy::LatestStable if version.pre == Prerelease::EMPTY => {
                            Some((tag.name, version))
                        }
                        VersionStrategy::LatestStable => None,
                    }
                })
                .max_by(|a, b| a.1.cmp(&b.1));

            if let Some(max_version) = max_version
                && latest_version
                    .as_ref()
                    .is_none_or(|(_, latest_version)| max_version.1 > *latest_version)
            {
                latest_version = Some(max_version);
            }

            next_url = link_header.and_then(|link_header| next_link_header(&link_header));
            tag_pages_limit -= 1;
        }

        latest_version.ok_or_else(|| {
            GithubUpdaterError::FetchFailed("Unable to find the URL of the requested tag.")
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
        let download_path: PathBuf = if tokio::fs::try_exists(&target_path).await? {
            self.download_path.join(format!("new_{}", asset.name))
        } else {
            if !tokio::fs::try_exists(&self.download_path).await? {
                tokio::fs::create_dir_all(&self.download_path).await?;
            }

            target_path.to_owned()
        };

        let downloaded_version: Option<String> = self.downloaded_version(&asset.name).await?;

        if tokio::fs::try_exists(&download_path).await? {
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

        let file_digest =
            file_hasher.map(|file_hasher| format!("{:x}", HexDisplay(&file_hasher.finalize())));
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
            self.download_path.join(format!(".{}.txt", asset.name)),
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

        Ok(format!("{:x}", HexDisplay(&hasher.finalize())))
    }

    fn resolve_pattern(&self, pattern: &str, app_version: Option<&str>) -> String {
        let mut pattern: String = pattern.replace("{app_name}", &self.app_name);
        if let Some(app_version) = app_version {
            pattern = pattern.replace("{app_version}", app_version);
        }
        if let Some(rust_target) = &self.rust_target {
            pattern = pattern.replace("{rust_target}", rust_target);
        }

        pattern
    }
}
