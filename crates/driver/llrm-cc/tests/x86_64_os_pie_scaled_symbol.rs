//! Arrays of stride 2 and 4 in a position independent loop each got a pointer of their own
//! (`add rax,0x2`, `add rcx,0x4`): the pin refuses a scaled index beside a symbol. One counter
//! with `[sym + i*4]` is what gcc and the non-PIE build make. Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn fill() -> Vec<String> {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("x86_64-os-pie-scaled-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let object = dir.join("a.o");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-Os", "-w", "-ffreestanding", "-c", "-o"])
        .arg(&object)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/os_pie_scaled_symbol.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let dump = Command::new("objdump").args(["-d", "--no-show-raw-insn", "-M", "intel"]).arg(&object).output().unwrap();
    String::from_utf8_lossy(&dump.stdout)
        .lines()
        .skip_while(|line| !line.contains("<fill>:"))
        .skip(1)
        .take_while(|line| line.contains(":\t"))
        .map(str::to_owned)
        .collect()
}

#[test]
fn arrays_of_stride_two_and_four_share_one_counter_in_a_pie_loop() {
    let body = fill();
    let text = body.join("\n");
    let bumps = body.iter().filter(|line| line.contains("add ") && (line.ends_with(",0x2") || line.ends_with(",0x4")));
    assert_eq!(bumps.count(), 0, "a pointer of its own per stride:\n{text}");
    assert!(text.contains("*4]") || text.contains("*4+"), "the scaled index of the int arrays:\n{text}");
}
