//! At -Os in real mode a popped x87 argument is stored into an area of its
//! own and the word beside it is pushed (SD405). All the arguments stored into
//! one shared area cost 14 bytes here against 10.

#[test]
fn a_float_and_a_word_argument_are_an_area_and_a_push_at_os() {
    let fixture = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/x87/arg_push_m16.ll");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_llrm-llc"))
        .args(["-Os", "-mabi=cdecl", "-S", "-o", "-", fixture])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("push word ptr [bp+4]"), "{text}");
    assert!(!text.contains("mov word ptr [bx], ax"), "{text}");
}
