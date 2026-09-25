use std::fs;

use zed_extension_api::{self as zed, settings::LspSettings, Architecture, DownloadedFileType, LanguageServerId, LanguageServerInstallationStatus, Os, Result};

const SERVER: &str = "nib-lsp";
const REPOSITORY: &str = "ali-mosavian/llrm";
/// The server release this extension speaks to: tagged `nib-lsp-v` plus the extension's version.
const TAG: &str = concat!("nib-lsp-v", env!("CARGO_PKG_VERSION"));

struct Nib {
    cached: Option<String>,
}

impl Nib {
    /// The release's server for this platform, downloaded into the extension's own directory once.
    fn server(&mut self, id: &LanguageServerId) -> Result<String> {
        if let Some(path) = self.cached.as_ref().filter(|path| fs::metadata(path).is_ok()) {
            return Ok(path.clone());
        }
        let (os, architecture) = zed::current_platform();
        let triple = match (os, architecture) {
            (Os::Mac, Architecture::Aarch64) => "aarch64-apple-darwin",
            (Os::Mac, Architecture::X8664) => "x86_64-apple-darwin",
            (Os::Linux, Architecture::Aarch64) => "aarch64-unknown-linux-gnu",
            (Os::Linux, Architecture::X8664) => "x86_64-unknown-linux-gnu",
            (Os::Windows, Architecture::X8664) => "x86_64-pc-windows-msvc",
            _ => return Err(format!("{SERVER} has no build for {os:?} {architecture:?}; set lsp.{SERVER}.binary.path")),
        };
        let (archive, kind, binary) = match os {
            Os::Windows => ("zip", DownloadedFileType::Zip, "nib-lsp.exe"),
            _ => ("tar.gz", DownloadedFileType::GzipTar, "nib-lsp"),
        };
        let path = format!("{TAG}/{binary}");
        if fs::metadata(&path).is_err() {
            zed::set_language_server_installation_status(id, &LanguageServerInstallationStatus::Downloading);
            let name = format!("{SERVER}-{triple}.{archive}");
            let release = zed::github_release_by_tag_name(REPOSITORY, TAG)?;
            let asset = release.assets.iter().find(|one| one.name == name).ok_or_else(|| format!("release {TAG} has no {name}"))?;
            zed::download_file(&asset.download_url, TAG, kind)?;
            zed::make_file_executable(&path)?;
            for entry in fs::read_dir(".").map_err(|error| error.to_string())?.flatten() {
                if entry.file_name().to_str().is_some_and(|one| one.starts_with("nib-lsp-v") && one != TAG) {
                    fs::remove_dir_all(entry.path()).ok();
                }
            }
            zed::set_language_server_installation_status(id, &LanguageServerInstallationStatus::None);
        }
        self.cached = Some(path.clone());
        Ok(path)
    }
}

impl zed::Extension for Nib {
    fn new() -> Self {
        Nib { cached: None }
    }

    /// `lsp.nib-lsp.binary` from the settings, else the release's server.
    fn language_server_command(&mut self, id: &LanguageServerId, worktree: &zed::Worktree) -> Result<zed::Command> {
        let binary = LspSettings::for_worktree(SERVER, worktree).ok().and_then(|settings| settings.binary);
        let arguments = binary.as_ref().and_then(|one| one.arguments.clone()).unwrap_or_default();
        let command = match binary.and_then(|one| one.path) {
            Some(path) => path,
            None => self.server(id)?,
        };
        Ok(zed::Command { command, args: arguments, env: worktree.shell_env() })
    }
}

zed::register_extension!(Nib);
