//! `MachineLICM`, the early pass before `MachineCSE`
//! ([source](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/MachineLICM.cpp#L827)):
//! an instruction in a loop that reads no register and is trivially
//! rematerializable (a constant) moves to the preheader of the outermost loop
//! holding it, which an edge split makes where the loop has none (inner loops
//! get none: the pin starts from `MLI->begin()`); where an identical one is
//! already there, the loop's is dropped for it. A cheap one that feeds a PHI of
//! the loop stays (`HasLoopPHIUse`).
//!
//! An instruction that reads registers moves too when everything it reads is
//! defined outside the loop (`IsLoopInvariantInst`) and it is profitable
//! (`IsProfitableToHoist`): a hoist that makes no register live that was not
//! already (its result, less the operands this is the last use of:
//! `calcRegisterCost`). A cheap one that feeds a PHI of the loop stays. One
//! the outermost loop cannot take is tried at the preheader of each loop
//! around it, outermost first (`HoistOutOfLoop`'s sub-loop retry).
//!
//! DEVIATE 1: register pressure is not modelled beyond that rule. The pin's
//! wasm pressure limits are the two reserved registers, so
//! `CanCauseHighRegPressure` holds for every definition that adds a live
//! register, and only a rematerializable instruction or one of net weight 0
//! or less is hoisted (pin `llc -print-after=early-machinelicm`: a
//! `ADD x, killed const` leaves the inner loop, a `SHL x, const` whose
//! constant another use shares does not). No `ExtractHoistableLoad`, no
//! block-hotness check. Exit: the shared machine LICM over the neutral
//! machine function (S4), no later than 2026-11-20.
//!
//! # See also
//! `MachineLICMImpl::{HoistOutOfLoop, Hoist, IsProfitableToHoist,
//! HasLoopPHIUse, EliminateCSE, getOrCreatePreheader}`, pin d1106deb71cc.

use llrm_support::hash::HashMap;

use llrm_mir::module::BlockId;

use crate::cfg::Cfg;
use crate::dagisel::generated::instrinfo::{desc, flag};
use crate::machine_sink::is_safe_to_move;
use crate::mir::{Function, Inst, Operand, PHI, VReg, is_copy};

/// The expression an instruction computes: its opcode and the operands that
/// are no definition.
fn same_value(
    a: &Inst,
    b: &Inst,
) -> bool {
    let uses = |i: &Inst| i.operands.iter().filter(|o| !matches!(o, Operand::Def(_))).copied().collect::<Vec<_>>();
    a.opcode == b.opcode && uses(a) == uses(b)
}

/// `isTriviallyReMaterializable` for what is hoisted: a constant, with no
/// register to read.
fn is_hoistable(inst: &Inst) -> bool {
    let Some(info) = desc(inst.opcode) else { return false };
    info.has(flag::REMATERIALIZABLE)
        && !info.has(flag::CONVERGENT)
        && !inst.operands.iter().any(|o| matches!(o, Operand::Use(_)))
        && inst.defs().next().is_some()
        && is_safe_to_move(inst, &mut true)
}

/// Whether `inst` may leave a loop at all besides being a constant: it
/// defines one register, reads no memory and has no effect (`isSafeToMove`).
fn is_movable(inst: &Inst) -> bool {
    let Some(info) = desc(inst.opcode) else { return false };
    !info.has(flag::CONVERGENT)
        && !info.has(flag::MAY_LOAD)
        && inst.defs().count() == 1
        && !inst.is_terminator()
        && is_safe_to_move(inst, &mut true)
}

/// `IsProfitableToHoist` where the target has no registers to spare
/// (`CanCauseHighRegPressure` holds for a hoist that makes a register live):
/// the register class weights of `calcRegisterCost` sum to nothing or less,
/// the result against each operand that is the last use of its register.
fn adds_no_live_register(
    f: &Function,
    inst: &Inst,
    use_count: &HashMap<VReg, usize>,
) -> bool {
    let mut weight = [0i32; 4];
    let class = |r: VReg| f.vregs[r as usize] as usize;
    for o in &inst.operands {
        match o {
            Operand::Def(r) => weight[class(*r)] += 1,
            Operand::Use(r) if use_count.get(r) == Some(&1) => weight[class(*r)] -= 1,
            _ => {}
        }
    }
    weight.iter().all(|&w| w <= 0)
}

/// `HasLoopPHIUse`: a PHI of the loop, or one in a block the loop exits to,
/// reads the result, or a COPY of it in the loop does.
fn has_loop_phi_use(
    f: &Function,
    users: &HashMap<VReg, Vec<(usize, usize)>>,
    contains: &dyn Fn(usize) -> bool,
    exits: &[usize],
    b: usize,
    i: usize,
) -> bool {
    let mut work = vec![(b, i)];
    while let Some((b, i)) = work.pop() {
        for d in f.blocks[b][i].defs() {
            for &(ub, ui) in users.get(&d).into_iter().flatten() {
                let user = &f.blocks[ub][ui];
                if user.opcode == PHI {
                    if contains(ub) || exits.contains(&ub) {
                        return true;
                    }
                } else if is_copy(user.opcode) && contains(ub) {
                    work.push((ub, ui));
                }
            }
        }
    }
    false
}

/// Whether anything changed.
pub fn machine_licm(f: &mut Function) -> bool {
    if f.personality || f.blocks.is_empty() {
        return false;
    }
    let mut changed = false;
    // `getOrCreatePreheader`: an edge split where the loop has none.
    loop {
        let cfg = Cfg::of(f);
        let (tree, loops) = cfg.analyses();
        if cfg.irreducible_edge(&tree).is_some() {
            return changed;
        }
        let missing = loops
            .top_level_loops()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .find_map(
                |l| {
                    let header = l.get_header();
                    if f.is_eh_pad(header.0 as usize) || l.get_loop_preheader(&cfg).is_some() {
                        return None;
                    }
                    l.get_loop_predecessor(&cfg).map(|pred| (pred.0 as usize, header.0 as usize))
                },
            );
        let Some((pred, header)) = missing else { break };
        crate::phi_elimination::split_edge(f, pred, header);
        changed = true;
    }
    let cfg = Cfg::of(f);
    let (tree, loops) = cfg.analyses();
    let n = f.blocks.len();
    let mut children = vec![Vec::new(); n];
    for b in 1..n {
        if let Some(up) = tree.immediate_dominator(BlockId(b as u32)) {
            children[up.0 as usize].push(b);
        }
    }
    let mut replaced: HashMap<VReg, VReg> = HashMap::default();
    for l in loops.top_level_loops().collect::<Vec<_>>().into_iter().rev() {
        let header = l.get_header().0 as usize;
        let Some(preheader) = l.get_loop_preheader(&cfg).map(|b| b.0 as usize) else { continue };
        if f.is_eh_pad(header) {
            continue;
        }
        let in_loop = |b: usize| l.contains(BlockId(b as u32));
        // The blocks the header dominates within the loop, in the pin's order.
        let mut scopes = Vec::new();
        let mut work = vec![header];
        while let Some(b) = work.pop() {
            if !in_loop(b) {
                continue;
            }
            scopes.push(b);
            if f.succs[b].len() >= 25 {
                continue;
            }
            work.extend(children[b].iter().rev());
        }
        // Where each register is read, for `HasLoopPHIUse`.
        let mut users: HashMap<VReg, Vec<(usize, usize)>> = HashMap::default();
        for (b, insts) in f.blocks.iter().enumerate() {
            for (i, inst) in insts.iter().enumerate() {
                for o in &inst.operands {
                    if let Operand::Use(r) = o {
                        users.entry(*r).or_default().push((b, i));
                    }
                }
            }
        }
        // `MRI`'s use counts as the scan changes them: an operand that is the
        // only use of its register is a kill (`isOperandKill`).
        let mut use_count: HashMap<VReg, usize> = users.iter().map(|(r, v)| (*r, v.len())).collect();
        // Where each register is defined now.
        let mut def_block: HashMap<VReg, usize> = HashMap::default();
        for (b, insts) in f.blocks.iter().enumerate() {
            for inst in insts {
                for d in inst.defs() {
                    def_block.insert(d, b);
                }
            }
        }
        let phi_use = |f: &Function, contains: &dyn Fn(usize) -> bool, exits: &[usize], b: usize, i: usize| {
            has_loop_phi_use(f, &users, contains, exits, b, i)
        };
        // The instructions each preheader has, and those hoisted into it.
        let mut known: HashMap<usize, Vec<Inst>> = HashMap::default();
        let mut moves: Vec<(usize, usize, usize)> = Vec::new();
        let mut dropped: Vec<(usize, usize)> = Vec::new();
        for &b in &scopes {
            for i in 0..f.blocks[b].len() {
                let inst = &f.blocks[b][i];
                let constant = is_hoistable(inst);
                if !constant && !is_movable(inst) {
                    continue;
                }
                let renamed = |r: VReg| {
                    let mut r = r;
                    while let Some(&next) = replaced.get(&r) {
                        r = next;
                    }
                    r
                };
                let mut inst = inst.clone();
                for o in &mut inst.operands {
                    if let Operand::Use(r) = o {
                        *r = renamed(*r);
                    }
                }
                let cheap = desc(inst.opcode).is_some_and(|d| d.has(flag::CHEAP_AS_A_MOVE));
                // The outermost loop first, then each loop around the block, inwards:
                // `HoistOutOfLoop` retries a subloop's preheader where the outermost
                // loop's did not take the instruction.
                let mut around = Vec::new();
                let mut cur = loops.get_loop_for(BlockId(b as u32));
                while let Some(x) = cur {
                    if x.get_header() == l.get_header() {
                        break;
                    }
                    around.push(x);
                    cur = x.get_parent_loop();
                }
                around.reverse();
                let mut target = None;
                for (at, candidate) in std::iter::once(l).chain(around).enumerate() {
                    let pre = if at == 0 { Some(preheader) } else { candidate.get_loop_preheader(&cfg).map(|p| p.0 as usize) };
                    let Some(pre) = pre else { continue };
                    if !constant
                        && !inst.operands.iter().all(|o| match o {
                            Operand::Use(r) => def_block.get(r).is_none_or(|&d| !candidate.contains(BlockId(d as u32))),
                            _ => true,
                        })
                    {
                        continue;
                    }
                    if cheap {
                        let contains = |x: usize| candidate.contains(BlockId(x as u32));
                        let exits: Vec<usize> = candidate
                            .get_blocks()
                            .iter()
                            .flat_map(|x| f.succs[x.0 as usize].iter().copied())
                            .filter(|&x| !contains(x))
                            .collect();
                        if phi_use(f, &contains, &exits, b, i) {
                            continue;
                        }
                    }
                    if !constant && !adds_no_live_register(f, &inst, &use_count) {
                        continue;
                    }
                    target = Some(pre);
                    break;
                }
                let Some(pre) = target else { continue };
                let seen = known.entry(pre).or_insert_with(|| f.blocks[pre].clone());
                if let Some(dup) = seen.iter().find(|p| same_value(&inst, p)) {
                    for (old, new) in inst.defs().zip(dup.defs()) {
                        replaced.insert(old, new);
                        let moved = use_count.remove(&old).unwrap_or(0);
                        *use_count.entry(new).or_default() += moved;
                    }
                    for o in &inst.operands {
                        if let Operand::Use(r) = o {
                            *use_count.entry(*r).or_default() -= 1;
                        }
                    }
                    dropped.push((b, i));
                } else {
                    for d in inst.defs() {
                        def_block.insert(d, pre);
                    }
                    seen.push(inst);
                    moves.push((b, i, pre));
                }
            }
        }
        if moves.is_empty() && dropped.is_empty() {
            continue;
        }
        let hoisted: Vec<(usize, Inst)> = moves.iter().map(|&(b, i, pre)| (pre, f.blocks[b][i].clone())).collect();
        let mut gone: Vec<(usize, usize)> =
            moves.iter().map(|&(b, i, _)| (b, i)).chain(dropped.iter().copied()).collect();
        gone.sort_unstable_by(|a, b| b.cmp(a));
        for (b, i) in gone {
            f.blocks[b].remove(i);
        }
        for (pre, inst) in hoisted {
            let at = f.blocks[pre].iter().position(|i| i.is_terminator()).unwrap_or(f.blocks[pre].len());
            f.blocks[pre].insert(at, inst);
        }
        changed = true;
        // Later loops read the moved instructions at their new places.
        if !replaced.is_empty() {
            let resolve = |mut r: VReg| {
                while let Some(&next) = replaced.get(&r) {
                    r = next;
                }
                r
            };
            for operand in f.blocks.iter_mut().flatten().flat_map(|inst| inst.operands.iter_mut()) {
                if let Operand::Use(r) = operand {
                    *r = resolve(*r);
                }
            }
            replaced.clear();
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dagisel::generated::ids::opcode as op;
    use crate::mir::ValType;

    fn konst(
        dst: VReg,
        value: i64,
    ) -> Inst {
        Inst::new(op::CONST_I32, &[Operand::Def(dst), Operand::Imm(value)])
    }

    /// bb1 loops on itself: `k = 7; s = k + n; br_if bb1 s; br bb2`.
    /// `entry` is bb0's code, which ends in a jump to bb1 or in a branch to
    /// bb1 or bb2.
    fn counted_loop(
        branching_entry: bool,
        entry_const: bool,
    ) -> (Function, VReg, VReg, VReg) {
        let mut f = Function::default();
        let (n, k, s) = (f.new_vreg(ValType::I32), f.new_vreg(ValType::I32), f.new_vreg(ValType::I32));
        let mut head = vec![konst(n, 3)];
        if entry_const {
            head.push(konst(f.new_vreg(ValType::I32), 7));
        }
        if branching_entry {
            head.push(Inst::new(op::BR_IF, &[Operand::Block(1), Operand::Use(n)]));
            head.push(Inst::new(op::BR, &[Operand::Block(2)]));
        } else {
            head.push(Inst::new(op::BR, &[Operand::Block(1)]));
        }
        f.blocks = vec![
            head,
            vec![
                konst(k, 7),
                Inst::new(op::ADD_I32, &[Operand::Def(s), Operand::Use(k), Operand::Use(n)]),
                Inst::new(op::BR_IF, &[Operand::Block(1), Operand::Use(s)]),
                Inst::new(op::BR, &[Operand::Block(2)]),
            ],
            vec![Inst::new(op::RETURN, &[])],
        ];
        f.succs = vec![if branching_entry { vec![1, 2] } else { vec![1] }, vec![1, 2], vec![]];
        (f, n, k, s)
    }

    fn consts(block: &[Inst]) -> usize {
        block.iter().filter(|i| i.opcode == op::CONST_I32).count()
    }

    /// A constant the loop recomputes every trip was left in the loop: the
    /// pin runs it once, in the preheader.
    #[test]
    fn a_constant_in_a_loop_moves_to_the_preheader() {
        let (mut f, ..) = counted_loop(false, false);
        assert!(machine_licm(&mut f));
        assert_eq!(consts(&f.blocks[1]), 0);
        assert_eq!(consts(&f.blocks[0]), 2);
    }

    /// A loop entered from a branch has no preheader: the pin splits the
    /// edge, and the constant goes to the new block, not into the branching
    /// block (which the other path would also run).
    #[test]
    fn a_loop_entered_from_a_branch_gets_a_preheader_of_its_own() {
        let (mut f, ..) = counted_loop(true, false);
        assert!(machine_licm(&mut f));
        assert_eq!(f.blocks.len(), 4);
        assert_eq!(consts(&f.blocks[0]), 1);
        assert_eq!(consts(&f.blocks[1]), 1);
        assert_eq!(consts(&f.blocks[2]), 0);
    }

    /// An identical constant already in the preheader is reused: the loop's
    /// copy is dropped and its uses read the preheader's register.
    #[test]
    fn a_constant_the_preheader_already_has_is_not_computed_twice() {
        let (mut f, ..) = counted_loop(false, true);
        assert!(machine_licm(&mut f));
        assert_eq!(consts(&f.blocks[0]), 2);
        assert_eq!(consts(&f.blocks[1]), 0);
    }

    /// bb0 -> bb1 (outer header) -> bb2 (inner loop on itself) -> bb3 -> bb1.
    /// The inner loop's only predecessor branches twice, so it has no
    /// preheader.
    fn nested_loops() -> Function {
        let mut f = Function::default();
        let (n, k, s) = (f.new_vreg(ValType::I32), f.new_vreg(ValType::I32), f.new_vreg(ValType::I32));
        f.blocks = vec![
            vec![konst(n, 3), Inst::new(op::BR, &[Operand::Block(1)])],
            vec![Inst::new(op::BR_IF, &[Operand::Block(2), Operand::Use(n)]), Inst::new(op::BR, &[Operand::Block(4)])],
            vec![
                konst(k, 7),
                Inst::new(op::ADD_I32, &[Operand::Def(s), Operand::Use(k), Operand::Use(n)]),
                Inst::new(op::BR_IF, &[Operand::Block(2), Operand::Use(s)]),
                Inst::new(op::BR, &[Operand::Block(3)]),
            ],
            vec![Inst::new(op::BR, &[Operand::Block(1)])],
            vec![Inst::new(op::RETURN, &[])],
        ];
        f.succs = vec![vec![1], vec![2, 4], vec![2, 3], vec![1], vec![]];
        f
    }

    /// An inner loop got a block of its own for a preheader (queens -O1 had
    /// 17 blocks to the pin's 16): the pin runs the pass on the outermost
    /// loops only, and a constant of the inner loop goes to the outer
    /// preheader.
    #[test]
    fn only_the_outermost_loop_gets_a_preheader() {
        let mut f = nested_loops();
        assert!(machine_licm(&mut f));
        assert_eq!(f.blocks.len(), 5);
        assert_eq!(consts(&f.blocks[0]), 2);
        assert_eq!(consts(&f.blocks[2]), 0);
    }
    /// bb0 -> bb1 (outer header: defines `v`) -> bb2 (inner loop on itself:
    /// `k = 7; a = left + k; t = a + n`) -> bb3 (outer latch) -> bb1. `left` is
    /// `v` (bb1's division) or, for `variant`, a register bb2's own division defines. bb1 has one successor, so
    /// it is the inner loop's preheader.
    fn inner_add(
        variant: bool,
        shared: bool,
    ) -> Function {
        let mut f = Function::default();
        let n = f.new_vreg(ValType::I32);
        let (v, k, a, t, j) = (
            f.new_vreg(ValType::I32),
            f.new_vreg(ValType::I32),
            f.new_vreg(ValType::I32),
            f.new_vreg(ValType::I32),
            f.new_vreg(ValType::I32),
        );
        let left = if variant { j } else { v };
        let mut inner = if shared { vec![] } else { vec![konst(k, 7)] };
        if variant {
            inner.push(Inst::new(op::DIV_S_I32, &[Operand::Def(j), Operand::Use(n), Operand::Use(n)]));
        }
        inner.extend([
            Inst::new(op::ADD_I32, &[Operand::Def(a), Operand::Use(left), Operand::Use(k)]),
            Inst::new(op::ADD_I32, &[Operand::Def(t), Operand::Use(a), Operand::Use(n)]),
            Inst::new(op::BR_IF, &[Operand::Block(2), Operand::Use(t)]),
            Inst::new(op::BR, &[Operand::Block(3)]),
        ]);
        // `shared`: the outer loop reads `v` and a constant 7 of its own, so the
        // inner add ends the last use of neither operand.
        let mut latch = vec![Inst::new(op::BR_IF, &[Operand::Block(1), Operand::Use(n)]), Inst::new(op::BR, &[Operand::Block(4)])];
        if shared {
            let w = f.new_vreg(ValType::I32);
            latch.insert(0, Inst::new(op::ADD_I32, &[Operand::Def(w), Operand::Use(v), Operand::Use(k)]));
        }
        f.blocks = vec![
            vec![konst(n, 3), Inst::new(op::BR, &[Operand::Block(1)])],
            // A division has an effect: it stays where it is, and `v` varies with the outer loop.
            {
                let mut outer = vec![Inst::new(op::DIV_S_I32, &[Operand::Def(v), Operand::Use(n), Operand::Use(n)])];
                if shared {
                    outer.push(konst(k, 7));
                }
                outer.push(Inst::new(op::BR, &[Operand::Block(2)]));
                outer
            },
            inner,
            latch,
            vec![Inst::new(op::RETURN, &[])],
        ];
        f.succs = vec![vec![1], vec![2], vec![2, 3], vec![1, 4], vec![]];
        f
    }

    fn adds(block: &[Inst]) -> usize {
        block.iter().filter(|i| i.opcode == op::ADD_I32).count()
    }

    /// grep -O2: `fail[s]` is `ADD s, @fail`, read in an inner loop of the
    /// loop that defines `s`. The pin hoists the add to the inner loop's
    /// preheader (net weight 0: its result for the constant it ends); ours
    /// kept it in the loop, 3 instructions a trip (+3,672 of 5.75 M).
    #[test]
    fn an_add_of_an_outer_value_and_a_last_use_constant_leaves_the_inner_loop() {
        let mut f = inner_add(false, false);
        assert!(machine_licm(&mut f));
        // bb1 is the inner loop's preheader: both adds read only what the loop does not
        // change, and each ends the last use of its operand, so they leave it.
        assert_eq!(adds(&f.blocks[1]), 2);
        assert_eq!(adds(&f.blocks[2]), 0);
    }

    /// An add that makes a register live and ends the last use of none (both
    /// operands are read elsewhere) stays: the pin's `CanCauseHighRegPressure`
    /// holds, and only a remat or a net-zero hoist passes it. The latch's
    /// `v + k` is the other reader here.
    #[test]
    fn an_add_whose_operands_are_read_elsewhere_stays() {
        let mut f = inner_add(false, true);
        machine_licm(&mut f);
        assert_eq!(adds(&f.blocks[1]), 0);
        assert_eq!(adds(&f.blocks[2]), 2);
    }

    /// An add that reads a register the inner loop defines is not invariant.
    #[test]
    fn an_add_of_a_loop_variant_value_stays() {
        let mut f = inner_add(true, false);
        machine_licm(&mut f);
        assert_eq!(adds(&f.blocks[2]), 2);
    }
}
