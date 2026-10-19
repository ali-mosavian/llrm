//! `SSAUpdaterBulk`: values defined in several blocks, rewritten for all their
//! uses at once, with a phi where definitions meet.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `SSAUpdaterBulk.cpp`. The client adds
//! a variable for each value, says where it is available
//! ([`SsaUpdaterBulk::add_available_value`]) and which uses read it
//! ([`SsaUpdaterBulk::add_use`]); [`SsaUpdaterBulk::rewrite_all_uses`] then
//! does every variable. For one: the blocks the value is live into are found
//! walking back from the using blocks to the defining ones, the iterated
//! dominance frontier of the defining blocks pruned to those is where the phis
//! go ([`llrm_mir::idf`]), and each use reads the value that reaches its block,
//! found walking up the dominator tree to the first block that defines or
//! receives the value. A block the value is not defined above reads `undef`.
//!
//! [`crate::ssaupdater::SsaUpdater`] is the one-variable updater that walks the
//! predecessors itself; it reads the incoming blocks of a phi a block begins
//! with where this reads the control flow graph, and places phis by its own
//! dominator search, so the two can differ in where a phi stands.
//!
//! # Example
//! `%v` defined in `b1` and `b2`, used in `b4` below them:
//! ```mir
//! b0: br i1 %c, label %b1, label %b2
//! b1: %v1 = ...    br label %b4
//! b2: %v2 = ...    br label %b4
//! b4: use %v
//! ```
//! `add_available_value(var, b1, %v1)`, `(var, b2, %v2)` and `add_use(var, the
//! use)`, then `rewrite_all_uses`, place `%v = phi [ %v1, %b1 ], [ %v2, %b2 ]`
//! in `b4` and read it.
//!
//! # Deviations
//! - B1 `RewriteAndOptimizeAllUses` (the phi simplification and the
//!   deduplication passes) is not translated: nothing asks for it yet.
//! - B2 No value handles: `ValueHandleBase::ValueIsRAUWd` has nothing to
//!   notify.
//!
//! # See also
//! [`SSAUpdaterBulk::RewriteAllUses`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SSAUpdaterBulk.cpp#L111).

use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::dense::{IdMap, IdSet};
use llrm_mir::domtree::DomTree;
use llrm_mir::edit::Position;
use llrm_mir::idf;
use llrm_mir::loopinfo::inverse_children;
use llrm_mir::module::{BlockId, Function, InstId, Operand, Use};
use llrm_mir::opcode::{Flags, Opcode};
use llrm_mir::types::TypeId;

/// `RewriteInfo`: one variable.
struct RewriteInfo {
    /// `Defines`: where the value is available, and as what.
    defines: Vec<(BlockId, Operand)>,
    /// `Uses`.
    uses: Vec<Use>,
    name: Option<String>,
    ty: TypeId,
}

/// `BBValueInfo`.
#[derive(Clone, Copy, Default)]
struct BbValueInfo {
    live_in_value: Option<Operand>,
    live_out_value: Option<Operand>,
}

/// `SSAUpdaterBulk`.
#[derive(Default)]
pub struct SsaUpdaterBulk {
    rewrites: Vec<RewriteInfo>,
}

impl SsaUpdaterBulk {
    pub fn new() -> Self {
        Self::default()
    }

    /// `AddVariable`: a new variable, to be added to before the values and
    /// uses.
    pub fn add_variable(
        &mut self,
        name: Option<&str>,
        ty: TypeId,
    ) -> usize {
        self.rewrites.push(RewriteInfo { defines: Vec::new(), uses: Vec::new(), name: name.map(str::to_owned), ty });
        self.rewrites.len() - 1
    }

    /// `AddAvailableValue`: the variable has the value in the block.
    pub fn add_available_value(
        &mut self,
        var: usize,
        block: BlockId,
        value: Operand,
    ) {
        self.rewrites[var].defines.push((block, value));
    }

    /// `AddUse`: this use reads the variable, to be rewritten.
    pub fn add_use(
        &mut self,
        var: usize,
        one: Use,
    ) {
        self.rewrites[var].uses.push(one);
    }

    /// `RewriteAllUses`
    /// ([L111](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SSAUpdaterBulk.cpp#L111)):
    /// the phis the variables need and the requested uses rewritten, over the
    /// dominator tree `dt` of `function`; the phis made are added to
    /// `inserted_phis`.
    pub fn rewrite_all_uses(
        &mut self,
        context: &mut Context,
        function: &mut Function,
        dt: &DomTree,
        mut inserted_phis: Option<&mut Vec<InstId>>,
    ) {
        for rewrite in &self.rewrites {
            // Compute locations for new phi-nodes: DefBlocks from the
            // definitions, UsingBlocks from the uses, then LiveInBlocks, then
            // the iterated dominance frontier (IDF) of the definitions pruned
            // to those. The IDF blocks are where we need new phi-nodes.
            let mut def_blocks = IdSet::<BlockId>::new();
            for &(block, _) in &rewrite.defines {
                def_blocks.insert(block);
            }
            let mut using_blocks = Vec::new();
            for &one in &rewrite.uses {
                using_blocks.push(user_bb(function, one));
            }
            let live_in_blocks = compute_live_in_blocks(function, &using_blocks, &def_blocks);
            let idf_blocks = idf::forward(dt, &*function, &def_blocks, Some(&live_in_blocks));

            let mut infos = IdMap::<BlockId, BbValueInfo>::new();
            for &(block, value) in &rewrite.defines {
                infos.get_or_insert_with(block, BbValueInfo::default).live_out_value = Some(value);
            }

            // We've computed IDF, now insert new phi-nodes there.
            for &frontier in &idf_blocks {
                let phi = function.create_instruction(
                    Opcode::Phi,
                    rewrite.ty,
                    Vec::new(),
                    Flags::default(),
                    rewrite.name.as_deref(),
                );
                let first = function.block(frontier).first();
                function.insert(phi, first.map_or(Position::End(frontier), Position::Before)).expect("a placed block");
                let value = Operand::Value(function.instruction(phi).result.expect("a phi has a value"));
                infos.get_or_insert_with(frontier, BbValueInfo::default).live_in_value = Some(value);
                if let Some(found) = inserted_phis.as_deref_mut() {
                    found.push(phi);
                }
            }

            // Fill in arguments of the inserted PHIs.
            for &block in &idf_blocks {
                let phi = function.block(block).first().expect("the phi just made");
                let mut operands = Vec::new();
                // `for (BasicBlock *Pred : predecessors(BB))`: one input per
                // edge, so a switch with two cases to this
                // block gives it two.
                for pred in inverse_children(function, block) {
                    let value = compute_value(context, function, dt, &mut infos, rewrite.ty, pred, true);
                    operands.extend([value, Operand::Block(pred)]);
                }
                function.set_operands(phi, operands);
            }

            // Rewrite actual uses with the inserted definitions.
            let mut processed = llrm_mir::hash::HashSet::<Use>::default();
            for &one in &rewrite.uses {
                if !processed.insert(one) {
                    continue;
                }
                let bb = user_bb(function, one);
                let live_out = function.parent(one.user) != Some(bb);
                let value = compute_value(context, function, dt, &mut infos, rewrite.ty, bb, live_out);
                function.set_operand(one.user, one.index as usize, value);
            }
        }
    }
}

/// `getUserBB`: the block that should have a value for the use. For a phi it
/// is the corresponding predecessor, for other instructions their parent block.
fn user_bb(
    function: &Function,
    one: Use,
) -> BlockId {
    let user = function.instruction(one.user);
    if user.opcode == Opcode::Phi {
        let Operand::Block(from) = user.operands[one.index as usize + 1] else { unreachable!("a phi's block operand") };
        from
    } else {
        function.parent(one.user).expect("a placed user")
    }
}

/// `ComputeLiveInBlocks`
/// ([L77](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SSAUpdaterBulk.cpp#L77)):
/// given the using and the defining blocks, the blocks the value is live into:
/// a subgraph limited by them.
fn compute_live_in_blocks(
    function: &Function,
    using_blocks: &[BlockId],
    def_blocks: &IdSet<BlockId>,
) -> IdSet<BlockId> {
    let mut live_in = IdSet::<BlockId>::new();
    // Start with all the using blocks.
    let mut worklist: Vec<BlockId> = using_blocks.to_vec();

    // Now that we have a set of blocks where the phi is live-in, recursively
    // add their predecessors until we find the full region the value is live.
    while let Some(bb) = worklist.pop() {
        // The block really is live in here, insert it into the set. If already
        // in the set, then it has already been processed.
        if !live_in.insert(bb) {
            continue;
        }
        // Since the value is live into BB, it is either defined in a
        // predecessor or live into it to. Add the preds to the worklist unless
        // they are a defining block.
        for pred in function.predecessors(bb) {
            // The value is not live into a predecessor if it defines the value.
            if def_blocks.contains(&pred) {
                continue;
            }
            worklist.push(pred);
        }
    }
    live_in
}

/// `ComputeValue`: the value live out (or live in) at `bb`, found up the
/// dominator tree, and remembered for the blocks passed on the way.
fn compute_value(
    context: &mut Context,
    function: &Function,
    dt: &DomTree,
    infos: &mut IdMap<BlockId, BbValueInfo>,
    ty: TypeId,
    bb: BlockId,
    is_live_out: bool,
) -> Operand {
    let info = infos.get_or_insert_with(bb, BbValueInfo::default);
    if is_live_out && let Some(value) = info.live_out_value {
        return value;
    }
    if let Some(value) = info.live_in_value {
        return value;
    }

    let mut stack = vec![bb];
    let mut found: Option<Operand> = None;
    let mut at = bb;
    while dt.is_reachable_from_entry(at) && !function.predecessors(at).is_empty() {
        let Some(idom) = dt.immediate_dominator(at) else { break };
        at = idom;
        let info = infos.get_or_insert_with(at, BbValueInfo::default);
        if let Some(value) = info.live_out_value {
            found = Some(value);
            break;
        }
        if let Some(value) = info.live_in_value {
            found = Some(value);
            break;
        }
        stack.push(at);
    }

    let value =
        found.unwrap_or_else(|| Operand::Constant(context.constant(Constant { ty, kind: ConstantKind::Undef })));
    for block in stack {
        infos.get_or_insert_with(block, BbValueInfo::default).live_in_value = Some(value);
    }
    value
}

#[cfg(test)]
mod tests {
    use llrm_mir::domtree::DomTree;
    use llrm_mir::module::Operand;
    use llrm_mir::opcode::Opcode;

    use super::SsaUpdaterBulk;
    use crate::testing::{block, parsed, value};

    /// `%v` is also made in `b1` and `b2`: the use below them gets a phi of the
    /// two and of nothing else, the value of the block `b0` that defines
    /// the original reaching no path.
    #[test]
    fn test_a_use_below_two_definitions_reads_a_phi_of_them() {
        let mut module = parsed(
            "define i32 @f(i1 %c, i32 %a) {
b0:
  %v = add i32 %a, 1
  br i1 %c, label %b1, label %b2
b1:
  %v1 = add i32 %a, 2
  br label %b4
b2:
  %v2 = add i32 %a, 3
  br label %b4
b4:
  %u = add i32 %v, 0
  ret i32 %u
}
",
        );
        let (context, function) = module.function_mut("f").expect("f");
        let (b0, b1, b2, b4) =
            (block(function, "b0"), block(function, "b1"), block(function, "b2"), block(function, "b4"));
        let (v, v1, v2, u) = (value(function, "v"), value(function, "v1"), value(function, "v2"), value(function, "u"));
        let use_of_v = function.users(v).iter().copied().next().expect("the use in %u");
        let ty = function.value(v).ty;

        let mut bulk = SsaUpdaterBulk::new();
        let var = bulk.add_variable(Some("v"), ty);
        bulk.add_available_value(var, b0, Operand::Value(v));
        bulk.add_available_value(var, b1, Operand::Value(v1));
        bulk.add_available_value(var, b2, Operand::Value(v2));
        bulk.add_use(var, use_of_v);
        let dt = DomTree::of(function);
        let mut phis = Vec::new();
        bulk.rewrite_all_uses(context, function, &dt, Some(&mut phis));

        assert_eq!(phis.len(), 1, "one phi, where the definitions meet");
        let phi = phis[0];
        assert_eq!(function.parent(phi), Some(b4));
        assert_eq!(function.instruction(phi).opcode, Opcode::Phi);
        let phi_value = function.instruction(phi).result.expect("a phi has a value");
        let user = function.value(u).def;
        let llrm_mir::module::ValueDef::Instruction(user) = user else { panic!("%u is an instruction") };
        assert_eq!(function.instruction(user).operands[0], Operand::Value(phi_value), "the use reads the phi");
        assert_eq!(function.instruction(phi).operands.len(), 4, "an input for each of the two predecessors");
    }

    /// `rewrite_all_uses` gave a phi one input for each predecessor block,
    /// where the pin gives one for each edge: a switch with two cases to
    /// the block left the phi an input short (DFAJumpThreading on x_switch
    /// -O2 m16 then grew a 3 GB allocation in SCEV/liveness over the
    /// malformed phi).
    #[test]
    fn test_a_phi_has_an_input_for_each_edge_of_a_switch_into_its_block() {
        let mut module = parsed(
            "define i32 @f(i32 %a) {
b0:
  %v = add i32 %a, 1
  switch i32 %a, label %b1 [
    i32 0, label %b4
    i32 1, label %b4
  ]
b1:
  %v1 = add i32 %a, 2
  br label %b4
b4:
  %u = add i32 %v, 0
  ret i32 %u
}
",
        );
        let (context, function) = module.function_mut("f").expect("f");
        let (b0, b1, b4) = (block(function, "b0"), block(function, "b1"), block(function, "b4"));
        let (v, v1) = (value(function, "v"), value(function, "v1"));
        let use_of_v = function.users(v).iter().copied().next().expect("the use in %u");
        let ty = function.value(v).ty;

        let mut bulk = SsaUpdaterBulk::new();
        let var = bulk.add_variable(Some("v"), ty);
        bulk.add_available_value(var, b0, Operand::Value(v));
        bulk.add_available_value(var, b1, Operand::Value(v1));
        bulk.add_use(var, use_of_v);
        let dt = DomTree::of(function);
        let mut phis = Vec::new();
        bulk.rewrite_all_uses(context, function, &dt, Some(&mut phis));

        assert_eq!(phis.len(), 1);
        assert_eq!(function.parent(phis[0]), Some(b4));
        assert_eq!(function.instruction(phis[0]).operands.len(), 6, "an input for each of the three edges");
    }
}
