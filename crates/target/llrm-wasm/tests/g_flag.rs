//! `-g` on this target's compiler: a flag the driver knows and cannot do yet
//! is said so, not called unrecognized (`g-for-all-targets-card.md` step 1).

use std::process::Command;

/// `llrm-cc -gdwarf-4` on this target failed with the compiler's `unrecognized
/// argument -g`, as if the flag were a typo.
#[test]
fn a_debug_flag_this_target_does_not_write_is_refused_as_not_yet_written_not_as_unrecognized() {
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc")).args(["-gdwarf-4", "x.ll"]).output().unwrap();
    let stderr = String::from_utf8_lossy(&said.stderr);
    assert_eq!(said.status.code(), Some(2), "{stderr}");
    assert!(stderr.contains("llrm-wasm-llc: -gdwarf-4: no debug writer for wasm yet"), "{stderr}");
}

/// `-g -filetype=obj` wrote the same object as without: no `.debug_line` in it.
#[test]
fn g_writes_a_line_table_into_the_object() {
    let out = std::env::temp_dir().join(format!("wasm-llc-g-{}.o", std::process::id()));
    let ir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/ir/debug_lines.ll");
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-g", "-filetype=obj", "-o"])
        .arg(&out)
        .arg(ir)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    let bytes = std::fs::read(&out).unwrap();
    let _ = std::fs::remove_file(&out);
    assert!(bytes.windows(11).any(|w| w == b".debug_line"));
}
