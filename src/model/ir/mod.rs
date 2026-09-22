//! Direct port of Python's `qbopt.model.ir` machine-semantics vocabulary.
//!
//! This is deliberately distinct from the existing `MachineInstruction`
//! representation.  Python LIR carries selected semantics, source-byte
//! provenance, and value identities before allocation; those facts must not be
//! projected onto the newer generic selected-IR model.

use std::collections::BTreeSet;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::LazyLock;

pub use crate::objectfile::module::{Addr, Space};
use iced_x86::Register;

pub mod nodes;
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
/// Direct port of `qbopt.model.ir:Address`.  Its encoding fields are
/// deliberately excluded from equality and hashing, just as Python's
/// `compare=False` fields are.
#[derive(Clone, Debug)]
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

impl PartialEq for Address {
    fn eq(&self, other: &Self) -> bool {
        self.addr == other.addr
    }
}

impl Eq for Address {}

impl Hash for Address {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.addr.hash(state);
    }
}

/// A memory operand.
///
/// Direct port of `qbopt.model.ir:Mem`.  Encoding details (`through`,
/// `offset`, `disp_width`, and `index_through`) deliberately do not take part
/// in equality or hashing.  The logical address values do.
#[derive(Clone, Debug)]
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
}

impl Mem {
    pub const fn new(addr: Option<Addr>, width: u32) -> Self {
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
        }
    }
}

impl PartialEq for Mem {
    fn eq(&self, other: &Self) -> bool {
        self.addr == other.addr
            && self.width == other.width
            && self.base == other.base
            && self.stack_argument == other.stack_argument
            && self.selector == other.selector
            && self.index == other.index
            && self.scale == other.scale
    }
}

impl Eq for Mem {}

impl Hash for Mem {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.addr.hash(state);
        self.width.hash(state);
        self.base.hash(state);
        self.stack_argument.hash(state);
        self.selector.hash(state);
        self.index.hash(state);
        self.scale.hash(state);
    }
}

/// An x87 stack position relative to the current top.
///
/// Direct port of `qbopt.model.ir:St`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct St {
    pub index: u32,
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
    St(St),
    Held(Held),
}

pub use crate::analysis::flags::Flag;

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
    pub const ALL: [Self; 28] = [
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
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
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
    pub fn new(op: Operation) -> Self {
        Self {
            op,
            name: None,
            dests: Vec::new(),
            sources: Vec::new(),
            target: None,
            indirect: false,
        }
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

/// Python `values`: every SSA value named by one selected operand.
pub fn values(where_: &Loc) -> Vec<Held> {
    match where_ {
        Loc::Held(held) => vec![*held],
        Loc::Mem(memory) => [memory.base, memory.index, memory.selector]
            .into_iter()
            .flatten()
            .collect(),
        Loc::Reg(_) | Loc::Imm(_) | Loc::Address(_) | Loc::St(_) => Vec::new(),
    }
}

/// Python `mapped`: replace every SSA value nested in one selected operand.
pub fn mapped<F>(where_: &Loc, mut made: F) -> Loc
where
    F: FnMut(&Held) -> Held,
{
    match where_ {
        Loc::Held(held) => Loc::Held(made(held)),
        Loc::Mem(memory)
            if memory.base.is_some() || memory.selector.is_some() || memory.index.is_some() =>
        {
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
pub fn restoring(wide: Loc, low: Loc, high: Loc) -> Semantics {
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
    use super::*;
    use super::root;

    #[test]
    fn root_normalises_every_sub_register_of_the_ax_pair() {
        // Port of tests/test_ir.py::test_root_normalises_every_sub_register_of_the_ax_pair.
        for register in [
            iced_x86::Register::AL,
            iced_x86::Register::AH,
            iced_x86::Register::AX,
            iced_x86::Register::EAX,
        ] {
            assert_eq!(root(register), iced_x86::Register::EAX);
        }
    }

    #[test]
    fn two_cells_reached_by_different_values_are_different_cells() {
        // Port of tests/test_ir.py::test_two_cells_reached_by_different_values_are_different_cells.
        let mut left = Mem::new(None, 2);
        left.base = Some(Held { value: 1, width: 2 });
        left.through = iced_x86::Register::BX;
        let mut right = left.clone();
        right.base = Some(Held { value: 2, width: 2 });
        right.through = iced_x86::Register::SI;

        assert_ne!(left, right);
    }

    #[test]
    fn encoding_details_do_not_change_memory_identity() {
        let mut left = Mem::new(Some(Addr::new(Space::Segment, 4)), 2);
        left.base = Some(Held { value: 4, width: 2 });
        let mut right = left.clone();
        right.through = iced_x86::Register::BX;
        right.offset = 6;
        right.disp_width = 2;
        right.index_through = iced_x86::Register::DI;

        assert_eq!(left, right);
        let hash = |memory: &Mem| {
            let mut state = std::collections::hash_map::DefaultHasher::new();
            memory.hash(&mut state);
            state.finish()
        };
        assert_eq!(hash(&left), hash(&right));
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
        assert_eq!(
            values(&Loc::Mem(memory)),
            vec![
                held,
                Held { value: 8, width: 2 },
                Held { value: 9, width: 2 }
            ],
        );
    }

    #[test]
    fn mapped_replaces_a_nested_base_and_leaves_the_rest() {
        // Port of tests/test_ir.py::test_mapped_replaces_a_nested_base_and_leaves_the_rest.
        let mut memory = Mem::new(Some(Addr::new(Space::Far, 8)), 4);
        memory.base = Some(Held { value: 1, width: 2 });
        memory.selector = Some(Held { value: 2, width: 2 });
        memory.index = Some(Held { value: 3, width: 2 });
        memory.through = iced_x86::Register::BX;

        let Loc::Mem(mapped_memory) = mapped(&Loc::Mem(memory.clone()), |held| Held {
            value: held.value + 10,
            width: held.width,
        }) else {
            panic!("a memory operand remains memory");
        };
        assert_eq!(
            mapped_memory.base,
            Some(Held {
                value: 11,
                width: 2
            })
        );
        assert_eq!(
            mapped_memory.index,
            Some(Held {
                value: 13,
                width: 2
            })
        );
        assert_eq!(
            mapped_memory.selector,
            Some(Held {
                value: 12,
                width: 2
            })
        );
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
        // Port of tests/test_ir.py::test_a_restore_and_a_table_carry_their_own_operations.
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
        assert_eq!(
            effects.flags_written.bits(),
            Flag::CF.bits() | Flag::ZF.bits()
        );
    }

    #[test]
    fn address_identity_ignores_encoding_details_but_not_the_address() {
        let left = Address::new(Some(Addr::new(Space::Literal, 12)));
        let mut right = left.clone();
        right.through = iced_x86::Register::BX;
        right.index = iced_x86::Register::SI;
        right.scale = 4;
        right.offset = -8;
        right.disp_width = 2;
        assert_eq!(left, right);
        assert!(Addr::new(Space::Frame, -2).direct());
        assert_eq!(Addr::new(Space::Frame, -2).plus(4).disp, 2);
    }
}

