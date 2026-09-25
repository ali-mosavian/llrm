use zed_extension_api::{self as zed, settings::LspSettings, LanguageServerId, Result};

const SERVER: &str = "nib-lsp";

struct Nib;

impl zed::Extension for Nib {
    fn new() -> Self {
        Nib
    }

    /// `lsp.nib-lsp.binary` from the settings, else `nib-lsp` on the worktree's PATH.
    fn language_server_command(&mut self, _id: &LanguageServerId, worktree: &zed::Worktree) -> Result<zed::Command> {
        let binary = LspSettings::for_worktree(SERVER, worktree).ok().and_then(|settings| settings.binary);
        let arguments = binary.as_ref().and_then(|one| one.arguments.clone()).unwrap_or_default();
        let command = match binary.and_then(|one| one.path) {
            Some(path) => path,
            None => worktree
                .which(SERVER)
                .ok_or_else(|| format!("{SERVER} is not on PATH: cargo build --release --no-default-features --bin {SERVER}, or set lsp.{SERVER}.binary.path"))?,
        };
        Ok(zed::Command { command, args: arguments, env: worktree.shell_env() })
    }
}

zed::register_extension!(Nib);
