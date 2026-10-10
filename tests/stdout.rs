//! `-o -` writes standard output, as gcc's does: llrm-c, llrm-nib and llrm-qb
//! each wrote a file named `-` in the working directory and printed nothing.

use std::path::{Path, PathBuf};
use std::process::Command;

fn bin(name: &str) -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_llrm-qb")).parent().unwrap().join(name)
}

#[test]
fn o_dash_writes_standard_output_in_every_frontend_and_no_file_named_dash() {
    let sources = [
        ("llrm-c", "p.c", "int add(int a, int b)\n{\n    return a + b;\n}\n"),
        ("llrm-nib", "p.nib", "fn main() -> i16:\n    return 3 + 4\n"),
        ("llrm-qb", "p.bas", "DIM total AS INTEGER\ntotal = 3 + 4\nPRINT total\n"),
    ];
    for (tool, source, text) in sources {
        let scratch = tempfile::tempdir().unwrap();
        std::fs::write(scratch.path().join(source), text).unwrap();
        let made = Command::new(bin(tool))
            .args(["-m16", "-S"])
            .arg(source)
            .args(["-o", "-"])
            .current_dir(scratch.path())
            .output()
            .unwrap();
        assert!(made.status.success(), "{tool}: {}", String::from_utf8_lossy(&made.stderr));
        let text = String::from_utf8_lossy(&made.stdout);
        assert!(text.contains("proc"), "{tool} wrote no assembly to standard output: {text:?}");
        assert!(!scratch.path().join("-").exists(), "{tool} wrote a file named -");
    }
}
