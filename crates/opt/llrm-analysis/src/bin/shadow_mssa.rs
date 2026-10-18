//! Shadow of the translated MemorySSA build: `shadow_mssa FILE.ll
//! [--parse-only]` parses a module dump and, for each function, builds the
//! translated graph and today's, verifies the first and compares the two
//! (`SHADOW=new` builds only the translated one, `SHADOW=old` only today's).
//! The instructions of a mode are the run's minus `--parse-only`'s.
use llrm_analysis::memory::Unit;
use llrm_analysis::memoryssa::{self, Accesses};
use llrm_analysis::mssa::shadow;
use llrm_analysis::testing::layout;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let text = std::fs::read_to_string(&args[1]).expect("a readable dump");
    let module = llrm_mir::parse::module(&text).unwrap_or_else(|error| panic!("{error}"));
    if args.iter().any(|arg| arg == "--parse-only") {
        return;
    }
    let mode = std::env::var("SHADOW").unwrap_or_default();
    let layout = layout(&module);
    let (mut functions, mut instructions, mut memory_ops, mut differ) = (0, 0, 0, 0);
    for (_, _, function) in module.functions() {
        if function.layout().is_empty() {
            continue;
        }
        let unit = llrm_analysis::testing::with_registers(Unit::of(&module, &layout, function));
        functions += 1;
        instructions += function.walk().count();
        match mode.as_str() {
            "new" => {
                let (built, _) = shadow::translated(&unit);
                memory_ops += built.len();
            }
            "walk" => match shadow::walk(&unit) {
                Ok(report) => {
                    memory_ops += report.queries;
                    println!(
                        "WALK {} {} {} {} {} {} {}",
                        report.queries,
                        report.steps,
                        report.asked,
                        report.worked,
                        report.in_today,
                        report.phi,
                        report.elsewhere
                    );
                }
                Err(why) => {
                    differ += 1;
                    eprintln!("DIFFER {why}");
                }
            },
            "newwalk" => {
                let accesses = Accesses::plain(&unit, &Default::default());
                let shape = unit.shape();
                let mut built = llrm_analysis::mssa::Mssa::build(
                    function,
                    &shape.dominance,
                    &shadow::effect_of(&accesses, unit.function),
                );
                let aa = llrm_analysis::mssa::alias::AliasQuery::new(&unit, &accesses);
                for (_, inst) in function.walk() {
                    if let Some(access) = built.get_memory_access(inst) {
                        if aa.location_of(inst).is_some() {
                            let _ = built.get_clobbering_memory_access(&aa, access);
                            memory_ops += 1;
                        }
                    }
                }
            }
            "oldwalk" => {
                let accesses = Accesses::plain(&unit, &Default::default());
                let old = memoryssa::built(&unit, &accesses);
                for (_, inst) in function.walk() {
                    if old.sites.contains_key(&inst) {
                        if let Some(reference) = accesses.references.get(&inst) {
                            let _ = old.clobbers(inst, reference);
                            memory_ops += 1;
                        }
                    }
                }
            }
            "facts" => {
                let accesses = Accesses::plain(&unit, &Default::default());
                let _ = unit.shape();
                memory_ops += accesses.references.len();
            }
            "old" => {
                let accesses = Accesses::plain(&unit, &Default::default());
                let old = memoryssa::built(&unit, &accesses);
                memory_ops += old.sites.len();
            }
            _ => {
                if let Err(why) = shadow::compare(&unit) {
                    differ += 1;
                    eprintln!("DIFFER {why}");
                }
            }
        }
    }
    println!("functions {functions} instructions {instructions} accesses {memory_ops} differ {differ}");
}
