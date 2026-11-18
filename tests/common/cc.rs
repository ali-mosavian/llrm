//! `llrm-c`'s command line served by `llrm-cc` over the pinned clang: what the
//! tests that spawned `CARGO_BIN_EXE_llrm-c` use instead. The target is the
//! Watcom one the arguments name (`-m32`, else `-m16` as llrm-c defaulted).
//! Needs the pinned clang (`LLRM_PIN_CLANG`, `LLRM_CLANG_M16` at m16) and the
//! Open Watcom headers (`{ow}`, tools/ow-fetch.sh) in the environment.

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

/// The toolchain variables `llrm-cc` needs but finds unset or pointing nowhere
/// (`OW_TREE`: the Open Watcom tree is missing, tools/ow-fetch.sh). Without
/// them it falls back to the system clang, whose datalayout differs: the tests
/// then fail (or pass) for the wrong reason.
pub fn missing_env(
    value: impl Fn(&str) -> Option<OsString>,
    wide: bool,
    flat64: bool,
) -> Vec<&'static str> {
    let mut wanted = vec!["LLRM_PIN_CLANG"];
    if !flat64 {
        wanted.push("OW_TREE");
    }
    if !wide && !flat64 {
        wanted.push("LLRM_CLANG_M16");
    }
    wanted.retain(|name| value(name).is_none_or(|path| path.is_empty() || !Path::new(&path).exists()));
    wanted
}

/// The sha `clang --version` prints in its parentheses, if it ran.
pub fn clang_sha(path: &OsStr) -> Option<String> {
    let out = Command::new(path).arg("--version").output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let tail = &text[text.find("llvm-project.git ")? + "llvm-project.git ".len()..];
    Some(tail.chars().take_while(char::is_ascii_hexdigit).collect())
}

/// One line per clang variable that runs but is not the pin
/// `toolchain/clang/pin` names (line 1 `LLRM_PIN_CLANG`,
/// line 2 `LLRM_CLANG_M16`): a stale cached clang refused every llrm-cc line,
/// 114 reds on 2026-11-12. `sha` is how a path reports its version.
pub fn unpinned_clangs(
    value: impl Fn(&str) -> Option<OsString>,
    pin: &str,
    sha: impl Fn(&OsStr) -> Option<String>,
) -> Vec<String> {
    let want: Vec<&str> = pin.lines().map(|line| line.split_whitespace().next().unwrap_or("")).collect();
    ["LLRM_PIN_CLANG", "LLRM_CLANG_M16"]
        .iter()
        .zip(want)
        .filter_map(|(name, want)| {
            let path = value(name).filter(|path| !path.is_empty())?;
            let have = sha(&path).filter(|have| !have.is_empty())?;
            (!want.starts_with(&have) && !have.starts_with(want)).then(|| {
                format!(
                    "{name}={} is clang {}, toolchain/clang/pin wants {}",
                    path.to_string_lossy(),
                    &have[..12.min(have.len())],
                    &want[..12.min(want.len())]
                )
            })
        })
        .collect()
}

/// The pin file, as the tests were built against it.
pub const PIN: &str = include_str!("../../toolchain/clang/pin");

/// `missing_env` for a compile with `arguments` (`-m32` is wide, `-m64` flat).
pub fn missing_env_for<S: AsRef<std::ffi::OsStr>>(
    arguments: &[S],
    value: impl Fn(&str) -> Option<OsString>,
) -> Vec<&'static str> {
    let has = |flag: &str| arguments.iter().any(|one| one.as_ref() == std::ffi::OsStr::new(flag));
    missing_env(value, has("-m32"), has("-m64"))
}

/// The clang the pin file names for `name` (`LLRM_PIN_CLANG` line 1,
/// `LLRM_CLANG_M16` line 2), in the cache `toolchain/clangbuild.sh pin|m16`
/// fills (`<cache>/llrm/clang[-m16]-<sha12>`), if it is there.
pub fn pinned_clang(
    name: &str,
    pin: &str,
    cache: &Path,
) -> Option<PathBuf> {
    let (line, dir) = match name {
        "LLRM_PIN_CLANG" => (0, "clang"),
        "LLRM_CLANG_M16" => (1, "clang-m16"),
        _ => return None,
    };
    let sha = pin.lines().nth(line)?.split_whitespace().next()?.get(..12)?;
    Some(cache.join("llrm").join(format!("{dir}-{sha}")).join("bin/clang")).filter(|path| path.exists())
}

/// The process environment, with the Open Watcom tree where this machine keeps
/// it, and an unset clang variable resolved to the pinned build in the cache
/// (`toolchain/clangbuild.sh pin|m16` puts it there).
pub fn toolchain_env(name: &str) -> Option<OsString> {
    if name == "OW_TREE" {
        return Some(llrm_cc::ow_tree().into());
    }
    std::env::var_os(name)
        .filter(|value| !value.is_empty())
        .or_else(
            || {
                let home = std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache"));
                let cache = std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from).or(home)?;
                pinned_clang(name, PIN, &cache).map(Into::into)
            },
        )
}

/// Once per process: the clang `llrm-cc` will use, by path and md5, so a run's
/// log says which toolchain it had.
fn announce_toolchain() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let path = std::env::var("LLRM_PIN_CLANG").unwrap_or_default();
        let sum = Command::new("md5sum")
            .arg(&path)
            .output()
            .ok()
            .map_or_else(
                || "?".to_string(),
                |done| String::from_utf8_lossy(&done.stdout).split_whitespace().next().unwrap_or("?").to_string(),
            );
        eprintln!("llrm-cc toolchain: LLRM_PIN_CLANG={path} md5={sum}");
    });
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
        let flat64 = self.arguments.iter().any(|one| one == "-m64");
        let missing = missing_env_for(&self.arguments, toolchain_env);
        assert!(
            missing.is_empty(),
            "llrm-cc needs {missing:?} set to existing paths (the pinned clang, Open Watcom headers, the m16 clang); without them it falls back to the system clang"
        );
        announce_toolchain();
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
            .envs(["LLRM_PIN_CLANG", "LLRM_CLANG_M16"].iter().filter_map(|name| Some((*name, toolchain_env(name)?))))
            .env(
                "LLRM_CC_TARGET",
                if flat64 {
                    "x86_64-linux"
                } else if wide {
                    "x86_32-wat"
                } else {
                    "x86_16-wat"
                },
            )
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
