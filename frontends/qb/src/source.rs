//! BASIC source loading, including the compiler's quoted `$INCLUDE` form.

use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceLocation {
    pub path: PathBuf,
    pub line: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoadedSource {
    pub text: String,
    locations: Vec<SourceLocation>,
}

impl LoadedSource {
    pub fn location(&self, expanded_line: usize) -> Option<&SourceLocation> {
        expanded_line.checked_sub(1).and_then(|index| self.locations.get(index))
    }
}

pub fn load(path: &Path, include_dirs: &[PathBuf]) -> Result<String, String> {
    load_with_map(path, include_dirs).map(|loaded| loaded.text)
}

pub fn load_with_map(path: &Path, include_dirs: &[PathBuf]) -> Result<LoadedSource, String> {
    let mut loaded = LoadedSource { text: String::new(), locations: Vec::new() };
    expand(path, include_dirs, &mut Vec::new(), &mut loaded)?;
    Ok(loaded)
}

fn expand(
    path: &Path,
    include_dirs: &[PathBuf],
    active: &mut Vec<PathBuf>,
    loaded: &mut LoadedSource,
) -> Result<(), String> {
    let canonical = path
        .canonicalize()
        .map_err(|error| format!("{}: {error}", path.display()))?;
    if active.contains(&canonical) {
        return Err(format!("recursive $INCLUDE of {}", path.display()));
    }
    active.push(canonical);
    let source =
        fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    for (index, line) in source.lines().enumerate() {
        if let Some(name) = include_name(line) {
            let found = std::iter::once(path.parent().unwrap_or_else(|| Path::new(".")))
                .chain(include_dirs.iter().map(PathBuf::as_path))
                .map(|directory| directory.join(name))
                .find(|candidate| candidate.is_file())
                .ok_or_else(|| {
                    format!("{}: included file {name:?} was not found", path.display())
                })?;
            expand(&found, include_dirs, active, loaded)?;
        } else {
            loaded.text.push_str(line);
            loaded.text.push('\n');
            loaded.locations.push(SourceLocation { path: path.to_path_buf(), line: index + 1 });
        }
    }
    active.pop();
    Ok(())
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
    use super::{include_name, load_with_map};
    use std::fs;

    #[test]
    fn recognizes_compiler_metacommand() {
        assert_eq!(include_name("  '$include: 'q_map.bi'"), Some("q_map.bi"));
        assert_eq!(include_name("  ' $INCLUDE: 'q_map.bi'"), Some("q_map.bi"));
        assert_eq!(include_name("' ordinary comment"), None);
    }

    #[test]
    fn expanded_lines_retain_their_physical_source_location() {
        let root = std::env::temp_dir().join(format!("qbfront-source-map-{}", std::process::id()));
        let main = root.join("main.bas");
        let include = root.join("nested.bi");
        fs::create_dir_all(&root).unwrap();
        fs::write(&include, "first include line\nsecond include line\n").unwrap();
        fs::write(&main, "first main line\n'$include: 'nested.bi'\nlast main line\n").unwrap();
        let loaded = load_with_map(&main, &[]).unwrap();
        assert_eq!(loaded.text, "first main line\nfirst include line\nsecond include line\nlast main line\n");
        assert_eq!((loaded.location(1).unwrap().path.as_path(), loaded.location(1).unwrap().line), (main.as_path(), 1));
        assert_eq!((loaded.location(2).unwrap().path.as_path(), loaded.location(2).unwrap().line), (include.as_path(), 1));
        assert_eq!((loaded.location(3).unwrap().path.as_path(), loaded.location(3).unwrap().line), (include.as_path(), 2));
        assert_eq!((loaded.location(4).unwrap().path.as_path(), loaded.location(4).unwrap().line), (main.as_path(), 3));
        fs::remove_dir_all(root).unwrap();
    }
}
