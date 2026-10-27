<!--suppress HtmlDeprecatedAttribute -->
<div align="center">
    <h1><strong>github-updater</strong></h1>
    <div>
        <a href="https://www.rust-lang.org/">
            <img src="https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white" alt="Made with Rust">
        </a>
        <a href="https://github.com/Asthowen/github-updater-rust/blob/main/LICENSE">
            <img src="https://img.shields.io/github/license/Asthowen/github-updater-rust?style=for-the-badge" alt="License">
        </a>
        <a href="https://github.com/Asthowen/github-updater-rust/stargazers">
            <img src="https://img.shields.io/github/stars/Asthowen/github-updater-rust?style=for-the-badge" alt="Stars">
        </a>
    </div>
    <h3>
        <strong>A small library to update rust binaries from GitHub releases.</strong>
    </h3>
</div>

## Basic usage
### Add to dependencies
```toml
[dependencies]
github-updater = { git = "https://github.com/Asthowen/github-updater-rust", version = "0.2.0" }
```

### Create client
#### Use latest release
This mode retrieves the version marked as the latest release on GitHub. The release name will be used as the version.

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = GithubUpdater::builder()
        .download_path("~/downloads")
        .repository_info("repository-owner", "repository-name")
        .app_name("app-name")
        .rust_target("i686-unknown-linux-musl")
        .release_file_name_pattern("{app_name}-{app_version}-{rust_target}")
        .local_file_name_pattern("{app_name}-{rust_target}")
        .github_token("token")
        .build()?;

    Ok(())
}
```

### Use latest tag
This mode retrieves the latest version according to semantic versioning from Git tags, which allows support for monorepos, for example.

**The version must always appear at the end of the tag name (after the prefix).**

In this example, tags such as `app-name-1.0.0` will be considered:

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = GithubUpdater::builder()
        .download_path("~/downloads")
        .repository_info("repository-owner", "repository-name")
        .app_name("app-name")
        .rust_target("i686-unknown-linux-musl")
        .tag_prefix("{app_name}-")
        .tag_version_strategy(VersionStrategy::LatestStable)
        .release_file_name_pattern("{app_name}-{app_version}-{rust_target}")
        .local_file_name_pattern("{app_name}-{rust_target}")
        .github_token("token")
        .build()?;

    Ok(())
}
```

### Download update if needed
```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    client.update().await?;

    Ok(())
}
```

### Force update
```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    client.force_update().await?;

    Ok(())
}
```

## Contributors
[<img width="45" src="https://avatars.githubusercontent.com/u/59535754?v=4" alt="Asthowen">](https://github.com/Asthowen)

## License
**[github-updater-rust](https://github.com/Asthowen/github-updater-rust) | [GNU General Public License v3.0](https://github.com/Asthowen/github-updater-rust/blob/main/LICENSE)**
