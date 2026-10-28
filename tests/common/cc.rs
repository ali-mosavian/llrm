//! `llrm-c`'s command line served by `llrm-cc` over the pinned clang: what the
//! tests that spawned `CARGO_BIN_EXE_llrm-c` use instead. The target is the
//! Watcom one the arguments name (`-m32`, else `-m16` as llrm-c defaulted).
//! Needs the pinned clang (`LLRM_PIN_CLANG`, `LLRM_CLANG_M16` at m16) and the
//! Open Watcom headers (`LLRM_OW`) in the environment.

#![allow(dead_code)]

use std::ffi::{OsStr, OsString};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};

/// A `Command` for `llrm-cc`, built as `llrm-c`'s was: the target is chosen
/// when it runs, from the arguments added after it was made.
#[derive(Default)]
pub struct LlrmC {
    arguments: Vec<OsString>,
    environment: Vec<(OsString, OsString)>,
    removed: Vec<OsString>,
    directory: Option<PathBuf>,
    stdin: Option<Stdio>,
    stdout: Option<Stdio>,
    stderr: Option<Stdio>,
}

pub fn llrm_c() -> LlrmC {
    LlrmC::default()
}

impl LlrmC {
    pub fn arg(
        &mut self,
        argument: impl AsRef<OsStr>,
    ) -> &mut Self {
        self.arguments.push(argument.as_ref().to_owned());
        self
    }

    pub fn args<I, S>(
        &mut self,
        arguments: I,
    ) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.arguments.extend(arguments.into_iter().map(|one| one.as_ref().to_owned()));
        self
    }

    pub fn env(
        &mut self,
        key: impl AsRef<OsStr>,
        value: impl AsRef<OsStr>,
    ) -> &mut Self {
        self.environment.push((key.as_ref().to_owned(), value.as_ref().to_owned()));
        self
    }

    pub fn env_remove(
        &mut self,
        key: impl AsRef<OsStr>,
    ) -> &mut Self {
        self.removed.push(key.as_ref().to_owned());
        self
    }

    pub fn current_dir(
        &mut self,
        directory: impl Into<PathBuf>,
    ) -> &mut Self {
        self.directory = Some(directory.into());
        self
    }

    pub fn stdin(
        &mut self,
        stdin: impl Into<Stdio>,
    ) -> &mut Self {
        self.stdin = Some(stdin.into());
        self
    }

    pub fn stdout(
        &mut self,
        stdout: impl Into<Stdio>,
    ) -> &mut Self {
        self.stdout = Some(stdout.into());
        self
    }

    pub fn stderr(
        &mut self,
        stderr: impl Into<Stdio>,
    ) -> &mut Self {
        self.stderr = Some(stderr.into());
        self
    }

    fn command(&mut self) -> Command {
        let wide = self.arguments.iter().any(|one| one == "-m32");
        // llrm-c chose by the output's extension: `-o x.obj` compiled, `-o
        // x.asm` wrote the listing; llrm-cc links unless told `-c` or
        // `-S`.
        let stops = self.arguments.iter().any(|one| one == "-c" || one == "-S" || one == "-E");
        let output = self.arguments.iter().position(|one| one == "-o").and_then(|at| self.arguments.get(at + 1));
        let extension =
            output.and_then(|path| Path::new(path).extension()).map(|one| one.to_string_lossy().to_ascii_lowercase());
        let stop = match extension.as_deref() {
            _ if stops => None,
            Some("obj" | "o") => Some("-c"),
            Some("asm" | "s") => Some("-S"),
            _ => None,
        };
        let mut command = Command::new(env!("CARGO_BIN_EXE_llrm-cc"));
        command
            .env("LLRM_CC_TARGET", if wide { "x86_32-wat" } else { "x86_16-wat" })
            .args(self.arguments.iter().filter(|one| *one != "-m16" && *one != "-m32"))
            .args(stop);
        // In the order they were asked, as `Command` does: a removal beside a
        // later `env` of the same key must not win over it.
        for key in &self.removed {
            command.env_remove(key);
        }
        command.envs(self.environment.iter().map(|(key, value)| (key, value)));
        if let Some(directory) = &self.directory {
            command.current_dir(directory);
        }
        if let Some(stdin) = self.stdin.take() {
            command.stdin(stdin);
        }
        if let Some(stdout) = self.stdout.take() {
            command.stdout(stdout);
        }
        if let Some(stderr) = self.stderr.take() {
            command.stderr(stderr);
        }
        command
    }

    pub fn output(&mut self) -> io::Result<Output> {
        self.command().output()
    }

    pub fn status(&mut self) -> io::Result<std::process::ExitStatus> {
        self.command().status()
    }

    pub fn spawn(&mut self) -> io::Result<Child> {
        self.command().spawn()
    }
}
