//! A dump that calls a value by another's name is wrong, however plain it
//! reads. `total`'s argument showed as `sum.2.lcssa` after register
//! allocation: the coalescer had merged the argument with the value returned in
//! the same register, and the allocator gave the argument's number a new
//! value that had the name of one merged away.

use std::process::Command;

const TOTAL: &str = "fn total(values: &[u16]) -> u16:
    let mut sum: u16 = 0
    for value in &values:
        if value == 4:
            return sum
        sum += value
    return sum

fn main() -> i16:
    let values: u16[3] = [1, 2, 3]
    let t = total(&values)
    return 0
";

#[test]
fn an_argument_is_never_called_by_a_value_it_was_merged_with() {
    let source = std::env::temp_dir().join(format!("lir-names-{}.nib", std::process::id()));
    std::fs::write(&source, TOTAL).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-nib"))
        .args(["-m16", "-O2", "-fno-inline-functions", "-fno-inline-functions-called-once", "-S", "-o", "/dev/null"])
        .arg(&source)
        .env("ISEL_DUMP", "1")
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&source);
    let dump = String::from_utf8_lossy(&run.stdout);
    let lines: Vec<&str> = dump.lines().filter(|line| line.starts_with("fn total(")).collect();
    assert!(lines.len() > 10, "{dump}");
    for line in lines {
        assert!(!line.contains("sum"), "the argument is called {line:?}");
    }
}
