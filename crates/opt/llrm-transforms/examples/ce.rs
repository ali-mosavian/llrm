//! Runs `constraint-elimination` over the functions of the given modules and
//! prints each module, for `tools/llvmport/cecompare.py`.
//! usage: ce FILE.ll...

use llrm_mir::datalayout::DataLayout;
use llrm_mir::parse;
use llrm_transforms::constraintelim::eliminate_constraints;

fn main() {
    for path in std::env::args().skip(1) {
        let text = std::fs::read_to_string(&path).expect("a readable file");
        let mut module = parse::module(&text).unwrap_or_else(|why| panic!("{path}: {why}"));
        let layout =
            module.datalayout.as_deref().map(|text| DataLayout::parse(text).expect("a datalayout")).unwrap_or_default();
        let declarations = module.declarations();
        let metadata = module.metadata.clone();
        let names: Vec<String> = module
            .functions()
            .filter(|(_, _, function)| !function.is_declaration())
            .filter_map(|(_, global, _)| global.name.owned())
            .collect();
        for name in names {
            let (context, function) = module.function_mut(&name).expect("a defined function");
            eliminate_constraints(context, &layout, &declarations, &metadata, function);
        }
        print!("{}", llrm_mir::print::module(&module));
    }
}
