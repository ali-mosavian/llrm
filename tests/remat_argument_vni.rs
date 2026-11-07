//! A stack argument read back after a call is a load from its slot, not a
//! spill: the pin remakes the load per value number.

mod common;

/// `f` kept its first parameter in a register across the call to `g` by storing
/// it to a new slot (`mov [esp+4], eax`, a frame and a reload) because the
/// register is also redefined in place (`add`): 5 B over the pin, which reloads
/// from the incoming slot.
#[test]
fn an_argument_read_after_a_call_is_reloaded_from_its_slot_not_spilled() {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(
        directory.join("a.c"),
        "int g(int, int);\n\
         int f(int a, int b, int c) {\n\
           if (b > 254) return a | 0x7f800000;\n\
           if (b < 1) c = g(c, 1 - b);\n\
           return a + (b << 23) + c;\n\
         }\n",
    )
    .unwrap();
    let output = common::cc::llrm_c()
        .current_dir(directory)
        .args(["--target=x86_32-linux", "-Os", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(directory.join("a.s")).unwrap();
    assert!(!text.contains("sub esp"), "a frame for a spill slot: {text}");
}
