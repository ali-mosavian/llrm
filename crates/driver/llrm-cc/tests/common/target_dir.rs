//! Where cargo put the release binaries: `CARGO_TARGET_DIR` when it is set,
//! else `<root>/target`. The tests named `<root>/target/release/llrm-llc`,
//! which does not exist when the runner builds elsewhere, and every compile in
//! the file failed with "No such file or directory".

use std::ffi::OsString;
use std::path::{Path, PathBuf};

pub fn of(root: &Path) -> PathBuf {
    from(root, std::env::var_os("CARGO_TARGET_DIR"))
}

pub fn from(
    root: &Path,
    set: Option<OsString>,
) -> PathBuf {
    set.map(PathBuf::from).unwrap_or_else(|| root.join("target"))
}
