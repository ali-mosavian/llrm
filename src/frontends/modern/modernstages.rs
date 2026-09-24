//! Port of `tools/modernstages.py`: dump every implemented modern-language
//! frontend stage to text files.

use std::path::{Path, PathBuf};
use std::rc::Rc;

use super::compile as modern;
use super::driver;
use crate::backend::cpu::{self as targets, ProfileOrName};
use crate::backend::masm;
use crate::frontends::qb::abi::physicalize;
use crate::hir;
use crate::model::mir::MirBody;
use crate::model::passes::Options;
use crate::tools::stages;

/// Python's text-mode read: universal newlines.
fn _text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).replace("\r\n", "\n").replace('\r', "\n")
}

fn write(path: &Path, text: &str) -> Result<(), String> {
    std::fs::write(path, text).map_err(|error| error.to_string())
}

/// Runs `optimize`, writing the body after each pass to `passes/RUN/NN-PASS.txt` (rule 4).
fn passes<T>(
    output: &Path,
    run: &str,
    optimize: impl FnOnce(&mut dyn FnMut(&str, &MirBody)) -> Result<T, String>,
) -> Result<T, String> {
    let directory = output.join("passes").join(run);
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let mut seen = Vec::new();
    let done = optimize(&mut |stage: &str, body: &MirBody| seen.push((stage.to_owned(), body.clone())))?;
    for (number, (stage, body)) in seen.into_iter().enumerate() {
        let (text, _) = stages::mir_stage(&stage, &[(run.to_owned(), Rc::new(body))], None, None, None, true, true);
        write(&directory.join(format!("{number:03}-{stage}.txt")), &text)?;
    }
    Ok(done)
}

/// Write source, lexical, syntax, HIR, and semantic-MIR snapshots.
pub fn dumped(source: &Path, output: &Path, options: &Options) -> Result<PathBuf, String> {
    std::fs::create_dir_all(output).map_err(|error| error.to_string())?;
    let program = driver::parsed(source, None).map_err(|error| error.0)?;
    let lowered = modern::semantic_lowered(&program)?;
    let target = targets::profile(modern::CPU)?;

    let input = std::fs::read(source).map_err(|error| error.to_string())?;
    write(&output.join("00-input.mod"), &_text(&input))?;
    let text = String::from_utf8_lossy(&input);
    let refused = |error| driver::refused(source, &error).0;
    write(&output.join("01-tokens.txt"), &super::tokens_text(&text).map_err(refused)?)?;
    write(&output.join("02-syntax.txt"), &super::syntax_text(&text).map_err(refused)?)?;
    write(&output.join("03-hir.json"), &hir::encode(&program, Some(2)).map_err(|error| error.to_string())?)?;
    let mut mir_files = Vec::new();
    let mut number = 4;
    assert_eq!(program.modules[0].functions.len(), lowered.len(), "zip(strict=True)");
    for (function, semantic) in program.modules[0].functions.iter().zip(&lowered) {
        let name = semantic.name.replace('.', "-");
        let optimized = passes(output, &format!("{name}-optimized"), |watch| {
            modern::watched(&program, function, semantic, target, None, options, Some(watch))
        })?;
        let physical = physicalize(&program, function, &optimized).map_err(|error| error.to_string())?;
        let optimized_physical = passes(output, &format!("{name}-optimized-physical"), |watch| {
            modern::watched(&program, function, &physical.lowered, target, Some(&physical.calls), options, Some(watch))
        })?;
        let stages = [
            ("source", semantic),
            ("optimized", &optimized),
            ("physical", &physical.lowered),
            ("optimized-physical", &optimized_physical),
        ];
        for (stage, body) in stages {
            let filename = format!("{number:02}-{name}-{stage}-mir.txt");
            write(&output.join(&filename), &hir::mir_text(body))?;
            mir_files.push(format!("{filename}  {stage} MIR for {}", semantic.name));
            number += 1;
        }
    }

    // The emitted code, of a program with its entry or a library with exports.
    let functions = &program.modules[0].functions;
    if functions.iter().any(|function| function.name == "main" || function.linkage == hir::model::FunctionLinkage::External) {
        let module = modern::assembled(&program, "main", ProfileOrName::Name(modern::CPU), options)?;
        let filename = format!("{number:02}-listing.asm");
        write(&output.join(&filename), &masm::text(&module).map_err(|error| error.0)?)?;
        mir_files.push(format!("{filename}  the program as emitted, for {}", modern::CPU));
    }

    let mut files = vec![
        "00-input.mod       exact source presented to the frontend".to_owned(),
        "01-tokens.txt      lexer output with source positions".to_owned(),
        "02-syntax.txt      indentation-aware syntax tree".to_owned(),
        "03-hir.json        verified, source-neutral common HIR".to_owned(),
    ];
    files.extend(mir_files);
    write(
        &output.join("README.txt"),
        &format!(
            "Modern frontend stage dumps\n===========================\n\n{}\n\n\
             Native compilation runs the common MIR fixed point before and after ABI\n\
             physicalization, then continues through legalization, LIR, allocation, and emission.\n",
            files.join("\n")
        ),
    )?;
    Ok(output.to_path_buf())
}

#[cfg(test)]
mod tests {
    //! Port of `tests/test_modernstages.py`.

    use super::dumped;
    use crate::frontends::modern::test_modern_frontend::fixture;
    use crate::model::passes::O2;
    use crate::support::pyjson::{self, Json};

    #[test]
    fn test_nbody_stage_dumps_cover_every_implemented_boundary() {
        // nbody used to expose HIR and MIR only through separate ad-hoc commands.
        let directory = tempfile::tempdir().expect("a directory");
        let nbody = fixture("nbody.mod");
        let output = dumped(&nbody, &directory.path().join("nbody"), &O2()).expect("dumps");

        let mut names: Vec<String> = std::fs::read_dir(&output)
            .expect("lists")
            .map(|one| one.expect("an entry").file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(
            names,
            [
                "00-input.mod",
                "01-tokens.txt",
                "02-syntax.txt",
                "03-hir.json",
                "04-nbody-nbody-source-mir.txt",
                "05-nbody-nbody-optimized-mir.txt",
                "06-nbody-nbody-physical-mir.txt",
                "07-nbody-nbody-optimized-physical-mir.txt",
                "08-nbody-main-source-mir.txt",
                "09-nbody-main-optimized-mir.txt",
                "10-nbody-main-physical-mir.txt",
                "11-nbody-main-optimized-physical-mir.txt",
                "12-listing.asm",
                "README.txt",
                "passes",
            ]
        );
        let read = |name: &str| std::fs::read_to_string(output.join(name)).expect("dumped");
        assert_eq!(std::fs::read(output.join("00-input.mod")).unwrap(), std::fs::read(&nbody).unwrap());
        assert!(read("01-tokens.txt").contains("Fixed"));
        let syntax = read("02-syntax.txt");
        assert!(syntax.contains("Struct {\n            name: \"body\""));
        assert!(syntax.contains("ForRange {"));
        let Json::Dict(document) = pyjson::loads(&read("03-hir.json")).expect("JSON") else { panic!("an object") };
        assert_eq!(document.get("schema"), Some(&Json::Int(1)));
        let source_mir = read("04-nbody-nbody-source-mir.txt");
        let optimized_mir = read("05-nbody-nbody-optimized-mir.txt");
        let physical_mir = read("06-nbody-nbody-physical-mir.txt");
        let optimized_physical_mir = read("07-nbody-nbody-optimized-physical-mir.txt");
        assert!(source_mir.contains("function nbody.nbody"));
        assert!(optimized_mir.contains(&format!("call {}", crate::abi::modern::PRINT_Q4)));
        assert!(source_mir.contains("mul"));
        assert_ne!(physical_mir, optimized_physical_mir);
    }

    #[test]
    fn test_a_library_without_main_dumps_its_listing() {
        // A library had no listing, so the runtime's code could not be read.
        let directory = tempfile::tempdir().expect("a directory");
        let source = directory.path().join("lib.mod");
        std::fs::write(&source, "export \"cdecl16\":\n    fn twice(value: i16) -> i16:\n        return value * 2\n").expect("writes");
        let output = dumped(&source, &directory.path().join("dump"), &O2()).expect("dumps");
        let listing = std::fs::read_to_string(output.join("08-listing.asm")).expect("a listing");
        assert!(listing.contains("_twice"), "{listing}");
    }
}
