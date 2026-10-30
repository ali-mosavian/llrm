//! A load through the default data segment sinks into the arm that uses it.

mod common;

/// `g` is read only after `a > 0`. The load reads DS here and counted as machine
/// state, so it stayed above the first branch (`mov edx,[g]` before `jle`,
/// 1 instruction on every call, a register held across the branch); the
/// pin's `MOV32rm` names no segment register and sinks it. jpeg_fixed bq32 -Os
/// had its loads of globals above branches, as in this function.
#[test]
fn a_load_of_a_global_sinks_below_the_branch_that_guards_its_use() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(
        scratch.path().join("a.c"),
        "int g;\nextern void h(void);\nint f(int a)\n{\n    if (a <= 0)\n        return 1;\n    if (g != 0)\n        return 1;\n    h();\n    return 2;\n}\n",
    )
    .unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["-m32", "-march=i486", "-O2", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let asm = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let body: Vec<&str> = asm.lines().skip_while(|line| !line.starts_with("f_ proc")).collect();
    let branch = body.iter().position(|line| line.trim_start().starts_with('j')).expect("a branch in f");
    let load = body.iter().position(|line| line.contains("_g") && !line.contains("label")).expect("a use of g in f");
    assert!(load > branch, "the load of g is above the first branch:\n{}", body.join("\n"));
}
