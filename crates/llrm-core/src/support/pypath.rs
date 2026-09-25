//! Python's `pathlib.Path.resolve()` and `str(OSError)`, where a port reads
//! files and reports their errors as Python did.

use std::io;
use std::path::{Component, Path, PathBuf};

use crate::support::pyrepr;

/// `Path(path).resolve()`, which is `posixpath.realpath` without `strict`:
/// absolute, each existing symlink followed, `..` applied to what came
/// before it, and every other component kept as it was spelled -- a
/// case-insensitive file system does not change the case.
pub fn resolve(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().map_or_else(|_| path.to_path_buf(), |cwd| cwd.join(path))
    };
    let mut out = PathBuf::from("/");
    _join(&mut out, &absolute, 0);
    out
}

/// Walk `rest` onto `out` the way `realpath` does; `depth` bounds a symlink loop.
fn _join(out: &mut PathBuf, rest: &Path, depth: usize) {
    for component in rest.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => *out = PathBuf::from(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            Component::Normal(name) => {
                let candidate = out.join(name);
                let linked = std::fs::symlink_metadata(&candidate).is_ok_and(|meta| meta.file_type().is_symlink());
                match std::fs::read_link(&candidate) {
                    Ok(target) if linked && depth < 40 => _join(out, &target, depth + 1),
                    _ => *out = candidate,
                }
            }
        }
    }
}

/// `str(error)` for the `OSError` Python raised reading `filename`:
/// `[Errno N] strerror: 'filename'`.
pub fn os_error(error: &io::Error, filename: &Path) -> String {
    let text = error.to_string();
    match error.raw_os_error() {
        Some(code) => {
            let strerror = text.strip_suffix(&format!(" (os error {code})")).unwrap_or(&text);
            format!("[Errno {code}] {strerror}: {}", pyrepr::string(&filename.to_string_lossy()))
        }
        None => text,
    }
}

/// `bytes.decode("utf-8")`'s `UnicodeDecodeError` text, or the string.
pub fn decode_utf8(data: &[u8]) -> Result<String, String> {
    match std::str::from_utf8(data) {
        Ok(text) => Ok(text.to_owned()),
        Err(error) => {
            let at = error.valid_up_to();
            let byte = data[at];
            let reason = if matches!(byte, 0x80..=0xC1 | 0xF5..=0xFF) {
                "invalid start byte"
            } else if error.error_len().is_none() {
                "unexpected end of data"
            } else {
                "invalid continuation byte"
            };
            Err(format!("'utf-8' codec can't decode byte {byte:#04x} in position {at}: {reason}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::resolve;

    /// `canonicalize` gave the on-disk spelling, `arith-q-o.obj`, so the
    /// link-unit fingerprint and every label disagreed with Python's.
    #[test]
    fn test_resolve_keeps_the_spelling_it_was_given() {
        // Python: Path(concat!(env!("LLRM_ROOT"), "/tests/fixtures/omf/arith-q-O.obj")).resolve() ends with the name as typed.
        let root = Path::new(env!("LLRM_ROOT"));
        let found = resolve(&root.join("tests/fixtures/omf/./../omf/arith-q-O.obj"));
        assert!(found.ends_with(concat!(env!("LLRM_ROOT"), "/tests/fixtures/omf/arith-q-O.obj")), "{}", found.display());
        assert!(found.is_absolute());
    }
}
