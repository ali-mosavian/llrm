//! The QB device interface (runtime/qb/dev/interface.toml) is the only way
//! runtime/qb reaches the machine: its list, its headers, its platforms and the
//! portable code agree. Before the split the portable code called `dev_int21`,
//! `dev_int16` and `dev_sincos`, DOS's own interrupts, and included the DOS
//! target's `device.h`, so a host build had no screen, keyboard or clock to
//! give it.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const ROOT: &str = env!("CARGO_MANIFEST_DIR");

fn read(path: impl AsRef<Path>) -> String {
    let path = Path::new(ROOT).join(path);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn files(
    directory: &str,
    extensions: &[&str],
) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = fs::read_dir(Path::new(ROOT).join(directory))
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|ext| extensions.iter().any(|one| ext == *one)))
                .collect()
        })
        .unwrap_or_default();
    found.sort();
    found
}

/// The ops interface.toml lists.
fn listed() -> BTreeSet<String> {
    read("runtime/qb/dev/interface.toml")
        .lines()
        .filter_map(|line| line.strip_prefix("name = \"")?.strip_suffix('"').map(str::to_owned))
        .collect()
}

fn is_op_name(word: &str) -> bool {
    word.starts_with("dev_") || word.starts_with("gd_")
}

/// Identifiers of `text` that are op names, followed by `(` or not.
fn mentioned(
    text: &str,
    called_only: bool,
) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let bytes = text.as_bytes();
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at].is_ascii_alphabetic() || bytes[at] == b'_' {
            let start = at;
            while at < bytes.len() && (bytes[at].is_ascii_alphanumeric() || bytes[at] == b'_') {
                at += 1;
            }
            let word = &text[start..at];
            let before_ok = start == 0 || !(bytes[start - 1] == b'"' || bytes[start - 1] == b'.');
            if is_op_name(word) && before_ok && (!called_only || text[at..].starts_with('(')) {
                found.insert(word.to_owned());
            }
        } else {
            at += 1;
        }
    }
    found
}

fn without_comments(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("/*") {
        out += &rest[..start];
        rest = &rest[start..];
        match rest.find("*/") {
            Some(end) => rest = &rest[end + 2..],
            None => return out,
        }
    }
    out + rest
}

#[test]
fn every_listed_op_is_declared_and_every_declared_op_is_listed() {
    let declared: BTreeSet<String> = ["runtime/qb/dev/dev.h", "runtime/qb/dev/gfxdev.h"]
        .iter()
        .flat_map(|header| {
            without_comments(&read(header))
                .lines()
                .filter(|line| !line.starts_with("#") && !line.starts_with(' '))
                .flat_map(|line| mentioned(line, true))
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(declared, listed(), "dev.h and gfxdev.h against interface.toml");
}

#[test]
fn every_platform_defines_every_op() {
    let ops = listed();
    for platform in ["dos/m16", "dos/m32", "headless", "sdl"] {
        let directory = format!("runtime/qb/dev/{platform}");
        if !Path::new(ROOT).join(&directory).is_dir() {
            continue;
        }
        let mut sources = files(&directory, &["c"]);
        if platform.starts_with("dos/") {
            sources.extend(files("runtime/qb/dev/dos", &["c"]));
        } else {
            sources.extend(files("runtime/qb/dev/host", &["c"]));
        }
        // An op the platform's assembly provides is named in its devsym.h:
        // `#pragma aux op "SYMBOL"`.
        let assembly: String =
            files(&directory, &["asm"]).iter().map(|path| fs::read_to_string(path).unwrap()).collect();
        let provided: BTreeSet<String> = fs::read_to_string(Path::new(ROOT).join(&directory).join("devsym.h"))
            .unwrap_or_default()
            .lines()
            .filter_map(|line| {
                let mut words = line.strip_prefix("#pragma aux ")?.split_whitespace();
                let (op, symbol) = (words.next()?, words.next()?.trim_matches('"'));
                assembly
                    .lines()
                    .any(|line| {
                        let words: Vec<&str> = line.split(';').next().unwrap_or("").split_whitespace().collect();
                        words.contains(&"public") && words.contains(&symbol)
                    })
                    .then(|| op.to_owned())
            })
            .collect();
        let defined: BTreeSet<String> = sources
            .iter()
            .flat_map(|path| {
                without_comments(&fs::read_to_string(path).unwrap())
                    .lines()
                    .filter(|line| {
                        line.starts_with(|c: char| c.is_ascii_alphabetic()) && !line.trim_end().ends_with(';')
                    })
                    .flat_map(|line| mentioned(line, true))
                    .collect::<Vec<_>>()
            })
            .chain(provided)
            .collect();
        let missing: Vec<&String> = ops.difference(&defined).collect();
        assert!(missing.is_empty(), "{platform} does not define {missing:?}");
    }
}

#[test]
fn portable_code_reaches_the_machine_only_through_the_listed_ops() {
    let ops = listed();
    let mut wrong = Vec::new();
    for path in files("runtime/qb", &["c", "h"]) {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let text = without_comments(&fs::read_to_string(&path).unwrap());
        if text.contains("#include \"device.h\"") {
            wrong.push(format!("{name} includes a platform's device.h"));
        }
        for op in mentioned(&text, false) {
            if !ops.contains(&op) {
                wrong.push(format!("{name} names {op}, which is not in interface.toml"));
            }
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}
