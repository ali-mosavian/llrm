//! `PHITransAddr`: an address expression carried up the CFG from a block to
//! one of its predecessors, through the phis of the block.
//!
//! A translation of LLVM's
//! [`PHITransAddr.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/PHITransAddr.cpp)
//! and `PHITransAddr.h` at d1106deb71cc, `gvn-add-phi-translation` at its
//! default (off). Each function names the C++ function it translates and keeps
//! its order of checks. A value is an [`Operand`]; an instruction input is an
//! [`InstId`].
//!
//! How it works: the expression is a value plus the list of instruction
//! inputs it is built from. Translating from `cur` to `pred` replaces each
//! input defined in `cur` by its phi incoming value on that edge, or folds it
//! into the expression (a cast, a GEP, an add of a constant) and translates its
//! operands. A rebuilt expression must simplify or already exist in a block
//! that dominates `pred`; nothing is created unless
//! [`PhiTransAddr::translate_with_insertion`] is asked to.
//!
//! Used by: `MemoryDependenceAnalysis` (non-local pointer queries) and GVN's
//! load PRE.
//!
//! DEVIATE, permanent (MIR lacks the construct):
//! 1. Debug locations: `setDebugLoc` is omitted; an instruction has none.
//! 2. `DataLayout`/`TargetLibraryInfo` in the simplify query: the simplifier
//!    takes the layout; there is no TLI.
//!
//! DEVIATE, stated facts:
//! 3. A constant has no use list, so the scan of a base's users for an equal
//!    GEP is a scan of the blocks that dominate the predecessor (the users the
//!    C++ keeps are those in this function in such a block): the same GEPs,
//!    found by walking the dominator chain, the newest (the highest instruction
//!    id, a stand-in for use-list order) returned. The scan of a cast's users
//!    ends at once for a constant (it folds first). Exit: a use-list model for
//!    constants. `test_a_gep_off_a_global_base_is_found_in_a_dominating_block`.
//! 4. Users come newest first, as LLVM's use list does; any match is an equal
//!    value and the newest is returned.
//!    `test_the_newest_of_two_equal_casts_is_returned`.
//! 5. The simplifier runs with no dominator tree and no assumption cache (`{DL,
//!    TLI, DT, AC}` in C++): the folds that need them are owed by `simplify.rs`
//!    (#2249).

use llrm_mir::constfold;
use llrm_mir::context::{ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::domtree::DomTree;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, Flags, Opcode};
use llrm_mir::simplify::{simplify_binop_operands, simplify_cast_operand, simplify_gep_operands};

/// What a translation reads: the context and layout the simplifier folds with,
/// the function, and the dominator tree (`DT`, absent where C++ passes null).
pub struct Env<'a> {
    pub context: &'a Context,
    pub layout: &'a DataLayout,
    pub function: &'a Function,
    pub dt: Option<&'a DomTree>,
}

/// `SelectAddr::SelectAddrs`: the address on the true side and on the false.
pub type SelectAddrs = (Option<Operand>, Option<Operand>);

/// [`PHITransAddr`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/PHITransAddr.h#L68).
#[derive(Clone, Debug)]
pub struct PhiTransAddr {
    addr: Option<Operand>,
    inputs: Vec<InstId>,
}

/// The instruction that defines `value`.
fn inst_of(
    function: &Function,
    value: Operand,
) -> Option<InstId> {
    let Operand::Value(id) = value else { return None };
    match function.value(id).def {
        ValueDef::Instruction(inst) => Some(inst),
        ValueDef::Argument(_) => None,
    }
}

fn result_of(
    function: &Function,
    inst: InstId,
) -> Operand {
    Operand::Value(function.instruction(inst).result.expect("a value"))
}

fn is_constant_int(
    context: &Context,
    value: Operand,
) -> bool {
    matches!(
        value,
        Operand::Constant(c) if matches!(context.get(c).kind, ConstantKind::Int(_))
    )
}

/// An add of a constant: the one binary operation that is phi translatable.
fn is_add_of_constant(
    context: &Context,
    function: &Function,
    inst: InstId,
) -> bool {
    let instruction = function.instruction(inst);
    matches!(instruction.opcode, Opcode::Binary(BinaryOp::Add)) && is_constant_int(context, instruction.operands[1])
}

/// [`canPHITrans`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/PHITransAddr.cpp#L29).
fn can_phi_trans(
    context: &Context,
    function: &Function,
    inst: InstId,
) -> bool {
    matches!(
        function.instruction(inst).opcode,
        Opcode::Phi | Opcode::GetElementPtr { .. } | Opcode::Cast(_)
    )
        || is_add_of_constant(context, function, inst)
}

/// `getIncomingValueForBlock`.
fn incoming_value_for_block(
    function: &Function,
    phi: InstId,
    block: BlockId,
) -> Operand {
    function
        .instruction(phi)
        .operands
        .chunks(2)
        .find(|pair| pair[1] == Operand::Block(block))
        .map(|pair| pair[0])
        .expect("a phi has a value for each predecessor")
}

/// [`RemoveInstInputs`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/PHITransAddr.cpp#L111).
fn remove_inst_inputs(
    function: &Function,
    value: Operand,
    inputs: &mut Vec<InstId>,
) {
    let Some(inst) = inst_of(function, value) else { return };
    if let Some(at) = inputs.iter().position(|&one| one == inst) {
        inputs.remove(at);
        return;
    }
    assert!(!matches!(function.instruction(inst).opcode, Opcode::Phi), "Error, removing something that isn't an input");
    for &operand in &function.instruction(inst).operands {
        remove_inst_inputs(function, operand, inputs);
    }
}

/// [`verifySubExpr`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/PHITransAddr.cpp#L52).
fn verify_sub_expr(
    context: &Context,
    function: &Function,
    expr: Operand,
    inputs: &mut Vec<InstId>,
) -> bool {
    let Some(inst) = inst_of(function, expr) else { return true };
    if let Some(at) = inputs.iter().position(|&one| one == inst) {
        inputs.remove(at);
        return true;
    }
    assert!(
        can_phi_trans(context, function, inst),
        "Either something is missing from InstInputs or canPHITrans is wrong."
    );
    function
        .instruction(inst)
        .operands
        .clone()
        .into_iter()
        .all(|operand| verify_sub_expr(context, function, operand, inputs))
}

impl PhiTransAddr {
    /// `PHITransAddr(Addr, DL, AC)`.
    pub fn new(
        function: &Function,
        addr: Operand,
    ) -> Self {
        let mut one = Self { addr: Some(addr), inputs: Vec::new() };
        one.add_as_input(function, addr);
        one
    }

    /// `getAddr`.
    pub fn addr(&self) -> Option<Operand> {
        self.addr
    }

    /// `addAsInput`: an instruction is an input from now on.
    fn add_as_input(
        &mut self,
        function: &Function,
        value: Operand,
    ) -> Operand {
        if let Some(inst) = inst_of(function, value) {
            self.inputs.push(inst);
        }
        value
    }

    /// The translation is lost: there is no address.
    pub fn make_unavailable(&mut self) {
        self.addr = None;
    }

    /// `needsPHITranslationFromBlock`.
    pub fn needs_phi_translation_from_block(
        &self,
        function: &Function,
        block: BlockId,
    ) -> bool {
        self.inputs.iter().any(|&input| function.parent(input) == Some(block))
    }

    /// `getSelectCondition`
    /// ([:361](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/PHITransAddr.cpp#L361)).
    pub fn select_condition(
        &self,
        function: &Function,
    ) -> Option<Operand> {
        self.inputs
            .iter()
            .map(|&input| function.instruction(input))
            .find(|instruction| matches!(instruction.opcode, Opcode::Select))
            .map(|select| select.operands[0])
    }

    /// `verify`
    /// ([:82](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/PHITransAddr.cpp#L82)).
    pub fn verify(
        &self,
        context: &Context,
        function: &Function,
    ) -> bool {
        let Some(addr) = self.addr else { return true };
        let mut tmp = self.inputs.clone();
        verify_sub_expr(context, function, addr, &mut tmp) && tmp.is_empty()
    }

    /// `isPotentiallyPHITranslatable`
    /// ([:104](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/PHITransAddr.cpp#L104)).
    pub fn is_potentially_phi_translatable(
        &self,
        context: &Context,
        function: &Function,
    ) -> bool {
        let addr = self.addr.expect("an address");
        inst_of(function, addr).is_none_or(|inst| can_phi_trans(context, function, inst))
    }

    /// `translateValue(CurBB, PredBB, DT, MustDominate)`
    /// ([:321](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/PHITransAddr.cpp#L321)):
    /// the address in `pred`, or None.
    pub fn translate_value(
        &mut self,
        env: &mut Env,
        cur: BlockId,
        pred: BlockId,
        must_dominate: bool,
    ) -> Option<Operand> {
        assert!(env.dt.is_some() || !must_dominate);
        debug_assert!(self.verify(env.context, env.function), "Invalid PHITransAddr!");
        self.addr = match env.dt {
            Some(dt) if dt.is_reachable_from_entry(pred) => {
                let addr = self.addr.expect("an address");
                self.translate_sub_expr(env, addr, cur, pred, None)
            }
            _ => None,
        };
        debug_assert!(self.verify(env.context, env.function), "Invalid PHITransAddr!");
        if must_dominate
            && let Some(addr) = self.addr
            && let Some(inst) = inst_of(env.function, addr)
        {
            let dt = env.dt.expect("a dominator tree");
            if !env.function.parent(inst).is_some_and(|block| dt.dominates(block, pred)) {
                self.addr = None;
            }
        }
        self.addr
    }

    /// `translateValue(CurBB, PredBB, DT, Cond)`
    /// ([:341](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/PHITransAddr.cpp#L341)):
    /// the address on each side of the select on `cond`; `self` is unchanged.
    pub fn translate_value_select(
        &self,
        env: &mut Env,
        cur: BlockId,
        pred: BlockId,
        cond: Operand,
    ) -> SelectAddrs {
        debug_assert!(self.verify(env.context, env.function), "Invalid PHITransAddr!");
        if !env.dt.is_some_and(|dt| dt.is_reachable_from_entry(pred)) {
            return (None, None);
        }
        let mut side = |cond_val: bool| {
            let mut tmp = self.clone();
            let addr = tmp.addr.expect("an address");
            tmp.translate_sub_expr(env, addr, cur, pred, Some((cond, cond_val)))
        };
        (side(true), side(false))
    }

    /// `translateSubExpr`
    /// ([:130](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/PHITransAddr.cpp#L130)).
    fn translate_sub_expr(
        &mut self,
        env: &mut Env,
        v: Operand,
        cur: BlockId,
        pred: BlockId,
        select: Option<(Operand, bool)>,
    ) -> Option<Operand> {
        let function = env.function;
        let Some(inst) = inst_of(function, v) else { return Some(v) };
        let dominates_pred = |env: &Env, block: BlockId| env.dt.is_none_or(|dt| dt.dominates(block, pred));

        if let Some(at) = self.inputs.iter().position(|&one| one == inst) {
            // An input defined in another block remains an input.
            if function.parent(inst) != Some(cur) {
                return Some(v);
            }
            // Defined here, so it is incorporated into the expression or the
            // translation fails: either way it is not an input any longer.
            self.inputs.remove(at);

            if matches!(function.instruction(inst).opcode, Opcode::Phi) {
                let incoming = incoming_value_for_block(function, inst, pred);
                // A select on the condition being resolved folds to the
                // requested side.
                if let Some((cond, cond_val)) = select
                    && let Some(select_inst) = inst_of(function, incoming)
                {
                    let sel = function.instruction(select_inst);
                    if matches!(sel.opcode, Opcode::Select) && sel.operands[0] == cond {
                        let side = if cond_val { sel.operands[1] } else { sel.operands[2] };
                        return Some(self.add_as_input(function, side));
                    }
                }
                return Some(self.add_as_input(function, incoming));
            }

            if !can_phi_trans(env.context, function, inst) {
                return None;
            }
            // The operands are inputs now, and may be defined here as well.
            for operand in function.instruction(inst).operands.clone() {
                self.add_as_input(function, operand);
            }
        }

        // An intermediate result: rebuild it if an operand translates.
        let instruction = function.instruction(inst);
        match instruction.opcode.clone() {
            Opcode::Cast(op) => {
                let operand = instruction.operands[0];
                let ty = instruction.ty;
                let translated = self.translate_sub_expr(env, operand, cur, pred, select)?;
                if translated == operand {
                    return Some(v);
                }
                if let Some(simplified) = simplify_cast_operand(env.context, env.layout, function, op, translated, ty) {
                    remove_inst_inputs(function, translated, &mut self.inputs);
                    return Some(self.add_as_input(function, simplified));
                }
                // A constant has no use list here (DEVIATE 3).
                let Operand::Value(base) = translated else { return None };
                function
                    .users(base)
                    .iter()
                    .find_map(
                        |user| {
                            let candidate = function.instruction(user.user);
                            let block = function.parent(user.user)?;
                            (matches!(candidate.opcode, Opcode::Cast(other) if other == op)
                                && candidate.ty == ty
                                && dominates_pred(env, block))
                            .then(|| result_of(function, user.user))
                        },
                    )
            }
            Opcode::GetElementPtr { source } => {
                let (ty, operands, flags) = (instruction.ty, instruction.operands.clone(), instruction.flags);
                let mut translated = Vec::with_capacity(operands.len());
                let mut any_changed = false;
                for &operand in &operands {
                    let one = self.translate_sub_expr(env, operand, cur, pred, select)?;
                    any_changed |= one != operand;
                    translated.push(one);
                }
                if !any_changed {
                    return Some(v);
                }
                // `gep x, 0` is x.
                if let Some(simplified) = simplify_gep_operands(
                    env.context,
                    env.layout,
                    function,
                    source,
                    translated[0],
                    &translated[1..],
                    flags,
                ) {
                    for &operand in &translated {
                        remove_inst_inputs(function, operand, &mut self.inputs);
                    }
                    return Some(self.add_as_input(function, simplified));
                }
                // Is this GEP available? A constant base has no use list here
                // (DEVIATE 3): its users that the C++ keeps are the GEPs of
                // this function in a block that dominates `pred`, the blocks
                // of `pred`'s dominator chain; the newest is returned, as from
                // a use list.
                let Operand::Value(base) = translated[0] else {
                    let dt = env.dt?;
                    let mut newest: Option<InstId> = None;
                    let mut block = Some(pred);
                    while let Some(here) = block {
                        for candidate_id in function.iter_block(here) {
                            let candidate = function.instruction(candidate_id);
                            if matches!(
                                candidate.opcode,
                                Opcode::GetElementPtr { source: other } if other == source
                            ) && candidate.ty == ty
                                && candidate.operands == translated
                                && newest.is_none_or(|best| candidate_id.0 > best.0)
                            {
                                newest = Some(candidate_id);
                            }
                        }
                        block = dt.immediate_dominator(here);
                    }
                    return newest.map(|found| result_of(function, found));
                };
                function
                    .users(base)
                    .iter()
                    .find_map(
                        |user| {
                            let candidate = function.instruction(user.user);
                            let block = function.parent(user.user)?;
                            (matches!(
                                candidate.opcode,
                                Opcode::GetElementPtr { source: other } if other == source
                            ) && candidate.ty == ty
                                && candidate.operands == translated
                                && dominates_pred(env, block))
                            .then(|| result_of(function, user.user))
                        },
                    )
            }
            Opcode::Binary(BinaryOp::Add) if is_constant_int(env.context, instruction.operands[1]) => {
                let (lhs_in, rhs_in) = (instruction.operands[0], instruction.operands[1]);
                let (mut nsw, mut nuw) =
                    (instruction.flags.contains(Flags::NSW), instruction.flags.contains(Flags::NUW));
                let mut rhs = rhs_in;
                let mut lhs = self.translate_sub_expr(env, lhs_in, cur, pred, select)?;
                // An add of a constant on the left: fold the immediates.
                if let Some(inner) = inst_of(function, lhs)
                    && is_add_of_constant(env.context, function, inner)
                {
                    let (Operand::Constant(a), Operand::Constant(b)) = (rhs, function.instruction(inner).operands[1])
                    else {
                        unreachable!("both are constant ints")
                    };
                    let folded = constfold::binary(env.context, BinaryOp::Add, a, b).expect("two constant ints add");
                    let old = lhs;
                    lhs = function.instruction(inner).operands[0];
                    rhs = Operand::Constant(folded);
                    (nsw, nuw) = (false, false);
                    // If the old left was an input, the new one is.
                    if self.inputs.contains(&inner) {
                        remove_inst_inputs(function, old, &mut self.inputs);
                        self.add_as_input(function, lhs);
                    }
                }
                let mut flags = Flags::default();
                if nsw {
                    flags.insert(Flags::NSW);
                }
                if nuw {
                    flags.insert(Flags::NUW);
                }
                if let Some(simplified) =
                    simplify_binop_operands(env.context, env.layout, function, BinaryOp::Add, flags, lhs, rhs)
                {
                    // The left is no longer an input; the result is.
                    remove_inst_inputs(function, lhs, &mut self.inputs);
                    return Some(self.add_as_input(function, simplified));
                }
                if lhs == lhs_in && rhs == rhs_in {
                    return Some(v);
                }
                let Operand::Value(base) = lhs else { return None };
                function
                    .users(base)
                    .iter()
                    .find_map(
                        |user| {
                            let candidate = function.instruction(user.user);
                            let block = function.parent(user.user)?;
                            (matches!(candidate.opcode, Opcode::Binary(BinaryOp::Add))
                                && candidate.operands[0] == lhs
                                && candidate.operands[1] == rhs
                                && dominates_pred(env, block))
                            .then(|| result_of(function, user.user))
                        },
                    )
            }
            _ => None,
        }
    }

    /// `translateWithInsertion`
    /// ([:375](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/PHITransAddr.cpp#L375)):
    /// the address in `pred`, made there if it is not available. The
    /// instructions made are in `new_insts`; on failure they are erased.
    pub fn translate_with_insertion(
        &mut self,
        context: &mut Context,
        layout: &DataLayout,
        function: &mut Function,
        dt: &DomTree,
        cur: BlockId,
        pred: BlockId,
        new_insts: &mut Vec<InstId>,
    ) -> Option<Operand> {
        let size = new_insts.len();
        let addr = self.addr.expect("an address");
        self.addr = insert_translated_sub_expr(context, layout, function, dt, addr, cur, pred, new_insts);
        if self.addr.is_some() {
            return self.addr;
        }
        while new_insts.len() != size {
            let made = new_insts.pop().expect("one made");
            function.erase(made).expect("nothing uses it");
        }
        None
    }
}

/// `insertTranslatedSubExpr`
/// ([:398](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/PHITransAddr.cpp#L398)):
/// `in_val` for the edge `pred` -> `cur`, made at the end of `pred`
/// where it is not available. The add arm is off (`gvn-add-phi-translation`).
fn insert_translated_sub_expr(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    dt: &DomTree,
    in_val: Operand,
    cur: BlockId,
    pred: BlockId,
    new_insts: &mut Vec<InstId>,
) -> Option<Operand> {
    // A version already available and dominating `pred` needs no new one.
    let mut tmp = PhiTransAddr::new(function, in_val);
    let mut env = Env { context, layout, function, dt: Some(dt) };
    if let Some(addr) = tmp.translate_value(&mut env, cur, pred, true) {
        return Some(addr);
    }
    // Values that are not instructions need no translation.
    let inst = inst_of(function, in_val)?;
    let instruction = function.instruction(inst).clone();
    let name = |function: &Function| {
        format!(
            "{}.phi.trans.insert",
            function
                .value(match in_val {
                    Operand::Value(id) => id,
                    _ => unreachable!("an instruction's result"),
                })
                .name
                .as_deref()
                .unwrap_or("")
        )
    };
    let terminator = function.terminator(pred).expect("a terminated block");
    match instruction.opcode {
        Opcode::Cast(op) => {
            let operand = insert_translated_sub_expr(
                context,
                layout,
                function,
                dt,
                instruction.operands[0],
                cur,
                pred,
                new_insts,
            )?;
            let name = name(function);
            let made = function.create_instruction(
                Opcode::Cast(op),
                instruction.ty,
                vec![operand],
                Flags::default(),
                Some(&name),
            );
            function.insert(made, Position::Before(terminator)).expect("a placed terminator");
            new_insts.push(made);
            Some(result_of(function, made))
        }
        Opcode::GetElementPtr { source } => {
            // C++ translates from the GEP's own block.
            let own = function.parent(inst).expect("a placed GEP");
            let mut operands = Vec::with_capacity(instruction.operands.len());
            for &operand in &instruction.operands {
                operands
                    .push(insert_translated_sub_expr(context, layout, function, dt, operand, own, pred, new_insts)?);
            }
            let name = name(function);
            let made = function.create_instruction(
                Opcode::GetElementPtr { source },
                instruction.ty,
                operands,
                instruction.flags,
                Some(&name),
            );
            function.insert(made, Position::Before(terminator)).expect("a placed terminator");
            new_insts.push(made);
            Some(result_of(function, made))
        }
        _ => None,
    }
}

#[cfg(test)]
#[path = "phitrans_tests.rs"]
mod tests;
