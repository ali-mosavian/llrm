//! Prints what `constantfolding::constant_fold_instruction` makes of every
//! instruction of a module whose operands are all constants, in the format of
//! `tools/llvmport/constfold_dump.cpp`: `<function> <n> <opcode> <constant |
//! ->`. usage: cfdump FILE.ll...

fn main() {
    for path in std::env::args().skip(1) {
        let text = std::fs::read_to_string(&path).expect("a readable file");
        match llrm_mir::constantfolding_dump::dump(&text) {
            Ok(lines) => print!("{lines}"),
            Err(why) => panic!("{path}: {why}"),
        }
    }
}
