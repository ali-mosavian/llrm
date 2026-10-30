//! Prints what LazyValueInfo answers for every value, edge, icmp and use of a
//! module, in the format of `tools/llvmport/lvi_dump.cpp`.
//! usage: lvidump FILE.ll...

fn main() {
    for path in std::env::args().skip(1) {
        let text = std::fs::read_to_string(&path).expect("a readable file");
        print!("{}", llrm_mir::lvidump::dump(&text).unwrap_or_else(|why| panic!("{path}: {why}")));
    }
}
