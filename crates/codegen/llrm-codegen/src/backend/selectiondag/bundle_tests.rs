//! The builder pieces of the bundle PR against the pin: `i64` parts, byval and
//! sret, the `memset.pattern` loop, and `lib_call`.

use std::collections::BTreeMap;

use llrm_mir::datalayout::DataLayout;

use super::builder::{BlockDag, BuilderTarget, build_block};
use super::dump::normalized;
use super::function_lowering::FunctionLoweringInfo;
use super::{DagConfig, EVT, ISD, SDValue, SelectionDAG};

/// The pin's "Initial selection DAG" dumps by block name (`f:b1`).
fn pin_blocks(text: &str) -> BTreeMap<String, String> {
    let mut blocks = BTreeMap::new();
    let mut current: Option<(String, String)> = None;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("Initial selection DAG: ") {
            if let Some((name, body)) = current.take() {
                blocks.insert(name, body);
            }
            let name = rest.split('\'').nth(1).expect("quoted name").to_string();
            current = Some((name, String::new()));
        } else if let Some((_, body)) = current.as_mut() {
            body.push_str(line);
            body.push('\n');
        }
    }
    blocks.extend(current);
    blocks.into_iter().map(|(name, body)| (name, normalized(&body))).collect()
}

/// Our dump of every block the builder takes, by the pin's block name.
fn our_blocks(text: &str) -> (BTreeMap<String, String>, Vec<String>) {
    let text: String =
        text.lines().filter(|line| !line.starts_with("target triple")).map(|line| format!("{line}\n")).collect();
    let module = llrm_mir::parse::module(&text).unwrap_or_else(|error| panic!("{error}"));
    let layout = module
        .datalayout
        .as_deref()
        .map_or_else(DataLayout::default, |text| DataLayout::parse(text).expect("a datalayout"));
    let target = BuilderTarget::m32().with_libcalls(&["memcpy", "memmove", "memset"]);
    let mut built = BTreeMap::new();
    let mut refused = Vec::new();
    for (id, global, function) in module.functions() {
        let convention =
            crate::backend::isel::convention(&module, &layout, id, &llrm_x86_m32::M32).expect("a convention");
        let mut info = FunctionLoweringInfo::new(function);
        for &block in function.layout() {
            let name =
                format!("{}:{}", global.name.as_deref().unwrap(), function.block(block).name.as_deref().unwrap_or(""));
            match build_block(&module, function, &layout, &target, &llrm_x86_m32::M32, &convention, &mut info, block) {
                Ok(done) => {
                    built.insert(name, normalized(&done.dag.dump()));
                }
                Err(why) => refused.push(format!("{name}: {}", why.0)),
            }
        }
    }
    (built, refused)
}

/// An `i64` at m32 travels as two `i32` registers: parameters are two
/// loads and a `build_pair`, a value across blocks is two
/// `extract_element` copies, a call or return uses two registers. These
/// functions (the shadow's x_ll_arith, x_ll_div) were refused as wider
/// than a register.
#[test]
fn two_register_integers_match_the_pins_initial_dag() {
    let ll = include_str!("golden/wide.ll");
    let pin = pin_blocks(include_str!("golden/wide.pin.txt"));
    let (ours, refused) = our_blocks(ll);
    assert_eq!(refused, Vec::<String>::new());
    assert_eq!(ours.len(), 4);
    for (name, dag) in &ours {
        assert_eq!(dag, pin.get(name).expect("the pin has the block"), "block {name}");
    }
}

/// `byval` parameters are the address of the caller's bytes, a `byval`
/// argument is copied into its slot, and `sret` is kept in a register and
/// returned in eax: the pin's. Each was refused.
#[test]
fn byval_and_sret_match_the_pins_initial_dag() {
    // the pin's i486-linux is the System V convention: the callee pops the sret
    // slot
    let ll = include_str!("golden/byval.ll")
        .replace("define ", "define sysvcc ")
        .replace("declare ", "declare sysvcc ")
        .replace("call ", "call sysvcc ");
    let pin = pin_blocks(include_str!("golden/byval.pin.txt"));
    let (ours, refused) = our_blocks(&ll);
    assert_eq!(refused, Vec::<String>::new());
    assert_eq!(ours.len(), 4);
    for (name, dag) in &ours {
        assert_eq!(dag, pin.get(name).expect("the pin has the block"), "block {name}");
    }
}

fn flat(
    done: &BlockDag,
    out: &mut Vec<String>,
) {
    out.push(normalized(&done.dag.dump()));
    for (_, extra) in &done.extra {
        flat(extra, out);
    }
}

/// The blocks of each function, ours and the pin's, as sorted dumps.
pub(super) fn ours(text: &str) -> std::collections::BTreeMap<String, Vec<String>> {
    let text: String =
        text.lines().filter(|line| !line.starts_with("target triple")).map(|line| format!("{line}\n")).collect();
    let mut module = llrm_mir::parse::module(&text).unwrap_or_else(|error| panic!("{error}"));
    crate::backend::memsetpattern::expanded(&mut module).expect("the memset patterns expand");
    let layout = DataLayout::parse(module.datalayout.as_deref().unwrap()).unwrap();
    let mut all = std::collections::BTreeMap::new();
    for (id, global, function) in module.functions() {
        if function.is_declaration() {
            continue;
        }
        let convention =
            crate::backend::isel::convention(&module, &layout, id, &llrm_x86_m32::M32).expect("a convention");
        let mut info = FunctionLoweringInfo::new(function);
        let mut dumps = Vec::new();
        for &block in function.layout() {
            let done = build_block(
                &module,
                function,
                &layout,
                &BuilderTarget::m32().with_libcalls(&["memcpy", "memmove", "memset"]).with_stack_alignment(16),
                &llrm_x86_m32::M32,
                &convention,
                &mut info,
                block,
            )
            .unwrap_or_else(|why| panic!("{} {block:?}: {}", global.name.as_deref().unwrap(), why.0));
            flat(&done, &mut dumps);
        }
        dumps.sort();
        all.insert(global.name.as_deref().unwrap().to_string(), dumps);
    }
    all
}

pub(super) fn pins(text: &str) -> std::collections::BTreeMap<String, Vec<String>> {
    let mut all: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    let mut current: Option<(String, String)> = None;
    let finish = |current: &mut Option<(String, String)>, all: &mut std::collections::BTreeMap<String, Vec<String>>| {
        if let Some((name, body)) = current.take() {
            all.entry(name).or_default().push(normalized(&body));
        }
    };
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("Initial selection DAG: ") {
            finish(&mut current, &mut all);
            let name = rest.split('\'').nth(1).unwrap().split(':').next().unwrap().to_string();
            current = Some((name, String::new()));
        } else if let Some((_, body)) = current.as_mut() {
            body.push_str(line);
            body.push('\n');
        }
    }
    finish(&mut current, &mut all);
    all.values_mut().for_each(|dumps| dumps.sort());
    all
}

/// `memset.pattern` is a store loop before selection (the pin's
/// `PreISelIntrinsicLowering`): the blocks of the expanded functions are
/// the pin's. The intrinsic was a builder refusal (shadow: scroll).
#[test]
fn memset_pattern_loops_match_the_pins_initial_dags() {
    let (ours, pins) = (ours(include_str!("golden/memsetpat.ll")), pins(include_str!("golden/memsetpat.pin.txt")));
    assert_eq!(ours.keys().collect::<Vec<_>>(), pins.keys().collect::<Vec<_>>());
    for (name, dumps) in &ours {
        assert_eq!(dumps, &pins[name], "function {name}");
    }
}

mod libcall_shape {
    use super::super::libcall::{CallEnv, Place, Returned, lib_call};
    use super::*;

    fn dag() -> SelectionDAG {
        SelectionDAG::new(DagConfig::default())
    }

    fn arg(
        dag: &mut SelectionDAG,
        vt: EVT,
        reg: u32,
    ) -> SDValue {
        dag.get_register(vt, reg)
    }

    /// A helper call with a stack argument and a register argument reads
    /// `callseq_start, store, CopyToReg, CALL, callseq_end, CopyFromReg`
    /// in that order, the result on the call's chain and glue. Without
    /// `lib_call` a legalizer would build its own variant of the sequence.
    #[test]
    fn a_helper_call_has_the_builders_shape() {
        let mut dag = dag();
        let env = CallEnv { arch: &llrm_x86_m32::M32, pointer: EVT::I32 };
        let (a, b) = (arg(&mut dag, EVT::I32, 1), arg(&mut dag, EVT::I32, 2));
        let callee = dag.get_target_external_symbol(EVT::I32, "helper");
        let eax = llrm_target::Target::register_named(&llrm_x86_m32::M32, "eax");
        let edx = llrm_target::Target::register_named(&llrm_x86_m32::M32, "edx");
        let root = dag.entry_node();
        let (chain, results) = lib_call(
            &mut dag,
            &env,
            root,
            callee,
            &[(a, Place::Stack(0)), (b, Place::Register(edx))],
            4,
            0,
            &[Returned { reg: eax, vt: EVT::I32 }],
        );
        let opcodes: Vec<String> = dag.nodes().iter().map(|node| node.opcode.name()).collect();
        let position =
            |name: &str| opcodes.iter().position(|one| one == name).unwrap_or_else(|| panic!("{name} in {opcodes:?}"));
        assert!(position("callseq_start") < position("store") && position("store") < position("CopyToReg"));
        assert!(position("CopyToReg") < position("X86ISD::CALL") && position("X86ISD::CALL") < position("callseq_end"));
        assert_eq!(results.len(), 1);
        assert_eq!(dag.opcode(results[0]), ISD::CopyFromReg);
        assert_eq!(chain, SDValue { node: results[0].node, res: 1 });
    }
}
