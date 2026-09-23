//! Port of `tools/modernstages.py`: dump every implemented modern-language
//! frontend stage to text files.

use std::path::{Path, PathBuf};
use std::process::Command;

use super::compile as modern;
use super::driver::{self, FrontendError};
use crate::backend::cpu as targets;
use crate::frontends::qb::abi::physicalize;
use crate::hir;
use crate::model::passes::Options;

/// Python's text-mode read: universal newlines.
fn _text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).replace("\r\n", "\n").replace('\r', "\n")
}

/// Run one diagnostic frontend boundary and return its complete output.
fn _frontend_text(source: &Path, option: &str) -> Result<String, FrontendError> {
    let command = driver::command();
    let result = Command::new(&command[0])
        .args(&command[1..])
        .arg(option)
        .arg(source)
        .current_dir(driver::ROOT())
        .output()
        .map_err(|error| FrontendError(format!("could not start modern frontend: {error}")))?;
    if !result.status.success() {
        let stderr = _text(&result.stderr).trim().to_owned();
        let message = if stderr.is_empty() {
            format!("modernfront exited with status {}", result.status.code().unwrap_or(-1))
        } else {
            stderr
        };
        return Err(FrontendError(message));
    }
    Ok(_text(&result.stdout))
}

fn write(path: &Path, text: &str) -> Result<(), String> {
    std::fs::write(path, text).map_err(|error| error.to_string())
}

/// Write source, lexical, syntax, HIR, and semantic-MIR snapshots.
pub fn dumped(source: &Path, output: &Path, options: &Options) -> Result<PathBuf, String> {
    std::fs::create_dir_all(output).map_err(|error| error.to_string())?;
    let program = driver::parsed(source, None).map_err(|error| error.0)?;
    let lowered = modern::semantic_lowered(&program)?;
    let target = targets::profile("386")?;

    let input = std::fs::read(source).map_err(|error| error.to_string())?;
    write(&output.join("00-input.mod"), &_text(&input))?;
    write(&output.join("01-tokens.txt"), &_frontend_text(source, "--tokens").map_err(|error| error.0)?)?;
    write(&output.join("02-syntax.txt"), &_frontend_text(source, "--syntax").map_err(|error| error.0)?)?;
    write(&output.join("03-hir.json"), &hir::encode(&program, Some(2)).map_err(|error| error.to_string())?)?;
    let mut mir_files = Vec::new();
    let mut number = 4;
    assert_eq!(program.modules[0].functions.len(), lowered.len(), "zip(strict=True)");
    for (function, semantic) in program.modules[0].functions.iter().zip(&lowered) {
        let name = semantic.name.replace('.', "-");
        let optimized = modern::optimized(&program, function, semantic, target, None, options)?;
        let physical = physicalize(&program, function, &optimized).map_err(|error| error.to_string())?;
        let optimized_physical =
            modern::optimized(&program, function, &physical.lowered, target, Some(&physical.calls), options)?;
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
                "README.txt",
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
        assert!(optimized_mir.contains("call _pf4"));
        assert!(source_mir.contains("mul"));
        assert_ne!(physical_mir, optimized_physical_mir);
    }
}
