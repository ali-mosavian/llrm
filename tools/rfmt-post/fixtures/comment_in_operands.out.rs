fn f() {
    let found = index.instruction(id, instruction).is_some_and(|i| {
        (0..i.operands.len() as i64).contains(&operand)
            // What a callee does with a pointer is stated of a call's argument.
            && (i.op == model::Op::Call || matches!(fact, llrm_mir::facts::Fact::InBounds))
    });
}
