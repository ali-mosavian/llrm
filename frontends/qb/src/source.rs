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
    let source = read_source(path)?;
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

/// Decode one physical QB-family text file.
///
/// QB/QBasic editors save source in the active DOS OEM code page, and DOS 6's
/// samples retain the conventional 0x1A text EOF marker. UTF-8 stays exact
/// where it is already present; otherwise the QB default, CP437, preserves
/// both comments and literal source characters before the lexer sees them.
fn read_source(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let bytes = bytes.split(|byte| *byte == 0x1a).next().unwrap_or_default();
    match String::from_utf8(bytes.to_vec()) {
        Ok(source) => Ok(source),
        Err(_) => Ok(bytes
            .iter()
            .map(|byte| if byte.is_ascii() { char::from(*byte) } else { CP437[usize::from(*byte - 0x80)] })
            .collect()),
    }
}

const CP437: [char; 128] = [
    '\u{00C7}', '\u{00FC}', '\u{00E9}', '\u{00E2}', '\u{00E4}', '\u{00E0}', '\u{00E5}', '\u{00E7}',
    '\u{00EA}', '\u{00EB}', '\u{00E8}', '\u{00EF}', '\u{00EE}', '\u{00EC}', '\u{00C4}', '\u{00C5}',
    '\u{00C9}', '\u{00E6}', '\u{00C6}', '\u{00F4}', '\u{00F6}', '\u{00F2}', '\u{00FB}', '\u{00F9}',
    '\u{00FF}', '\u{00D6}', '\u{00DC}', '\u{00A2}', '\u{00A3}', '\u{00A5}', '\u{20A7}', '\u{0192}',
    '\u{00E1}', '\u{00ED}', '\u{00F3}', '\u{00FA}', '\u{00F1}', '\u{00D1}', '\u{00AA}', '\u{00BA}',
    '\u{00BF}', '\u{2310}', '\u{00AC}', '\u{00BD}', '\u{00BC}', '\u{00A1}', '\u{00AB}', '\u{00BB}',
    '\u{2591}', '\u{2592}', '\u{2593}', '\u{2502}', '\u{2524}', '\u{2561}', '\u{2562}', '\u{2556}',
    '\u{2555}', '\u{2563}', '\u{2551}', '\u{2557}', '\u{255D}', '\u{255C}', '\u{255B}', '\u{2510}',
    '\u{2514}', '\u{2534}', '\u{252C}', '\u{251C}', '\u{2500}', '\u{253C}', '\u{255E}', '\u{255F}',
    '\u{255A}', '\u{2554}', '\u{2569}', '\u{2566}', '\u{2560}', '\u{2550}', '\u{256C}', '\u{2567}',
    '\u{2568}', '\u{2564}', '\u{2565}', '\u{2559}', '\u{2558}', '\u{2552}', '\u{2553}', '\u{256B}',
    '\u{256A}', '\u{2518}', '\u{250C}', '\u{2588}', '\u{2584}', '\u{258C}', '\u{2590}', '\u{2580}',
    '\u{03B1}', '\u{00DF}', '\u{0393}', '\u{03C0}', '\u{03A3}', '\u{03C3}', '\u{00B5}', '\u{03C4}',
    '\u{03A6}', '\u{0398}', '\u{03A9}', '\u{03B4}', '\u{221E}', '\u{03C6}', '\u{03B5}', '\u{2229}',
    '\u{2261}', '\u{00B1}', '\u{2265}', '\u{2264}', '\u{2320}', '\u{2321}', '\u{00F7}', '\u{2248}',
    '\u{00B0}', '\u{2219}', '\u{00B7}', '\u{221A}', '\u{207F}', '\u{00B2}', '\u{25A0}', '\u{00A0}',
];

pub(crate) fn encode_cp437(text: &str) -> Option<Vec<u8>> {
    text.chars()
        .map(|character| {
            if character.is_ascii() {
                Some(character as u8)
            } else {
                CP437
                    .iter()
                    .position(|candidate| *candidate == character)
                    .map(|index| index as u8 + 0x80)
            }
        })
        .collect()
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

    #[test]
    fn loads_cp437_source_and_stops_at_the_dos_text_eof_marker() {
        let root = std::env::temp_dir().join(format!("qbfront-cp437-{}", std::process::id()));
        let main = root.join("nibble.bas");
        fs::create_dir_all(&root).unwrap();
        fs::write(&main, b"' \xdb comment\r\nmono: data 15, 7\r\n\x1aignored = 1\r\n").unwrap();

        let loaded = load_with_map(&main, &[]).unwrap();

        assert_eq!(loaded.text, "' \u{2588} comment\nmono: data 15, 7\n");
        assert_eq!(loaded.locations.len(), 2);
        fs::remove_dir_all(root).unwrap();
    }
}
