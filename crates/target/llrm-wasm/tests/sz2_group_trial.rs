use std::path::Path;
use std::process::Command;

/// `-Os` kept all three `message("p")`, `message("a")`, `message("b")` calls of
/// switch_header_marker_loop: SZ2 tried the sites of a callee that stays one at
/// a time, and a single copy is no smaller than its call. The pin inlines all
/// three and the copies end in one `report` (-Os 314 B against its 276).
#[test]
fn the_sites_of_a_staying_callee_are_tried_together_at_os() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/sz2_group_trial.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-Os", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let asm = String::from_utf8_lossy(&out.stdout).into_owned();
    let from = asm.find("\nscan:").expect("scan");
    let scan = asm[from..].split("end_function").next().unwrap();
    assert!(!scan.contains("call\tmessage"), "{scan}");
}

/// -O1 kept the three `message` calls of `scan`: the size bound refused each site and no trial
/// followed, so the copies that fold into one `report` never formed (wasm32 -O1, the pin inlines them).
#[test]
fn the_sites_of_a_staying_callee_are_tried_together_at_o1() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/sz2_group_trial.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O1", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let asm = String::from_utf8_lossy(&out.stdout).into_owned();
    let from = asm.find("\nscan:").expect("scan");
    let scan = asm[from..].split("end_function").next().unwrap();
    assert!(!scan.contains("call\tmessage"), "{scan}");
}
