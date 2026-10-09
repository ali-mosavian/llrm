//! `llrm-qb --dump DIR`: the source and its HIR, as adjacent text files.
//! The MIR passes are dumped by `LLRM_MIR_STAGES`.

use std::path::{Path, PathBuf};

use llrm_core::hir::codec;

use super::compile;
use super::driver::{Frontend, parsed};

/// Decode the physical source exactly as qbfront does before lexing.
pub(super) fn _source_text(path: &Path) -> Result<String, String> {
    let raw = std::fs::read(path).map_err(|error| error.to_string())?;
    let raw = raw.split(|byte| *byte == 0x1a).next().unwrap_or_default();
    Ok(match std::str::from_utf8(raw) {
        Ok(text) => text.to_owned(),
        Err(_) => raw
            .iter()
            .map(|byte| if byte.is_ascii() { char::from(*byte) } else { CP437[usize::from(*byte - 0x80)] })
            .collect(),
    })
}

/// Python's `cp437` codec above 0x7F.
const CP437: [char; 128] = [
    '\u{00C7}', '\u{00FC}', '\u{00E9}', '\u{00E2}', '\u{00E4}', '\u{00E0}', '\u{00E5}', '\u{00E7}', '\u{00EA}',
    '\u{00EB}', '\u{00E8}', '\u{00EF}', '\u{00EE}', '\u{00EC}', '\u{00C4}', '\u{00C5}', '\u{00C9}', '\u{00E6}',
    '\u{00C6}', '\u{00F4}', '\u{00F6}', '\u{00F2}', '\u{00FB}', '\u{00F9}', '\u{00FF}', '\u{00D6}', '\u{00DC}',
    '\u{00A2}', '\u{00A3}', '\u{00A5}', '\u{20A7}', '\u{0192}', '\u{00E1}', '\u{00ED}', '\u{00F3}', '\u{00FA}',
    '\u{00F1}', '\u{00D1}', '\u{00AA}', '\u{00BA}', '\u{00BF}', '\u{2310}', '\u{00AC}', '\u{00BD}', '\u{00BC}',
    '\u{00A1}', '\u{00AB}', '\u{00BB}', '\u{2591}', '\u{2592}', '\u{2593}', '\u{2502}', '\u{2524}', '\u{2561}',
    '\u{2562}', '\u{2556}', '\u{2555}', '\u{2563}', '\u{2551}', '\u{2557}', '\u{255D}', '\u{255C}', '\u{255B}',
    '\u{2510}', '\u{2514}', '\u{2534}', '\u{252C}', '\u{251C}', '\u{2500}', '\u{253C}', '\u{255E}', '\u{255F}',
    '\u{255A}', '\u{2554}', '\u{2569}', '\u{2566}', '\u{2560}', '\u{2550}', '\u{256C}', '\u{2567}', '\u{2568}',
    '\u{2564}', '\u{2565}', '\u{2559}', '\u{2558}', '\u{2552}', '\u{2553}', '\u{256B}', '\u{256A}', '\u{2518}',
    '\u{250C}', '\u{2588}', '\u{2584}', '\u{258C}', '\u{2590}', '\u{2580}', '\u{03B1}', '\u{00DF}', '\u{0393}',
    '\u{03C0}', '\u{03A3}', '\u{03C3}', '\u{00B5}', '\u{03C4}', '\u{03A6}', '\u{0398}', '\u{03A9}', '\u{03B4}',
    '\u{221E}', '\u{03C6}', '\u{03B5}', '\u{2229}', '\u{2261}', '\u{00B1}', '\u{2265}', '\u{2264}', '\u{2320}',
    '\u{2321}', '\u{00F7}', '\u{2248}', '\u{00B0}', '\u{2219}', '\u{00B7}', '\u{221A}', '\u{207F}', '\u{00B2}',
    '\u{25A0}', '\u{00A0}',
];

fn write(
    path: &Path,
    text: &str,
) -> Result<(), String> {
    std::fs::write(path, text).map_err(|error| format!("{}: {error}", path.display()))
}

pub fn dumped(
    source: &Path,
    output: &Path,
    frontend: &Frontend,
    codegen: &llrm_core::driver::Options,
) -> Result<PathBuf, String> {
    std::fs::create_dir_all(output).map_err(|error| error.to_string())?;
    let program = parsed(source, frontend, None).map_err(|error| error.0)?;
    write(&output.join("00-input.bas"), &_source_text(source)?)?;
    let mut observe = |program: &llrm_core::hir::model::Program| -> Result<(), String> {
        write(&output.join("01-hir.json"), &codec::encode(program, None).map_err(|error| error.0)?)
    };
    compile::object_bytes(&program, source, Some(&mut observe), codegen).map_err(|error| error.to_string())?;
    Ok(output.to_path_buf())
}
