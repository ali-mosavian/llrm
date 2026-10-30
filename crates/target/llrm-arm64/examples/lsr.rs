//! `cargo run -p llrm-arm64 --example lsr -- FILE.ll`: the module after loop
//! strength reduction alone, priced by the AArch64 `Tti`
//! (`tools/lsr_shadow.py` compares it with the pin's `opt
//! -passes=loop-reduce`).
use llrm_arm64::tti::AArch64;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::passes::{Declared, Outer};

fn main() {
    let text = std::fs::read_to_string(std::env::args().nth(1).expect("FILE.ll")).expect("readable");
    let mut module = llrm_mir::parse::module(&text).expect("parses");
    let layout = DataLayout::parse(module.datalayout.as_deref().expect("a datalayout")).expect("datalayout");
    let tti = AArch64::of_cpu("generic").unwrap();
    let names: Vec<String> = module
        .functions()
        .filter(|(_, _, function)| !function.is_declaration())
        .filter_map(|(_, global, _)| global.name.as_deref().map(str::to_owned))
        .collect();
    let outer = Outer::of(&module, None);
    let mut declared = Declared::of(&module);
    for name in names {
        let (context, function) = module.function_mut(&name).expect("a function");
        llrm_transforms::lsr_pass::reduced_with(context, &layout, function, &mut declared, &outer, &tti, false);
    }
    print!("{}", llrm_mir::print::module(&module));
}
