//! Prints what `computeKnownFPClass` proves of every float and double scalar of
//! a module, one line each, in the format of `tools/llvmport/fpdump.cpp`:
//! `<function> <n> all=<mask>/<sign> nan=... neg=...`, `<n>` numbering the
//! function's arguments and then its instructions with such a result.
//! usage: fpdump FILE.ll...

use llrm_mir::parse;

fn main() {
    for path in std::env::args().skip(1) {
        let text = std::fs::read_to_string(&path).expect("a readable file");
        let module = parse::module(&text).unwrap_or_else(|why| panic!("{path}: {why}"));
        print!("{}", llrm_mir::valuetracking::fp::dump(&module));
    }
}
