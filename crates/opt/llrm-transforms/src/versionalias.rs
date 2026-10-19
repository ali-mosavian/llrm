//! A loop version's second reason: the pointers a loop reads and writes may be
//! one object, and where the bytes they reach do not meet, they are not.
//!
//! # How it works
//! `Disjoint::found` lists the accesses of an innermost counted loop
//! ([`llrm_analysis::access`]), each an affine address on a root, and the pairs
//! of them, one a store, through different roots that nothing proves apart
//! (`access::Apart::unknown`). Accesses of one root with one invariant part and
//! one step are a group, their bytes at the trip `i` `[low, high)` from
//! `root + sum of terms + step * i`.
//!
//! Its test, made in the entry, is for each such pair of groups that their
//! bytes over the loop's trips do not meet: with `start` the lowest byte of a
//! group (`root + low + terms`, less `|step| * (trips - 1)` where it steps
//! down) and `length = |step| * (trips - 1) + high - low`, the groups `a` and
//! `b` are apart where `start_b - start_a >= length_a` and `start_a - start_b
//! >= length_b`, unsigned and modulo the pointer's width, which holds whether
//! or not the ranges end at the top of the space.
//!
//! The copy's code is told so: each root of such a pair is replaced there by
//! `llrm.noalias(root)` ([`llrm_mir::intrinsics::Intrinsic::Restrict`]), whose
//! provenance the alias owner makes a restrict root of its own, so every pass
//! after asks it as it would of a `noalias` parameter. Every pair of roots the
//! copy is told apart was tested, and any other pair is the alias owner's as it
//! was.
//!
//! ```text
//!   for i: a[i] = *c + b[i]        if a, b, c apart: for i: a'[i] = *c' + b'[i]
//!                                  else:             for i: a[i]   = *c  + b[i]
//! ```
//!
//! A loop is copied only where there is something for the copy to take out of
//! it: a load of one place every trip, or one read twice, which a store through
//! a pointer that may be another's keeps in. Only pointers in one flat space
//! are tested (an address there is an integer): not far pointers. At most
//! `MAX_CHECKS` pairs of groups are, and `MAX_ACCESSES` accesses are listed.
//!
//! # Example
//! `a[i] = *c + b[i]` over parameters: `*c` is read again every trip, as a
//! store through `a` may reach it. The copy that the test of `a`, `b` and `c`
//! sends to has `*c` read once, before the loop. See [`crate::loopversion`].
//!
//! # Provides
//! `Disjoint`, an [`crate::loopversion::Assumption`].
//!
//! # See also
//! LLVM `LoopAccessAnalysis` (`RuntimePointerChecking`, `generateChecks`) and
//! `LoopVersioning::versionLoop` with its `!noalias` scopes, here the restrict
//! root.

use llrm_analysis::access::{self, Apart, Reference, Roots};
use llrm_analysis::cfg::Canonical;
use llrm_analysis::induction::{self, AffineOperand, CountedLoop};
use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::edit::Position;
use llrm_mir::module::{Function, InstId, Operand, ValueId};
use llrm_mir::opcode::{BinaryOp, CallInfo, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::passes::Declared;
use llrm_mir::types::Type;

use crate::counting::Seeds;
use crate::loopclone::LoopCopy;
use crate::loopshape;
use crate::loopversion::{Assumption, RESTRICT, Within};

/// Pairs of groups tested.
const MAX_CHECKS: usize = 4;
/// Accesses of the loop that are listed.
const MAX_ACCESSES: usize = 32;

/// The bytes of the accesses of one root with one invariant part and one step.
#[derive(Clone, Debug)]
struct Group {
    root: Operand,
    symbols: Vec<(u32, i64)>,
    step: i64,
    low: i64,
    high: i64,
}

pub struct Disjoint {
    proof: CountedLoop,
    groups: Vec<Group>,
    pairs: Vec<(usize, usize)>,
    /// The width of an address, in bits, and its space.
    index_bits: u32,
    space: u32,
    /// Each root, and what the copy reads in its place: made by the test.
    restricted: Vec<(Operand, Operand)>,
}

impl Disjoint {
    /// The pairs of `shape` that need a test, where there are some and they
    /// can be tested.
    pub fn found(
        view: &Within,
        shape: &Canonical,
    ) -> Option<Self> {
        let function = view.function;
        let proof = induction::counted(view.within, &shape.loop_, None, false)
            .into_iter()
            .find(|proof| loopshape::controls(function, proof, shape))?;
        let mut roots = Roots::default();
        let found = access::references(view.within, &[&shape.loop_], &shape.loop_.body, &shape.loop_.body, &mut roots)?;
        if found.len() > MAX_ACCESSES || !found.iter().any(|one| one.access.write) {
            return None;
        }
        let apart = Apart::new(view.within);
        let unknown = |x: &Reference, y: &Reference| (x.access.write || y.access.write) && apart.unknown(x, y);
        // The copy is for what restrict lets other passes take out of the loop:
        // a load of one place every trip, or the same load twice, which a store
        // through a pointer that may be another's keeps in.
        let same = |x: &Reference, y: &Reference| {
            x.root == y.root && x.access.subscripts == y.access.subscripts && x.access.symbols == y.access.symbols
        };
        let kept = |read: &Reference| {
            let again =
                found.iter().any(|other| !std::ptr::eq(other, read) && !other.access.write && same(read, other));
            (read.access.subscripts[0].inner == 0 || again)
                && found.iter().any(|store| store.access.write && unknown(read, store))
        };
        if !found.iter().any(|one| !one.access.write && kept(one)) {
            return None;
        }
        if !found.iter().enumerate().any(|(at, x)| found[at + 1..].iter().any(|y| unknown(x, y))) {
            return None;
        }
        // One address width, in a space where an address is an integer.
        // The flat space [`RESTRICT`] is declared for.
        let space = 0;
        let spec = view.within.layout.pointer(space);
        if spec.bits != spec.index_bits
            || found.iter().any(|one| view.within.space(one.root).unwrap_or(0) != space)
            || found.iter().any(|one| one.access.subscripts.len() != 1 || one.access.subscripts[0].outer != 0)
            || found.iter().flat_map(|one| &one.access.symbols).any(|&(value, _)| {
                view.within.int_bits(Operand::Value(ValueId(value))).is_none_or(|bits| bits > spec.index_bits)
            })
        {
            return None;
        }
        let mut groups: Vec<(Group, Vec<usize>)> = Vec::new();
        for (at, one) in found.iter().enumerate() {
            let subscript = one.access.subscripts[0];
            let (low, high) = (subscript.constant, subscript.constant + i64::from(one.access.width));
            match groups.iter_mut().find(|(group, _)| {
                group.root == one.root && group.symbols == one.access.symbols && group.step == subscript.inner
            }) {
                Some((group, members)) => {
                    group.low = group.low.min(low);
                    group.high = group.high.max(high);
                    members.push(at);
                }
                None => groups.push((
                    Group { root: one.root, symbols: one.access.symbols.clone(), step: subscript.inner, low, high },
                    vec![at],
                )),
            }
        }
        let mut pairs = Vec::new();
        for (a, (_, mine)) in groups.iter().enumerate() {
            for (b, (_, theirs)) in groups.iter().enumerate().skip(a + 1) {
                if mine.iter().any(|&x| theirs.iter().any(|&y| unknown(&found[x], &found[y]))) {
                    pairs.push((a, b));
                }
            }
        }
        if pairs.is_empty() || pairs.len() > MAX_CHECKS {
            return None;
        }
        Some(Self {
            proof,
            groups: groups.into_iter().map(|(group, _)| group).collect(),
            pairs,
            index_bits: spec.index_bits,
            space,
            restricted: Vec::new(),
        })
    }
}

/// `opcode` of `operands` before `before`, its value.
fn emitted(
    function: &mut Function,
    before: InstId,
    opcode: Opcode,
    ty: llrm_mir::types::TypeId,
    operands: Vec<Operand>,
) -> Result<Operand, String> {
    let inst = function.create_instruction(opcode, ty, operands, Flags::default(), None);
    function.insert(inst, Position::Before(before))?;
    Ok(Operand::Value(function.instruction(inst).result.ok_or("a value")?))
}

impl Assumption for Disjoint {
    fn name(&self) -> &'static str {
        "pointers apart"
    }

    fn condition(
        &mut self,
        context: &mut Context,
        declared: &mut Declared,
        function: &mut Function,
        before: InstId,
    ) -> Result<Operand, String> {
        let (bits, bit) = (self.index_bits, context.types.int(1));
        let integer = context.types.int(bits);
        // The trips, in an address's width, less one.
        let last = {
            let mut seeds = Seeds { context, function, at: before, width: self.proof.width() };
            let trips = induction::trips(&self.proof, &mut |kind, args| seeds.computed(kind, args))
                .ok_or("the loop's trips are not an expression")?;
            seeds.width = bits;
            let trips = seeds.widened(&trips, bits);
            seeds.computed(BinaryOp::Sub, vec![trips, AffineOperand::constant(1, bits)])
        };
        let mut starts = Vec::new();
        let mut lengths = Vec::new();
        for group in &self.groups {
            let Operand::Value(root) =
                emitted(function, before, Opcode::Cast(CastOp::PtrToInt), integer, vec![group.root])?
            else {
                return Err("a pointer's integer".to_owned());
            };
            let mut seeds = Seeds { context, function, at: before, width: bits };
            let mut start = seeds.computed(
                BinaryOp::Add,
                vec![AffineOperand::Value(root, bits), AffineOperand::constant(group.low, bits)],
            );
            for &(value, factor) in &group.symbols {
                let value = ValueId(value);
                let from = seeds.context.types.int_bits(seeds.function.value(value).ty).ok_or("an integer term")?;
                let wide = if from < bits {
                    let extended = emitted(
                        seeds.function,
                        before,
                        Opcode::Cast(CastOp::SExt),
                        integer,
                        vec![Operand::Value(value)],
                    )?;
                    let Operand::Value(extended) = extended else { return Err("a value".to_owned()) };
                    AffineOperand::Value(extended, bits)
                } else {
                    AffineOperand::Value(value, bits)
                };
                let term = seeds.computed(BinaryOp::Mul, vec![wide, AffineOperand::constant(factor, bits)]);
                start = seeds.computed(BinaryOp::Add, vec![start, term]);
            }
            let reach = seeds.computed(
                BinaryOp::Mul,
                vec![last.clone(), AffineOperand::constant(group.step.unsigned_abs() as i64, bits)],
            );
            if group.step < 0 {
                let down = seeds.computed(BinaryOp::Mul, vec![last.clone(), AffineOperand::constant(group.step, bits)]);
                start = seeds.computed(BinaryOp::Add, vec![start, down]);
            }
            let length =
                seeds.computed(BinaryOp::Add, vec![reach, AffineOperand::constant(group.high - group.low, bits)]);
            starts.push(start);
            lengths.push(length);
        }
        let mut holds: Option<Operand> = None;
        for &(a, b) in &self.pairs {
            let mut seeds = Seeds { context, function, at: before, width: bits };
            let forward = seeds.computed(BinaryOp::Sub, vec![starts[b].clone(), starts[a].clone()]);
            let back = seeds.computed(BinaryOp::Sub, vec![starts[a].clone(), starts[b].clone()]);
            let (forward, back) = (seeds.operand(&forward), seeds.operand(&back));
            let (length_a, length_b) = (seeds.operand(&lengths[a]), seeds.operand(&lengths[b]));
            let first = emitted(function, before, Opcode::ICmp(IntPredicate::Uge), bit, vec![forward, length_a])?;
            let second = emitted(function, before, Opcode::ICmp(IntPredicate::Uge), bit, vec![back, length_b])?;
            let both = emitted(function, before, Opcode::Binary(BinaryOp::And), bit, vec![first, second])?;
            holds = Some(match holds {
                None => both,
                Some(all) => emitted(function, before, Opcode::Binary(BinaryOp::And), bit, vec![all, both])?,
            });
        }
        // Each root of a pair, as the copy reads it.
        let pointer = context.types.ptr(self.space);
        let ty = context.types.intern(Type::Function { returns: pointer, parameters: vec![pointer], variadic: false });
        let callee = declared.declare(RESTRICT, ty);
        let callee = Operand::Constant(context.constant(Constant { ty: pointer, kind: ConstantKind::Global(callee) }));
        let mut roots: Vec<Operand> = Vec::new();
        for &(a, b) in &self.pairs {
            for group in [a, b] {
                if !roots.contains(&self.groups[group].root) {
                    roots.push(self.groups[group].root);
                }
            }
        }
        for root in roots {
            let info = CallInfo {
                function_type: ty,
                calling_convention: 0,
                return_attrs: Vec::new(),
                argument_attrs: vec![Vec::new(); 1],
                attrs: Vec::new(),
                tail: Default::default(),
            };
            let call = emitted(function, before, Opcode::Call(Box::new(info)), pointer, vec![root, callee])?;
            self.restricted.push((root, call));
        }
        holds.ok_or_else(|| "no pair".to_owned())
    }

    fn assumed(
        &self,
        _: &Context,
        function: &mut Function,
        copy: &LoopCopy,
    ) -> Result<(), String> {
        for (_, &inst) in copy.instructions.iter() {
            let operands = function
                .instruction(inst)
                .operands
                .iter()
                .map(|&operand| match self.restricted.iter().find(|(root, _)| *root == operand) {
                    Some(&(_, restricted)) => restricted,
                    None => operand,
                })
                .collect();
            function.set_operands(inst, operands);
        }
        Ok(())
    }
}
