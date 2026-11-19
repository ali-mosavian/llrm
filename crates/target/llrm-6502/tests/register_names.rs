use llrm_codegen::greedy::virtregmap::RegisterInfo;
use llrm_lir::registers::Regs;

/// `LLRM_GREEDY_DECISIONS` printed a 6502 interval's register through iced's
/// names (`assign R12W` for a zero-page cell), so no line could be read
/// against the asm.
#[test]
fn a_6502_register_is_named_by_its_own_file_in_the_allocator_trace() {
    let info = RegisterInfo::new(Regs(&llrm_6502::REGISTER_INFO));
    let mut differs = 0;
    for register in llrm_6502::REGISTER_INFO.bytes.iter() {
        differs +=
            usize::from(format!("{register:?}") != llrm_6502::REGISTER_INFO.name(*register).unwrap().to_uppercase());
        let own = llrm_6502::REGISTER_INFO.name(*register).expect("a named register");
        assert_eq!(info.name(*register), own.to_uppercase());
    }
    assert!(differs > 0, "Debug already names them: the test would not have failed");
}
