use std::path::Path;
use std::process::Command;

/// The pin's -O1 has no GVN: `getbit` keeps four loads of `inpos` (one per
/// inlined `next`'s test and increment). The write-through `Promote` forwarded
/// a store across blocks with phis, leaving two: jpeg_fixed -O1 was 259 B over.
/// `next` is `alwaysinline` in the IR: the test is about the loads, not the -O1 inline
/// policy (the size hold keeps `next` out of `getbit`, so it read 0 loads, not 4).
#[test]
fn a_global_stored_in_one_block_is_loaded_again_in_the_next_at_o1() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/getbit_inpos.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O1", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let asm = String::from_utf8_lossy(&out.stdout);
    let (getbit, _) = asm.split_once("end_function").expect("a function");
    assert_eq!(getbit.matches("i32.load\tinpos").count(), 4, "{getbit}");
}
