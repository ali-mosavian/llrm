/// `sysv32` kept the DOS conventions' 8: a call from ELF m32 code left the
/// stack 8-aligned where the System V i386 ABI (and gcc/clang) keep 16.
#[test]
fn sysv32_keeps_the_stack_16_aligned_at_a_call() {
    let calling = llrm_target::calling::Calling::parse(include_str!("../src/machines/calling.toml"))
        .unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(calling.named("sysv32").expect("sysv32").stack_alignment, Some(16));
}
