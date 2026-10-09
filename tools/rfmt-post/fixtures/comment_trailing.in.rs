pub struct Indexes {
    pub at: IndexMap<usize, i64>, // id(insn) -> the instruction's first slot, in a note that runs past column 80
    pub span: IndexMap<i64, (i64, i64)>, // block address -> [first, last)
    pub order: Vec<i64>, // block addresses, in the order they are numbered, and a note long enough to pass 80
}

fn f(operand: &Operand) -> Operand {
    if operand.is_none() {
        return operand.clone(); // refuses below, which is the safe answer, said at a length that passes 80
    }
    let sum = compute(operand); // a note that, said on its own line, goes on past the width of eighty columns
    sum // short
}

fn g() {
    let text = "a
b"; // a trailing comment after a string that began on an earlier line cannot move above this one
}
