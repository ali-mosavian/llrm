//! Each memory access of a loop as an affine function of its counters, in the
//! form [`crate::dependence`] tests.
//!
//! # How it works
//! `references(unit, counters, variant, scanned, roots)` reads every load and
//! store of the blocks `scanned` and writes its address as `root + constant +
//! outer * i + inner * j + sum of invariant terms`: `root` is the pointer no
//! `getelementptr` computes, `i` and `j` are the iteration numbers of the
//! `counters` loops (outermost first; one loop is `inner`), and a term is an
//! invariant value with its coefficient. A counter is a header phi stepped by
//! a constant from one start (`start + step * iteration`).
//!
//! A value defined in a block of `variant` (the blocks whose values change
//! with the counters) is expanded through `add`, `sub`, `mul` and `shl` by a
//! constant and `sext`, to depth 12; any other operation, or a root that
//! `variant` defines, makes the whole listing `None`, as does a volatile
//! access, a call, an `invoke` or an `alloca` in `scanned`. A value defined
//! anywhere else is an invariant term.
//!
//! `Apart` says which references through different roots may meet. `Roots`
//! numbers the distinct roots, so two listings made with one `Roots`
//! (two loops, say) agree on which accesses share an object. A `Reference`
//! says whether its root is an object's own address (`memory::object_of`: an
//! alloca or a global): two such roots that differ are different objects, and
//! `dependence` calls their accesses independent. Roots that are not
//! identified (a pointer parameter) are the caller's to relate.
//!
//! ```text
//!   %o = mul i32 %j, 4
//!   %p = getelementptr i8, ptr @a, i32 %o        root @a, constant 0,
//!   store i32 %v, ptr %p                         inner 4, width 4, write
//! ```
//!
//! # Example
//! The body of `for (j = 0; j < n; j++) a[j + 2] = a[j]` with `a` an `i32`
//! array, listed with the loop as `inner`:
//!
//! ```text
//! load  a[j]       constant 0, inner 4, width 4, read
//! store a[j + 2]   constant 8, inner 4, width 4, write
//! ```
//!
//! From unit test `an_array_walked_by_a_counter_is_an_affine_access` in
//! `access_tests.rs`.
//!
//! # Provides
//! `references`, `Reference`, `Roots`, `Apart`. Consumers: `unroll_jam` and
//! `loopfuse` (`crates/opt/llrm-transforms`).
//!
//! # See also
//! LLVM ScalarEvolution add recurrences read by `DependenceAnalysis`; gcc's
//! `dr_analyze_innermost` and `access_fn` of `tree-data-ref.cc`.

use std::collections::BTreeMap;

use llrm_mir::context::signed as signed_bits;
use llrm_mir::dense::IdMap;
use llrm_mir::module::{Function, InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{BinaryOp, CastOp, Opcode};

use crate::dependence::{Access, Subscript};
use crate::graph::loops::Loop;
use crate::occurrence::arms;
use crate::{cfg, memory};

/// One access of a loop, as `dependence` takes it, and where it came from.
#[derive(Clone, Debug)]
pub struct Reference {
    pub access: Access,
    pub inst: InstId,
    /// The pointer the address is a constant and invariant offset from.
    pub root: Operand,
    /// `root` is an object's own address: an alloca or a global.
    pub identified: bool,
}

/// Asked of two references through different roots: whether they cannot meet.
/// Both roots are objects' own addresses (`memory::object_of`: two such that
/// differ are two objects), or what each may reach, as the alias owner's
/// points-to puts it, is disjoint. The points-to is found at the first pair
/// that needs it.
pub struct Apart<'a> {
    unit: &'a memory::Unit<'a>,
    points: std::cell::OnceCell<Option<std::borrow::Cow<'a, crate::alias::PointsTo>>>,
}

impl<'a> Apart<'a> {
    pub fn new(unit: &'a memory::Unit<'a>) -> Self {
        Self { unit, points: std::cell::OnceCell::new() }
    }

    /// `one` and `other` are through different roots and may meet: nothing
    /// says they do not.
    pub fn unknown(
        &self,
        one: &Reference,
        other: &Reference,
    ) -> bool {
        if one.root == other.root || (one.identified && other.identified) {
            return false;
        }
        let points = self.points.get_or_init(|| self.unit.pointers().ok());
        let provenance = |access: &Reference| {
            let reference = self.unit.reference(access.inst)?;
            points.as_ref()?.reference(self.unit, &reference)
        };
        match (provenance(one), provenance(other)) {
            (Some(left), Some(right)) => left.intersects(&right),
            _ => true,
        }
    }
}

/// The distinct roots of the references listed with it, numbered in order of
/// appearance: `Access::object`.
#[derive(Default)]
pub struct Roots(Vec<Operand>);

impl Roots {
    fn number(
        &mut self,
        root: Operand,
    ) -> u64 {
        self.0.iter().position(|one| *one == root).unwrap_or_else(|| {
            self.0.push(root);
            self.0.len() - 1
        }) as u64
    }
}

/// `bits` as the signed number `width` bits wide it is.
fn signed(
    bits: u128,
    width: u32,
) -> i64 {
    signed_bits(bits, width.min(64)) as i64
}

/// An address or an index as an affine function of the counters: bytes (or
/// units) `c + outer * i + inner * j + sum of invariant terms`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Lin {
    c: i64,
    outer: i64,
    inner: i64,
    symbols: BTreeMap<u32, i64>,
}

impl Lin {
    fn constant(c: i64) -> Self {
        Self { c, ..Self::default() }
    }

    fn is_constant(&self) -> bool {
        self.outer == 0 && self.inner == 0 && self.symbols.is_empty()
    }

    fn plus(
        &self,
        other: &Self,
        sign: i64,
    ) -> Self {
        let mut symbols = self.symbols.clone();
        for (&symbol, &factor) in &other.symbols {
            *symbols.entry(symbol).or_insert(0) += sign * factor;
        }
        symbols.retain(|_, factor| *factor != 0);
        Self {
            c: self.c + sign * other.c,
            outer: self.outer + sign * other.outer,
            inner: self.inner + sign * other.inner,
            symbols,
        }
    }

    fn scaled(
        &self,
        by: i64,
    ) -> Self {
        Self {
            c: self.c * by,
            outer: self.outer * by,
            inner: self.inner * by,
            symbols: self
                .symbols
                .iter()
                .map(|(&symbol, &factor)| (symbol, factor * by))
                .filter(|(_, factor)| *factor != 0)
                .collect(),
        }
    }
}

struct Addresses<'a> {
    unit: &'a memory::Unit<'a>,
    variant: &'a llrm_mir::forest::Body,
    /// The counters of the loops, as functions of the iteration number.
    counters: IdMap<ValueId, Lin>,
}

impl<'a> Addresses<'a> {
    fn new(
        unit: &'a memory::Unit<'a>,
        counters: &[&Loop],
        variant: &'a llrm_mir::forest::Body,
    ) -> Self {
        let function = unit.function;
        let mut me = Self { unit, variant, counters: IdMap::new() };
        let mut found = IdMap::new();
        for (at, loop_) in counters.iter().enumerate() {
            let outer = at + 1 < counters.len();
            let latch = cfg::block(*loop_.latches.first().expect("one latch"));
            for phi in function
                .block(cfg::block(loop_.header))
                .instructions()
                .iter()
                .copied()
                .take_while(|&inst| function.instruction(inst).opcode == Opcode::Phi)
            {
                let result = function.instruction(phi).result.expect("a phi's value");
                let mut start = None;
                let mut next = None;
                for (value, from) in arms(function, phi) {
                    if from == latch {
                        next = Some(value);
                    } else {
                        start = Some(value);
                    }
                }
                let (Some(start), Some(next)) = (start, next) else { continue };
                let Some(start) = me.lin(start, 0) else { continue };
                let Some(step) = me.step(next, result) else { continue };
                let counter = if outer { Lin { outer: step, ..start } } else { Lin { inner: step, ..start } };
                found.insert(result, counter);
            }
        }
        me.counters = found;
        me
    }

    /// The constant `next` adds to `phi`.
    fn step(
        &self,
        next: Operand,
        phi: ValueId,
    ) -> Option<i64> {
        let (_, op) = self.unit.defining(next)?;
        let Opcode::Binary(kind @ (BinaryOp::Add | BinaryOp::Sub)) = op.opcode else { return None };
        let width = self.unit.int_bits(next)?;
        let constant = |operand: Operand| self.unit.int_constant(operand).map(|bits| signed(bits, width));
        match (op.operands[0] == Operand::Value(phi), op.operands[1] == Operand::Value(phi)) {
            (true, false) => constant(op.operands[1]).map(|step| if kind == BinaryOp::Add { step } else { -step }),
            (false, true) if kind == BinaryOp::Add => constant(op.operands[0]),
            _ => None,
        }
    }

    fn lin(
        &self,
        operand: Operand,
        depth: u32,
    ) -> Option<Lin> {
        if depth > 12 {
            return None;
        }
        let width = self.unit.int_bits(operand)?;
        if let Some(bits) = self.unit.int_constant(operand) {
            return Some(Lin::constant(signed(bits, width)));
        }
        let Operand::Value(value) = operand else { return None };
        if let Some(counter) = self.counters.get(&value) {
            return Some(counter.clone());
        }
        let function = self.unit.function;
        let Some(inst) = defined(function, value) else {
            return Some(Lin { symbols: BTreeMap::from([(value.0, 1)]), ..Lin::default() });
        };
        let at = cfg::id(function.parent(inst)?);
        if !self.variant.contains(&at) {
            return Some(Lin { symbols: BTreeMap::from([(value.0, 1)]), ..Lin::default() });
        }
        let op = function.instruction(inst);
        let one = |i: usize| self.lin(op.operands[i], depth + 1);
        match &op.opcode {
            Opcode::Binary(BinaryOp::Add) => Some(one(0)?.plus(&one(1)?, 1)),
            Opcode::Binary(BinaryOp::Sub) => Some(one(0)?.plus(&one(1)?, -1)),
            Opcode::Binary(BinaryOp::Mul) => {
                let (left, right) = (one(0)?, one(1)?);
                if right.is_constant() {
                    Some(left.scaled(right.c))
                } else if left.is_constant() {
                    Some(right.scaled(left.c))
                } else {
                    None
                }
            }
            Opcode::Binary(BinaryOp::Shl) => {
                let (left, right) = (one(0)?, one(1)?);
                (right.is_constant() && (0..32).contains(&right.c)).then(|| left.scaled(1 << right.c))
            }
            Opcode::Cast(CastOp::SExt) => one(0),
            _ => None,
        }
    }

    /// `pointer` as a root and the bytes from it.
    fn address(
        &self,
        pointer: Operand,
    ) -> Option<(Operand, Lin)> {
        let function = self.unit.function;
        if let Some((_, op)) = self.unit.defining(pointer) {
            if let Opcode::GetElementPtr { source } = op.opcode {
                let [base, index] = op.operands[..] else { return None };
                let size = self.unit.layout.alloc_size(&self.unit.context.types, source) as i64;
                let (root, bytes) = self.address(base)?;
                return Some((root, bytes.plus(&self.lin(index, 0)?.scaled(size), 1)));
            }
        }
        if let Operand::Value(value) = pointer {
            if defined(function, value)
                .and_then(|inst| function.parent(inst))
                .is_some_and(|block| self.variant.contains(&cfg::id(block)))
            {
                return None;
            }
        }
        Some((pointer, Lin::default()))
    }
}

/// The instruction that defines `value`, if one does.
fn defined(
    function: &Function,
    value: ValueId,
) -> Option<InstId> {
    match function.value(value).def {
        ValueDef::Instruction(inst) => Some(inst),
        ValueDef::Argument(_) => None,
    }
}

/// The accesses of the blocks `scanned`, for the dependence tester; None where
/// one is not affine in the counters of `counters` from a root, or a block
/// calls out.
pub fn references(
    unit: &memory::Unit,
    counters: &[&Loop],
    variant: &llrm_mir::forest::Body,
    scanned: &llrm_mir::forest::Body,
    roots: &mut Roots,
) -> Option<Vec<Reference>> {
    assert!(counters.len() <= 2, "an access is affine in at most two counters");
    let function = unit.function;
    let addresses = Addresses::new(unit, counters, variant);
    let mut out = Vec::new();
    for &at in scanned {
        for inst in function.block(cfg::block(at)).iter() {
            let op = function.instruction(inst);
            let (pointer, ty, write) = match op.opcode {
                Opcode::Load { volatile: false, .. } => (op.operands[0], op.ty, false),
                Opcode::Store { volatile: false, .. } => (op.operands[1], unit.operand_type(op.operands[0])?, true),
                Opcode::Load { .. }
                | Opcode::Store { .. }
                | Opcode::Call(_)
                | Opcode::Invoke(_)
                | Opcode::Alloca { .. } => {
                    return None;
                }
                _ => continue,
            };
            let (root, bytes) = addresses.address(pointer)?;
            out.push(Reference {
                access: Access {
                    object: roots.number(root),
                    subscripts: vec![Subscript { constant: bytes.c, outer: bytes.outer, inner: bytes.inner }],
                    symbols: bytes.symbols.into_iter().collect(),
                    width: unit.layout.store_size(&unit.context.types, ty) as u32,
                    write,
                },
                inst,
                root,
                identified: memory::object_of(unit, root).is_some(),
            });
        }
    }
    Some(out)
}

#[cfg(test)]
#[path = "access_tests.rs"]
mod tests;
