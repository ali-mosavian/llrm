//! The `sub`/`cmp` a branch reads sank past four `fld`s: MachineSink took the
//! x87 status lanes of `fld` for EFLAGS being rewritten. QCport's `pl_move`
//! then took `jl` on stale flags and the DAG build drew another picture.

#[test]
fn a_compare_stays_before_the_branch_on_it_with_x87_arguments_loaded_between() {
    let fixture = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/x87/args_across_a_branch.ll");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_llrm-llc"))
        .args(["-Os", "-mabi=cdecl", "-S", "-o", "-", fixture])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = String::from_utf8(output.stdout).unwrap();
    let entry: Vec<&str> = text.lines().skip_while(|line| !line.starts_with("_f proc")).collect();
    let branch = entry.iter().position(|line| line.trim_start().starts_with('j')).expect("a branch");
    assert!(entry[..branch].iter().any(|line| line.trim_start().starts_with("cmp ")), "{text}");
}
