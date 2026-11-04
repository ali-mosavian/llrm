//! A directory in `$TMPDIR` for one run of an example, deleted when the run
//! ends however it ends. Examples left `llrm-m68k-corpus-*` (2,673) and
//! `llrm-m68k-cycles-*` (554) behind until the temp directory ran out of
//! inodes.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

pub struct Scratch(PathBuf);

impl Scratch {
    /// `llrm-m68k-<kind>-<pid>`, created.
    pub fn new(kind: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("llrm-m68k-{kind}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }
}

impl std::ops::Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl AsRef<Path> for Scratch {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}
