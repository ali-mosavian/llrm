//! The machine operand model: `Reg`, `Held`, `Imm`, `Address`, `Mem`, `Loc`,
//! `Effects`, `Operation` and `Semantics`. Direct port of Python's
//! `qbopt.model.ir` machine-semantics vocabulary, in a crate of its own so the
//! BC lifter and the backend both depend on it and neither owns it.
//!
//! This is deliberately distinct from the existing `MachineInstruction`
//! representation.  Python LIR carries selected semantics, source-byte
//! provenance, and value identities before allocation; those facts must not be
//! projected onto the newer generic selected-IR model.

use std::collections::BTreeSet;
use std::fmt;
use std::hash::Hash;
use std::sync::LazyLock;

use iced_x86::Register;
pub use llrm_omf::module::{Addr, Space};
use llrm_support::pyrepr::{self, Repr};

pub mod flag;
mod root;

pub use root::root;

/// One physical register operand, at the instruction's width.
///
/// Direct port of `qbopt.model.ir:Reg`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Reg {
    pub register: iced_x86::Register,
    pub width: u32,
}

impl Reg {
    /// The x87 register `st(index)`, relative to the current top. A float is
    /// ten bytes there.
    pub const fn st(index: u32) -> Self {
        let register = [
            iced_x86::Register::ST0,
            iced_x86::Register::ST1,
            iced_x86::Register::ST2,
            iced_x86::Register::ST3,
            iced_x86::Register::ST4,
            iced_x86::Register::ST5,
            iced_x86::Register::ST6,
            iced_x86::Register::ST7,
        ][index as usize];
        Self { register, width: 10 }
    }

    /// Its position, where it is an x87 register.
    pub fn st_index(&self) -> Option<u32> {
        self.register.is_st().then(|| self.register as u32 - iced_x86::Register::ST0 as u32)
    }
}

/// An SSA value held in an as-yet undecided physical register.
///
/// Direct port of `qbopt.model.ir:Held`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Held {
    pub value: u32,
    pub width: u32,
}

/// An immediate instruction operand.
///
/// Direct port of `qbopt.model.ir:Imm`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Imm {
    pub value: i64,
    pub width: u32,
    pub address: Option<Addr>,
}

/// An address used as a value, rather than a memory access.
///
/// Direct port of `qbopt.model.ir:Address`. Equality and hashing take every
/// field: two addresses spelled through different registers or displacements
/// are different addresses. (Python left the encoding fields out;
/// that made `==` mean "the same address modulo how it is encoded", which is no
/// answer to the question a caller asks of two operands that will be emitted.)
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Address {
    pub addr: Option<Addr>,
    pub through: iced_x86::Register,
    pub index: iced_x86::Register,
    pub scale: i64,
    pub offset: i64,
    pub disp_width: u32,
}

impl Address {
    pub const fn new(addr: Option<Addr>) -> Self {
        Self {
            addr,
            through: iced_x86::Register::None,
            index: iced_x86::Register::None,
            scale: 1,
            offset: 0,
            disp_width: 0,
        }
    }
}

impl Address {
    /// The same address, not necessarily spelled the same way: `==` less its
    /// encoding fields, which are the registers it is reached through, its
    /// scale, and the displacement of one that has an `addr`.
    pub fn same_place(
        &self,
        other: &Self,
    ) -> bool {
        self.addr == other.addr && (self.addr.is_some() || self.offset == other.offset)
    }

    /// Its displacement is BP's: see `Mem::in_frame`.
    pub fn in_frame(&self) -> bool {
        _in_frame(self.addr, self.through, false)
    }
}

/// A memory operand.
///
/// Direct port of `qbopt.model.ir:Mem`. Equality and hashing take every field,
/// the encoding details (`through`, `offset`, `disp_width`, `index_through`)
/// included: see `Address`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Mem {
    pub addr: Option<Addr>,
    pub width: u32,
    pub through: iced_x86::Register,
    pub offset: i64,
    pub disp_width: u32,
    pub base: Option<Held>,
    pub stack_argument: bool,
    pub selector: Option<Held>,
    pub index: Option<Held>,
    pub scale: i64,
    pub index_through: iced_x86::Register,
    /// Lowering proved (`ranges::exact_offsets`) that this address names the
    /// same byte summed through 32-bit registers: its start is the object's,
    /// and every partial sum of its offset is a non-negative 16-bit integer.
    pub exact: bool,
}

impl Mem {
    /// The same cell, not necessarily spelled the same way: `==` less its
    /// encoding fields (`through`, `offset` of a cell that has an `addr`,
    /// `disp_width`, `index_through`, `exact`). What a caller means by "the
    /// same operand" when it asks of operands not yet emitted, or of one in
    /// two spellings.
    pub fn same_place(
        &self,
        other: &Self,
    ) -> bool {
        self.addr == other.addr
            && self.width == other.width
            && self.base == other.base
            && self.stack_argument == other.stack_argument
            && self.selector == other.selector
            && self.index == other.index
            && self.scale == other.scale
            && (self.addr.is_some() || self.offset == other.offset)
    }

    /// `self` is the word above `low`: the cell two bytes on, however either is
    /// spelled.
    pub fn word_above(
        &self,
        low: &Self,
    ) -> bool {
        self.same_place(&Self {
            addr: low.addr.map(|addr| addr.plus(2)),
            offset: if low.addr.is_some() { low.offset } else { low.offset + 2 },
            ..low.clone()
        })
    }

    pub const fn new(
        addr: Option<Addr>,
        width: u32,
    ) -> Self {
        Self {
            addr,
            width,
            through: iced_x86::Register::None,
            offset: 0,
            disp_width: 0,
            base: None,
            stack_argument: false,
            selector: None,
            index: None,
            scale: 1,
            index_through: iced_x86::Register::None,
            exact: false,
        }
    }

    /// Its displacement is BP's: a frame cell, or an indexed one, which is
    /// spelled through BP with a literal displacement.
    pub fn in_frame(&self) -> bool {
        _in_frame(self.addr, self.through, self.base.is_some())
    }
}

fn _in_frame(
    addr: Option<Addr>,
    through: Register,
    valued: bool,
) -> bool {
    // A base value is the register's: the frame register is a frame only where
    // nothing was given it.
    addr.is_some_and(|addr| {
        addr.space == Space::Frame
            || (addr.space == Space::Literal && !valued && matches!(through, Register::BP | Register::EBP))
    })
}

/// Every selected machine operand Python LIR may carry.
///
/// Direct port of `qbopt.model.ir:Loc`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Loc {
    Reg(Reg),
    Mem(Mem),
    Imm(Imm),
    Address(Address),
    Held(Held),
}

impl Loc {
    /// The x87 register `st(index)`.
    pub const fn st(index: u32) -> Self {
        Loc::Reg(Reg::st(index))
    }

    /// Its position, where this is an x87 register.
    pub fn st_index(&self) -> Option<u32> {
        match self {
            Loc::Reg(register) => register.st_index(),
            _ => None,
        }
    }
}

pub use flag::Flag;

/// Conservative decoded effects of one selected operation.
///
/// Direct port of `qbopt.model.ir:Effects`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Effects {
    pub defs: Option<BTreeSet<iced_x86::Register>>,
    pub uses: Option<BTreeSet<iced_x86::Register>>,
    pub flags_written: Flag,
    pub flags_read: Flag,
    pub loads: Vec<Mem>,
    pub stores: Vec<Mem>,
    pub fp_stack: bool,
    pub memory_complete: bool,
}

impl Effects {
    pub fn no_effect() -> Self {
        Self {
            defs: Some(BTreeSet::new()),
            uses: Some(BTreeSet::new()),
            flags_written: Flag::NONE,
            flags_read: Flag::NONE,
            loads: Vec::new(),
            stores: Vec::new(),
            fp_stack: false,
            memory_complete: false,
        }
    }

    /// Python `Effects.touches_memory`.
    pub const fn touches_memory(&self) -> bool {
        !self.loads.is_empty() || !self.stores.is_empty()
    }
}

/// Python `NO_EFFECT`.
pub static NO_EFFECT: LazyLock<Effects> = LazyLock::new(Effects::no_effect);

/// Python `ANY_MEMORY`: a single unknown-width, unknown-address memory cell.
pub static ANY_MEMORY: LazyLock<Vec<Mem>> = LazyLock::new(|| vec![Mem::new(None, 0)]);

/// What a selected operation computes.
///
/// Direct port of `qbopt.model.ir:Operation`; the display spelling is the
/// Python `StrEnum` value, not a target opcode.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Operation {
    Move,
    Exchange,
    Address,
    Binary,
    Multiply,
    Divide,
    Compare,
    Unary,
    Funnel,
    Extend,
    Push,
    Pop,
    Leave,
    Fill,
    /// A string move, `movs`: cells from `ds:si` to `es:di`.
    Copy,
    Jump,
    Branch,
    Escape,
    Call,
    Return,
    Nothing,
    Restore,
    Data,
    FloatLoad,
    FloatStore,
    FloatArith,
    FloatArithPop,
    FloatUnary,
    Barrier,
}

impl Operation {
    pub const ALL: [Self; 29] = [
        Self::Move,
        Self::Exchange,
        Self::Address,
        Self::Binary,
        Self::Multiply,
        Self::Divide,
        Self::Compare,
        Self::Unary,
        Self::Funnel,
        Self::Extend,
        Self::Push,
        Self::Pop,
        Self::Leave,
        Self::Fill,
        Self::Copy,
        Self::Jump,
        Self::Branch,
        Self::Escape,
        Self::Call,
        Self::Return,
        Self::Nothing,
        Self::Restore,
        Self::Data,
        Self::FloatLoad,
        Self::FloatStore,
        Self::FloatArith,
        Self::FloatArithPop,
        Self::FloatUnary,
        Self::Barrier,
    ];

    /// The operation `spelled` as `as_str` spells it, as `x86.instr` does.
    pub fn named(spelled: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|op| op.as_str() == spelled)
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Move => "move",
            Self::Exchange => "xchg",
            Self::Address => "addr",
            Self::Binary => "binary",
            Self::Multiply => "mul",
            Self::Divide => "div",
            Self::Compare => "cmp",
            Self::Unary => "unary",
            Self::Funnel => "funnel",
            Self::Extend => "extend",
            Self::Push => "push",
            Self::Pop => "pop",
            Self::Leave => "leave",
            Self::Fill => "fill",
            Self::Copy => "copy",
            Self::Jump => "jump",
            Self::Branch => "branch",
            Self::Escape => "escape",
            Self::Call => "call",
            Self::Return => "ret",
            Self::Nothing => "nothing",
            Self::Restore => "restore",
            Self::Data => "data",
            Self::FloatLoad => "fload",
            Self::FloatStore => "fstore",
            Self::FloatArith => "farith",
            Self::FloatArithPop => "farithp",
            Self::FloatUnary => "funary",
            Self::Barrier => "barrier",
        }
    }
}

impl fmt::Display for Operation {
    fn fmt(
        &self,
        formatter: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Typed selected semantics for one instruction.
///
/// Direct port of `qbopt.model.ir:Semantics`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Semantics {
    pub op: Operation,
    pub name: Option<String>,
    pub dests: Vec<Loc>,
    pub sources: Vec<Loc>,
    pub target: Option<i64>,
    pub indirect: bool,
}

impl Semantics {
    /// The same instruction, its operands not necessarily spelled the same way
    /// (`Mem::same_place`).
    pub fn same_meaning(
        &self,
        other: &Self,
    ) -> bool {
        let same = |left: &[Loc], right: &[Loc]| {
            left.len() == right.len()
                && left.iter().zip(right).all(|pair| match pair {
                    (Loc::Mem(x), Loc::Mem(y)) => x.same_place(y),
                    (Loc::Address(x), Loc::Address(y)) => x.same_place(y),
                    (x, y) => x == y,
                })
        };
        self.op == other.op
            && self.name == other.name
            && self.target == other.target
            && self.indirect == other.indirect
            && same(&self.dests, &other.dests)
            && same(&self.sources, &other.sources)
    }

    pub fn new(op: Operation) -> Self {
        Self { op, name: None, dests: Vec::new(), sources: Vec::new(), target: None, indirect: false }
    }
}

/// Python `UNMODELLED`.
pub static UNMODELLED: LazyLock<Semantics> = LazyLock::new(|| Semantics::new(Operation::Barrier));

/// Python `RESTORE_IDIOM`.
pub static RESTORE_IDIOM: LazyLock<Semantics> = LazyLock::new(|| Semantics {
    op: Operation::Restore,
    name: Some("restore".to_owned()),
    dests: Vec::new(),
    sources: Vec::new(),
    target: None,
    indirect: false,
});

/// Python `TABLE_DATA`.
pub static TABLE_DATA: LazyLock<Semantics> = LazyLock::new(|| Semantics::new(Operation::Data));

/// The values one operand names: at most three (a cell's base, index and
/// selector), kept in the value itself, so that asking for them of every
/// operand of every instruction allocates nothing.
#[derive(Clone, Copy, Debug)]
pub struct Values {
    held: [Held; 3],
    len: usize,
}

impl std::ops::Deref for Values {
    type Target = [Held];

    fn deref(&self) -> &[Held] {
        &self.held[..self.len]
    }
}

impl PartialEq<Vec<Held>> for Values {
    fn eq(
        &self,
        other: &Vec<Held>,
    ) -> bool {
        **self == **other
    }
}

impl IntoIterator for Values {
    type Item = Held;
    type IntoIter = std::iter::Take<std::array::IntoIter<Held, 3>>;

    fn into_iter(self) -> Self::IntoIter {
        self.held.into_iter().take(self.len)
    }
}

/// Python `values`: every SSA value named by one selected operand.
pub fn values(where_: &Loc) -> Values {
    let nothing = Held { value: 0, width: 0 };
    let mut found = Values { held: [nothing; 3], len: 0 };
    let mut named = |held: &Held| {
        found.held[found.len] = *held;
        found.len += 1;
    };
    match where_ {
        Loc::Held(held) => named(held),
        Loc::Mem(memory) => {
            for held in [memory.base, memory.index, memory.selector].iter().flatten() {
                named(held);
            }
        }
        Loc::Reg(_) | Loc::Imm(_) | Loc::Address(_) => {}
    }
    found
}

/// Python `mapped`: replace every SSA value nested in one selected operand.
pub fn mapped<F>(
    where_: &Loc,
    mut made: F,
) -> Loc
where
    F: FnMut(&Held) -> Held,
{
    match where_ {
        Loc::Held(held) => Loc::Held(made(held)),
        Loc::Mem(memory) if memory.base.is_some() || memory.selector.is_some() || memory.index.is_some() => {
            let mut mapped = memory.clone();
            mapped.base = memory.base.as_ref().map(&mut made);
            mapped.index = memory.index.as_ref().map(&mut made);
            mapped.selector = memory.selector.as_ref().map(&mut made);
            Loc::Mem(mapped)
        }
        _ => where_.clone(),
    }
}

/// Python `restoring`: one wide value returned to the two original halves.
pub fn restoring(
    wide: Loc,
    low: Loc,
    high: Loc,
) -> Semantics {
    Semantics {
        op: Operation::Restore,
        name: Some("restore".to_owned()),
        dests: vec![low, high],
        sources: vec![wide],
        target: None,
        indirect: false,
    }
}

/// Python `modelled`.
pub fn modelled(semantics: &Semantics) -> bool {
    semantics.op != Operation::Barrier
}

/// Python `barrier`.
pub fn barrier(semantics: &Semantics) -> bool {
    semantics.op == Operation::Barrier
}

#[cfg(test)]
mod tests {
    use super::root;
    use super::*;

    /// An indexed access through a register that holds a value (`[ebp+edx-16]`,
    /// the frame register freed) is not a frame cell: relayout moved its
    /// displacement by the frame's hole and nib's dictionary lookups read 16
    /// bytes off (tests/run nib/flat_containers at -O2).
    #[test]
    fn a_literal_displacement_through_a_register_holding_a_value_is_not_in_the_frame() {
        let table = |base| Mem {
            through: iced_x86::Register::EBP,
            base,
            index: Some(Held { value: 2, width: 4 }),
            index_through: iced_x86::Register::EDX,
            ..Mem::new(Some(Addr::new(Space::Literal, -16)), 4)
        };
        assert!(table(None).in_frame(), "an indexed frame array is BP's");
        assert!(!table(Some(Held { value: 1, width: 4 })).in_frame(), "a base value is the register's");
    }

    #[test]
    fn root_normalises_every_sub_register_of_the_ax_pair() {
        // Port of tests/test_ir.
        // py::test_root_normalises_every_sub_register_of_the_ax_pair.
        for register in
            [iced_x86::Register::AL, iced_x86::Register::AH, iced_x86::Register::AX, iced_x86::Register::EAX]
        {
            assert_eq!(root(register), iced_x86::Register::EAX);
        }
    }

    /// isel keeps a based cell's displacement only in `offset`; leaving it out
    /// of equality made [p] and [p+2] one cell.
    #[test]
    fn two_addressless_cells_at_different_displacements_are_different_cells() {
        let at = |offset| Mem { base: Some(Held { value: 1, width: 2 }), offset, ..Mem::new(None, 2) };
        assert_ne!(at(0), at(2));
        let address = |offset| Address { offset, ..Address::new(None) };
        assert_ne!(address(0), address(2));
    }

    #[test]
    fn two_cells_reached_by_different_values_are_different_cells() {
        // Port of tests/test_ir.
        // py::test_two_cells_reached_by_different_values_are_different_cells.
        let mut left = Mem::new(None, 2);
        left.base = Some(Held { value: 1, width: 2 });
        left.through = iced_x86::Register::BX;
        let mut right = left.clone();
        right.base = Some(Held { value: 2, width: 2 });
        right.through = iced_x86::Register::SI;

        assert_ne!(left, right);
    }

    /// `==` left out how an operand is spelled (`through`, `offset` of a cell
    /// with an address, `disp_width`, `index_through`), so every caller
    /// that asked whether two operands are the same asked whether they are the
    /// same modulo their encoding: a cache of decoded instructions gave `mov
    /// es,[bx+2]` for `mov es,[si+2]`.
    #[test]
    fn how_a_memory_operand_is_spelled_is_part_of_its_identity() {
        let mut left = Mem::new(Some(Addr::new(Space::Segment, 4)), 2);
        left.base = Some(Held { value: 4, width: 2 });
        let hash = |memory: &Mem| {
            use std::hash::Hasher;
            let mut state = std::collections::hash_map::DefaultHasher::new();
            memory.hash(&mut state);
            state.finish()
        };
        for change in [
            |cell: &mut Mem| cell.through = iced_x86::Register::BX,
            |cell: &mut Mem| cell.offset = 6,
            |cell: &mut Mem| cell.disp_width = 2,
            |cell: &mut Mem| cell.index_through = iced_x86::Register::DI,
        ] {
            let mut right = left.clone();
            change(&mut right);
            assert_ne!(left, right);
            assert_ne!(hash(&left), hash(&right));
        }
        let address = |through| Address { through, ..Address::new(Some(Addr::new(Space::Segment, 4))) };
        assert_ne!(address(iced_x86::Register::BX), address(iced_x86::Register::SI));
        assert_eq!(left, left.clone());
    }

    #[test]
    fn values_finds_a_held_wherever_it_is() {
        // Port of tests/test_ir.py::test_values_finds_a_held_wherever_it_is.
        let held = Held { value: 7, width: 2 };
        assert_eq!(values(&Loc::Held(held)), vec![held]);

        let mut memory = Mem::new(None, 4);
        memory.base = Some(held);
        memory.index = Some(Held { value: 8, width: 2 });
        memory.selector = Some(Held { value: 9, width: 2 });
        assert_eq!(values(&Loc::Mem(memory)), vec![held, Held { value: 8, width: 2 }, Held { value: 9, width: 2 }],);
    }

    #[test]
    fn mapped_replaces_a_nested_base_and_leaves_the_rest() {
        // Port of tests/test_ir.
        // py::test_mapped_replaces_a_nested_base_and_leaves_the_rest.
        let mut memory = Mem::new(Some(Addr::new(Space::Far, 8)), 4);
        memory.base = Some(Held { value: 1, width: 2 });
        memory.selector = Some(Held { value: 2, width: 2 });
        memory.index = Some(Held { value: 3, width: 2 });
        memory.through = iced_x86::Register::BX;

        let Loc::Mem(mapped_memory) =
            mapped(&Loc::Mem(memory.clone()), |held| Held { value: held.value + 10, width: held.width })
        else {
            panic!("a memory operand remains memory");
        };
        assert_eq!(mapped_memory.base, Some(Held { value: 11, width: 2 }));
        assert_eq!(mapped_memory.index, Some(Held { value: 13, width: 2 }));
        assert_eq!(mapped_memory.selector, Some(Held { value: 12, width: 2 }));
        assert_eq!(mapped_memory.through, memory.through);
        assert_eq!(mapped_memory.addr, memory.addr);
    }

    #[test]
    fn restoring_and_modelled_barrier_keep_the_python_contract() {
        let restored = restoring(
            Loc::Held(Held { value: 1, width: 4 }),
            Loc::Held(Held { value: 2, width: 2 }),
            Loc::Held(Held { value: 3, width: 2 }),
        );
        assert_eq!(restored.op, Operation::Restore);
        assert_eq!(restored.name.as_deref(), Some("restore"));
        assert_eq!(restored.dests.len(), 2);
        assert!(modelled(&restored));
        assert!(barrier(&UNMODELLED));
        assert!(!modelled(&UNMODELLED));
    }

    #[test]
    fn restore_and_table_carry_their_own_operations() {
        // Port of tests/test_ir.
        // py::test_a_restore_and_a_table_carry_their_own_operations.
        assert_eq!(RESTORE_IDIOM.op, Operation::Restore);
        assert_eq!(TABLE_DATA.op, Operation::Data);
        for operation in Operation::ALL {
            let semantics = Semantics::new(operation);
            assert_eq!(modelled(&semantics), !barrier(&semantics));
        }
    }

    #[test]
    fn effects_and_any_memory_keep_the_python_field_contract() {
        assert_eq!(*NO_EFFECT, Effects::no_effect());
        assert!(!NO_EFFECT.touches_memory());
        assert_eq!(ANY_MEMORY.len(), 1);
        assert_eq!(ANY_MEMORY[0], Mem::new(None, 0));

        let effects = Effects {
            defs: None,
            uses: None,
            flags_written: Flag::CF.union(Flag::ZF),
            flags_read: Flag::PF,
            loads: vec![Mem::new(None, 2)],
            stores: Vec::new(),
            fp_stack: true,
            memory_complete: true,
        };
        assert!(effects.touches_memory());
        assert_eq!(effects.flags_written.bits(), Flag::CF.bits() | Flag::ZF.bits());
    }

    #[test]
    fn the_same_place_ignores_encoding_details_but_not_the_address() {
        let left = Address::new(Some(Addr::new(Space::Literal, 12)));
        let mut right = left.clone();
        right.through = iced_x86::Register::BX;
        right.index = iced_x86::Register::SI;
        right.scale = 4;
        right.offset = -8;
        right.disp_width = 2;
        assert!(left.same_place(&right) && left != right);
        let moved = Address::new(Some(Addr::new(Space::Literal, 14)));
        assert!(!left.same_place(&moved));
        assert!(Addr::new(Space::Frame, -2).direct());
        assert_eq!(Addr::new(Space::Frame, -2).plus(4).disp, 2);
    }
}

/// `Register_` is an int to Python, and prints as one.
fn register_repr(register: Register) -> String {
    (register as u32).to_string()
}

impl Operation {
    /// An x87 stack instruction.
    pub const fn is_x87(self) -> bool {
        matches!(
            self,
            Self::FloatLoad | Self::FloatStore | Self::FloatArith | Self::FloatArithPop | Self::FloatUnary
        )
    }

    /// The member name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Move => "MOVE",
            Self::Exchange => "EXCHANGE",
            Self::Address => "ADDRESS",
            Self::Binary => "BINARY",
            Self::Multiply => "MULTIPLY",
            Self::Divide => "DIVIDE",
            Self::Compare => "COMPARE",
            Self::Unary => "UNARY",
            Self::Funnel => "FUNNEL",
            Self::Extend => "EXTEND",
            Self::Push => "PUSH",
            Self::Pop => "POP",
            Self::Leave => "LEAVE",
            Self::Fill => "FILL",
            Self::Copy => "COPY",
            Self::Jump => "JUMP",
            Self::Branch => "BRANCH",
            Self::Escape => "ESCAPE",
            Self::Call => "CALL",
            Self::Return => "RETURN",
            Self::Nothing => "NOTHING",
            Self::Restore => "RESTORE",
            Self::Data => "DATA",
            Self::FloatLoad => "FLOAT_LOAD",
            Self::FloatStore => "FLOAT_STORE",
            Self::FloatArith => "FLOAT_ARITH",
            Self::FloatArithPop => "FLOAT_ARITH_POP",
            Self::FloatUnary => "FLOAT_UNARY",
            Self::Barrier => "BARRIER",
        }
    }
}

impl Repr for Operation {
    fn repr(&self) -> String {
        pyrepr::str_enum("Operation", self.name(), self.as_str())
    }
}

impl Repr for Reg {
    fn repr(&self) -> String {
        pyrepr::dataclass("Reg", &[("register", register_repr(self.register)), ("width", self.width.repr())])
    }
}

impl Repr for Held {
    fn repr(&self) -> String {
        pyrepr::dataclass("Held", &[("value", self.value.repr()), ("width", self.width.repr())])
    }
}

impl Repr for Imm {
    fn repr(&self) -> String {
        pyrepr::dataclass(
            "Imm",
            &[("value", self.value.repr()), ("width", self.width.repr()), ("address", self.address.repr())],
        )
    }
}

impl Repr for Address {
    fn repr(&self) -> String {
        pyrepr::dataclass(
            "Address",
            &[
                ("addr", self.addr.repr()),
                ("through", register_repr(self.through)),
                ("index", register_repr(self.index)),
                ("scale", self.scale.repr()),
                ("offset", self.offset.repr()),
                ("disp_width", self.disp_width.repr()),
            ],
        )
    }
}

impl Repr for Mem {
    fn repr(&self) -> String {
        pyrepr::dataclass(
            "Mem",
            &[
                ("addr", self.addr.repr()),
                ("width", self.width.repr()),
                ("through", register_repr(self.through)),
                ("offset", self.offset.repr()),
                ("disp_width", self.disp_width.repr()),
                ("base", self.base.repr()),
                ("stack_argument", self.stack_argument.repr()),
                ("selector", self.selector.repr()),
                ("index", self.index.repr()),
                ("scale", self.scale.repr()),
                ("index_through", register_repr(self.index_through)),
            ],
        )
    }
}

impl Repr for Loc {
    fn repr(&self) -> String {
        match self {
            Loc::Reg(one) => one.repr(),
            Loc::Mem(one) => one.repr(),
            Loc::Imm(one) => one.repr(),
            Loc::Address(one) => one.repr(),
            Loc::Held(one) => one.repr(),
        }
    }
}

impl Repr for Semantics {
    fn repr(&self) -> String {
        pyrepr::dataclass(
            "Semantics",
            &[
                ("op", self.op.repr()),
                ("name", self.name.repr()),
                ("dests", pyrepr::tuple(&self.dests)),
                ("sources", pyrepr::tuple(&self.sources)),
                ("target", self.target.repr()),
                ("indirect", self.indirect.repr()),
            ],
        )
    }
}

#[cfg(test)]
mod repr_tests {
    use super::*;

    /// Expected strings printed by Python's `repr` of the same values.
    #[test]
    fn reprs_match_python() {
        assert_eq!(
            Mem::new(None, 2).repr(),
            "Mem(addr=None, width=2, through=0, offset=0, disp_width=0, base=None, stack_argument=False, \
             selector=None, index=None, scale=1, index_through=0)"
        );
        assert_eq!(
            Address::new(None).repr(),
            "Address(addr=None, through=0, index=0, scale=1, offset=0, disp_width=0)"
        );
        assert_eq!(Held { value: 3, width: 2 }.repr(), "Held(value=3, width=2)");
        let semantics = Semantics {
            name: Some("mov".to_owned()),
            dests: vec![Loc::Reg(Reg { register: Register::AX, width: 2 })],
            sources: vec![Loc::Imm(Imm { value: 1, width: 2, address: None })],
            ..Semantics::new(Operation::Move)
        };
        assert_eq!(
            semantics.repr(),
            "Semantics(op=<Operation.MOVE: 'move'>, name='mov', dests=(Reg(register=21, width=2),), \
             sources=(Imm(value=1, width=2, address=None),), target=None, indirect=False)"
        );
    }
}
