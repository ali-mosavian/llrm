//! `MachineCombiner`: a pair of instructions is replaced by a cheaper one
//! when that does not lengthen the critical path of its block. Translated
//! from
//! [MachineCombiner.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineCombiner.cpp)
//! (`combineInstructions`, `improvesCriticalPathLen`, `getDepth`,
//! `getLatency`, `insertDeleteInstructions`) and the depth half of
//! [MachineTraceMetrics.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineTraceMetrics.cpp).
//!
//! # How it works
//! The pass knows no instruction set. A target gives it, for the instructions
//! of one block, a [`Node`] each: the virtual registers it defines and reads,
//! its latency and, per read, how many cycles late the read happens
//! (`ReadAdvance`). It also gives, through [`CombinerIsa`], the patterns a
//! root instruction can start (`getMachineCombinerPatterns`) and the
//! replacement each makes (`genAlternativeCodeSequence`).
//!
//! Each instruction of the block is tried as a root, in order. A pattern's
//! replacement goes in when it is cheaper than what it replaces, the first
//! pattern that qualifies wins, and the instruction before the root is not
//! visited again. Cheaper means, in turn:
//! - a throughput pattern inside a loop: always;
//! - the block is optimized for size and the replacement is shorter: always;
//! - otherwise the new root's depth plus latency is no more than the old root's
//!   depth plus latency plus its slack (`improvesCriticalPathLen`), or strictly
//!   less where the pattern's objective is `MustReduceDepth`, and the target
//!   says the block's resource use does not grow (`preservesResourceLen`).
//!
//! The depth of an instruction is the largest, over the registers it reads
//! and that an earlier instruction of the block defines, of that definition's
//! depth plus the latency from the definition to the read. The slack of an
//! instruction is the block's critical path less its depth and height.
//!
//! ```text
//!   t = mul a, b   (3)        r = madd a, b, c   (3, c read 2 cycles late)
//!   r = add t, c   (1)   =>
//!   ready at 4 for c (arrives at 0)                ready at 3
//! ```
//!
//! DEVIATE MC-T1 (owner w15, exit: a trace over single-predecessor chains,
//! `MachineTraceMetrics`' min-instruction strategy): the trace is the block
//! alone, as the pin's `TS_Local`; a register made outside the block has depth
//! 0 and no latency. The pin's default strategy also follows predecessors.
//!
//! DEVIATE MC-S (owner w15, same exit): the slack is computed inside the
//! block; a value read after the block has height 0.
//!
//! # Example
//! The unit tests in `combine_tests.rs`: `a_multiply_feeding_an_add_is_fused`
//! and `a_fusion_that_lengthens_the_path_is_left`.
//!
//! # Runs
//! Before register allocation, on the target's own function body, at every
//! level above -O0, where the target has patterns.
//!
//! # See also
//! `backend::combiner`, the reassociation-only version i486 uses.

/// A virtual register, numbered by the target.
pub type Reg = u32;

/// One read of a register by an instruction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Use {
    pub reg: Reg,
    /// `ReadAdvance`: the cycles the read happens after the instruction
    /// issues; the operand latency is the definition's latency less this.
    pub advance: u32,
}

/// What the pass needs to know of one instruction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Node {
    pub defs: Vec<Reg>,
    pub uses: Vec<Use>,
    /// `computeInstrLatency`.
    pub latency: u32,
    /// `isTransientMI`: a copy that will be coalesced: its definition adds no
    /// latency to a read.
    pub transient: bool,
}

impl Node {
    pub fn new(
        defs: &[Reg],
        uses: &[Use],
        latency: u32,
    ) -> Self {
        Self { defs: defs.to_vec(), uses: uses.to_vec(), latency, transient: false }
    }
}

/// What a pattern asks of the cost rule (`CombinerObjective`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Objective {
    /// The new root may be as deep as the old one's with its slack.
    Default,
    /// The new root must be strictly shallower.
    MustReduceDepth,
}

/// The replacement a pattern makes: `genAlternativeCodeSequence`.
#[derive(Clone, Debug)]
pub struct Replacement {
    /// The new instructions, in order; the last is the new root. They go in
    /// before the root.
    pub inserted: Vec<Node>,
    /// The indices, in the block, of the instructions that go.
    pub deleted: Vec<usize>,
}

/// The target's side of the pass.
pub trait CombinerIsa {
    type Pattern: Copy;

    /// One `Node` per instruction of the block, in order.
    fn nodes(&self) -> Vec<Node>;

    /// `getMachineCombinerPatterns`: the patterns the instruction at `root`
    /// starts, best first.
    fn patterns(
        &self,
        root: usize,
    ) -> Vec<Self::Pattern>;

    /// `genAlternativeCodeSequence`: none where the pattern turns out not to
    /// apply (an immediate that needs more than one instruction).
    fn generate(
        &mut self,
        root: usize,
        pattern: Self::Pattern,
    ) -> Option<Replacement>;

    /// Make the replacement in the block: the new instructions of the last
    /// `generate` go in before `root`, `deleted` go.
    fn apply(
        &mut self,
        root: usize,
        pattern: Self::Pattern,
        replacement: &Replacement,
    );

    /// `getCombinerObjective`.
    fn objective(
        &self,
        _pattern: Self::Pattern,
    ) -> Objective {
        Objective::Default
    }

    /// `isThroughputPattern`.
    fn is_throughput(
        &self,
        _pattern: Self::Pattern,
    ) -> bool {
        false
    }

    /// `accumulateInstrSeqToRootLatency`: the latency of the whole inserted
    /// sequence, not only its root, is compared with the deleted one.
    fn accumulates_sequence_latency(
        &self,
        _root: usize,
    ) -> bool {
        true
    }

    /// `preservesResourceLen`: whether the block's use of the machine's
    /// units does not grow with the replacement.
    fn preserves_resources(
        &self,
        _inserted: &[Node],
        _deleted: &[Node],
    ) -> bool {
        true
    }
}

/// How the block is optimized.
#[derive(Clone, Copy, Debug, Default)]
pub struct Setting {
    /// `shouldOptimizeForSize`.
    pub optimize_for_size: bool,
    /// The block is in a loop (`MLI->getLoopFor`).
    pub in_loop: bool,
}

/// Depth and slack of the block's instructions (`MachineTraceMetrics`).
struct Trace {
    depth: Vec<u32>,
    height: Vec<u32>,
    critical: u32,
}

/// The latency from `def`'s result to the read `used` makes:
/// `computeOperandLatency`.
fn operand_latency(
    def: &Node,
    used: Use,
) -> u32 {
    if def.transient { 0 } else { def.latency.saturating_sub(used.advance) }
}

/// The index of the last definition of `reg` before `at`.
fn definition_before(
    nodes: &[Node],
    at: usize,
    reg: Reg,
) -> Option<usize> {
    nodes[..at].iter().rposition(|node| node.defs.contains(&reg))
}

impl Trace {
    fn of(nodes: &[Node]) -> Self {
        let mut depth = vec![0u32; nodes.len()];
        let mut users: Vec<Vec<(usize, u32)>> = vec![Vec::new(); nodes.len()];
        for at in 0..nodes.len() {
            for &used in &nodes[at].uses {
                if let Some(def) = definition_before(nodes, at, used.reg) {
                    let latency = operand_latency(&nodes[def], used);
                    depth[at] = depth[at].max(depth[def] + latency);
                    users[def].push((at, latency));
                }
            }
        }
        let mut height = vec![0u32; nodes.len()];
        for at in (0..nodes.len()).rev() {
            for &(user, latency) in &users[at] {
                height[at] = height[at].max(height[user] + latency);
            }
        }
        let critical = (0..nodes.len()).map(|at| depth[at] + height[at]).max().unwrap_or(0);
        Self { depth, height, critical }
    }

    /// `getInstrSlack`: how many cycles the instruction can be late without
    /// lengthening the block's critical path.
    fn slack(
        &self,
        at: usize,
    ) -> u32 {
        self.critical.saturating_sub(self.depth[at] + self.height[at])
    }
}

/// `getDepth`: the depth of the last inserted instruction, whose reads may be
/// of registers an earlier inserted one defines.
fn new_root_depth(
    nodes: &[Node],
    trace: &Trace,
    root: usize,
    inserted: &[Node],
) -> u32 {
    let mut depths: Vec<u32> = Vec::with_capacity(inserted.len());
    for (index, node) in inserted.iter().enumerate() {
        let mut depth = 0;
        for &used in &node.uses {
            let within = inserted[..index].iter().rposition(|earlier| earlier.defs.contains(&used.reg));
            let reach = match within {
                Some(def) => depths[def] + operand_latency(&inserted[def], used),
                None => match definition_before(nodes, root, used.reg) {
                    Some(def) => trace.depth[def] + operand_latency(&nodes[def], used),
                    None => 0,
                },
            };
            depth = depth.max(reach);
        }
        depths.push(depth);
    }
    depths.last().copied().unwrap_or(0)
}

/// `getLatency`: the new root's latency, from the first read of its result in
/// the block after the root, else its own.
fn new_root_latency(
    nodes: &[Node],
    root: usize,
    new_root: &Node,
) -> u32 {
    let mut latency = 0;
    for def in &new_root.defs {
        let reader = nodes[root + 1..].iter().find_map(|node| node.uses.iter().find(|used| used.reg == *def));
        latency = latency.max(match reader {
            Some(&used) => operand_latency(new_root, used),
            None => new_root.latency,
        });
    }
    latency
}

/// `improvesCriticalPathLen`.
fn improves_critical_path<I: CombinerIsa>(
    isa: &I,
    nodes: &[Node],
    trace: &Trace,
    root: usize,
    pattern: I::Pattern,
    replacement: &Replacement,
) -> bool {
    let new_depth = new_root_depth(nodes, trace, root, &replacement.inserted);
    let old_depth = trace.depth[root];
    if isa.objective(pattern) == Objective::MustReduceDepth {
        return new_depth < old_depth;
    }
    let last = replacement.inserted.last().expect("a replacement inserts an instruction");
    let (new_latency, old_latency) = if isa.accumulates_sequence_latency(root) {
        let before: u32 = replacement.inserted[..replacement.inserted.len() - 1].iter().map(|node| node.latency).sum();
        let removed: u32 = replacement.deleted.iter().map(|&at| nodes[at].latency).sum();
        (before + new_root_latency(nodes, root, last), removed)
    } else {
        (last.latency, nodes[root].latency)
    };
    let slack = trace.slack(root);
    new_depth + new_latency <= old_depth + old_latency + slack
}

/// `combineInstructions`: whether anything in the block was replaced.
pub fn combined<I: CombinerIsa>(
    isa: &mut I,
    setting: Setting,
) -> bool {
    let mut nodes = isa.nodes();
    let mut changed = false;
    let mut root = 0;
    while root < nodes.len() {
        let mut replaced = false;
        for pattern in isa.patterns(root) {
            let Some(replacement) = isa.generate(root, pattern) else { continue };
            if replacement.inserted.is_empty() {
                continue;
            }
            let removed: Vec<Node> = replacement.deleted.iter().map(|&at| nodes[at].clone()).collect();
            let accepted = if setting.in_loop && isa.is_throughput(pattern) {
                true
            } else if setting.optimize_for_size && replacement.inserted.len() < replacement.deleted.len() {
                true
            } else {
                let trace = Trace::of(&nodes);
                improves_critical_path(isa, &nodes, &trace, root, pattern, &replacement)
                    && isa.preserves_resources(&replacement.inserted, &removed)
            };
            if !accepted {
                continue;
            }
            isa.apply(root, pattern, &replacement);
            nodes = isa.nodes();
            changed = true;
            replaced = true;
            break;
        }
        // The pin goes back one instruction: it may start a pattern now.
        root = if replaced { root.saturating_sub(1) } else { root + 1 };
    }
    changed
}

#[cfg(test)]
#[path = "combine_tests.rs"]
mod tests;
