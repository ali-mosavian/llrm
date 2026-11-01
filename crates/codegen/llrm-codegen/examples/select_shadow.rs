//! Selects every block of an LLVM IR file with the DAG path and prints what it
//! makes.
//!
//!     cargo run --release -p llrm-codegen --example select_shadow -- FILE.ll
//!
//! Per function, one line: `FUNC name blocks=N built=B refused=R
//! machine=OP:n,... unselected=NODE:n,...`. `machine` counts the machine nodes
//! the selector left in each block's DAG, `unselected` the nodes it
//! could not select; a block the builder or a legalizer refuses is `refused`
//! with its reason on a `REFUSED name:block reason` line.
//! `tools/dagisel/select_shadow.py` compares this with the pin's
//! `llc -stop-after=finalize-isel` on the same file.

use std::collections::BTreeMap;

use llrm_codegen::backend::selectiondag::actions::{Features, TargetLowering};
use llrm_codegen::backend::selectiondag::builder::{BuilderTarget, build_block};
use llrm_codegen::backend::selectiondag::dag_select::legalized;
use llrm_codegen::backend::selectiondag::dagisel_adapter::DagisAdapter;
use llrm_codegen::backend::selectiondag::function_lowering::FunctionLoweringInfo;
use llrm_codegen::backend::selectiondag::x86_lowering::X86Lowering;
use llrm_codegen::backend::selectiondag::{ISD, MVT, SDValue};
use llrm_dagisel::generated::x86_i486 as table;
use llrm_dagisel::interp::{Outcome, Selector};
use llrm_dagisel::x86_addr::AddrMode;
use llrm_dagisel::x86_hooks::{Cpu, Options, X86Hooks};
use llrm_dagisel::x86_select::select_node;
use llrm_mir::datalayout::DataLayout;

/// The runtime routines both targets' `c.toml` list: a large
/// `memset`/`memcpy`/`memmove` calls one. Without them the builder refuses the
/// call ("library call whose symbol and convention are the target fact's"),
/// which the shadow counted against the DAG path.
const RUNTIME: &[&str] = &["memcpy", "memmove", "memset"];

fn opcode_name(code: u32) -> String {
    table::MACHINE_OPCODES
        .iter()
        .find(|(v, _)| u32::from(*v) == code)
        .map(|(_, n)| n.split('|').next().unwrap())
        // The hand matchers' instructions (`CDQ`, `DIV32r`) are in no pattern
        // of the table.
        .or_else(|| {
            llrm_dagisel::generated::ids::x86::NAMES.iter().find(|(v, _)| u32::from(*v) == code).map(|(_, n)| *n)
        })
        .map_or_else(|| format!("#{code}"), str::to_string)
}

fn main() {
    let path = std::env::args().nth(1).expect("a .ll file");
    let text: String = std::fs::read_to_string(&path)
        .expect("readable")
        .lines()
        .filter(|l| !l.starts_with("target triple"))
        .map(|l| format!("{l}\n"))
        .collect();
    let mut module = llrm_mir::parse::module(&text).unwrap_or_else(|e| panic!("{path}: {e}"));
    // The driver expands `memset.pattern` for the DAG path before it selects.
    llrm_codegen::backend::memsetpattern::expanded(&mut module).unwrap_or_else(|e| panic!("{path}: {e}"));
    let layout =
        module.datalayout.as_deref().map_or_else(DataLayout::default, |t| DataLayout::parse(t).expect("a datalayout"));
    // `M16=1` selects for real mode: 16-bit pointers, the m16 table, real-mode
    // addresses.
    let m16 = std::env::var_os("M16").is_some();
    let features =
        Features { cmov: false, bmi: false, lzcnt: false, popcnt: false, movbe: false, shld_slow: false, sse2: false };
    let (target, tli, arch, cpu, address_mode): (_, _, &dyn llrm_target::Target, _, _) = if m16 {
        (
            BuilderTarget::m16().with_libcalls(RUNTIME),
            TargetLowering::x86_16(features, MVT::I16),
            &llrm_x86_m16::M16,
            Cpu::M16,
            AddrMode::Real16,
        )
    } else {
        (
            BuilderTarget::m32().with_libcalls(RUNTIME),
            TargetLowering::x86_32(features, MVT::I32),
            &llrm_x86_m32::M32,
            Cpu::I486,
            AddrMode::Flat32,
        )
    };
    let selector = Selector::new(if m16 { llrm_dagisel::generated::x86_m16::TABLE } else { table::TABLE });
    let mut hooks = X86Hooks::new(Options {
        cpu,
        opt_none: false,
        opt_for_size: std::env::var_os("OPTSIZE").is_some(),
        min_size: false,
        has_frame_pointer: true,
        multiply_early_out: false,
        address_mode,
    });

    for (id, global, function) in module.functions() {
        let convention = match llrm_codegen::backend::isel::convention(&module, &layout, id, arch) {
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
            let mut done = match build_block(&module, function, &layout, &target, &arch, &convention, &mut info, block)
            {
                Ok(done) => done,
                Err(why) => {
                    println!("REFUSED {name}:{block_name} builder: {}", why.0);
                    continue;
                }
            };
            // The block's DAG and, for a switch, the blocks it made.
            let dags: Vec<&mut llrm_codegen::backend::selectiondag::SelectionDAG> =
                std::iter::once(&mut done.dag).chain(done.extra.iter_mut().map(|(_, b)| &mut b.dag)).collect();
            for dag in dags {
                // The driver's own pipeline (`dag_select::legalized`): the
                // pin's order, with the target, the hooks and the action table
                // the real route gives each stage.
                let mut lowering =
                    X86Lowering { cmov: false, sse2: false, min_size: false, i16_mul_chains: false, profile: None };
                if let Err(why) = legalized(dag, &tli, &mut lowering, arch) {
                    println!("REFUSED {name}:{block_name} legalize: {why:?}");
                    continue;
                }
                built += 1;
                if std::env::var_os("DUMP").is_some() {
                    println!("DAG {name}:{block_name}\n{}", dag.dump());
                }
                let done_root = dag.get_root().node;
                let mut adapter = DagisAdapter::new(dag);
                // Users before operands, as the pin walks `allnodes` backwards.
                // Nodes a selection makes are visited next, latest first
                // (`insertDAGNode`).
                let mut made: Vec<u32> = Vec::new();
                let mut rest = reachable_users_first(&adapter).into_iter();
                while let Some(node) = made.pop().or_else(|| rest.next()) {
                    // As `DoInstructionSelection`: nothing reads a node that
                    // matching made dead (the root has a handle).
                    let root = done_root;
                    if llrm_dagisel::dag::Dag::is_machine(&adapter, node)
                        || adapter.is_dead(node)
                        || (node != root && llrm_dagisel::dag::Dag::use_empty(&adapter, node))
                    {
                        continue;
                    }
                    let before = adapter.dag.nodes().len();
                    let outcome = select_node(&selector, &mut adapter, &mut hooks, node);
                    made.extend(before as u32..adapter.dag.nodes().len() as u32);
                    if std::env::var_os("DUMP").is_some() {
                        println!("  select t{node} {:?}: {outcome:?}", adapter.dag.nodes()[node as usize].opcode);
                    }
                    if outcome == Outcome::CannotSelect {
                        *unselected.entry(adapter.dag.nodes()[node as usize].opcode.name()).or_default() += 1;
                    }
                }
                if std::env::var_os("DUMP").is_some() {
                    println!("SELECTED {name}:{block_name}\n{}", dag.dump());
                }
                for (at, node) in dag.nodes().iter().enumerate() {
                    if let ISD::Machine(code) = node.opcode {
                        if !DagisAdapterView::dead(&dag, at as u32) {
                            *machine.entry(opcode_name(code)).or_default() += 1;
                        }
                    }
                }
            }
        }
        let list =
            |m: &BTreeMap<String, usize>| m.iter().map(|(k, v)| format!("{k}:{v}")).collect::<Vec<_>>().join(",");
        println!(
            "FUNC {name} blocks={blocks} built={built} machine={} unselected={}",
            list(&machine),
            list(&unselected)
        );
    }
}

/// The nodes reachable from the root, users before their operands.
fn reachable_users_first(adapter: &DagisAdapter<'_>) -> Vec<u32> {
    let dag = &*adapter.dag;
    let mut order = Vec::new();
    let mut seen = llrm_support::hash::HashSet::default();
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

/// Whether the node is still read by a node reachable from the root (selection
/// removes the rest).
struct DagisAdapterView;

impl DagisAdapterView {
    fn dead(
        dag: &llrm_codegen::backend::selectiondag::SelectionDAG,
        node: u32,
    ) -> bool {
        let mut seen = llrm_support::hash::HashSet::default();
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
