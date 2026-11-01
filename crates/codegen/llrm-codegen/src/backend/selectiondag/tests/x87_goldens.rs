//! The machine opcodes selection makes for floating-point functions against the
//! pin's (`llc -O2 -stop-after=finalize-isel`, `golden/x87.pin.txt`: one line
//! per function, `name OPCODE:count,...`, bookkeeping opcodes left out).
//! Regenerate with `tools/dagisel/golden_pin.py`.

use std::collections::BTreeMap;

use llrm_dagisel::generated::x86_i486 as table;
use llrm_dagisel::interp::Selector;
use llrm_dagisel::x86_addr::AddrMode;
use llrm_dagisel::x86_hooks::{Cpu, Options, X86Hooks};
use llrm_mir::datalayout::DataLayout;

use super::super::actions::{Features, TargetLowering};
use super::super::builder::{BuilderTarget, build_block};
use super::super::dag_select::{do_instruction_selection, legalized, users_first};
use super::super::function_lowering::FunctionLoweringInfo;
use super::super::x86_lowering::X86Lowering;
use super::*;

const BOOKKEEPING: [&str; 6] = ["COPY", "PHI", "IMPLICIT_DEF", "ADJCALLSTACKDOWN32", "ADJCALLSTACKUP32", "KILL"];

type Counts = BTreeMap<String, usize>;

/// The machine opcodes each function of `text` selects to, or why it stopped.
fn selected(text: &str) -> BTreeMap<String, Result<Counts, String>> {
    // The triple is for the pin's llc; MIR has no such line.
    let text: String = text.lines().filter(|l| !l.starts_with("target triple")).map(|l| format!("{l}\n")).collect();
    let module = llrm_mir::parse::module(&text).expect("parses");
    let layout = module.datalayout.as_deref().map_or_else(DataLayout::default, |t| DataLayout::parse(t).unwrap());
    let machine = llrm_x86_m32::M32;
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let selector = Selector::new(table::TABLE);
    let names: BTreeMap<u16, &str> =
        table::MACHINE_OPCODES.iter().map(|(v, n)| (*v, n.split('|').next().unwrap())).collect();
    let mut out = BTreeMap::new();
    for (id, global, function) in module.functions() {
        let name = global.name.as_deref().unwrap_or_default().to_owned();
        let Ok(convention) = crate::backend::isel::convention(&module, &layout, id, &machine) else {
            out.insert(name, Err("convention".to_owned()));
            continue;
        };
        let mut hooks = X86Hooks::new(Options {
            cpu: Cpu::I486,
            opt_none: false,
            opt_for_size: false,
            min_size: false,
            has_frame_pointer: true,
            multiply_early_out: false,
            address_mode: AddrMode::Flat32,
        });
        let mut info = FunctionLoweringInfo::new(function);
        let mut counts = Counts::new();
        let mut failure = None;
        for &block in function.layout() {
            let built =
                build_block(&module, function, &layout, &BuilderTarget::m32(), &machine, &convention, &mut info, block);
            let mut built = match built {
                Ok(b) => b,
                Err(why) => {
                    failure = Some(format!("builder: {}", why.0));
                    break;
                }
            };
            let dag = &mut built.dag;
            if let Err(why) = legalized(
                dag,
                &tli,
                &mut X86Lowering { cmov: false, sse2: false, min_size: false, i16_mul_chains: false, profile: None },
                &machine,
            )
            .and_then(|()| do_instruction_selection(dag, &selector, &mut hooks))
            {
                failure = Some(format!("{why:?}"));
                break;
            }
            for node in users_first(dag) {
                if let ISD::Machine(code) = dag.node(SDValue { node, res: 0 }).opcode {
                    let name = names.get(&(code as u16)).map_or_else(|| format!("#{code}"), |n| (*n).to_owned());
                    if !BOOKKEEPING.contains(&name.as_str()) {
                        *counts.entry(name).or_default() += 1;
                    }
                }
            }
        }
        out.insert(name, failure.map_or(Ok(counts), Err));
    }
    out
}

fn pin(text: &str) -> BTreeMap<String, Counts> {
    text.lines()
        .map(|line| {
            let (name, rest) = line.split_once(' ').unwrap_or((line, ""));
            let counts = rest
                .split(',')
                .filter(|p| !p.is_empty())
                .map(|p| {
                    let (op, n) = p.rsplit_once(':').unwrap();
                    (op.to_owned(), n.parse().unwrap())
                })
                .collect();
            (name.to_owned(), counts)
        })
        .collect()
}

/// The functions that differ from the pin today, and whose stage it is. The
/// test requires this list to be exact, so a fixed function must come off it.
const DIFFERS: [(&str, &str); 4] = [
    ("compare", "sdag-b: SetCC on f64 has no lowering (x87 UCOM_FPr + FNSTSW)"),
    ("narrow", "sdag-b: FP_ROUND through a stack slot (truncating store, reload)"),
    ("to_int", "sdag-d: the custom inserter for FP_TO_INT32_IN_MEM (control word) is not run"),
    ("widen", "sdag-b: fp_extend of a load is not an extending load"),
];

#[test]
fn floating_point_selection_matches_the_pins_opcodes_except_the_listed() {
    let ours = selected(include_str!("../golden/x87.ll"));
    let pins = pin(include_str!("../golden/x87.pin.txt"));
    let differ: Vec<&str> = pins
        .iter()
        .filter(|(name, want)| !matches!(ours.get(*name), Some(Ok(have)) if have == *want))
        .map(|(name, _)| name.as_str())
        .collect();
    let listed: Vec<&str> = DIFFERS.iter().map(|(name, _)| *name).collect();
    assert_eq!(differ, listed, "functions that differ from the pin vs the list above");
}
