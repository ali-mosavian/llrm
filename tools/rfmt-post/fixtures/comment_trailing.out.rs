pub struct Indexes {
    // id(insn) -> the instruction's first slot, in a note that runs past column
    // 80
    pub at: IndexMap<usize, i64>,
    pub span: IndexMap<i64, (i64, i64)>, // block address -> [first, last)
    // block addresses, in the order they are numbered, and a note long enough
    // to pass 80
    pub order: Vec<i64>,
}

fn f(operand: &Operand) -> Operand {
    if operand.is_none() {
        // refuses below, which is the safe answer, said at a length that passes
        // 80
        return operand.clone();
    }
    // a note that, said on its own line, goes on past the width of eighty
    // columns
    let sum = compute(operand);
    sum // short
}

fn g() {
    let text = "a
b"; // a trailing comment after a string that began on an earlier line cannot move above this one
}
