use std::path::Path;
use std::process::Command;

/// Code-section bytes of the object `llrm-wasm-llc` writes for `ir` at `level`.
fn code_bytes(
    ir: &str,
    level: &str,
) -> usize {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir").join(ir);
    let dir = std::env::temp_dir().join(format!("code_not_above_pin_{}_{level}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let obj = dir.join("x.o");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args([&format!("-{level}"), "-filetype=obj", "-o"])
        .arg(&obj)
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let bytes = std::fs::read(&obj).unwrap();
    std::fs::remove_dir_all(&dir).ok();
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

/// WSZ-7 and WSZ-11 listed these cells as above the pin (+18 B at -O1/-Os,
/// +4 B) after they had closed, and nothing would have shown a reopening. The
/// pin's `clang --target=wasm32 -O1/-Os -c` gives 0x1f, 0xd6 and 0xb4 bytes of
/// code (pin d1106deb71cc, `tests/run/c`).
#[test]
fn the_wsz_7_and_wsz_11_cells_stay_at_or_under_the_pin() {
    for level in ["O1", "Os"] {
        let bytes = code_bytes("byvalue_struct_address.ll", level);
        assert!(bytes <= 0x1f, "byvalue_struct_address -{level}: {bytes} B, pin 31");
    }
    let bytes = code_bytes("byte_multiply.ll", "O1");
    assert!(bytes <= 0xd6, "byte_multiply -O1: {bytes} B, pin 214");
    let bytes = code_bytes("byte_multiply.ll", "Os");
    assert!(bytes <= 0xb4, "byte_multiply -Os: {bytes} B, pin 180");
}
