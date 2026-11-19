//! `emit SEED [int16|int32] [opaque]`: the program for SEED on stdout.
use llrm_cgen::{DataModel, Options, program};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let model = if args.get(2).map(String::as_str) == Some("int16") { DataModel::INT16 } else { DataModel::INT32 };
    let options = Options { opaque: args.iter().any(|arg| arg == "opaque"), ..Options::new(model) };
    print!("{}", program(args[1].parse().expect("a seed"), &options));
}
