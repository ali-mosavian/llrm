//! Helpers adapted from llrm-core's `optimize/transform.rs`, each copied as
//! a ported pass needs it; the pipeline itself is `pipeline`.
//! `_unreachable` is `llrm_analysis::cfg::_unreachable`.
//!
//! What `gvn` reads -- `_PURE`, `_computation`, `_reaches`, `_undisturbed`
//! and `subexpressions` -- came with it. Left behind, each of the old
//! representation:
//! - `_widths`, `_full`, `_width` and `_copied`: a value was split into
//!   word halves and copied between registers; here every value is whole
//!   and nothing is a copy, so `stands` (what a copy numbered as) is the
//!   substitution itself.
//! - `halves`: which half of a value was read; a value here is whole.
//!   `live` is here for Dead.
//! - `reused_divides`, `divided_twice`: one divide had two answers, one
//!   dead. `sdiv` and `srem` are separate here, and two equal divides are
//!   one expression `subexpressions` finds.
//! - `_read` and the `flags` checks: no value is the machine's flags.
//! - `_reusable_float_path`, `_exact_floating`, `_exact_stored_load`,
//!   `_unchanged_float_environment`, `_erased_floating`: x87 exceptions,
//!   precision and the float environment. Floating arithmetic has no
//!   exceptions here (see `llrm_analysis::effects`), so it is as pure as
//!   integer arithmetic.
//! - `_phi_reading`, `_reclaimed`, `_empty_operation`: an erased
//!   operation's source bytes. `replace_all_uses_with` reaches phis too.
//!
//! `_undisturbed` asks `memoryssa::Accesses` what a store or call writes;
//! the old one refused every call. `forwarded` serves only a load: no
//! other instruction reads memory.
//! - The `dgroup` parameter: a segment is not a MIR fact.
//!
//! Tests of these, from `optimize/transform_tests.rs` and
//! `analysis/induction_tests.rs`, are here, the addresses and constants
//! now instructions; `test_cse_replaces_phi_uses_of_a_deleted_initializer`
//! keeps one case, its others varying variables, source bytes and a copied
//! constant. `test_a_third_equal_divide_reads_the_answer_the_first_computed`
//! tests `subexpressions`, which serves a divide here. Stay behind:
//! - test_cse_propagates_a_complete_narrow_copy_to_an_opaque_reader and
//!   test_cse_refuses_an_operand_that_is_only_half_its_value: halves and
//!   copies.
//! - test_deferred_runtime_float_reuse_respects_environment,
//!   test_unknown_integer_loads_share_a_value_but_unknown_floats_do_not
//!   and test_proven_copy_unlocks_strict_floating_cse (ignored there): the
//!   x87 environment and exactness, the last two reading BC fixtures.
//! - test_both_lngmix_divides_absorb: a BC corpus through `wholeseg`.
//! - test_value_reuse_is_one_gvn_pre_pass: the pipeline is not ported.
//! - test_a_served_read_names_the_value_and_not_a_register: a BC corpus,
//!   and a load here is replaced by its value outright.

use std::collections::{BTreeMap, BTreeSet};

use llrm_analysis::memory;
use llrm_analysis::memoryssa::Accesses;
use llrm_analysis::{avail, cfg, regions, ssa};
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::memory::Callees;
use llrm_mir::module::{BlockId, Function, InstId, Instruction, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::passes::Outer;
use llrm_mir::program::ProgramProxy;
use llrm_mir::types::TypeId;
use llrm_support::hash::IndexMap;

use crate::edges;
use crate::lcssa::{arms, from_arms};

/// Values something that stays reads, to a fixed point.
pub fn live(context: &Context, callees: &Callees, function: &Function) -> BTreeSet<ValueId> {
    live_except(context, callees, function, |_| false)
}

/// `live`, as if the instructions `skipped` picks did not stay.
pub fn live_except(context: &Context, callees: &Callees, function: &Function, skipped: impl Fn(InstId) -> bool) -> BTreeSet<ValueId> {
    let mut alive = BTreeSet::new();
    let mut pending = function.walk().map(|(_, inst)| inst).filter(|&inst| !skipped(inst) && crate::dead::_kept(context, callees, function, inst)).collect::<Vec<_>>();
    while let Some(inst) = pending.pop() {
        for &operand in &function.instruction(inst).operands {
            if let Operand::Value(value) = operand
                && alive.insert(value)
                && let ValueDef::Instruction(defining) = function.value(value).def
            {
                pending.push(defining);
            }
        }
    }
    alive
}

/// The comparison supplying `branch`'s condition: the `icmp` in `block`
/// that defines it.
///
/// The old one found the flag-setting `cmp`, or a `test`-like and/or/xor
/// read for equality, whose flags the branch read; both are an `icmp` here.
/// A float compare was refused as floating work, so `fcmp` is not one.
pub fn _comparison(function: &Function, block: BlockId, branch: InstId) -> Option<InstId> {
    let instruction = function.instruction(branch);
    if instruction.opcode != Opcode::Br || instruction.operands.len() != 3 {
        return None;
    }
    let Operand::Value(condition) = instruction.operands[0] else {
        return None;
    };
    function
        .block(block)
        .instructions()
        .iter()
        .copied()
        .find(|&one| function.instruction(one).result == Some(condition))
        .filter(|&one| matches!(function.instruction(one).opcode, Opcode::ICmp(_)))
}

/// Resolve single-valued joins after an edge disappears: each phi keeps
/// only its predecessors' inputs, one naming a single other value is that
/// value, and one with an earlier phi's arms is that phi (LLVM's
/// `EliminateDuplicatePHINodes`).
pub fn _trivial_phis(function: &mut Function) -> Result<(), String> {
    let predecessors = function.layout().iter().map(|&block| (block, function.predecessors(block))).collect::<BTreeMap<_, _>>();
    loop {
        let mut changed = false;
        for block in function.layout().to_vec() {
            for phi in edges::phis(function, block) {
                let result = function.instruction(phi).result.expect("a phi's value");
                let incoming = arms(function, phi).into_iter().filter(|(_, at)| predecessors[&block].contains(at)).collect::<Vec<_>>();
                let mut values = incoming.iter().map(|&(value, _)| value).filter(|&value| value != Operand::Value(result)).collect::<Vec<_>>();
                values.dedup();
                if let [value] = values[..] {
                    function.replace_value(result, value);
                    function.set_operands(phi, Vec::new());
                    function.erase(phi)?;
                    changed = true;
                } else if from_arms(&incoming) != function.instruction(phi).operands {
                    function.set_operands(phi, from_arms(&incoming));
                }
            }
        }
        if !_duplicate_phis(function)? && !changed {
            return Ok(());
        }
    }
}

/// Each phi whose arms, edge for edge, are an earlier phi's in its block
/// replaced by that phi. Whether any was.
fn _duplicate_phis(function: &mut Function) -> Result<bool, String> {
    let mut changed = false;
    for block in function.layout().to_vec() {
        let mut seen = llrm_support::hash::HashMap::<(TypeId, Vec<(BlockId, Operand)>), ValueId>::default();
        for phi in edges::phis(function, block) {
            let op = function.instruction(phi);
            let (ty, result) = (op.ty, op.result.expect("a phi's value"));
            let mut incoming = arms(function, phi).into_iter().map(|(value, from)| (from, value)).collect::<Vec<_>>();
            incoming.sort_by_key(|&(from, _)| from);
            match seen.entry((ty, incoming)) {
                std::collections::hash_map::Entry::Occupied(earlier) => {
                    function.replace_value(result, Operand::Value(*earlier.get()));
                    function.set_operands(phi, Vec::new());
                    function.erase(phi)?;
                    changed = true;
                }
                std::collections::hash_map::Entry::Vacant(slot) => {
                    slot.insert(result);
                }
            }
        }
    }
    Ok(changed)
}

// CSE only ever removes an operation whose whole answer is in its operands.
//
// By mnemonic, each old kind's instruction: `sub` is also Neg, `xor` Not,
// `getelementptr` PtrOffset and Address, `icmp` each comparison, the casts
// Convert, SignExtend and Extract. FixedMul, Smulhi and Concat were
// machine idioms with no one instruction; Copy has none. The pointer casts
// are the rich MIR's own: a segment made a pointer twice is one pointer.
pub const _PURE: [&str; 22] = [
    "add", "sub", "mul", "getelementptr", "udiv", "sdiv", "urem", "srem", "and", "or", "xor", "shl", "lshr", "ashr",
    "trunc", "zext", "sext", "icmp", "inttoptr", "ptrtoint", "bitcast", "addrspacecast",
];

/// The old `op.floating` kinds `_computation` took: arithmetic and the
/// conversions to and from integers. Fsqrt is an intrinsic call here.
pub fn _floating(opcode: &Opcode) -> bool {
    matches!(
        opcode,
        Opcode::Binary(BinaryOp::FAdd | BinaryOp::FSub | BinaryOp::FMul | BinaryOp::FDiv)
            | Opcode::Cast(CastOp::FPTrunc | CastOp::FPExt | CastOp::FPToUI | CastOp::FPToSI | CastOp::UIToFP | CastOp::SIToFP)
    )
}

/// One operation where two computed the same thing from the same values;
/// whether any went.
///
/// `avoid_store_crossing` keeps a load from being served across a store.
pub fn subexpressions(function: &mut Function, accesses: &Accesses, avoid_store_crossing: bool, program: Option<&ProgramProxy>, crossed: &std::cell::Cell<bool>) -> Result<bool, String> {
    let doms = cfg::Dominance::of(function).dominators(function);
    let order: IndexMap<BlockId, usize> = function.layout().iter().enumerate().map(|(index, &block)| (block, index)).collect();

    let mut seen: IndexMap<_Computation, Vec<(usize, usize, InstId)>> = IndexMap::default();
    // What a name is rewritten to, and so what it numbers as.
    let mut swap: BTreeMap<ValueId, Operand> = BTreeMap::new();
    let mut gone: BTreeSet<InstId> = BTreeSet::new();
    for &block in function.layout() {
        let here = order[&block];
        let instructions = function.block(block).instructions();
        for (index, &inst) in instructions.iter().enumerate() {
            let op = function.instruction(inst);
            let Some(key) = _computation(op, &swap) else {
                continue;
            };
            let candidates = seen.entry(key).or_default();
            let first = candidates
                .iter()
                .rev()
                .find(|candidate| _reaches(candidate.0, candidate.1, here, index, &doms, function.layout(), block))
                .copied();
            let Some((at, where_, earlier)) = first else {
                candidates.push((here, index, inst));
                continue;
            };
            let loads = matches!(op.opcode, Opcode::Load { .. });
            if loads && (at != here || !_undisturbed(inst, &instructions[where_ + 1..index], accesses, program)) {
                candidates.push((here, index, inst));
                continue;
            }
            if loads && instructions[where_ + 1..index].iter().any(|&between| matches!(function.instruction(between).opcode, Opcode::Store { .. })) {
                // The one place `avoid_store_crossing` changes what is numbered: told, so a caller that runs this both ways can see
                // when the second way is the first.
                crossed.set(true);
                if avoid_store_crossing {
                    candidates.push((here, index, inst));
                    continue;
                }
            }
            let (Some(mine), Some(theirs)) = (op.result, function.instruction(earlier).result) else {
                continue;
            };
            swap.insert(mine, Operand::Value(theirs));
            gone.insert(inst);
        }
    }

    if gone.is_empty() {
        return Ok(false);
    }
    // A leader is never itself swapped, so the order is free.
    for (&value, &with) in &swap {
        function.replace_value(value, with);
    }
    for inst in gone {
        function.erase(inst)?;
    }
    Ok(true)
}

/// Python's `_computation` key tuple.
///
/// `name` was the old operation's variant; it is the opcode, whose
/// predicate, cast or element type is part of it, with the flags. The
/// result widths are the result type. `operands` is `(unordered, named)`:
/// a commutative pair is sorted and deduplicated.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct _Computation {
    pub opcode: Opcode,
    pub flags: Flags,
    pub ty: TypeId,
    pub operands: (bool, Vec<Operand>),
}

/// What this operation computes, or None where that is not only its
/// operands. Each operand is named as `stands` rewrites it.
///
/// A load is its pointer's bytes, as an old load was its cell's; a
/// volatile one is the old barrier.
pub fn _computation(op: &Instruction, stands: &BTreeMap<ValueId, Operand>) -> Option<_Computation> {
    let floating = _floating(&op.opcode);
    let load = matches!(op.opcode, Opcode::Load { volatile: false, .. });
    if !(_PURE.contains(&op.opcode.mnemonic()) || load) && !floating {
        return None;
    }
    if op.result.is_none() || op.operands.is_empty() {
        return None;
    }
    let mut named = Vec::new();
    for &one in &op.operands {
        match one {
            Operand::Value(_) => named.push(ssa::provider(one, stands).ok()?),
            Operand::Constant(_) => named.push(one),
            Operand::Block(_) => return None,
        }
    }
    let unordered = named.len() == 2
        && matches!(
            op.opcode,
            Opcode::Binary(BinaryOp::Add | BinaryOp::Mul | BinaryOp::And | BinaryOp::Or | BinaryOp::Xor)
                | Opcode::ICmp(IntPredicate::Eq | IntPredicate::Ne)
        );
    if unordered {
        // Any total order will do.
        let order = |operand: &Operand| format!("{operand:?}");
        if order(&named[0]) > order(&named[1]) {
            named.swap(0, 1);
        }
        if named[0] == named[1] {
            named.pop();
        }
    }
    Some(_Computation { opcode: op.opcode.clone(), flags: op.flags, ty: op.ty, operands: (unordered, named) })
}

/// Whether the earlier operation has certainly run by the later one.
pub fn _reaches(
    at: usize,
    where_: usize,
    then: usize,
    index: usize,
    doms: &BTreeMap<i64, BTreeSet<i64>>,
    layout: &[BlockId],
    block: BlockId,
) -> bool {
    if at == then {
        return where_ < index;
    }
    doms.get(&cfg::id(block)).is_some_and(|dominating| dominating.contains(&cfg::id(layout[at])))
}

thread_local! {
    static UNDISTURBED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times this thread has asked whether a load is undisturbed by a set of writes, for a test that a pass asks it once.
pub fn undisturbed_asked() -> usize {
    UNDISTURBED.with(std::cell::Cell::get)
}

/// Whether the load `one` still reads what the load before it read, with
/// `between` run in between: nothing there may write its bytes.
///
/// `accesses` says what each writes; a call writes its footprint.
/// `regions::overlapping` decides against each write, on `program`, and an
/// answer it cannot give overlaps.
pub fn _undisturbed(one: InstId, between: &[InstId], accesses: &Accesses, program: Option<&ProgramProxy>) -> bool {
    UNDISTURBED.with(|asked| asked.set(asked.get() + 1));
    let Some(read) = accesses.references.get(&one) else {
        return false;
    };
    between.iter().all(|&other| llrm_analysis::memoryssa::spares(accesses, program, read, other))
}

/// Store-to-load forwarding: each load a known value serves becomes that
/// value; whether any did. `accesses` are `function`'s as it stands.
///
/// `crossed` is set when a value serves a load across a store, the one thing `avoid_store_crossing` changes: where it is not set the
/// run is the same either way.
///
/// `avoid_store_crossing` keeps a value from serving a load when a store
/// lies on a path from its definition to the load.
pub fn forwarded(context: &Context, layout: &DataLayout, function: &mut Function, outer: &Outer, accesses: &Accesses, registers: &IndexMap<ValueId, llrm_analysis::consts::Known>, shape: &llrm_analysis::cfg::Shape, avoid_store_crossing: bool, held: &std::cell::OnceCell<avail::Held>, crossed: &std::cell::Cell<bool>) -> Result<bool, String> {
    let want = function.walk().map(|(_, inst)| inst).filter(|&inst| matches!(function.instruction(inst).opcode, Opcode::Load { .. })).collect::<BTreeSet<_>>();
    if want.is_empty() {
        return Ok(false);
    }
    let unit = memory::Unit::within(context, layout, function, outer).with_registers(registers).with_shape(shape);
    let crossings = Crossings::of(function);
    // What each block holds is a fact of the instructions `function` has now, which a caller that runs this twice on one function works out once.
    let served = avail::forwardable_by(&unit, accesses, &want, held.get_or_init(|| avail::holders(&unit, accesses)))
        .into_iter()
        .filter(|one| {
            let crosses = crossings.crosses(one.value, one.at);
            crossed.set(crossed.get() || crosses);
            !avoid_store_crossing || !crosses
        })
        .collect::<Vec<_>>();
    if served.is_empty() {
        return Ok(false);
    }
    // A load may be served by another served load.
    let replacements = served.iter().map(|one| (function.instruction(one.at).result.expect("a load's value"), one.value)).collect::<BTreeMap<_, _>>();
    for (&value, &with) in &replacements {
        function.replace_value(value, ssa::provider(with, &replacements).map_err(|error| error.to_string())?);
    }
    for one in served {
        function.erase(one.at)?;
    }
    Ok(true)
}

/// Whether stores lie on paths between instructions, asked many times of one function: where each instruction is and how many stores
/// each block holds before each position are found once, and the blocks that reach a block once for each block asked of. Each ask was
/// a scan of a block for the instruction, twice, and of the function for what reaches the load: quadratic in a large function.
struct Crossings<'f> {
    function: &'f Function,
    places: BTreeMap<InstId, (BlockId, i64)>,
    /// Per block, the stores among its first `n` instructions, at `n`.
    stores: BTreeMap<BlockId, Vec<u32>>,
    reaching: std::cell::RefCell<BTreeMap<BlockId, std::rc::Rc<BTreeSet<BlockId>>>>,
}

impl<'f> Crossings<'f> {
    fn of(function: &'f Function) -> Self {
        let mut places = BTreeMap::new();
        let mut stores = BTreeMap::new();
        for &block in function.layout() {
            let mut counted = vec![0u32];
            for (index, &inst) in function.block(block).instructions().iter().enumerate() {
                places.insert(inst, (block, index as i64));
                counted.push(counted[index] + u32::from(matches!(function.instruction(inst).opcode, Opcode::Store { .. })));
            }
            stores.insert(block, counted);
        }
        Self { function, places, stores, reaching: Default::default() }
    }

    /// The blocks with a path to `to`, itself included.
    fn reaching(&self, to: BlockId) -> std::rc::Rc<BTreeSet<BlockId>> {
        std::rc::Rc::clone(self.reaching.borrow_mut().entry(to).or_insert_with(|| {
            let mut reaching = BTreeSet::from([to]);
            let mut work = vec![to];
            while let Some(at) = work.pop() {
                for parent in self.function.predecessors(at) {
                    if reaching.insert(parent) {
                        work.push(parent);
                    }
                }
            }
            std::rc::Rc::new(reaching)
        }))
    }

    /// Whether a store lies on some path from `holder`'s definition to `load`, or `holder` is defined where no path from it reaches
    /// `load` first.
    fn crosses(&self, holder: Operand, load: InstId) -> bool {
        let function = self.function;
        let Operand::Value(holder) = holder else {
            return false;
        };
        let source = match function.value(holder).def {
            ValueDef::Instruction(inst) => self.places[&inst],
            _ => (function.entry().expect("a body"), -1),
        };
        let destination = self.places[&load];
        let stores_in = |block: BlockId, low: i64, high: i64| {
            let counted = &self.stores[&block];
            let (low, high) = (low.max(0) as usize, (high.max(0) as usize).min(counted.len() - 1));
            low < high && counted[high] > counted[low]
        };
        if source.0 == destination.0 {
            return source.1 >= destination.1 || stores_in(source.0, source.1 + 1, destination.1);
        }
        let reaching = self.reaching(destination.0);
        if !reaching.contains(&source.0) {
            return true;
        }
        let mut seen = BTreeSet::new();
        let mut work = vec![source.0];
        let mut arrived = false;
        while let Some(at) = work.pop() {
            if !reaching.contains(&at) || !seen.insert(at) {
                continue;
            }
            let low = if at == source.0 { source.1 + 1 } else { 0 };
            let high = if at == destination.0 { destination.1 } else { function.block(at).instructions().len() as i64 };
            if stores_in(at, low, high) {
                return true;
            }
            if at == destination.0 {
                arrived = true;
            } else {
                work.extend(function.successors(at));
            }
        }
        !arrived
    }
}

#[cfg(test)]
mod tests {
    use llrm_analysis::memory::Unit;
    use llrm_mir::datalayout::DataLayout;
    use llrm_mir::module::Module;

    use crate::testing::{f, parsed, printed, results};

    use llrm_analysis::memoryssa::Accesses;
    use llrm_mir::passes::Outer;
    use llrm_support::hash::IndexMap;

    use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
    use llrm_mir::opcode::Opcode;
    use std::collections::BTreeSet;

    use super::{Crossings, _trivial_phis, forwarded, subexpressions};

    /// `module`'s @f numbered.
    fn subexpressions_of(module: &mut Module) -> bool {
        let layout = DataLayout::default();
        let (_, _, function) = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).expect("@f");
        let accesses = Accesses::resolved(&llrm_analysis::testing::with_registers(Unit::of(module, &layout, function)), &IndexMap::default()).unwrap();
        subexpressions(f(module), &accesses, false, None, &std::cell::Cell::new(false)).unwrap()
    }

    /// `text` numbered: its printed form, and whether anything went. What
    /// `@f` returns for `inputs` is what it returned before.
    fn numbered(text: &str, inputs: &[&[i128]]) -> (String, bool) {
        let before = parsed(text);
        let mut module: Module = before.clone();
        let changed = subexpressions_of(&mut module);
        let text = printed(&module);
        assert_eq!(results(&module, inputs), results(&before, inputs), "{text}");
        (text, changed)
    }

    /// `text`, left as it was.
    fn kept(text: &str) {
        let mut module = parsed(text);
        let before = printed(&module);
        assert!(!subexpressions_of(&mut module), "{before}");
        assert_eq!(printed(&module), before);
    }

    const XY: &[&[i128]] = &[&[0, 0], &[3, 5], &[-7, 2], &[0x7fff, 1]];

    /// A store to text memory misses @g on real-mode DOS: a load of @g is
    /// reused across it where the target is asked. It was not asked.
    #[test]
    fn test_a_load_is_reused_across_a_store_to_foreign_memory() {
        let text = format!(
            "{}@g = global i16 0

define i16 @f() {{
b0:
  %a = load i16, ptr @g
  %s = inttoptr i16 -18432 to ptr addrspace(2)
  %far = addrspacecast ptr addrspace(2) %s to ptr addrspace(1)
  store i16 1, ptr addrspace(1) %far
  %b = load i16, ptr @g
  %r = add i16 %a, %b
  ret i16 %r
}}
",
            llrm_analysis::testing::DOS
        );
        let reused = |dos: bool| {
            let mut module = parsed(&text);
            let layout = llrm_analysis::testing::layout(&module);
            let program = llrm_mir::program::ProgramProxy::of(&module, std::rc::Rc::new(llrm_x86_m16::Dos::default()));
            let program = dos.then_some(&*program);
            let (_, _, function) = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).expect("@f");
            let accesses = Accesses::resolved(&Unit { program, ..llrm_analysis::testing::with_registers(Unit::of(&module, &layout, function)) }, &IndexMap::default()).unwrap();
            subexpressions(f(&mut module), &accesses, false, program, &std::cell::Cell::new(false)).unwrap()
        };
        assert!(reused(true) && !reused(false));
    }


    /// `text`'s @f forwarded, printed; what it returns for `XY` stays.
    fn forwarded_of(text: &str, avoid_store_crossing: bool) -> String {
        let before = parsed(text);
        let mut module = before.clone();
        let (layout, outer) = (llrm_analysis::testing::layout(&module), Outer::of(&module, None));
        let accesses = {
            let (_, _, function) = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).expect("@f");
            Accesses::resolved(&llrm_analysis::testing::with_registers(Unit::within(&module.context, &layout, function, &outer)), &IndexMap::default()).unwrap()
        };
        let (context, function) = module.function_mut("f").expect("@f");
        let registers = llrm_analysis::consts::known(&llrm_analysis::testing::with_registers(Unit::within(context, &layout, function, &outer)), None, None, None);
        let shape = llrm_analysis::cfg::Shape::of(function);
        let changed = forwarded(context, &layout, function, &outer, &accesses, &registers, &shape, avoid_store_crossing, &std::cell::OnceCell::new(), &std::cell::Cell::new(false)).unwrap();
        let text = printed(&module);
        assert_eq!(changed, text != printed(&before), "{text}");
        assert_eq!(results(&module, XY), results(&before, XY), "{text}");
        text
    }

    /// The stored value serves the load of its cell; the load of the cell
    /// 8 bytes on stays.
    #[test]
    fn test_forwarding_extends_lifetime_without_conflating_shared_addresses() {
        let text = forwarded_of(
            "@g = global [8 x i16] zeroinitializer

define i16 @f(i16 %x, i16 %y) {
b0:
  store i16 %x, ptr @g
  %a = load i16, ptr @g
  %o = getelementptr i8, ptr @g, i16 8
  %b = load i16, ptr %o
  %r = add i16 %a, %b
  ret i16 %r
}
",
            false,
        );
        assert!(text.contains("  store i16 %x, ptr @g
  %o = getelementptr i8, ptr @g, i16 8
  %b = load i16, ptr %o
  %r = add i16 %x, %b
"), "{text}");
    }

    const CROSSING: &str = "@g = global [8 x i16] zeroinitializer

define i16 @f(i16 %x, i16 %y) {
b0:
  store i16 %x, ptr @g
  %o = getelementptr i8, ptr @g, i16 8
  STORE
  %a = load i16, ptr @g
  ret i16 %a
}
";

    /// A store to another cell leaves the value held; `avoid_store_crossing`
    /// declines to hold it across one.
    #[test]
    fn test_a_value_crosses_a_store_only_when_allowed() {
        for store in ["store i16 %y, ptr %o", "store volatile i16 %y, ptr %o"] {
            let text = CROSSING.replace("STORE", store);
            assert!(forwarded_of(&text, false).contains("  ret i16 %x
"), "{store}");
            assert_eq!(forwarded_of(&text, true), printed(&parsed(&text)), "{store}");
        }
    }

    /// What may write the cell between stops the value.
    #[test]
    fn test_a_value_is_not_forwarded_past_what_may_write_its_cell() {
        for between in ["%v = getelementptr i8, ptr @g, i16 %y\n  store i16 %y, ptr %v", "store i8 1, ptr @g", "call void @h()", "store volatile i8 1, ptr @g"] {
            let text = format!("{}\ndefine void @h() {{\nb0:\n  store i16 3, ptr @g\n  ret void\n}}\n", CROSSING.replace("STORE", between));
            assert_eq!(forwarded_of(&text, false), printed(&parsed(&text)), "{between}");
        }
    }

    #[test]
    fn test_a_repeated_expression_is_computed_once() {
        let (text, changed) = numbered(
            "define i16 @f(i16 %x, i16 %y) {
b0:
  %a = mul i16 %x, %y
  %b = mul i16 %x, %y
  %r = sub i16 %a, %b
  ret i16 %r
}
",
            XY,
        );
        assert!(changed);
        assert_eq!(
            text,
            "define i16 @f(i16 %x, i16 %y) {
b0:
  %a = mul i16 %x, %y
  %r = sub i16 %a, %a
  ret i16 %r
}
"
        );
    }

    #[test]
    fn test_operands_in_either_order_are_one_commutative_expression() {
        for (op, ty) in [("add", "i16"), ("xor", "i16"), ("icmp eq", "i1"), ("icmp ne", "i1")] {
            let text = format!(
                "define i32 @f(i16 %x, i16 %y) {{
b0:
  %a = {op} i16 %x, %y
  %b = {op} i16 %y, %x
  %s = zext {ty} %a to i32
  %t = zext {ty} %b to i32
  %r = add i32 %s, %t
  ret i32 %r
}}
"
            );
            let (text, changed) = numbered(&text, XY);
            assert!(changed, "{op}");
            // and so are the two widenings
            assert!(text.contains("%r = add i32 %s, %s"), "{text}");
        }
    }

    #[test]
    fn test_an_ordered_expression_with_its_operands_swapped_stays() {
        for (op, ty) in [("sub", "i16"), ("shl", "i16"), ("icmp slt", "i1")] {
            kept(&format!(
                "define i32 @f(i16 %x, i16 %y) {{
b0:
  %a = {op} i16 %x, %y
  %b = {op} i16 %y, %x
  %s = zext {ty} %a to i32
  %t = zext {ty} %b to i32
  %r = add i32 %s, %t
  ret i32 %r
}}
"
            ));
        }
    }

    /// Same opcode, but one operand, the flags or the type differs.
    #[test]
    fn test_expressions_differing_in_an_operand_flags_or_type_stay() {
        for (first, second) in [
            ("%a = add i16 %x, 1", "%b = add i16 %x, 2"),
            ("%a = add nsw i16 %x, 1", "%b = add i16 %x, 1"),
            ("%a = udiv exact i16 %x, 2", "%b = udiv i16 %x, 2"),
            ("%a = sext i8 %n to i16", "%b = zext i8 %n to i16"),
        ] {
            kept(&format!(
                "define i16 @f(i16 %x, i8 %n) {{
b0:
  {first}
  {second}
  %r = add i16 %a, %b
  ret i16 %r
}}
"
            ));
        }
        kept(
            "define i32 @f(i8 %n) {
b0:
  %a = zext i8 %n to i16
  %b = zext i8 %n to i32
  %w = zext i16 %a to i32
  %r = add i32 %w, %b
  ret i32 %r
}
",
        );
    }

    #[test]
    fn test_a_dominated_repeat_reads_the_dominating_value() {
        let (text, changed) = numbered(
            "define i16 @f(i16 %x, i16 %y) {
b0:
  %a = and i16 %x, %y
  %c = icmp eq i16 %x, 0
  br i1 %c, label %b1, label %b2

b1:
  %b = and i16 %y, %x
  ret i16 %b

b2:
  ret i16 %a
}
",
            XY,
        );
        assert!(changed);
        assert!(text.contains("b1:\n  ret i16 %a\n"), "{text}");
    }

    /// Neither arm runs before the other.
    #[test]
    fn test_a_repeat_in_a_sibling_block_is_kept() {
        kept(
            "define i16 @f(i16 %x, i16 %y) {
b0:
  %c = icmp eq i16 %x, 0
  br i1 %c, label %b1, label %b2

b1:
  %a = or i16 %x, %y
  ret i16 %a

b2:
  %b = or i16 %x, %y
  ret i16 %b
}
",
        );
    }

    /// matrix printed T=0 for T=380 after CSE deleted a zero still named by
    /// its loop phi.
    #[test]
    fn test_cse_replaces_phi_uses_of_a_deleted_initializer() {
        let (text, changed) = numbered(
            "define i16 @f(i16 %x, i16 %y) {
b0:
  %first = add i16 %x, 7
  br label %b1

b1:
  %second = add i16 %x, 7
  br label %b2

b2:
  %result = phi i16 [ %second, %b1 ]
  %r = add i16 %result, %second
  ret i16 %r
}
",
            XY,
        );
        assert!(changed);
        assert!(!text.contains("%second"), "{text}");
        assert!(text.contains("%result = phi i16 [ %first, %b1 ]\n  %r = add i16 %result, %first"), "{text}");
    }

    /// Descriptor addresses encoded as zero must not become the same value.
    #[test]
    fn test_cse_keeps_distinct_linker_addresses() {
        for other in ["@b", "null"] {
            kept(&format!(
                "@a = global i16 0
@b = global i16 0

define i16 @f() {{
b0:
  %first = ptrtoint ptr @a to i16
  %second = ptrtoint ptr {other} to i16
  %r = add i16 %first, %second
  ret i16 %r
}}
"
            ));
        }
    }

    #[test]
    fn test_cse_reuses_one_frame_object_address() {
        let (text, changed) = numbered(
            "define i16 @f(i16 %x, i16 %y) {
b0:
  %frame = alloca [66 x i16]
  %first = getelementptr i8, ptr %frame, i16 4
  %duplicate = getelementptr i8, ptr %frame, i16 4
  store i16 %x, ptr %first
  %r = load i16, ptr %duplicate
  ret i16 %r
}
",
            XY,
        );
        assert!(changed);
        assert!(!text.contains("%duplicate"), "{text}");
        assert!(text.contains("%r = load i16, ptr %first"), "{text}");
    }

    /// An address took its cell for a read, so no two addresses of one
    /// descriptor were ever one value and no load through them was shared.
    #[test]
    fn test_cse_reuses_one_cell_address() {
        let (text, changed) = numbered(
            "@descriptor = global [20 x i8] zeroinitializer

define i16 @f(i16 %x, i16 %y) {
b0:
  %first = getelementptr i8, ptr @descriptor, i16 18
  %duplicate = getelementptr i8, ptr @descriptor, i16 18
  store i16 %x, ptr %first
  %r = load i16, ptr %duplicate
  ret i16 %r
}
",
            XY,
        );
        assert!(changed);
        assert!(!text.contains("%duplicate"), "{text}");
    }

    /// lngmix under SROA refused 0x0071: "mov ... defines [v24_1] through no
    /// operand": the third divide was served the second's quotient, which
    /// the second no longer computed.
    #[test]
    fn test_a_third_equal_divide_reads_the_answer_the_first_computed() {
        let (text, changed) = numbered(
            "define i16 @f(i16 %x, i16 %y) {
b0:
  %q0 = sdiv i16 %x, %y
  %r0 = srem i16 %x, %y
  %q1 = sdiv i16 %x, %y
  %r1 = srem i16 %x, %y
  %q2 = sdiv i16 %x, %y
  %r2 = srem i16 %x, %y
  %a = add i16 %r1, %q2
  %b = add i16 %a, %q0
  ret i16 %b
}
",
            &[&[7, 2], &[-9, 4], &[100, -3]],
        );
        assert!(changed);
        assert!(text.contains("  %a = add i16 %r0, %q0\n  %b = add i16 %a, %q0\n"), "{text}");
        assert_eq!(text.matches("sdiv").count() + text.matches("srem").count(), 2, "{text}");
    }

    /// `%a` and `%b` load `%p` with `between` in between, in @f of `head`,
    /// which starts with `pointers`.
    fn loads(head: &str, pointers: &str, between: &str) -> String {
        format!(
            "{head}
define i16 @f(i16 %x, i16 %y{pointers}) {{
b0:
  %a = load i16, ptr %p
{between}  %b = load i16, ptr %p
  %r = add i16 %a, %b
  ret i16 %r
}}
"
        )
    }

    const LOCAL: &str = "  %p = alloca [2 x i16]\n  %q = alloca i16\n  store i16 %x, ptr %p\n";

    fn reused(text: String) {
        let (text, changed) = numbered(&text.replace("i16 %y) {\nb0:\n", &format!("i16 %y) {{\nb0:\n{LOCAL}")), XY);
        assert!(changed, "{text}");
        assert!(text.contains("%r = add i16 %a, %a"), "{text}");
    }

    #[test]
    fn test_a_load_is_reused_across_what_touches_no_memory() {
        reused(loads("", "", "  %n = add i16 %x, 1\n"));
    }

    #[test]
    fn test_a_load_is_reused_across_a_store_to_another_offset() {
        reused(loads("", "", "  %s = getelementptr i8, ptr %p, i16 2\n  store i16 %y, ptr %s\n"));
    }

    #[test]
    fn test_a_load_is_reused_across_a_store_to_another_alloca() {
        reused(loads("", "", "  store i16 %y, ptr %q\n"));
    }

    /// A volatile access touches only its own bytes: one of another alloca
    /// leaves the first load's value. Both were a barrier (#257).
    #[test]
    fn test_a_load_is_reused_across_a_volatile_access_of_another_alloca() {
        reused(loads("", "", "  store volatile i16 %y, ptr %q\n"));
        reused(loads("", "", "  %v = load volatile i16, ptr %q\n"));
    }

    /// Two noalias pointers' objects are apart; alias proves nothing of one
    /// noalias pointer against a plain one.
    #[test]
    fn test_a_load_through_a_noalias_pointer_is_reused_across_a_store_through_another() {
        let mut module = parsed(&loads("", "", "  store i16 %y, ptr %q\n").replace("i16 %y) {", "i16 %y, ptr noalias %p, ptr noalias %q) {"));
        assert!(subexpressions_of(&mut module), "{}", printed(&module));
        assert!(printed(&module).contains("%r = add i16 %a, %a"));
    }

    /// Neither alloca escaped, so the callee cannot reach them.
    #[test]
    fn test_a_load_is_reused_across_a_call_that_cannot_reach_it() {
        reused(loads("define void @g(ptr %s) {\nb0:\n  store i16 9, ptr %s\n  ret void\n}\n", "", "  call void @g(ptr %q)\n"));
    }

    #[test]
    fn test_a_load_is_refused_across_what_may_write_it() {
        let pointers = |between: &str| loads("declare void @g(ptr)\n", ", ptr %p, ptr %q", between);
        for between in ["  store i16 %y, ptr %q\n", "  call void @g(ptr %q)\n", "  call void @g(ptr %p)\n", "  store volatile i16 %y, ptr %q\n"] {
            kept(&pointers(between));
        }
        kept(&pointers("").replace("%b = load i16", "%b = load volatile i16"));
    }

    /// The first load has not certainly run, nor did the old one look.
    #[test]
    fn test_a_load_in_another_block_is_loaded_again() {
        kept(
            "define i16 @f(ptr %p) {
b0:
  %a = load i16, ptr %p
  br label %b1

b1:
  %b = load i16, ptr %p
  %r = add i16 %a, %b
  ret i16 %r
}
",
        );
    }

    #[test]
    fn test_floating_arithmetic_is_one_expression() {
        let (text, changed) = numbered(
            "define i16 @f(i16 %x, i16 %y) {
b0:
  %u = sitofp i16 %x to double
  %v = sitofp i16 %x to double
  %a = fdiv double %u, 3.0
  %b = fdiv double %v, 3.0
  %s = fadd double %a, %b
  %r = fptosi double %s to i16
  ret i16 %r
}
",
            XY,
        );
        assert!(changed);
        assert!(text.contains("%s = fadd double %a, %a"), "{text}");
    }

    /// A dead arm and the block it came from go; the join it leaves with one
    /// value is that value.
    #[test]
    fn test_a_dead_arm_leaves_a_join_that_is_its_one_value() {
        let mut module = parsed(
            "define i16 @f(i16 %x) {
b0:
  br label %b2

b1:
  %y = add i16 %x, 1
  br label %b2

b2:
  %r = phi i16 [ %x, %b0 ], [ %y, %b1 ]
  %s = add i16 %r, 2
  ret i16 %s
}
",
        );
        let mut function = module.function_mut("f").unwrap().1.clone();
        llrm_analysis::cfg::_unreachable(&mut module.context, &mut function);
        _trivial_phis(&mut function).unwrap();
        *module.function_mut("f").unwrap().1 = function;
        assert_eq!(
            printed(&module),
            "define i16 @f(i16 %x) {
b0:
  br label %b2

b2:
  %s = add i16 %x, 2
  ret i16 %s
}
"
        );
    }

    /// A phi that reads only itself and one other value is that value.
    #[test]
    fn test_a_loop_phi_of_one_outside_value_is_that_value() {
        let mut module = parsed(
            "define i16 @f(i16 %x, i1 %c) {
b0:
  br label %b1

b1:
  %r = phi i16 [ %x, %b0 ], [ %r, %b1 ]
  br i1 %c, label %b1, label %b2

b2:
  ret i16 %r
}
",
        );
        _trivial_phis(module.function_mut("f").unwrap().1).unwrap();
        assert_eq!(
            printed(&module),
            "define i16 @f(i16 %x, i1 %c) {
b0:
  br label %b1

b1:
  br i1 %c, label %b1, label %b2

b2:
  ret i16 %x
}
"
        );
    }

    /// SROA built a loop phi per slice beside promote's identical ones,
    /// and nbody carried twelve i32 values twice.
    #[test]
    fn a_phi_with_an_earlier_phis_arms_is_that_phi() {
        let mut module = parsed(
            "define i16 @f(i16 %x, i1 %c) {
b0:
  br label %b1

b1:
  %r = phi i16 [ %x, %b0 ], [ %t, %b1 ]
  %s = phi i16 [ %t, %b1 ], [ %x, %b0 ]
  %t = add i16 %r, 1
  %u = add i16 %s, %t
  br i1 %c, label %b1, label %b2

b2:
  ret i16 %u
}
",
        );
        _trivial_phis(module.function_mut("f").unwrap().1).unwrap();
        let after = printed(&module);
        assert!(after.matches("phi").count() == 1 && after.contains("add i16 %r, %t"), "{after}");
    }
    /// The scan the pass made for every served load (a block searched for the instruction twice, the function for what reaches the load),
    /// kept here as the reference: `Crossings` answers the same for every definition and load of a function with a diamond, a loop and
    /// stores in every block, and finds each position once.
    fn reference_crosses_store(function: &Function, holder: Operand, load: InstId) -> bool {
        let Operand::Value(holder) = holder else {
            return false;
        };
        let place = |inst: InstId| {
            let block = function.parent(inst).expect("placed");
            (block, function.block(block).instructions().iter().position(|&one| one == inst).expect("in its block") as i64)
        };
        let source = match function.value(holder).def {
            ValueDef::Instruction(inst) => place(inst),
            _ => (function.entry().expect("a body"), -1),
        };
        let destination = place(load);
        let stores_in = |block: BlockId, low: i64, high: i64| {
            let instructions = function.block(block).instructions();
            let range = low.max(0) as usize..(high.max(0) as usize).min(instructions.len());
            instructions.get(range).is_some_and(|run| run.iter().any(|&one| matches!(function.instruction(one).opcode, Opcode::Store { .. })))
        };
        if source.0 == destination.0 {
            return source.1 >= destination.1 || stores_in(source.0, source.1 + 1, destination.1);
        }
        let mut reaching = BTreeSet::from([destination.0]);
        let mut work = vec![destination.0];
        while let Some(at) = work.pop() {
            for parent in function.predecessors(at) {
                if reaching.insert(parent) {
                    work.push(parent);
                }
            }
        }
        if !reaching.contains(&source.0) {
            return true;
        }
        let mut seen = BTreeSet::new();
        let mut work = vec![source.0];
        let mut arrived = false;
        while let Some(at) = work.pop() {
            if !reaching.contains(&at) || !seen.insert(at) {
                continue;
            }
            let low = if at == source.0 { source.1 + 1 } else { 0 };
            let high = if at == destination.0 { destination.1 } else { function.block(at).instructions().len() as i64 };
            if stores_in(at, low, high) {
                return true;
            }
            if at == destination.0 {
                arrived = true;
            } else {
                work.extend(function.successors(at));
            }
        }
        !arrived
    }

    #[test]
    fn test_crossings_answer_as_the_scan_for_each_load_did() {
        let mut module = parsed(
            "@g = global i16 0
@h = global i16 0

define i16 @f(i16 %x, i1 %c, i16 %n) {
b0:
  %a = load i16, ptr @g
  store i16 %x, ptr @h
  br i1 %c, label %b1, label %b2

b1:
  %b = load i16, ptr @g
  store i16 %b, ptr @g
  br label %b3

b2:
  %d = load i16, ptr @h
  br label %b3

b3:
  %i = phi i16 [ 0, %b1 ], [ 0, %b2 ], [ %i2, %b3 ]
  %e = load i16, ptr @g
  %i2 = add i16 %i, 1
  %m = icmp slt i16 %i2, %n
  store i16 %e, ptr @h
  br i1 %m, label %b3, label %b4

b4:
  %r = load i16, ptr @h
  ret i16 %r
}
",
        );
        let function = f(&mut module);
        let crossings = Crossings::of(function);
        let values: Vec<_> = function.walk().filter_map(|(_, inst)| function.instruction(inst).result).collect();
        let loads: Vec<_> = function.walk().map(|(_, inst)| inst).filter(|&inst| matches!(function.instruction(inst).opcode, llrm_mir::opcode::Opcode::Load { .. })).collect();
        assert!(loads.len() >= 5 && values.len() >= 8);
        for &holder in &values {
            for &load in &loads {
                let operand = llrm_mir::module::Operand::Value(holder);
                assert_eq!(crossings.crosses(operand, load), reference_crosses_store(function, operand, load), "{holder:?} to {load:?}");
            }
        }
    }

}
