//! Every frame cell names the slot it lies in once instruction selection has tagged them, and the verifier fails a compile
//! in which a phase makes one without. Each program here made one: the end pointer of a strength-reduced loop one past an
//! array (matmul), a pointer before an array's start (quicksort), a word slot loopslots promoted (sieve), a frame address
//! folded into an indexed cell (the spiller; the nib loop test of llrm-nib).

use std::process::Command;

fn compiles(program: &str, flags: &[&str]) {
    let scratch = tempfile::tempdir().unwrap();
    let source = format!("{}/bench/{program}/{program}.c", env!("CARGO_MANIFEST_DIR"));
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c")).current_dir(scratch.path()).args(flags).args(["-o", "a.obj", &source]).output().unwrap();
    assert!(output.status.success(), "{program} {flags:?}: {}", String::from_utf8_lossy(&output.stderr));
}

#[test]
fn test_frame_cells_keep_their_slot_through_the_machine_phases() {
    for program in ["matmul", "sieve", "quicksort", "nbody", "lru"] {
        for level in ["-O0", "-O2", "-Os"] {
            compiles(program, &["-m32", "-march=i486", level]);
        }
    }
}
