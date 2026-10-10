//! What values past the target's registers cost: the one spill model every
//! pricing of register pressure shares.
//!
//! A value kept in memory costs its traffic there: a store after each
//! definition, a load at each read, and a memory update each time a counter
//! is stepped in place; a value its one instruction rebuilds for less than a
//! load costs that at each read instead. A counter and its step share one
//! cell, as a copy-joined web does, and a phi's own edges into that cell
//! move nothing. Where more values live than the target's registers hold,
//! or than a call leaves across it, the cheapest are spilled, and stay
//! spilled.

use std::collections::{BTreeMap, BTreeSet};

use llrm_analysis::cfg;
use llrm_analysis::liveness::{self, LivePoint, Liveness};
use llrm_mir::context::{ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::{Dense, IdMap, IdSet};
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{BinaryOp, CastOp, Opcode};
use llrm_mir::passes::Outer;
use llrm_mir::target::OperationCosts;
use llrm_mir::types::Type;

/// How many integer values the target holds in registers, and how many of
/// them a call leaves; none leaves pressure unpriced.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Room {
    pub registers: i64,
    pub across_call: i64,
    /// What an access through a far pointer takes besides its address: its
    /// selector.
    pub far_access: i64,
    /// The segment registers a far pointer's selector is held in.
    pub segments: i64,
    /// The registers an address's pointer and index must be held in.
    pub addresses: i64,
    /// Whether a result is made in its first operand's register.
    pub two_address: bool,
    /// The target's address spaces by role.
    pub spaces: llrm_mir::spaces::Spaces,
    /// The scales (bit `k` for `2^k`) an address takes beside any registers:
    /// `[ebp+esi*8+d]` folds `i * 8`.
    pub index_scales: u8,
}

impl Room {
    pub fn of(outer: &Outer) -> Room {
        Room {
            registers: outer.target().registers(),
            across_call: outer.target().call_registers(),
            far_access: outer.target().far_access_registers(),
            segments: outer.target().segment_registers(),
            addresses: outer.target().address_registers(),
            two_address: outer.target().two_address(),
            spaces: outer.target().spaces(),
            index_scales: index_scales(&outer.target().address_forms()),
        }
    }

    pub fn priced(&self) -> bool {
        self.registers > 0
    }
}

/// The scales the native form takes, where any register may be its base and its
/// index (the others cost a prefix and bytes), as `Room::index_scales` has
/// them.
fn index_scales(forms: &[llrm_mir::target::AddressForm]) -> u8 {
    forms
        .iter()
        .take(1)
        .filter(|form| form.bases.is_none() && form.indices.is_none())
        .flat_map(|form| form.scales.iter())
        .filter_map(|&scale| [1, 2, 4, 8].iter().position(|&one| one == scale))
        .fold(0, |bits, at| bits | 1 << at)
}

/// A value's traffic in memory, each count weighted by its block's frequency.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Traffic {
    pub stores: i64,
    pub updates: i64,
    pub loads: i64,
    /// What rebuilding it costs at a read, where one instruction makes it.
    pub rebuild: Option<i64>,
}

impl Traffic {
    pub fn price(
        &self,
        costs: &OperationCosts,
    ) -> i64 {
        let cell = self.stores * costs.store + self.updates * costs.memory_update + self.loads * costs.load;
        self.rebuild.map_or(cell, |each| cell.min(self.loads * each))
    }
}

/// Where too many values may live: the registers there, and who lives.
#[derive(Clone, Debug)]
pub struct Point<K> {
    pub registers: i64,
    pub residents: Vec<K>,
}

/// What fitting `points` costs and spills.
pub struct Forecast<S> {
    pub cost: i64,
    pub spilled: S,
    /// The most residents past a point's registers, before any spill.
    pub peak: i64,
}

/// The cells spilled so far.
pub trait Spilled<K>: Default {
    fn has(
        &self,
        cell: &K,
    ) -> bool;
    fn add(
        &mut self,
        cell: K,
    );
}

impl<K: Ord> Spilled<K> for BTreeSet<K> {
    fn has(
        &self,
        cell: &K,
    ) -> bool {
        self.contains(cell)
    }
    fn add(
        &mut self,
        cell: K,
    ) {
        self.insert(cell);
    }
}

impl<K: Eq + std::hash::Hash> Spilled<K> for llrm_support::hash::HashSet<K> {
    fn has(
        &self,
        cell: &K,
    ) -> bool {
        self.contains(cell)
    }
    fn add(
        &mut self,
        cell: K,
    ) {
        self.insert(cell);
    }
}

impl<K: Dense> Spilled<K> for IdSet<K> {
    fn has(
        &self,
        cell: &K,
    ) -> bool {
        self.contains(cell)
    }
    fn add(
        &mut self,
        cell: K,
    ) {
        self.insert(cell);
    }
}

/// What spilling costs to fit `points`, in order: at each, the cheapest
/// residents past its registers are spilled, and stay spilled.
pub fn spilled<K: Ord + std::hash::Hash + Copy>(
    points: impl IntoIterator<Item = Point<K>>,
    price: impl Fn(K) -> i64,
) -> i64 {
    fitted::<K, llrm_support::hash::HashSet<K>>(points, price).cost
}

/// `spilled`, remembering the spilled in a set of the caller's.
pub fn spilled_in<K: Ord + Copy, S: Spilled<K>>(
    points: impl IntoIterator<Item = Point<K>>,
    price: impl Fn(K) -> i64,
) -> i64 {
    fitted::<K, S>(points, price).cost
}

/// `spilled`, and which cells it spills and how far past its registers the
/// pressure goes.
pub fn forecast<K: Ord + Dense>(
    points: impl IntoIterator<Item = Point<K>>,
    price: impl Fn(K) -> i64,
) -> Forecast<IdSet<K>> {
    fitted(points, price)
}

// Cells handled by `fitted` and by the sweep: a test that the sweep's work
// follows what changes and not what is live.
#[cfg(test)]
thread_local! {
    pub(crate) static TOUCHED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn touched(_cells: usize) {
    #[cfg(test)]
    TOUCHED.with(|touched| touched.set(touched.get() + _cells));
}

fn fitted<K: Ord + Copy, S: Spilled<K>>(
    points: impl IntoIterator<Item = Point<K>>,
    price: impl Fn(K) -> i64,
) -> Forecast<S> {
    let mut spilled = S::default();
    let (mut cost, mut peak) = (0, 0);
    for point in points {
        touched(point.residents.len());
        // Sorted and without repeats, as a set would hold them: a set built per
        // point was most of lsr's work on a loop of many uses.
        let mut resident = point.residents.into_iter().filter(|one| !spilled.has(one)).collect::<Vec<_>>();
        resident.sort_unstable();
        resident.dedup();
        let excess = resident.len() as i64 - point.registers.max(0);
        peak = peak.max(excess);
        if excess <= 0 {
            continue;
        }
        // The `excess` cheapest, dearer ties to the larger resident: a partial
        // sort picks the same ones.
        let take = excess as usize;
        let mut cheapest = resident.into_iter().map(|one| (price(one), one)).collect::<Vec<_>>();
        if take < cheapest.len() {
            cheapest.select_nth_unstable(take);
            cheapest.truncate(take);
        }
        for (each, one) in cheapest {
            cost += each;
            spilled.add(one);
        }
    }
    Forecast { cost, spilled, peak }
}

/// Whether `inst` leaves only the registers a call does.
pub fn calls(
    function: &Function,
    inst: InstId,
) -> bool {
    matches!(
        function.instruction(inst).opcode,
        Opcode::Call(_) | Opcode::Invoke(_)
    )
}

/// How many registers call `inst` keeps: its callee's own count where the
/// call is direct.
pub fn kept_across(
    outer: &Outer,
    context: &Context,
    function: &Function,
    inst: InstId,
) -> i64 {
    outer.kept_across(llrm_mir::memory::callee(context, function, inst))
}

/// The registers `inst` needs for its address beyond the values `live`
/// before it. An address `getelementptr` only accesses take is folded into
/// them, so what it is made of is read into registers at the access: the
/// runtime pointer it is an offset from and each variable index, those not
/// live already. Any other address is a value, live, in a register. A
/// symbol or frame object is a displacement. A far pointer takes its
/// selector besides.
pub fn transient(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    inst: InstId,
    room: Room,
    live: &BTreeSet<ValueId>,
) -> i64 {
    transient_by(context, layout, function, inst, room, live, &mut |value| {
        folded_in(context, function, value, room.index_scales)
    })
}

/// `transient`, asking `is_folded` whether a pointer is folded, which a caller
/// that asks of every access can answer from what it has found.
fn transient_by(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    inst: InstId,
    room: Room,
    live: &BTreeSet<ValueId>,
    is_folded: &mut dyn FnMut(ValueId) -> bool,
) -> i64 {
    let op = function.instruction(inst);
    let address = match op.opcode {
        Opcode::Load { .. } => op.operands.first(),
        Opcode::Store { .. } => op.operands.get(1),
        _ => None,
    };
    let Some(Operand::Value(pointer)) = address else { return 0 };
    let read = address_values_by(function, *pointer, is_folded);
    read.iter().filter(|value| !live.contains(value)).count() as i64
        + if words(context, layout, function, *pointer) > 1 { room.far_access } else { 0 }
}

/// The register a result takes besides those live before it: where it is made
/// in its first operand's, an operand that stays live must first be copied, and
/// the copy lives with the second operand, which the operation reads. `sub cx,
/// di` after `mov cx, dx` holds both `dx` and `cx`: a 7th value where six
/// registers held six. A commutative operation takes either operand's; one of
/// them dying is enough. A shift is two-address too: `sar r, imm` makes its
/// result in the first operand's register.
pub fn copied(
    function: &Function,
    inst: InstId,
    past: &BTreeSet<ValueId>,
    room: Room,
    counted: &dyn Fn(ValueId) -> bool,
) -> i64 {
    let op = function.instruction(inst);
    let (Opcode::Binary(kind), Some(result), [first, second]) = (&op.opcode, op.result, &op.operands[..]) else {
        return 0;
    };
    if !room.two_address || !counted(result) {
        return 0;
    }
    let stays = |operand: &Operand| matches!(operand, Operand::Value(value) if past.contains(value));
    let tied = if matches!(
        kind,
        BinaryOp::Add | BinaryOp::Mul | BinaryOp::And | BinaryOp::Or | BinaryOp::Xor
    ) {
        !(matches!(first, Operand::Value(_)) && !stays(first))
            && !(matches!(second, Operand::Value(_)) && !stays(second))
    } else {
        stays(first)
    };
    i64::from(tied)
}

/// The values an access takes its address from: its pointer, or where the
/// access alone takes the `getelementptr` that makes it, what that is made
/// of: the pointer it is an offset from and each variable index.
pub fn addressed(function: &Function) -> BTreeSet<ValueId> {
    addressed_by(function, &mut |value| folded(function, value)).0
}

/// `addressed` where an address takes `scales`.
pub fn addressed_in(
    context: &Context,
    function: &Function,
    scales: u8,
) -> BTreeSet<ValueId> {
    addressed_by(function, &mut |value| folded_in(context, function, value, scales)).0
}

/// What `is_folded` said of each value it was asked about: the asked, and those
/// it said yes to.
#[derive(Clone, Debug, Default, PartialEq)]
struct Folds {
    asked: IdSet<ValueId>,
    yes: IdSet<ValueId>,
}

fn addressed_by(
    function: &Function,
    is_folded: &mut dyn FnMut(ValueId) -> bool,
) -> (BTreeSet<ValueId>, Folds) {
    let mut found = BTreeSet::new();
    // Whether a pointer is folded depends on all its users, and a frame slot
    // has one user for each access to it: asked once for each pointer, not
    // once for each access.
    let mut memo = llrm_mir::dense::IdMap::<ValueId, bool>::new();
    for &block in function.layout() {
        for &inst in function.block(block).instructions() {
            let op = function.instruction(inst);
            let address = match op.opcode {
                Opcode::Load { .. } => op.operands.first(),
                Opcode::Store { .. } => op.operands.get(1),
                _ => None,
            };
            if let Some(Operand::Value(pointer)) = address {
                found.extend(address_values_by(function, *pointer, &mut |value| {
                    *memo.get_or_insert_with(value, || is_folded(value))
                }));
            }
        }
    }
    let mut folds = Folds::default();
    for (value, &yes) in memo.iter() {
        folds.asked.insert(value);
        if yes {
            folds.yes.insert(value);
        }
    }
    (found, folds)
}

/// What an access through `pointer` reads its address from. Where `pointer`
/// is made by `getelementptr`s only accesses and each other take, what they
/// are made of: each variable index and the pointer they are offsets from,
/// unless that is a symbol or frame object, a displacement. Any other
/// `pointer` is itself a value.
pub fn address_values(
    function: &Function,
    pointer: ValueId,
) -> Vec<ValueId> {
    address_values_by(function, pointer, &mut |value| folded(function, value))
}

/// `address_values` where an address takes `scales` (`Room::index_scales`).
pub fn address_values_in(
    context: &Context,
    function: &Function,
    pointer: ValueId,
    scales: u8,
) -> Vec<ValueId> {
    address_values_by(function, pointer, &mut |value| folded_in(context, function, value, scales))
}

/// `address_values`, asking `folded` of each value through `is_folded`, which a
/// caller that asks of many pointers can answer from what it has found.
fn address_values_by(
    function: &Function,
    pointer: ValueId,
    is_folded: &mut dyn FnMut(ValueId) -> bool,
) -> Vec<ValueId> {
    let defined = |operand: Operand| match operand {
        Operand::Value(value) => match function.value(value).def {
            ValueDef::Instruction(def) => Some(function.instruction(def)),
            _ => None,
        },
        _ => None,
    };
    let mut read = Vec::new();
    let mut base = Operand::Value(pointer);
    while let Operand::Value(value) = base
        && is_folded(value)
        && let Some(op) = defined(base)
        && let (Opcode::GetElementPtr { .. }, [from, indices @ ..]) = (&op.opcode, &op.operands[..])
    {
        read.extend(
            indices.iter().filter_map(|index| if let Operand::Value(value) = index { Some(*value) } else { None }),
        );
        base = *from;
    }
    // A constant displacement from a symbol or frame object needs none.
    let symbolic = loop {
        match base {
            Operand::Value(_) => match defined(base).map(|op| (&op.opcode, &op.operands[..])) {
                Some((Opcode::GetElementPtr { .. }, [from, indices @ ..]))
                    if indices.iter().all(|index| matches!(index, Operand::Constant(_))) =>
                {
                    base = *from
                }
                Some((Opcode::Cast(CastOp::AddrSpaceCast), [from])) => base = *from,
                Some((Opcode::Alloca { .. }, _)) => break true,
                _ => break false,
            },
            _ => break true,
        }
    };
    if let (false, Operand::Value(runtime)) = (symbolic, base) {
        read.push(runtime);
    }
    read
}

/// Whether `value` is a far pointer of offset zero, a selector cast to the
/// far space: held in a segment register, where the target has one.
pub fn segment_view(
    context: &Context,
    layout: &DataLayout,
    spaces: llrm_mir::spaces::Spaces,
    function: &Function,
    value: ValueId,
) -> bool {
    let ValueDef::Instruction(def) = function.value(value).def else { return false };
    let op = function.instruction(def);
    let (Opcode::Cast(CastOp::AddrSpaceCast), [from]) = (&op.opcode, &op.operands[..]) else { return false };
    let space = |operand: Operand| {
        function
            .operand_type(context, operand)
            .and_then(
                |ty| match context.types.get(ty) {
                    Type::Pointer(space) => Some(*space),
                    _ => None,
                },
            )
    };
    let far = |space: u32| layout.pointer(space).bits > layout.pointer(space).index_bits;
    matches!(
        (space(Operand::Value(value)), space(*from)),
        (Some(to), Some(from)) if far(to) && from != spaces.near && !far(from)
    )
}

/// Whether `value` takes an integer register: floating values do not, nor
/// does a truth value only its own block's branch reads, which is flags.
pub fn integer(
    context: &Context,
    function: &Function,
    value: ValueId,
) -> bool {
    integer_in(context, function, value, 0)
}

/// `integer` where an address takes `scales`.
pub fn integer_in(
    context: &Context,
    function: &Function,
    value: ValueId,
    scales: u8,
) -> bool {
    match context.types.get(function.value(value).ty) {
        Type::Int(1) => !_flags(function, value),
        Type::Int(_) | Type::Pointer(_) => !folded_in(context, function, value, scales),
        _ => false,
    }
}

/// Whether `value` is read only as an address: by loads and stores through
/// it and `getelementptr`s that are such addresses. Each takes it in an
/// addressing form; a register to hold it across blocks is the allocator's.
pub fn address_only(
    function: &Function,
    value: ValueId,
    depth: u32,
) -> bool {
    let users = function.users(value);
    !users.is_empty()
        && users.iter().all(|one| {
            let user = function.instruction(one.user);
            match user.opcode {
                Opcode::Load { .. } => user.operands.first() == Some(&Operand::Value(value)),
                Opcode::Store { .. } => {
                    user.operands.get(1) == Some(&Operand::Value(value))
                        && user.operands.first() != Some(&Operand::Value(value))
                }
                Opcode::GetElementPtr { .. } => {
                    depth > 0 && user.result.is_some_and(|result| address_only(function, result, depth - 1))
                }
                _ => false,
            }
        })
}

thread_local! {
    static FOLDED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times this thread has asked whether a value is folded, for a test
/// that `addressed` asks of each pointer once.
pub fn folded_runs() -> usize {
    FOLDED.with(std::cell::Cell::get)
}

/// Whether `value` is an address only memory accesses of its own block, and
/// `getelementptr`s that are such addresses, take: folded into their
/// addressing modes, it takes no register.
pub fn folded(
    function: &Function,
    value: ValueId,
) -> bool {
    folded_with(None, function, value, 0)
}

/// `folded` where an address takes `scales` (`Room::index_scales`): a frame
/// object indexed by a scaled integer is `[ebp+esi*8+disp]` wherever it is
/// read, as it is where its offset is a constant.
pub fn folded_in(
    context: &Context,
    function: &Function,
    value: ValueId,
    scales: u8,
) -> bool {
    folded_with(Some(context), function, value, scales)
}

fn folded_with(
    context: Option<&Context>,
    function: &Function,
    value: ValueId,
    scales: u8,
) -> bool {
    FOLDED.with(|runs| runs.set(runs.get() + 1));
    let ValueDef::Instruction(def) = function.value(value).def else { return false };
    let block = function.parent(def);
    let users = function.users(value);
    // A constant offset into a frame object is a displacement wherever it is
    // read.
    let displacement = _frame_object(function, value)
        || _frame_offset(function, value)
        || context.is_some_and(|context| scales != 0 && _frame_indexed(context, function, value, scales));
    // The stack space's view of one is made where it is read, by whatever reads
    // it.
    if displacement && matches!(
        function.instruction(def).opcode,
        Opcode::Cast(CastOp::AddrSpaceCast)
    ) {
        return !users.is_empty();
    }
    if !matches!(
        function.instruction(def).opcode,
        Opcode::GetElementPtr { .. } | Opcode::Alloca { .. }
    ) {
        return false;
    }
    !users.is_empty()
        && users.iter().all(|one| {
            let op = function.instruction(one.user);
            (displacement || function.parent(one.user) == block)
                && match op.opcode {
                    Opcode::GetElementPtr { .. } => {
                        op.operands.first() == Some(&Operand::Value(value))
                            && folded_with(context, function, op.result.expect("a getelementptr's result"), scales)
                    }
                    Opcode::Load { .. } => op.operands.first() == Some(&Operand::Value(value)),
                    Opcode::Store { .. } => {
                        op.operands.get(1) == Some(&Operand::Value(value))
                            && op.operands.first() != Some(&Operand::Value(value))
                    }
                    _ => false,
                }
        })
}

/// Whether `value` is a frame object's address: a displacement from BP in each
/// access.
fn _frame_object(
    function: &Function,
    value: ValueId,
) -> bool {
    match function.value(value).def {
        ValueDef::Instruction(def) => {
            match (&function.instruction(def).opcode, &function.instruction(def).operands[..]) {
                (Opcode::Alloca { .. }, _) => true,
                // The same address in the stack's space.
                (Opcode::Cast(CastOp::AddrSpaceCast), [Operand::Value(from)]) => _frame_object(function, *from),
                _ => false,
            }
        }
        _ => false,
    }
}

/// Whether `value` is a frame object's or a symbol's address plus constants.
fn _frame_offset(
    function: &Function,
    value: ValueId,
) -> bool {
    let ValueDef::Instruction(def) = function.value(value).def else { return false };
    let op = function.instruction(def);
    let Opcode::GetElementPtr { .. } = op.opcode else { return false };
    let [base, indexes @ ..] = op.operands.as_slice() else { return false };
    indexes.iter().all(|one| matches!(one, Operand::Constant(_)))
        && match base {
            Operand::Value(base) => _frame_object(function, *base) || _frame_offset(function, *base),
            Operand::Constant(_) => true,
            _ => false,
        }
}

/// Whether `value` is a frame object's address plus one index scaled by what an
/// address takes (`x * 8`, a `mul` or a `shl`), and constants.
fn _frame_indexed(
    context: &Context,
    function: &Function,
    value: ValueId,
    scales: u8,
) -> bool {
    let ValueDef::Instruction(def) = function.value(value).def else { return false };
    let op = function.instruction(def);
    let Opcode::GetElementPtr { .. } = op.opcode else { return false };
    let [Operand::Value(base), indexes @ ..] = op.operands.as_slice() else { return false };
    if !(_frame_object(function, *base)
        || _frame_offset(function, *base)
        || _frame_indexed(context, function, *base, scales))
    {
        return false;
    }
    let variable = indexes.iter().filter(|one| !matches!(one, Operand::Constant(_))).collect::<Vec<_>>();
    // A constant displacement on a frame object indexed already.
    if variable.is_empty() {
        return _frame_indexed(context, function, *base, scales);
    }
    let [Operand::Value(index)] = variable[..] else { return false };
    let ValueDef::Instruction(made) = function.value(*index).def else { return false };
    let made = function.instruction(made);
    let number = |operand: &Operand| match operand {
        Operand::Constant(id) => match context.get(*id).kind {
            ConstantKind::Int(bits) => Some(bits),
            _ => None,
        },
        _ => None,
    };
    let scale = match (&made.opcode, &made.operands[..]) {
        (Opcode::Binary(BinaryOp::Mul), [_, factor]) => number(factor),
        (Opcode::Binary(BinaryOp::Shl), [_, amount]) => {
            number(amount).and_then(|amount| 1u128.checked_shl(u32::try_from(amount).ok()?))
        }
        _ => None,
    };
    scale.is_some_and(|scale| {
        [1u128, 2, 4, 8].iter().position(|&one| one == scale).is_some_and(|at| scales >> at & 1 == 1)
    })
}

fn _flags(
    function: &Function,
    value: ValueId,
) -> bool {
    let ValueDef::Instruction(def) = function.value(value).def else { return false };
    let block = function.parent(def);
    let users = function.users(value);
    !users.is_empty()
        && users
            .iter()
            .all(|one| function.instruction(one.user).opcode == Opcode::Br && function.parent(one.user) == block)
}

/// How many stores spill `value`: a pointer wider than its offset, a
/// segment and an offset, takes one each.
pub fn words(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    value: ValueId,
) -> i64 {
    match context.types.get(function.value(value).ty) {
        Type::Pointer(space) => {
            let spec = layout.pointer(*space);
            i64::from((spec.bits / spec.index_bits.max(1)).max(1))
        }
        _ => 1,
    }
}

/// The cell each value is spilled to, by the value that names it: a
/// counter's step, `p + c` or `p - c` taken back into phi `p`, shares `p`'s.
pub fn cells(function: &Function) -> BTreeMap<ValueId, ValueId> {
    let mut cells = BTreeMap::new();
    for &block in function.layout() {
        for &inst in function.block(block).instructions() {
            let phi = function.instruction(inst);
            let (Opcode::Phi, Some(counter)) = (&phi.opcode, phi.result) else { continue };
            for pair in phi.operands.chunks(2) {
                if let Operand::Value(step) = pair[0]
                    && _steps(function, step) == Some(counter)
                {
                    cells.insert(step, counter);
                }
            }
        }
    }
    cells
}

/// The counter `value` steps by a constant, if it does.
fn _steps(
    function: &Function,
    value: ValueId,
) -> Option<ValueId> {
    let ValueDef::Instruction(inst) = function.value(value).def else { return None };
    let instruction = function.instruction(inst);
    let constant = |operand: Operand| matches!(operand, Operand::Constant(_));
    match (&instruction.opcode, &instruction.operands[..]) {
        (Opcode::Binary(BinaryOp::Add), [Operand::Value(counter), other])
        | (Opcode::Binary(BinaryOp::Add), [other, Operand::Value(counter)])
            if constant(*other) =>
        {
            Some(*counter)
        }
        (Opcode::Binary(BinaryOp::Sub), [Operand::Value(counter), other]) if constant(*other) => Some(*counter),
        _ => None,
    }
}

fn _cell(
    cells: &BTreeMap<ValueId, ValueId>,
    value: ValueId,
) -> ValueId {
    cells.get(&value).copied().unwrap_or(value)
}

#[cfg(test)]
thread_local! {
    /// `traffic` calls, for a test that a pass finds the traffic of a function
    /// only when it asks.
    pub(crate) static TRAFFIC: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Each cell's traffic in `function`, weighted by `frequency`, from the
/// instructions `kept` says stay; a cell of `words` words is stored a word
/// at a time and loaded whole.
pub fn traffic(
    function: &Function,
    frequency: &BTreeMap<i64, i64>,
    cells: &BTreeMap<ValueId, ValueId>,
    costs: &OperationCosts,
    kept: &dyn Fn(InstId) -> bool,
    words: &dyn Fn(ValueId) -> i64,
) -> BTreeMap<ValueId, Traffic> {
    #[cfg(test)]
    TRAFFIC.with(|count| count.set(count.get() + 1));
    TrafficBase::of(function, frequency, &|value| cells.get(&value).copied(), kept).finished(function, costs, words)
}

/// Every instruction's traffic added up, before the stores are made words and
/// a lone maker's rebuild is priced: what a loop's own instructions are taken
/// out of (`without`), so that each loop of a function does not add up the
/// function again.
#[derive(Clone, Debug)]
pub struct TrafficBase {
    found: IdMap<ValueId, Traffic>,
    makers: IdMap<ValueId, usize>,
}

impl TrafficBase {
    /// The traffic of the instructions `kept` says stay.
    pub fn of(
        function: &Function,
        frequency: &BTreeMap<i64, i64>,
        cell: &dyn Fn(ValueId) -> Option<ValueId>,
        kept: &dyn Fn(InstId) -> bool,
    ) -> Self {
        let mut base = Self { found: IdMap::new(), makers: IdMap::new() };
        let often = |block: BlockId| frequency.get(&cfg::id(block)).copied().unwrap_or(1);
        for &block in function.layout() {
            let each = often(block);
            for &inst in function.block(block).instructions() {
                if let Some(value) = function.instruction(inst).result {
                    *base.makers.get_or_insert_with(value, || 0) += 1;
                }
                if kept(inst) {
                    base.add(function, inst, each, &often, cell, 1);
                }
            }
        }
        base
    }

    /// These instructions' traffic taken out: what `of` finds when they are not
    /// kept.
    pub fn without(
        &self,
        function: &Function,
        frequency: &BTreeMap<i64, i64>,
        cell: &dyn Fn(ValueId) -> Option<ValueId>,
        gone: impl IntoIterator<Item = InstId>,
    ) -> Self {
        let mut base = self.clone();
        let often = |block: BlockId| frequency.get(&cfg::id(block)).copied().unwrap_or(1);
        for inst in gone {
            if let Some(block) = function.parent(inst) {
                base.add(function, inst, often(block), &often, cell, -1);
            }
        }
        // A cell whose traffic all went is one the instructions left alone.
        let empty: Vec<ValueId> = base
            .found
            .iter()
            .filter(|(_, one)| (one.stores, one.updates, one.loads) == (0, 0, 0))
            .map(|(value, _)| value)
            .collect();
        for value in empty {
            base.found.remove(&value);
        }
        base
    }

    fn add(
        &mut self,
        function: &Function,
        inst: InstId,
        each: i64,
        often: &dyn Fn(BlockId) -> i64,
        cell: &dyn Fn(ValueId) -> Option<ValueId>,
        sign: i64,
    ) {
        let found = &mut self.found;
        let instruction = function.instruction(inst);
        let of = |value: ValueId| cell(value).unwrap_or(value);
        let own = instruction.result.map(of);
        if instruction.opcode == Opcode::Phi {
            // Each edge from another cell copies into this one.
            for pair in instruction.operands.chunks(2) {
                let from = match pair[1] {
                    Operand::Block(from) => often(from),
                    _ => each,
                } * sign;
                match pair[0] {
                    Operand::Value(value) if Some(of(value)) == own => {}
                    Operand::Value(value) => {
                        found.get_or_insert_with(of(value), Default::default).loads += from;
                        found.get_or_insert_with(own.expect("a phi's value"), Default::default).stores += from;
                    }
                    _ => found.get_or_insert_with(own.expect("a phi's value"), Default::default).stores += from,
                }
            }
            return;
        }
        if let Some(value) = instruction.result
            && cell(value).is_some()
        {
            found.get_or_insert_with(of(value), Default::default).updates += each * sign;
            return;
        }
        for operand in &instruction.operands {
            if let Operand::Value(value) = *operand {
                found.get_or_insert_with(of(value), Default::default).loads += each * sign;
            }
        }
        if let Some(cell) = own {
            found.get_or_insert_with(cell, Default::default).stores += each * sign;
        }
    }

    /// The traffic priced: stores in words, and a rebuild where one instruction
    /// makes the value.
    pub fn finished(
        &self,
        function: &Function,
        costs: &OperationCosts,
        words: &dyn Fn(ValueId) -> i64,
    ) -> BTreeMap<ValueId, Traffic> {
        self.found
            .iter()
            .map(|(value, one)| {
                let mut one = *one;
                one.stores *= words(value);
                if self.makers.get(&value) == Some(&1) {
                    one.rebuild = _rebuild(function, value, costs);
                }
                (value, one)
            })
            .collect()
    }
}

/// What making `value` again costs, where one instruction does from
/// nothing kept: a frame or symbol address, or a far view of one, which
/// also loads its segment.
fn _rebuild(
    function: &Function,
    value: ValueId,
    costs: &OperationCosts,
) -> Option<i64> {
    let ValueDef::Instruction(inst) = function.value(value).def else { return None };
    let instruction = function.instruction(inst);
    let constant = instruction.operands.iter().all(|operand| matches!(operand, Operand::Constant(_)));
    match instruction.opcode {
        Opcode::Alloca { .. } => Some(costs.address),
        Opcode::GetElementPtr { .. } if constant => Some(costs.address),
        Opcode::Cast(CastOp::AddrSpaceCast) => match instruction.operands[0] {
            Operand::Value(from) => _rebuild(function, from, costs).map(|near| near + costs.r#move),
            _ => Some(costs.address + costs.r#move),
        },
        _ => None,
    }
}

/// A point of an instruction: the cells `counted` says live before it, in
/// the target's registers, and for a call, those live across it, in the
/// registers `across` says it keeps.
pub struct Site {
    pub inst: InstId,
    pub before: Point<ValueId>,
    pub across: Option<Point<ValueId>>,
    /// The values live before it that the segment registers hold, against
    /// theirs.
    pub segments: Point<ValueId>,
    /// The values live before it that an address takes, against the registers
    /// one may be held in.
    pub addresses: Point<ValueId>,
}

/// Each instruction's site in a block but its phis, in order, from the values
/// live before and across each.
#[allow(clippy::too_many_arguments)]
pub fn sites(
    function: &Function,
    points: &[LivePoint],
    room: Room,
    across: &dyn Fn(InstId) -> i64,
    transient: &dyn Fn(InstId, &BTreeSet<ValueId>) -> i64,
    cells: &BTreeMap<ValueId, ValueId>,
    counted: &dyn Fn(ValueId) -> bool,
    segment: &dyn Fn(ValueId) -> bool,
    addressed: &dyn Fn(ValueId) -> bool,
) -> Vec<Site> {
    let viewed = |one: ValueId| room.segments > 0 && segment(one);
    // The cells of the live values that pass `keep`, in order and once each:
    // sorted in a vector, where a tree of them was most of a loop's model.
    let cells_of = |live: &BTreeSet<ValueId>, keep: &dyn Fn(ValueId) -> bool| -> Vec<ValueId> {
        let mut found: Vec<ValueId> =
            live.iter().copied().filter(|&one| keep(one)).map(|one| _cell(cells, one)).collect();
        found.sort_unstable();
        found.dedup();
        found
    };
    let residents = |live| cells_of(live, &|one| counted(one) && !viewed(one));
    let held = |live| cells_of(live, &|one| counted(one) && viewed(one));
    let routed = |live| cells_of(live, &|one| counted(one) && !viewed(one) && addressed(one));
    points
        .iter()
        .map(|point| (point.inst, &point.before, &point.across))
        .map(|(inst, before, past)| Site {
            inst,
            addresses: Point {
                registers: room.addresses,
                residents: if room.addresses > 0 { routed(before) } else { Vec::new() },
            },
            segments: Point { registers: room.segments, residents: held(before) },
            before: Point {
                registers: room.registers - transient(inst, before) - copied(function, inst, past, room, counted),
                residents: residents(before),
            },
            across: calls(function, inst).then(|| Point { registers: across(inst), residents: residents(past) }),
        })
        .collect()
}

impl Site {
    /// Its points, in order.
    pub fn points(self) -> impl Iterator<Item = Point<ValueId>> {
        std::iter::once(self.before)
            .chain(self.across)
            .chain((!self.segments.residents.is_empty()).then_some(self.segments))
            .chain((!self.addresses.residents.is_empty()).then_some(self.addresses))
    }
}

/// What the model knows of one function whatever room asks: liveness, the cell
/// each value is spilled to, which values take a register where they are live
/// (`integer_in`, asked once of each), and which are addressed. A manager
/// analysis, so each version of a function has it once.
#[derive(Clone, Debug, PartialEq)]
pub struct Pressure {
    found: Liveness,
    cells: BTreeMap<ValueId, ValueId>,
    counted: IdSet<ValueId>,
    addressed: IdSet<ValueId>,
    /// Whether each pointer an access is made through is folded: asked of the
    /// function once.
    folds: Folds,
    /// What is live before and across each instruction of a block, worked out
    /// when first asked and kept for every loop that asks of the block.
    points: BlockPoints,
}

/// The live sets of each block's instructions: a cache of what `found` says,
/// so two `Pressure`s of one function are equal whatever was asked.
#[derive(Clone, Default)]
struct BlockPoints(std::rc::Rc<std::cell::RefCell<BTreeMap<i64, std::rc::Rc<Vec<LivePoint>>>>>);

impl PartialEq for BlockPoints {
    fn eq(
        &self,
        _: &Self,
    ) -> bool {
        true
    }
}

impl std::fmt::Debug for BlockPoints {
    fn fmt(
        &self,
        formatter: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        write!(formatter, "BlockPoints({} blocks)", self.0.borrow().len())
    }
}

impl Pressure {
    /// `function`'s, for an address that takes `scales` (`Room::index_scales`).
    pub fn of(
        context: &Context,
        function: &Function,
        scales: u8,
    ) -> Self {
        let (addressed, folds) = addressed_by(function, &mut |value| folded_in(context, function, value, scales));
        Self {
            found: liveness::live(function),
            cells: cells(function),
            counted: (0..function.value_count() as u32)
                .map(ValueId)
                .filter(|&value| match function.value(value).def {
                    ValueDef::Instruction(inst) => !function.is_erased(inst),
                    ValueDef::Argument(_) => true,
                })
                .filter(|&value| integer_in(context, function, value, scales))
                .collect(),
            addressed: addressed.into_iter().collect(),
            folds,
            points: BlockPoints::default(),
        }
    }

    /// What is live before and across each instruction of `block` but its phis,
    /// as `liveness::live_points` has it, worked out once.
    pub fn points(
        &self,
        function: &Function,
        block: BlockId,
    ) -> std::rc::Rc<Vec<LivePoint>> {
        let at = cfg::id(block);
        if let Some(found) = self.points.0.borrow().get(&at) {
            return std::rc::Rc::clone(found);
        }
        let found = std::rc::Rc::new(liveness::live_points(function, &self.found, block));
        self.points.0.borrow_mut().insert(at, std::rc::Rc::clone(&found));
        found
    }
}

impl Pressure {
    /// What is live where.
    pub fn found(&self) -> &Liveness {
        &self.found
    }

    /// The cell each value is spilled to: itself, or the one it shares a cell
    /// with.
    pub fn cells(&self) -> &BTreeMap<ValueId, ValueId> {
        &self.cells
    }
}

impl llrm_mir::passes::Analysis for Pressure {
    type Result = Pressure;
    const NAME: &'static str = "pressure";
    fn run(
        context: &Context,
        _: &DataLayout,
        function: &Function,
        analyses: &mut llrm_mir::passes::Analyses,
    ) -> Self::Result {
        Pressure::of(context, function, Room::of(analyses.outer()).index_scales)
    }
}

/// `Pressure` under a room and the registers a call keeps: what every pass that
/// asks the model gets, a pass that is deciding between candidates `hide`ing
/// the values its candidates replace.
pub struct View<'a> {
    context: &'a Context,
    layout: &'a DataLayout,
    function: &'a Function,
    room: Room,
    across: &'a dyn Fn(InstId) -> i64,
    pressure: std::borrow::Cow<'a, Pressure>,
}

impl<'a> View<'a> {
    /// Over a function the manager has `pressure` of.
    pub fn over(
        pressure: &'a Pressure,
        context: &'a Context,
        layout: &'a DataLayout,
        function: &'a Function,
        room: Room,
        across: &'a dyn Fn(InstId) -> i64,
    ) -> Self {
        Self { context, layout, function, room, across, pressure: std::borrow::Cow::Borrowed(pressure) }
    }

    /// Over a function made to be asked about once, a candidate's.
    pub fn of(
        context: &'a Context,
        layout: &'a DataLayout,
        function: &'a Function,
        room: Room,
        across: &'a dyn Fn(InstId) -> i64,
    ) -> Self {
        Self {
            context,
            layout,
            function,
            room,
            across,
            pressure: std::borrow::Cow::Owned(Pressure::of(context, function, room.index_scales)),
        }
    }

    /// Each instruction's site in `block`, without the values `hide` names.
    pub fn sites(
        &self,
        block: BlockId,
        hide: &dyn Fn(ValueId) -> bool,
    ) -> Vec<Site> {
        let (context, layout, function, room) = (self.context, self.layout, self.function, self.room);
        sites(
            function,
            &self.pressure.points(function, block),
            room,
            self.across,
            &|inst, live| {
                transient_by(context, layout, function, inst, room, live, &mut |value| {
                    if self.pressure.folds.asked.contains(&value) {
                        self.pressure.folds.yes.contains(&value)
                    } else {
                        folded_in(context, function, value, room.index_scales)
                    }
                })
            },
            &self.pressure.cells,
            &|value| self.pressure.counted.contains(&value) && !hide(value),
            &|value| segment_view(context, layout, room.spaces, function, value),
            &|value| self.pressure.addressed.contains(&value),
        )
    }

    /// What fitting the function spills, each cell priced by its traffic.
    pub fn forecast(
        &self,
        costs: &OperationCosts,
        frequency: &BTreeMap<i64, i64>,
    ) -> Forecast<IdSet<ValueId>> {
        let traffic = traffic(self.function, frequency, &self.pressure.cells, costs, &|_| true, &|value| {
            words(self.context, self.layout, self.function, value)
        });
        let price = |cell: ValueId| traffic.get(&cell).map_or(0, |one| one.price(costs));
        self.swept(&price)
    }

    /// The forecast over the points of every site, found without a set copied
    /// for each: the live set is stepped through each block, and each kind of
    /// point keeps its cells not yet spilled in order of price, so a point
    /// costs what changed at it, not what is live.
    fn swept(
        &self,
        price: &dyn Fn(ValueId) -> i64,
    ) -> Forecast<IdSet<ValueId>> {
        let (context, layout, function, room) = (self.context, self.layout, self.function, self.room);
        let counted = |value: ValueId| self.pressure.counted.contains(&value);
        let mut folded = |value: ValueId| {
            if self.pressure.folds.asked.contains(&value) {
                self.pressure.folds.yes.contains(&value)
            } else {
                folded_in(context, function, value, room.index_scales)
            }
        };
        let mut sweep = Sweep { spilled: IdSet::default(), cost: 0, peak: 0 };
        let mut kinds = [Resident::default(), Resident::default(), Resident::default()];
        // A value's cell, and which kinds of point it is a resident of.
        let mut classes: llrm_support::hash::HashMap<ValueId, (ValueId, [bool; 3])> = Default::default();
        let mut class = |value: ValueId| {
            *classes
                .entry(value)
                .or_insert_with(
                    || {
                        let viewed = room.segments > 0 && segment_view(context, layout, room.spaces, function, value);
                        let counted = counted(value);
                        let resident = counted && !viewed;
                        let routed = room.addresses > 0 && resident && self.pressure.addressed.contains(&value);
                        (_cell(&self.pressure.cells, value), [resident, counted && viewed, routed])
                    },
                )
        };
        for &block in function.layout() {
            let liveness::Steps { first, steps } = liveness::live_steps(function, &self.pressure.found, block);
            for kind in &mut kinds {
                kind.clear();
            }
            let mut live = first.iter().copied().collect::<BTreeSet<_>>();
            for &value in &first {
                let (cell, member) = class(value);
                for (kind, _) in kinds.iter_mut().zip(member).filter(|(_, member)| *member) {
                    kind.add(cell, price(cell), &sweep.spilled);
                }
            }
            for step in &steps {
                let inst = step.inst;
                let wanted = room.registers - transient_by(context, layout, function, inst, room, &live, &mut folded);
                // Live across the instruction: what it reads and nothing else
                // holds is not.
                for value in &step.read {
                    live.remove(value);
                }
                let registers = wanted - copied(function, inst, &live, room, &counted);
                // Points, in order: before, across a call, held, routed.
                sweep.fit(&mut kinds, 0, registers);
                for value in &step.read {
                    let (cell, member) = class(*value);
                    if member[0] {
                        kinds[0].remove(cell);
                    }
                }
                if calls(function, inst) {
                    sweep.fit(&mut kinds, 0, (self.across)(inst));
                }
                sweep.fit(&mut kinds, 1, room.segments);
                sweep.fit(&mut kinds, 2, room.addresses);
                for value in &step.read {
                    let (cell, member) = class(*value);
                    for (kind, _) in kinds[1..].iter_mut().zip(&member[1..]).filter(|(_, member)| **member) {
                        kind.remove(cell);
                    }
                }
                if let Some(value) = step.made {
                    live.insert(value);
                    let (cell, member) = class(value);
                    for (kind, _) in kinds.iter_mut().zip(member).filter(|(_, member)| *member) {
                        kind.add(cell, price(cell), &sweep.spilled);
                    }
                }
            }
        }
        Forecast { cost: sweep.cost, spilled: sweep.spilled, peak: sweep.peak }
    }
}

/// What `View::swept` has spilled so far.
struct Sweep {
    spilled: IdSet<ValueId>,
    cost: i64,
    peak: i64,
}

impl Sweep {
    /// `fitted`'s step at one point of kind `which`: spill the cells that
    /// number past `registers`, the cheapest first, dearer ties to the larger
    /// cell, out of every kind.
    fn fit(
        &mut self,
        kinds: &mut [Resident; 3],
        which: usize,
        registers: i64,
    ) {
        let excess = kinds[which].order.len() as i64 - registers.max(0);
        self.peak = self.peak.max(excess);
        if excess <= 0 {
            return;
        }
        let victims = kinds[which].order.iter().take(excess as usize).copied().collect::<Vec<_>>();
        touched(victims.len());
        for (each, cell) in victims {
            self.cost += each;
            self.spilled.insert(cell);
            for kind in kinds.iter_mut() {
                kind.purge(cell);
            }
        }
    }
}

/// The cells live at a point and not spilled: how many values of each, and
/// their prices in order.
#[derive(Default)]
struct Resident {
    count: llrm_support::hash::HashMap<ValueId, (u32, i64)>,
    order: BTreeSet<(i64, ValueId)>,
}

impl Resident {
    fn clear(&mut self) {
        self.count.clear();
        self.order.clear();
    }

    fn add(
        &mut self,
        cell: ValueId,
        price: i64,
        spilled: &IdSet<ValueId>,
    ) {
        if spilled.contains(&cell) {
            return;
        }
        touched(1);
        let held = self.count.entry(cell).or_insert((0, price));
        held.0 += 1;
        if held.0 == 1 {
            self.order.insert((price, cell));
        }
    }

    fn remove(
        &mut self,
        cell: ValueId,
    ) {
        let Some(held) = self.count.get_mut(&cell) else { return };
        touched(1);
        held.0 -= 1;
        if held.0 == 0 {
            let price = held.1;
            self.count.remove(&cell);
            self.order.remove(&(price, cell));
        }
    }

    fn purge(
        &mut self,
        cell: ValueId,
    ) {
        if let Some((_, price)) = self.count.remove(&cell) {
            self.order.remove(&(price, cell));
        }
    }
}

#[cfg(test)]
#[path = "spill_tests.rs"]
mod tests;
