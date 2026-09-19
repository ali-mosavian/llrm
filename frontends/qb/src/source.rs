//! BASIC source loading, including the compiler's quoted `$INCLUDE` form.

use std::fs;
use std::path::{Path, PathBuf};

pub fn load(path: &Path, include_dirs: &[PathBuf]) -> Result<String, String> {
    expand(path, include_dirs, &mut Vec::new())
}

fn expand(
    path: &Path,
    include_dirs: &[PathBuf],
    active: &mut Vec<PathBuf>,
) -> Result<String, String> {
    let canonical = path
        .canonicalize()
        .map_err(|error| format!("{}: {error}", path.display()))?;
    if active.contains(&canonical) {
        return Err(format!("recursive $INCLUDE of {}", path.display()));
    }
    active.push(canonical);
    let source =
        fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut output = String::new();
    for line in source.lines() {
        if let Some(name) = include_name(line) {
            let found = std::iter::once(path.parent().unwrap_or_else(|| Path::new(".")))
                .chain(include_dirs.iter().map(PathBuf::as_path))
                .map(|directory| directory.join(name))
                .find(|candidate| candidate.is_file())
                .ok_or_else(|| {
                    format!("{}: included file {name:?} was not found", path.display())
                })?;
            output.push_str(&expand(&found, include_dirs, active)?);
        } else {
            output.push_str(line);
            output.push('\n');
        }
    }
    active.pop();
    Ok(output)
}

fn include_name(line: &str) -> Option<&str> {
    let comment = line.trim_start().strip_prefix('\'')?.trim_start();
    let rest = comment
        .get(..9)
        .filter(|prefix| prefix.eq_ignore_ascii_case("$include:"))
        .and_then(|_| comment.get(9..))?
        .trim();
    let rest = rest.strip_prefix('\'')?;
    rest.find('\'').map(|end| &rest[..end])
}

#[cfg(test)]
mod tests {
    use super::include_name;

    #[test]
    fn recognizes_compiler_metacommand() {
        assert_eq!(include_name("  '$include: 'q_map.bi'"), Some("q_map.bi"));
        assert_eq!(include_name("  ' $INCLUDE: 'q_map.bi'"), Some("q_map.bi"));
        assert_eq!(include_name("' ordinary comment"), None);
    }
}
