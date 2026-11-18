use std::path::Path;
use std::process::Command;

/// Code-section bytes of the object `llrm-wasm-llc` writes for `ir` at `-O1`,
/// the IR going to the backend as it is.
fn code_bytes(ir: &str) -> usize {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir").join(ir);
    let obj = std::env::temp_dir().join(format!("copy_sinks_{}.o", std::process::id()));
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-O1", "-filetype=obj", "-o"])
        .arg(&obj)
        .arg(ir)
        .env("LLRM_WASM_BACKEND_ONLY", "1")
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let bytes = std::fs::read(&obj).unwrap();
    std::fs::remove_file(&obj).ok();
    let mut at = 8;
    while at < bytes.len() {
        let id = bytes[at];
        let (mut size, mut shift) = (0usize, 0);
        at += 1;
        loop {
            let byte = bytes[at];
            at += 1;
            size |= usize::from(byte & 0x7f) << shift;
            shift += 7;
            if byte & 0x80 == 0 {
                break;
            }
        }
        if id == 10 {
            return size;
        }
        at += size;
    }
    panic!("no code section");
}

/// The Aho-Corasick build loop of bench/grep at -O1 was 0x12e bytes against
/// the pin's llc on the same IR at 0x12a, and `grep` ran 5,856 instructions
/// over: a loaded byte read in two other blocks was copied to a second
/// register at its definition. The pin defines the byte's own register with
/// the load and moves the copy of its zero-extension to the block that reads
/// it (`MachineSinking`); the copy is a `COPY` there, and was not movable as
/// `COPY_I32`. Which copy the load defines follows the order of the load's
/// uses (`CreateVirtualRegisters`).
#[test]
fn a_copy_of_a_loaded_byte_moves_to_the_block_that_reads_it() {
    let bytes = code_bytes("bfs_copy_sink.ll");
    assert!(bytes <= 0x12a, "{bytes} B, the pin's llc 298 B");
}
