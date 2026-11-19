//! How many times each block of a function runs per run of the function: the
//! weights the prices that choose between variants of its code multiply a
//! block's clocks by.
//!
//! A loop weighs ten trips by default (`timing`), whatever it counts. Here a
//! loop with one exit that ScalarEvolution counts
//! (`getSmallConstantTripCount`) weighs its trips, and nested loops multiply.
//! The trial unroll was kept at 2.4 clk/B on a 4-trip loop it had priced at
//! ten (`extension_chain_fused`: +96 B, 1,014 clocks estimated, 235 measured).

use llrm_analysis::scev::{Cx, ScalarEvolution};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::domtree::DomTree;
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::module::{Function, Module};
use llrm_mir::valuetracking::AssumptionCache;

/// The trips a loop weighs when ScalarEvolution does not count it.
const GUESSED_TRIPS: u64 = 10;
/// The most a block weighs: three nested guesses.
const MOST: u64 = 1000 * 100;

/// The weight of each block of `function`, by block number.
pub fn block_weights(
    module: &Module,
    function: &Function,
    layout: &DataLayout,
) -> Vec<u64> {
    let tree = DomTree::of(function);
    let loops = LoopInfo::analyze(function, || &tree);
    let assumptions = AssumptionCache::new(module, function);
    let cx = Cx {
        context: &module.context,
        globals: &module.globals,
        metadata: &module.metadata,
        function,
        layout,
        loops: &loops,
        tree: &tree,
        assumptions: &assumptions,
    };
    let mut scev = ScalarEvolution::new();
    let mut weights = vec![1u64; function.block_capacity()];
    for one in loops.get_loops_in_preorder() {
        let trips = match one.get_exiting_blocks(function)[..] {
            [_] => u64::from(scev.get_small_constant_trip_count(&cx, one.get_header())),
            _ => 0,
        };
        let trips = if trips == 0 { GUESSED_TRIPS } else { trips };
        for block in one.get_blocks() {
            let weight = &mut weights[block.0 as usize];
            *weight = (*weight * trips).min(MOST);
        }
    }
    by_branch_weights(module, function, &mut weights);
    weights
}

/// `weights` with each block that one branch with `!prof` weights decides
/// scaled by its share of the branch: the block a loop's counter wraps into
/// runs once per page, not once per trip. A block weighs at least one.
pub fn by_branch_weights(
    module: &Module,
    function: &Function,
    weights: &mut [u64],
) {
    use llrm_mir::module::{MetadataOperand, Operand};
    use llrm_mir::opcode::Opcode;
    for block in function.layout() {
        let [from] = function.predecessors(*block)[..] else { continue };
        let Some(jump) = function.terminator(from) else { continue };
        let branch = function.instruction(jump);
        if branch.opcode != Opcode::Br || branch.operands.len() != 3 {
            continue;
        }
        let Some((_, node)) = branch.metadata.iter().find(|(kind, _)| kind == "prof") else { continue };
        let Some(node) = module.metadata.get(node.0 as usize) else { continue };
        if node.operands.first() != Some(&MetadataOperand::String("branch_weights".to_owned())) {
            continue;
        }
        let shares: Vec<u64> = node.operands[1..]
            .iter()
            .filter_map(|one| match one {
                MetadataOperand::Constant(id) => match module.context.get(*id).kind {
                    llrm_mir::context::ConstantKind::Int(bits) => Some(bits as u64),
                    _ => None,
                },
                _ => None,
            })
            .collect();
        let targets = &branch.operands[1..];
        let Some(at) = targets.iter().position(|one| *one == Operand::Block(*block)) else { continue };
        let total: u64 = shares.iter().sum();
        if shares.len() == 2 && total > 0 && at < 2 && targets[0] != targets[1] {
            let weight = &mut weights[block.0 as usize];
            *weight = (*weight * shares[at] / total).max(1);
        }
    }
}

/// The clocks and bytes of a program from the text of its functions: the
/// clocks of a run, the bytes of all of them.
///
/// A run is `main`'s where the program has one, each call costing its callee's
/// clocks; else every function nothing else in the module calls. Summing every
/// function counted a function twice when it was inlined into its caller and
/// also kept for the world (`fill_nest`: 783,928 clocks priced for a run of
/// 436,825) and a trial that stopped inlining it looked 382,304 cheaper
/// (measured: 8,855).
pub fn run_cost(
    cpu: crate::dagisel::hooks::Cpu,
    names: &[String],
    pieces: &[String],
    weights: &[Vec<u64>],
) -> Result<(u64, u64), String> {
    let callees: Vec<Vec<usize>> = pieces
        .iter()
        .map(|piece| {
            let mut called: Vec<usize> = piece
                .lines()
                .filter_map(|line| line.trim().strip_prefix("jsr "))
                .filter_map(|target| names.iter().position(|name| name == target.trim()))
                .collect();
            called.sort_unstable();
            called.dedup();
            called
        })
        .collect();
    // What a runtime routine the module calls costs a call: its own text,
    // priced as the module's functions are, its loops at their guessed trips.
    let mut outside: Vec<Vec<(&str, u64)>> = Vec::new();
    for piece in pieces {
        let mut found: Vec<(&str, u64)> = Vec::new();
        for target in piece.lines().filter_map(|line| line.trim().strip_prefix("jsr ")).map(str::trim) {
            if names.iter().all(|name| name != target) && found.iter().all(|(one, _)| *one != target) {
                if let Some(clocks) = runtime_clocks(cpu, target) {
                    found.push((target, clocks));
                }
            }
        }
        outside.push(found);
    }
    struct Run<'a> {
        cpu: crate::dagisel::hooks::Cpu,
        names: &'a [String],
        pieces: &'a [String],
        weights: &'a [Vec<u64>],
        callees: &'a [Vec<usize>],
        outside: &'a [Vec<(&'a str, u64)>],
        memo: Vec<Option<u64>>,
        open: Vec<bool>,
    }
    impl Run<'_> {
        fn clocks(
            &mut self,
            at: usize,
        ) -> Result<u64, String> {
            if let Some(known) = self.memo[at] {
                return Ok(known);
            }
            // A call back into a function being priced adds nothing: its
            // trips are not known.
            if std::mem::replace(&mut self.open[at], true) {
                return Ok(0);
            }
            let mut routines = self.outside[at].clone();
            for &callee in &self.callees[at] {
                routines.push((self.names[callee].as_str(), self.clocks(callee)?));
            }
            self.open[at] = false;
            let weight_of = |block: u32| self.weights[at].get(block as usize).copied().unwrap_or(1);
            let cost =
                crate::timing::estimate_weighted(self.cpu, &self.pieces[at], &routines, Some(&weight_of))?.cycles;
            self.memo[at] = Some(cost);
            Ok(cost)
        }
    }
    let mut run = Run {
        cpu,
        names,
        pieces,
        weights,
        callees: &callees,
        outside: &outside,
        memo: vec![None; names.len()],
        open: vec![false; names.len()],
    };
    let called: Vec<bool> = (0..names.len())
        .map(|at| callees.iter().enumerate().any(|(by, list)| by != at && list.contains(&at)))
        .collect();
    let roots: Vec<usize> = match names.iter().position(|name| name == "main") {
        Some(main) => vec![main],
        None => (0..names.len()).filter(|&at| !called[at]).collect(),
    };
    let roots = if roots.is_empty() { (0..names.len()).collect() } else { roots };
    let mut clocks = 0;
    for root in roots {
        clocks += run.clocks(root)?;
    }
    let mut bytes = 0u64;
    for piece in pieces {
        bytes += u64::from(crate::timing::estimate_weighted(cpu, piece, &[], None)?.bytes);
    }
    Ok((clocks, bytes))
}

/// The clocks one call of the runtime routine `name` takes, from its text in
/// `mos_rt.s`, or None where the routine is not there or cannot be priced.
fn runtime_clocks(
    cpu: crate::dagisel::hooks::Cpu,
    name: &str,
) -> Option<u64> {
    let source = include_str!("../runtime/mos_rt.s");
    let start = source
        .lines()
        .position(
            |line| line.split(';').next().unwrap_or("").trim_end().strip_suffix(':').is_some_and(|label| label == name),
        )?;
    let text: String = source
        .lines()
        .skip(start)
        .enumerate()
        .take_while(|(at, line)| *at == 0 || !line.starts_with(".export"))
        .map(|(_, line)| format!("{line}\n"))
        .collect();
    crate::timing::estimate_weighted(cpu, &text, &[], None).ok().map(|one| one.cycles)
}
