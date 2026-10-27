//! The pin's `llc` on the IR llrm-llc gives its instruction selection: the
//! listing llrm-cc's is compared with, instruction for instruction, where the
//! pass under test mirrors LLVM's. Needs the pin's `llc` (`tools/pairs/pin.py`)
//! and `LLRM_OW`; m32 only (the m16 pin has no `llc`).

#![allow(dead_code)]

use std::path::Path;
use std::process::Command;

/// The pin's listing (Intel syntax) of `source`: its `llc` over the IR llrm-llc
/// hands to instruction selection (the last stage of the dump) for the same
/// flags (llrm-cc's `--dump`), so both backends read one module. `flags` are
/// llrm-cc's (`-O2 -m32 ...`).
pub fn listing(
    source: &Path,
    flags: &[&str],
) -> String {
    // On disk: `--dump` refuses a tmpfs (thousands of small files).
    let scratch = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap();
    let dump = scratch.path().join("dump");
    let output = super::cc::llrm_c()
        .current_dir(scratch.path())
        .args(flags)
        .args(["-S", "-o", "a.s", "--dump"])
        .arg(&dump)
        .arg(source)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let ir = std::fs::read_dir(&dump)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "ll"))
        .max()
        .expect("a stage in the dump");
    let root = env!("CARGO_MANIFEST_DIR");
    let output = Command::new("python3").current_dir(root).args(["-I", "-c", SCRIPT]).arg(&ir).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8(output.stdout).unwrap()
}

/// `tools/pairs/pin.py`'s `llc` on the dump, told the target as it tells it.
const SCRIPT: &str = "
import sys, pathlib
sys.path.insert(0, 'tools/pairs')
import pin
text = pathlib.Path(sys.argv[1]).read_text()
done = pin.llc(['-O2', '-filetype=asm', '-x86-asm-syntax=intel', '-o', '-'], text, pin.target('m32'))
sys.stdout.write(done.stdout)
sys.stderr.write(done.stderr)
sys.exit(done.returncode)
";

/// One line of a function's body.
#[derive(Debug, PartialEq)]
pub enum Line {
    Label(String),
    Instruction(String),
}

/// The body of `function` (all the listing if `None`) in llrm-cc's or the
/// pin's syntax: labels and instructions, whitespace folded, comments and
/// directives dropped.
pub fn body(
    text: &str,
    function: Option<&str>,
) -> Vec<Line> {
    let starts = |line: &str| {
        function.is_some_and(|name| {
            let head = match line.strip_suffix(':') {
                Some(head) => head,
                None if line.contains(" proc") => line.split_whitespace().next().unwrap_or(""),
                None => return false,
            };
            head.trim_matches('_') == name.trim_matches('_')
        })
    };
    let mut inside = function.is_none();
    let mut out = vec![];
    for line in text.lines() {
        let line = line.split(['#', ';']).next().unwrap_or("").trim();
        if function.is_some() {
            if starts(line) {
                inside = true;
                continue;
            }
            if line.contains(" endp") || line.starts_with(".Lfunc_end") {
                inside = false;
            }
        }
        let first = line.split_whitespace().next().unwrap_or("");
        let skipped = line.is_empty()
            || line.starts_with('.') && !line.ends_with(':')
            || line.contains(" label ")
            || matches!(
                first,
                "align" | "public" | "extern" | "db" | "dw" | "dd" | "end" | "assume" | "model"
            );
        if !inside || skipped {
            continue;
        }
        if let Some(label) = line.strip_suffix(':') {
            out.push(Line::Label(label.to_owned()));
        } else {
            out.push(Line::Instruction(line.split_whitespace().collect::<Vec<_>>().join(" ")));
        }
    }
    out
}

/// The instructions of `body`.
pub fn instructions(
    text: &str,
    function: Option<&str>,
) -> Vec<String> {
    body(text, function)
        .into_iter()
        .filter_map(|line| match line {
            Line::Instruction(text) => Some(text),
            Line::Label(_) => None,
        })
        .collect()
}

pub fn jumps(instructions: &[String]) -> usize {
    instructions.iter().filter(|line| line.starts_with("jmp ")).count()
}

/// The length, in instructions, of each loop closed by a backward jump: from
/// its target label to the jump, both counted.
pub fn loops(
    text: &str,
    function: Option<&str>,
) -> Vec<usize> {
    let mut at: Vec<(String, usize)> = vec![];
    let mut count = 0;
    let mut out = vec![];
    for line in body(text, function) {
        match line {
            Line::Label(label) => at.push((label, count)),
            Line::Instruction(text) => {
                count += 1;
                let target = text.strip_prefix('j').and_then(|rest| rest.split_whitespace().nth(1));
                if let Some((_, from)) = target.and_then(|name| at.iter().find(|(label, _)| label == name)) {
                    out.push(count - from);
                }
            }
        }
    }
    out
}
