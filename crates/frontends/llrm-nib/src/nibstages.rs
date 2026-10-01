//! `--dump`: the frontend's stages, then the pipeline's, as the compile
//! `-o` runs writes them.

use std::path::{Path, PathBuf};

use super::compile as nib;
use super::driver;
use llrm_core::driver as codegen;
use llrm_core::hir;

/// Python's text-mode read: universal newlines.
fn _text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).replace("\r\n", "\n").replace('\r', "\n")
}

fn write(path: &Path, text: &str) -> Result<(), String> {
    std::fs::write(path, text).map_err(|error| error.to_string())
}

/// Write source, lexical, syntax and HIR snapshots, then compile through
/// the one route `-S` and `-o` take, which writes each pipeline stage, the
/// listing and its costs in `mir/`.
pub fn dumped(source: &Path, output: &Path, frontend: &super::Frontend, options: &codegen::Options, entry: &str) -> Result<PathBuf, String> {
    std::fs::create_dir_all(output).map_err(|error| error.to_string())?;
    let program = driver::parsed(source, frontend, None).map_err(|error| error.0)?;
    let input = std::fs::read(source).map_err(|error| error.to_string())?;
    write(&output.join("00-input.nib"), &_text(&input))?;
    let text = String::from_utf8_lossy(&input);
    let refused = |error| driver::refused(source, &error).0;
    write(&output.join("01-tokens.txt"), &super::tokens_text(&text).map_err(refused)?)?;
    write(&output.join("02-syntax.txt"), &super::syntax_text(&text).map_err(refused)?)?;
    write(&output.join("03-hir.json"), &hir::encode(&program, Some(2)).map_err(|error| error.to_string())?)?;
    let stages = output.join("mir");
    std::fs::create_dir_all(&stages).map_err(|error| error.to_string())?;
    nib::assembled_from_mir(&program, entry, &codegen::Options { dump: Some(stages), ..options.clone() })?;
    write(
        &output.join("README.txt"),
        "Nib frontend stage dumps\n========================\n\n\
         00-input.nib       exact source presented to the frontend\n\
         01-tokens.txt      lexer output with source positions\n\
         02-syntax.txt      indentation-aware syntax tree\n\
         03-hir.json        verified, source-neutral common HIR\n\
         mir/               the MIR after each pipeline pass, listing.asm and cost:\n\
         \x20                  what -S and -o compile\n",
    )?;
    Ok(output.to_path_buf())
}

#[cfg(test)]
mod tests {
    //! Port of `tests/test_modernstages.py`.

    use super::dumped;
    use crate::test_nib_frontend::fixture;
    use llrm_core::support::pyjson::{self, Json};

    fn options() -> llrm_core::driver::Options {
        llrm_core::driver::Options { dump: None, ..llrm_core::driver::Options::of(crate::compile::machine()) }
    }

    #[test]
    fn test_nbody_stage_dumps_cover_every_implemented_boundary() {
        // nbody used to expose HIR and MIR only through separate ad-hoc commands.
        let directory = tempfile::tempdir().expect("a directory");
        let nbody = fixture("nbody.nib");
        let output = dumped(&nbody, &directory.path().join("nbody"), &super::super::Frontend::default(), &options(), "main").expect("dumps");
        let names = |directory: &std::path::Path| -> Vec<String> {
            let mut names: Vec<String> = std::fs::read_dir(directory)
                .expect("lists")
                .map(|one| one.expect("an entry").file_name().to_string_lossy().into_owned())
                .collect();
            names.sort();
            names
        };
        assert_eq!(names(&output), ["00-input.nib", "01-tokens.txt", "02-syntax.txt", "03-hir.json", "README.txt", "mir"]);
        let read = |name: &str| std::fs::read_to_string(output.join(name)).expect("dumped");
        assert_eq!(std::fs::read(output.join("00-input.nib")).unwrap(), std::fs::read(&nbody).unwrap());
        assert!(read("01-tokens.txt").contains("Fixed"));
        let syntax = read("02-syntax.txt");
        assert!(syntax.contains("Struct {\n            name: \"body\""));
        assert!(syntax.contains("ForRange {"));
        let Json::Dict(document) = pyjson::loads(&read("03-hir.json")).expect("JSON") else { panic!("an object") };
        assert_eq!(document.get("schema"), Some(&Json::Int(5)));
        let stages = names(&output.join("mir"));
        assert!(stages.iter().any(|one| one.ends_with(".ll")), "{stages:?}");
        assert!(stages.contains(&"listing.asm".to_owned()) && stages.contains(&"cost".to_owned()), "{stages:?}");
        assert!(read("mir/listing.asm").contains("_main proc"));
    }

    #[test]
    fn test_a_library_without_main_dumps_its_listing() {
        // A library had no listing, so the runtime's code could not be read.
        let directory = tempfile::tempdir().expect("a directory");
        let source = directory.path().join("lib.nib");
        std::fs::write(&source, "@export(\"cdecl16\")\nfn twice(value: i16) -> i16:\n    return value * 2\n").expect("writes");
        let output = dumped(&source, &directory.path().join("dump"), &super::super::Frontend::default(), &options(), "main").expect("dumps");
        let listing = std::fs::read_to_string(output.join("mir/listing.asm")).expect("a listing");
        assert!(listing.contains("_twice"), "{listing}");
    }

    /// The listing the dump holds, wherever it is written.
    fn dumped_listing(output: &std::path::Path) -> String {
        let mut pending = vec![output.to_path_buf()];
        while let Some(directory) = pending.pop() {
            for entry in std::fs::read_dir(&directory).expect("lists") {
                let path = entry.expect("an entry").path();
                if path.is_dir() {
                    pending.push(path);
                } else if path.to_string_lossy().ends_with("listing.asm") {
                    return std::fs::read_to_string(path).expect("reads");
                }
            }
        }
        panic!("no listing in the dump")
    }

    #[test]
    fn test_the_dump_shows_the_code_the_compiler_emits() {
        // #136: --dump ran the legacy lowering, which drops noalias, so its
        // listing reloaded `src.y` in the loop that -S hoists it out of.
        let directory = tempfile::tempdir().expect("a directory");
        let source = directory.path().join("bump.nib");
        std::fs::write(&source, "struct P:\n    mut x: i16\n    y: i16\n\nfn bump(dst: &mut P, src: &P, n: i16) -> void:\n    for i in 0..n:\n        dst.x += src.y\n\nfn main() -> i16:\n    let mut a = P(x=0, y=0)\n    let b = P(x=0, y=3)\n    bump(a, b, 4)\n    return a.x\n").expect("writes");
        let output = directory.path().join("dump");
        // Unrolled, the 4-trip loop is gone and there is nothing to count.
        let argv = [source.display().to_string(), "-O2".into(), "-fno-inline-functions".into(), "-fno-unroll-loops".into(), "-fno-peel-loops".into(), "--dump".into(), output.display().to_string()];
        assert_eq!(crate::cli::main(&argv), 0);
        let listing = dumped_listing(&output);
        let body = listing.split("_bump proc far\n").nth(1).and_then(|one| one.split("_bump endp").next()).expect("bump");
        // The loop: from the label its backward jump names to that jump.
        let jump = regex::Regex::new(r"\n    j\w+ (L\d+_\d+)\n").unwrap();
        let (head, end) = jump
            .captures_iter(body)
            .map(|one| (one[1].to_owned(), one.get(0).unwrap().start()))
            .find(|(label, at)| body[..*at].contains(&format!("{label}:\n")))
            .expect("a loop");
        let looped = body[..end].split(&format!("{head}:\n")).nth(1).expect("the loop");
        assert_eq!(looped.matches("ptr").count(), 1, "{looped}");
    }
}
