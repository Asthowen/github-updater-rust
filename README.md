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
### Create builder
```rust
fn main() {
    let mut updater_builder = GithubUpdater::builder()
        .with_initialized_reqwest_client()
        .with_download_path(&"~/downloads")
        .with_repository_infos("repository-owner", "repository-name")
        .with_app_name("app-name")
        .with_rust_target("i686-unknown-linux-musl")
        .with_release_file_name_pattern("{app_name}-{app_version}-{rust_target}")
        .with_file_extension("exe")
        .with_github_token("")
        .build()
        .unwrap();
}
```

### Download update if needed
```rust
fn main() {
    updater_builder.update_if_needed().await?;
}
```

### Force update
```rust
fn main() {
    updater_builder.force_update().await?;
}
```

## Contributors
[<img width="45" src="https://avatars.githubusercontent.com/u/59535754?v=4" alt="Asthowen">](https://github.com/Asthowen)

## License
**[github-updater-rust](https://github.com/Asthowen/github-updater-rust) | [GNU General Public License v3.0](https://github.com/Asthowen/github-updater-rust/blob/main/LICENSE)**