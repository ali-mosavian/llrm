//! The fork clang gives every enumerator of an enum the enum's own type once
//! one of them is outside `int` (16 bits at m16), and that type is unsigned:
//! `long n; n > LONGEST_NOTE` became an unsigned compare, so `PLAY "C"` (n =
//! -1) raised Illegal function call, and `(now - since + DAY) % DAY` went
//! unsigned. Open Watcom, which the runtime was written for, kept them `int`.
//! An enumerator wider than `int` is a `#define`.

use std::path::{Path, PathBuf};

/// The sources compiled for the 16-bit target: the portable ones and the DOS
/// m16 layers.
fn sources(
    dir: &Path,
    out: &mut Vec<PathBuf>,
) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if path.is_dir() {
            let other_target = ["tests", "src", "posix", "headless", "host", "sdl", "m32", "m64"];
            if !other_target.contains(&name.as_str()) {
                sources(&path, out);
            }
        } else if name.ends_with(".c") || name.ends_with(".h") {
            out.push(path);
        }
    }
}

/// `(enumerator, value)` of every `NAME = <integer literal>` inside an `enum {
/// ... }`.
fn enumerators(text: &str) -> Vec<(String, i64)> {
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find("enum") {
        rest = &rest[at + 4..];
        let Some(open) = rest.find('{') else { break };
        if rest[..open].contains(';') {
            continue;
        }
        let Some(close) = rest[open..].find('}') else { break };
        let mut body = rest[open + 1..open + close].to_owned();
        while let (Some(a), Some(b)) = (body.find("/*"), body.find("*/")) {
            body.replace_range(a..b + 2, "");
        }
        for item in body.split(',') {
            let Some((name, value)) = item.split_once('=') else { continue };
            let literal = value.trim().trim_end_matches(['u', 'U', 'l', 'L']);
            let parsed = match literal.strip_prefix("0x") {
                Some(hex) => i64::from_str_radix(hex, 16),
                None => literal.parse(),
            };
            if let Ok(value) = parsed {
                found.push((name.trim().to_owned(), value));
            }
        }
        rest = &rest[open + close..];
    }
    found
}

#[test]
fn no_enumerator_of_the_16_bit_runtime_is_wider_than_int() {
    let mut files = Vec::new();
    sources(Path::new(env!("CARGO_MANIFEST_DIR")), &mut files);
    let wide: Vec<String> = files
        .iter()
        .flat_map(|file| {
            let text = std::fs::read_to_string(file).unwrap();
            enumerators(&text)
                .into_iter()
                .filter(|(_, value)| !(-32768..=32767).contains(value))
                .map(|(name, value)| {
                    format!("{}: {name} = {value}", file.strip_prefix(env!("CARGO_MANIFEST_DIR")).unwrap().display())
                })
                .collect::<Vec<_>>()
        })
        .collect();
    assert!(wide.is_empty(), "make these #defines:\n{}", wide.join("\n"));
}
