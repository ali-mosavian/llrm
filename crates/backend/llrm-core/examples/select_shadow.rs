//! Selects every block of an LLVM IR file with the DAG path and prints what it makes.
//!
//!     cargo run --release -p llrm-core --example select_shadow -- FILE.ll
//!
//! Per function, one line: `FUNC name blocks=N built=B refused=R machine=OP:n,... unselected=NODE:n,...`.
//! `machine` counts the machine nodes the selector left in each block's DAG, `unselected` the nodes it
//! could not select; a block the builder or a legalizer refuses is `refused` with its reason on a
//! `REFUSED name:block reason` line. `tools/dagisel/select_shadow.py` compares this with the pin's
//! `llc -stop-after=finalize-isel` on the same file.

use std::collections::BTreeMap;

use llrm_core::backend::selectiondag::actions::{Features, TargetLowering};
use llrm_core::backend::selectiondag::builder::{BuilderTarget, build_block};
use llrm_core::backend::selectiondag::dagisel_adapter::DagisAdapter;
use llrm_core::backend::selectiondag::function_lowering::FunctionLoweringInfo;
use llrm_core::backend::selectiondag::legalize_dag::{DagLegalizer, NoHooks};
use llrm_core::backend::selectiondag::legalize_types::TypeLegalizer;
use llrm_core::backend::selectiondag::{ISD, MVT, SDValue};
use llrm_dagisel::generated::x86_i486 as table;
use llrm_dagisel::interp::{Outcome, Selector};
use llrm_dagisel::x86_addr::AddrMode;
use llrm_dagisel::x86_hooks::{Cpu, Options, X86Hooks};
use llrm_dagisel::x86_select::select_node;
use llrm_mir::datalayout::DataLayout;

fn opcode_name(code: u32) -> String {
    table::MACHINE_OPCODES.iter().find(|(v, _)| u32::from(*v) == code).map_or_else(|| format!("#{code}"), |(_, n)| n.split('|').next().unwrap().to_string())
}

fn main() {
    let path = std::env::args().nth(1).expect("a .ll file");
    let text: String = std::fs::read_to_string(&path).expect("readable").lines().filter(|l| !l.starts_with("target triple")).map(|l| format!("{l}\n")).collect();
    let module = llrm_mir::parse::module(&text).unwrap_or_else(|e| panic!("{path}: {e}"));
    let layout = module.datalayout.as_deref().map_or_else(DataLayout::default, |t| DataLayout::parse(t).expect("a datalayout"));
    let target = BuilderTarget::m32();
    let tli = TargetLowering::x86_32(Features { cmov: false, bmi: false, lzcnt: false, popcnt: false, movbe: false, shld_slow: false }, MVT::I32);
    let selector = Selector::new(table::TABLE);
    let mut hooks = X86Hooks::new(Options {
        cpu: Cpu::I486,
        opt_none: false,
        opt_for_size: false,
        min_size: false,
        has_frame_pointer: true,
        address_mode: AddrMode::Flat32,
    });

    for (id, global, function) in module.functions() {
        let convention = match llrm_core::backend::isel::convention(&module, &layout, id, &llrm_x86_m32::M32) {
            Ok(c) => c,
            Err(why) => {
                println!("REFUSED {}: convention: {why:?}", global.name.as_deref().unwrap_or("?"));
                continue;
            }
        };
        let name = global.name.as_deref().unwrap_or("?");
        let mut info = FunctionLoweringInfo::new(function);
        let (mut blocks, mut built) = (0, 0);
        let mut machine: BTreeMap<String, usize> = BTreeMap::new();
        let mut unselected: BTreeMap<String, usize> = BTreeMap::new();
        for &block in function.layout() {
            blocks += 1;
            let block_name = function.block(block).name.clone().unwrap_or_default();
            let mut done = match build_block(&module, function, &layout, &target, &convention, &mut info, block) {
                Ok(done) => done,
                Err(why) => {
                    println!("REFUSED {name}:{block_name} builder: {}", why.0);
                    continue;
                }
            };
            if let Err(why) = TypeLegalizer::new(&mut done.dag, &tli).run() {
                println!("REFUSED {name}:{block_name} types: {why:?}");
                continue;
            }
            if let Err(why) = DagLegalizer::new(&mut done.dag, &tli, &mut NoHooks).run() {
                println!("REFUSED {name}:{block_name} legalize: {why:?}");
                continue;
            }
            built += 1;
            if std::env::var_os("DUMP").is_some() {
                println!("DAG {name}:{block_name}\n{}", done.dag.dump());
            }
            let done_root = done.dag.get_root().node;
            let mut adapter = DagisAdapter::new(&mut done.dag);
            // Users before operands, as the pin walks `allnodes` backwards.
            for node in reachable_users_first(&adapter) {
                // As `DoInstructionSelection`: nothing reads a node that matching made dead (the root has a handle).
                let root = done_root;
                if llrm_dagisel::dag::Dag::is_machine(&adapter, node)
                    || adapter.is_dead(node)
                    || (node != root && llrm_dagisel::dag::Dag::use_empty(&adapter, node))
                {
                    continue;
                }
                let outcome = select_node(&selector, &mut adapter, &mut hooks, node);
                if std::env::var_os("DUMP").is_some() {
                    println!("  select t{node} {:?}: {outcome:?}", adapter.dag.nodes()[node as usize].opcode);
                }
                if outcome == Outcome::CannotSelect {
                    *unselected.entry(adapter.dag.nodes()[node as usize].opcode.name()).or_default() += 1;
                }
            }
            if std::env::var_os("DUMP").is_some() {
                println!("SELECTED {name}:{block_name}\n{}", done.dag.dump());
            }
            for (at, node) in done.dag.nodes().iter().enumerate() {
                if let ISD::Machine(code) = node.opcode {
                    if !DagisAdapterView::dead(&done.dag, at as u32) {
                        *machine.entry(opcode_name(code)).or_default() += 1;
                    }
                }
            }
        }
        let list = |m: &BTreeMap<String, usize>| m.iter().map(|(k, v)| format!("{k}:{v}")).collect::<Vec<_>>().join(",");
        println!("FUNC {name} blocks={blocks} built={built} machine={} unselected={}", list(&machine), list(&unselected));
    }
}

/// The nodes reachable from the root, users before their operands.
fn reachable_users_first(adapter: &DagisAdapter<'_>) -> Vec<u32> {
    let dag = &*adapter.dag;
    let mut order = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut stack = vec![(dag.get_root().node, false)];
    while let Some((n, expanded)) = stack.pop() {
        if expanded {
            order.push(n);
            continue;
        }
        if !seen.insert(n) {
            continue;
        }
        stack.push((n, true));
        for op in &dag.node(SDValue { node: n, res: 0 }).ops {
            stack.push((op.node, false));
        }
    }
    order.reverse();
    order
}

/// Whether the node is still read by a node reachable from the root (selection removes the rest).
struct DagisAdapterView;

impl DagisAdapterView {
    fn dead(dag: &llrm_core::backend::selectiondag::SelectionDAG, node: u32) -> bool {
        let mut seen = std::collections::HashSet::new();
        let mut stack = vec![dag.get_root().node];
        while let Some(n) = stack.pop() {
            if n == node {
                return false;
            }
            if seen.insert(n) {
                stack.extend(dag.node(SDValue { node: n, res: 0 }).ops.iter().map(|o| o.node));
            }
        }
        true
    }
}
