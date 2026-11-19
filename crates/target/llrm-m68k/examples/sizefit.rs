//! `sizefit CPU FILE.ll`: the bytes `function_bytes` (the inliner's and unroller's byte estimate, by the
//! machine's `size_costs`) gives each defined function of FILE.ll, one `name<TAB>bytes` line each. Run it on the
//! last IR stage `LLRM_MIR_STAGES` dumps (the module selection sees) and compare with the object's symbol
//! sizes (`nm -S`): that is how the byte table is calibrated against what the compiler emits.

use llrm_m68k::driver::cpu_of;
use llrm_m68k::machine::M68kMachine;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::target::Machine;
use llrm_transforms::walk::function_bytes;

fn main() {
    let mut arguments = std::env::args().skip(1);
    let (Some(cpu), Some(path)) = (arguments.next(), arguments.next()) else {
        eprintln!("usage: sizefit CPU FILE.ll");
        std::process::exit(2);
    };
    let cpu = cpu_of(&cpu).expect("a CPU");
    let text = std::fs::read_to_string(&path).expect("the file reads");
    let module = llrm_mir::parse::module(&text).expect("the module parses");
    let layout = DataLayout::parse(module.datalayout.as_deref().expect("a datalayout")).expect("a datalayout");
    let costs = M68kMachine { cpu }.size_costs();
    for (_, global, function) in module.functions().filter(|(_, _, function)| !function.is_declaration()) {
        println!("{}\t{}", global.name.as_deref().unwrap_or("?"), function_bytes(&module, &layout, &costs, function));
    }
}
