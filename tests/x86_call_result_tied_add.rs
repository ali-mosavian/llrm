//! The sum of two call results ties the earlier result, not the later one.

mod common;

/// `rep(c1() + c2())` was `mov rbx, rax; call c2; add rax, rbx; mov rdi, rax`:
/// the later result, still in rax, was tied, so every further sum of a chain
/// got a copy and a second callee-saved register (div_constants_long x86-64
/// -O1, `main` 181 B against the pin's 158). A call's own result lost its
/// source register in `remove_clobbered_src_reg_map`, so the commute rule never
/// saw it arrive in rax. The pin: `add rbx, rax; mov rdi, rbx`.
#[test]
fn the_sum_of_two_call_results_is_added_into_the_earlier_one() {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(
        directory.join("a.c"),
        "long c1(void); long c2(void); void rep(long);\nvoid f(void) { rep(c1() + c2()); }\n",
    )
    .unwrap();
    let output = common::cc::llrm_c()
        .current_dir(directory)
        .args(["--target=x86_64-linux", "-O1", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(directory.join("a.s")).unwrap();
    let adds: Vec<&str> = text.lines().map(str::trim).filter(|line| line.starts_with("add ")).collect();
    assert_eq!(adds.len(), 1, "{text}");
    assert!(adds[0].ends_with(", rax") && !adds[0].starts_with("add rax"), "{text}");
}
