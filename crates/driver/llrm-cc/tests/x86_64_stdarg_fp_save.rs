//! `sum_longs(n, ...)` saved the eight vector argument registers behind `test
//! al, al` and stored `fp_offset` (189 B in arg_registers_abi `-Os`, gcc 111):
//! gcc's `pass_stdarg` finds that no `va_arg` takes a floating-point value and
//! drops both. A `double` reader or a list passed on keeps them. Needs
//! `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn listing(function: &str) -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("x86_64-stdarg-fp-{}-{function}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let object = dir.join("a.o");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-Os", "-w", "-ffreestanding", "-c", "-o"])
        .arg(&object)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/stdarg_fp_save.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let dump = Command::new("objdump").args(["-d", "--no-show-raw-insn", "-M", "intel"]).arg(&object).output().unwrap();
    String::from_utf8_lossy(&dump.stdout)
        .lines()
        .skip_while(|line| !line.contains(&format!("<{function}>:")))
        .skip(1)
        .take_while(|line| line.contains(":\t"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn a_function_that_reads_no_double_saves_no_vector_register() {
    let text = listing("sum_longs");
    assert!(!text.contains("movaps") && !text.contains("test   al,al"), "the vector save stayed:\n{text}");
}

#[test]
fn a_function_that_reads_a_double_keeps_the_vector_save() {
    let text = listing("sum_doubles");
    assert!(text.contains("movaps"), "{text}");
}

#[test]
fn a_function_that_passes_its_list_on_keeps_the_vector_save() {
    let text = listing("passes_the_list");
    assert!(text.contains("movaps"), "{text}");
}
