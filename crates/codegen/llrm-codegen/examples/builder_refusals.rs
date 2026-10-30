//! What the DAG builder refuses in LLVM IR files: one line per refused block,
//! `REFUSED name:block reason`, and `FILE path functions=N blocks=B refused=R`
//! per file.
//!
//!     cargo run --release -p llrm-codegen --example builder_refusals --
//! m16|m32 FILE.ll...
//!
//! `tools/dagisel/builder_corpus.py` runs it over a corpus and tabulates the
//! reasons.

use llrm_codegen::backend::selectiondag::builder::{BuilderTarget, build_block};
use llrm_codegen::backend::selectiondag::function_lowering::FunctionLoweringInfo;
use llrm_mir::datalayout::DataLayout;

fn main() {
    let mut args = std::env::args().skip(1);
    let mode = args.next().expect("m16 or m32");
    let m16 = mode == "m16";
    for path in args {
        let Ok(raw) = std::fs::read_to_string(&path) else { continue };
        let text: String = raw.lines().filter(|l| !l.starts_with("target triple")).map(|l| format!("{l}\n")).collect();
        let mut module = match llrm_mir::parse::module(&text) {
            Ok(module) => module,
            Err(why) => {
                println!("FILE {path} unparsed {}", why.message.lines().next().unwrap_or(""));
                continue;
            }
        };
        let arch: &dyn llrm_target::Target = if m16 { &llrm_x86_m16::M16 } else { &llrm_x86_m32::M32 };
        // the driver's MIR rewrites between the pipeline and selection
        let _ = llrm_codegen::backend::ehprepare::prepared(&mut module, &llrm_target::Target::layout(arch));
        let _ = llrm_codegen::backend::memsetpattern::expanded(&mut module);
        let layout =
            module.datalayout.as_deref().map_or_else(DataLayout::default, |t| DataLayout::parse(t).unwrap_or_default());
        let target = if m16 { BuilderTarget::m16() } else { BuilderTarget::m32() };
        let (mut functions, mut blocks, mut refused) = (0, 0, 0);
        for (id, global, function) in module.functions() {
            if function.is_declaration() {
                continue;
            }
            functions += 1;
            let name = global.name.as_deref().unwrap_or("?");
            let Ok(convention) = llrm_codegen::backend::isel::convention(&module, &layout, id, arch) else {
                println!("REFUSED {name}: the convention is not described");
                refused += 1;
                continue;
            };
            let mut info = FunctionLoweringInfo::new(function);
            for &block in function.layout() {
                blocks += 1;
                if let Err(why) = build_block(&module, function, &layout, &target, &arch, &convention, &mut info, block)
                {
                    refused += 1;
                    println!("REFUSED {name}:{} {}", function.block(block).name.as_deref().unwrap_or(""), why.0);
                }
            }
        }
        println!("FILE {path} functions={functions} blocks={blocks} refused={refused}");
    }
}
