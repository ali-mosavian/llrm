fn f() {
    let a = matches!(
        insn.opcode,
        Opcode::Load | Opcode::Store | Opcode::Call if insn.width == "dword  wide"
    );
    let b = matches!(x, Some(_));
}
