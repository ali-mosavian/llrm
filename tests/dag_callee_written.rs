//! A call to a finished static callee destroys what the callee wrote, and no
//! more.

mod common;

/// `caller` keeps `b`, `c` and `d` across a call to a static function that
/// writes only memory. Under `--isel=dag` the call destroyed the convention's
/// whole list instead, so `caller` saved SI and DI and moved two arguments into
/// them (14 instructions, two more than the old selector's body).
#[test]
fn test_a_dag_call_keeps_the_registers_its_callee_did_not_write() {
    let scratch = tempfile::tempdir().unwrap();
    let source = scratch.path().join("a.c");
    std::fs::write(
        &source,
        "extern int sink;\n\
         static __attribute__((noinline)) void leaf(int a) { sink = a; }\n\
         int caller(int a, int b, int c, int d) { leaf(a); return b + c + d; }\n",
    )
    .unwrap();
    let output = common::cc::llrm_cc()
        .current_dir(scratch.path())
        .args(["-m16", "-march=i386", "-Os", "--isel=dag", "-S", "-o", "a.s"])
        .arg(&source)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let caller = text.split_once("_caller@3 proc").unwrap().1.split_once("endp").unwrap().0;
    assert!(
        !caller.contains("push si") && !caller.contains("push di"),
        "caller saved registers it need not:\n{caller}"
    );
}
