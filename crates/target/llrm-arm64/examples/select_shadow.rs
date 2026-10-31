//! `cargo run -p llrm-arm64 --example select_shadow -- FILE.ll [-fno-PASS
//! ...]`: per function, the machine opcodes the DAG path selects, one `@name
//! bb: OP OP ...` line per block, or `@name REFUSED: why`.
//! `tools/dagisel/arm64_shadow.py` compares them with the pin's `llc
//! -stop-after=finalize-isel`.
use llrm_arm64::dag::function::select_function_in;
use llrm_arm64::driver;
use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_dagisel::generated::aarch64::aarch64_apple_m1::MACHINE_OPCODES;

fn main() {
    let mut arguments = std::env::args().skip(1);
    let path = arguments.next().expect("FILE.ll");
    for flag in arguments {
        assert!(driver::pass_flag(&flag), "not a pass flag: {flag}");
    }
    let text = std::fs::read_to_string(&path).expect("readable");
    let (triple, _) = driver::without_triple(&text);
    let platform = triple.as_deref().and_then(driver::platform_of).unwrap_or(llrm_arm64::machine::Platform::Darwin);
    let mut program = driver::program(&text, platform).expect("reads");
    driver::optimized(&mut program).expect("optimizes");
    let module = &program.modules[0];
    // The pin reads the module with its atomics as instructions, not the calls
    // the pipeline saw.
    let mut shown = module.clone();
    llrm_mir::atomic::restored(&mut shown).expect("restores");
    println!("MODULE\n{}ENDMODULE", llrm_mir::print::module(&shown));
    let names: Vec<String> =
        module.functions().filter(|(_, _, f)| !f.is_declaration()).filter_map(|(_, g, _)| g.name.owned()).collect();
    for name in names {
        match select_function_in(module, &name, &program.layout, platform) {
            Err(why) => println!("@{name} REFUSED: {}", why.lines().next().unwrap_or("")),
            Ok(blocks) => {
                for (i, one) in blocks.iter().enumerate() {
                    let ops: Vec<&str> = one
                        .units
                        .iter()
                        .flat_map(|u| u.nodes.iter().rev().copied())
                        .filter_map(|n| match one.dag.nodes()[n as usize].opcode {
                            ISD::Machine(code) => {
                                MACHINE_OPCODES.iter().find(|(v, _)| u32::from(*v) == code).map(|(_, nm)| *nm)
                            }
                            _ => None,
                        })
                        .collect();
                    println!("@{name} bb{i}: {}", ops.join(" "));
                }
            }
        }
    }
}
