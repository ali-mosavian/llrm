//! `DependenceAnalysis` against the pinned LLVM's own tests: `tests/da/*.ll`
//! are `llvm/test/Analysis/DependenceAnalysis` without their CHECK lines, and
//! `*.expected` what `opt -passes=print<da>` answered for each of their RUN
//! lines (`tools/da_oracle.py --refresh` writes them).

use std::fmt::Write as _;
use std::path::Path;

use super::print::{PrintOptions, print_module_da};

/// The answers of one `print<da>` text, as `da_oracle.py` keeps them: per
/// function, each pair's answer line and the runtime assumptions under it.
fn answers(text: &str) -> String {
    let mut out = String::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("Printing analysis 'Dependence Analysis' for function '") {
            let _ = writeln!(out, "@{}", rest.split('\'').next().expect("a name"));
        } else if let Some(answer) = line.strip_prefix("  da analyze - ") {
            let _ = writeln!(out, "  {answer}");
        } else if line.starts_with("  ") && !line.starts_with("Src:") {
            let _ = writeln!(out, "  {line}");
        }
    }
    out
}

/// Every run of every file of the corpus: the options after `==`, then the
/// pin's answers.
#[test]
fn every_run_of_the_pins_tests_answers_as_the_pin_does() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/da");
    let mut files: Vec<_> = std::fs::read_dir(&directory)
        .expect("tests/da")
        .map(|entry| entry.expect("an entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "ll"))
        .collect();
    files.sort();
    assert!(files.len() > 50, "the corpus is there: {}", files.len());

    let mut wrong = Vec::new();
    for path in files {
        let module =
            llrm_mir::parse::module(&std::fs::read_to_string(&path).expect("a file")).expect("the corpus parses");
        let expected = std::fs::read_to_string(path.with_extension("expected")).expect("an expected file");
        for run in format!("\n{expected}").split("\n== ").skip(1) {
            let (options, body) = run.split_once('\n').expect("a run");
            let mut flags: Vec<String> = options.split_whitespace().map(str::to_owned).collect();
            let print = PrintOptions::take_flags(&mut flags).expect("known flags");
            assert!(flags.is_empty(), "{flags:?}");
            let got = answers(&print_module_da(&module, &print));
            if got.trim_end() != body.trim_end() {
                wrong.push(format!("{} [{}]", path.file_name().expect("a name").to_string_lossy(), options));
            }
        }
    }
    assert!(wrong.is_empty(), "differ from the pin: {wrong:?}");
}
