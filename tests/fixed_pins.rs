//! A value an instruction pins to a register is that
//! register's fixed range, not an allocatable one, so a loop counter used as a
//! shift count lives in ecx (the pin's `$cl = COPY k`).

mod common;

const SOURCE: &str = "int f(int x, int y, int n)
{
    int k, s = 0;
    for (k = 0; k < n; ++k) {
        s += (x >> k) ^ (y >> k);
        x += s;
        y -= s;
    }
    return s;
}
";

fn listing(fixed: bool) -> String {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), SOURCE).unwrap();
    let mut run = common::cc::llrm_c();
    run.current_dir(scratch.path()).args([
        "-m32",
        "-mabi=sysv",
        "-O2",
        "-march=i486",
        "--isel=dag",
        "-S",
        "-o",
        "a.s",
        "a.c",
    ]);
    run.env_remove("LLRM_PINS_AS_VREGS");
    if !fixed {
        run.env("LLRM_PINS_AS_VREGS", "1");
    }
    let done = run.output().unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    std::fs::read_to_string(scratch.path().join("a.s")).unwrap()
}

fn loop_body(asm: &str) -> Vec<&str> {
    asm.lines().skip_while(|line| !line.contains("_f proc")).take_while(|line| !line.contains("_f endp")).collect()
}

/// A counter used as a shift count sat in edx and was copied to ecx every trip
/// (`mov ecx, edx`): +1 mov per trip on x_cordic, 4.4% of its clocks.
#[test]
fn a_shift_count_lives_in_ecx_with_no_copy_per_trip() {
    let before = loop_body(&listing(false)).iter().filter(|line| line.trim_start().starts_with("mov ecx, e")).count();
    assert!(before > 0, "premise: with pinned values as registers the count is copied into ecx");
    let asm = listing(true);
    let after = loop_body(&asm);
    assert!(after.iter().any(|line| line.contains(", cl")), "a shift by cl: {asm}");
    assert!(!after.iter().any(|line| line.trim_start().starts_with("mov ecx, e")), "a copy into ecx: {asm}");
}

/// tests/run/c/switch_table_holes.c at -Os: the loop counter `i`, a shift count
/// pinned to cl, was pushed out of ecx by its own pinned copy, took eax and was
/// copied back to ecx every trip (`mov ecx, eax`), which in turn left no
/// register for the sum.
#[test]
fn a_pinned_copy_does_not_push_its_source_out_of_the_register() {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/run/c/switch_table_holes.c");
    let copies = |fixed: bool| {
        let scratch = tempfile::tempdir().unwrap();
        let mut run = common::cc::llrm_c();
        run.current_dir(scratch.path()).args([
            "-m32",
            "-mabi=sysv",
            "-Os",
            "-march=i486",
            "--isel=dag",
            "-S",
            "-o",
            "a.s",
        ]);
        run.arg(&source).env_remove("LLRM_PINS_AS_VREGS");
        if !fixed {
            run.env("LLRM_PINS_AS_VREGS", "1");
        }
        let done = run.output().unwrap();
        assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
        let asm = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
        asm.lines().filter(|line| line.trim_start().starts_with("mov ecx, e")).count()
    };
    assert!(copies(false) > 0, "premise: with pinned values as registers the counter is copied into ecx");
    assert_eq!(copies(true), 0);
}
