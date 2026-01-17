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
### Create client
```rust
fn main() {
    let client = GithubUpdater::builder()
        .download_path("~/downloads")
        .repository_info("repository-owner", "repository-name")
        .app_name("app-name")
        .rust_target("i686-unknown-linux-musl")
        .release_file_name_pattern("{app_name}-{app_version}-{rust_target}")
        .file_extension("exe")
        .github_token("token")
        .build()?;
}
```

### Download update if needed
```rust
fn main() {
    client.update().await?;
}
```

### Force update
```rust
fn main() {
    client.force_update().await?;
}
```

## Contributors
[<img width="45" src="https://avatars.githubusercontent.com/u/59535754?v=4" alt="Asthowen">](https://github.com/Asthowen)

## License
**[github-updater-rust](https://github.com/Asthowen/github-updater-rust) | [GNU General Public License v3.0](https://github.com/Asthowen/github-updater-rust/blob/main/LICENSE)**