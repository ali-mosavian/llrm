//! Runs `simplifycfg` over the defined functions of the given modules and
//! prints each module, for `tools/llvmport/simplifycfgcompare.py`.
//! usage: simplifycfg [--no-keep-loops] [--hoist-common-insts]
//! [--sink-common-insts] FILE.ll...

use llrm_mir::datalayout::DataLayout;
use llrm_mir::parse;
use llrm_mir::passes::{Declared, Outer};
use llrm_transforms::simplifycfg::{Options, simplified_with};
use llrm_transforms::twoentryphi::Speculation;

fn main() {
    let mut options = Options::default();
    for path in std::env::args().skip(1) {
        if path == "--no-keep-loops" {
            options.need_canonical_loop = false;
            continue;
        }
        if path == "--hoist-common-insts" {
            options.hoist_common_insts = true;
            continue;
        }
        if path == "--sink-common-insts" {
            options.sink_common_insts = true;
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("a readable file");
        let mut module = parse::module(&text).unwrap_or_else(|why| panic!("{path}: {why}"));
        let layout =
            module.datalayout.as_deref().map(|text| DataLayout::parse(text).expect("a datalayout")).unwrap_or_default();
        let outer = Outer::of(&module, None);
        let names: Vec<String> = module
            .functions()
            .filter(|(_, _, function)| !function.is_declaration())
            .filter_map(|(_, global, _)| global.name.owned())
            .collect();
        for name in names {
            let spec = Speculation { tti: &llrm_mir::tti::Basic, layout: &layout, outer: &outer };
            let mut declared = Declared::of(&module);
            let (context, function) = module.function_mut(&name).expect("a defined function");
            simplified_with(&options, context, &spec, &mut declared, function);
            declared.place(&mut module).expect("placed");
        }
        print!("{}", llrm_mir::print::module(&module));
    }
}
