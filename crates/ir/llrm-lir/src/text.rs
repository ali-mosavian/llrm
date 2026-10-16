//! One instruction's operands and semantics as text: what every LIR dump,
//! check message and test prints (the body, block and stage layout is
//! `llrm-core`'s `lirtext`).
//!
//! # How it works
//! `render` gives an instruction as a destination, an opcode and operands.
//! The opcode carries the type of the operation as MIR spells one (`sub.i16`,
//! `fld.f64`),
//! so operands do not repeat it; a source of another width says so (`t10:i8`).
//! Virtual registers are named by their source name or `t12`; real ones are
//! named by width (`al`, `ax`, `eax`, `st0`); a virtual register placed in a
//! real one is `t12@ax`. Memory is `[es:t3@bx+t4*4+8]`: segment, base, index
//! times scale, displacement; a frame cell is a slot (`[slot0]`, `[slot0+2]`)
//! where the caller names slots, else `[bp-4]`. A move to memory is `store`,
//! from memory `load`, and a memory destination is the first operand, not a
//! result. Immediates are decimal, a mask hex, an address `&place`. Branches
//! and jumps name their blocks (`jl B5 else B4`, `switch v3 [0..2->B5,
//! 3->B6]`), a call its callee and register arguments.
//!
//! ```text
//! t11     = sub.i32     t2, 2
//!           cmp.i32     t46, 5
//!           jl         B5 else B4
//! t13@eax = call       fib(t12@ax)
//!           store.i16   [slot0], t47
//! ```
//!
//! # Used by
//! `lirtext`, which lays instructions out in blocks; `Display` for `Loc` and
//! `Semantics`.

use std::fmt;

use llrm_omf::module::{Addr, Space};

use crate::registers::RegId;
use crate::{AddressRef, Held, Imm, Loc, Mem, Operation, Reg, Semantics};

/// What the text of an instruction needs from outside it.
pub struct Syntax<'a> {
    /// A block's name, by its address.
    pub block: &'a dyn Fn(i64) -> String,
    /// A value's name, by its number: its source name, else `t12`.
    pub value: &'a dyn Fn(u32) -> String,
    /// A frame cell as the slot it lies in (`slot0`, `slot0+2`), where the
    /// caller has named the frame's slots.
    pub frame: &'a dyn Fn(&Addr) -> Option<String>,
    /// Where a call goes, and whether far: not in the instruction.
    pub callee: Option<(&'a str, bool)>,
    /// The values a call or return takes in registers, as `v12@ax`.
    pub arguments: &'a [String],
    /// Where a branch goes when it is not taken.
    pub otherwise: Option<String>,
}

impl Syntax<'_> {
    /// Names blocks by address and says nothing else.
    pub fn plain() -> Syntax<'static> {
        Syntax {
            block: &|at| format!("B@{at}"),
            value: &|number| format!("t{number}"),
            frame: &|_| None,
            callee: None,
            arguments: &[],
            otherwise: None,
        }
    }
}

/// An instruction in three parts, so a block can set them in columns.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rendered {
    pub dest: String,
    pub op: String,
    pub operands: String,
}

impl fmt::Display for Rendered {
    fn fmt(
        &self,
        formatter: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        if !self.dest.is_empty() {
            write!(formatter, "{} = ", self.dest)?;
        }
        formatter.write_str(&self.op)?;
        if !self.operands.is_empty() {
            write!(formatter, " {}", self.operands)?;
        }
        Ok(())
    }
}

/// A real register, by its own name: `al`, `ax`, `eax`.
pub fn register(register: RegId) -> String {
    format!("{register:?}").to_lowercase()
}

/// A width as a type and its bits, as MIR prints one: `i8`, `i16`, `i32`,
/// `i64`, and `f32`, `f64`, `f80` where the operation is a float one.
pub fn bits(
    width: u32,
    float: bool,
) -> String {
    match width {
        0 => "?".to_owned(),
        bytes => format!("{}{}", if float { 'f' } else { 'i' }, bytes * 8),
    }
}

/// A value by its name (`t12`, `n`), or `t12@ax` where it is placed.
pub fn held(
    value: Held,
    placed: RegId,
    name: &dyn Fn(u32) -> String,
) -> String {
    let named = name(value.value);
    if placed == RegId::None { named } else { format!("{named}@{}", register(placed)) }
}

/// A displacement with its sign, none for zero.
fn displacement(
    value: i64,
    decimal: bool,
) -> String {
    match (value, decimal) {
        (0, _) => String::new(),
        (_, true) => format!("{value:+}"),
        (_, false) if value < 0 => format!("-{:#x}", value.unsigned_abs()),
        (_, false) => format!("+{value:#x}"),
    }
}

/// The place an address names, and whether its displacement reads in decimal
/// (a frame offset) rather than hex (an offset into an object).
fn place(addr: &Addr) -> (String, bool) {
    let base = |register| {
        if register == iced_x86::Register::None { String::new() } else { crate::text::register(RegId::new(register)) }
    };
    match addr.space {
        Space::Frame => ("bp".to_owned(), true),
        Space::Stack => ("stack".to_owned(), true),
        Space::Segment => (format!("seg{}", addr.index), false),
        Space::External => (format!("ext{}", addr.index), false),
        Space::Group => (format!("grp{}", addr.index), false),
        Space::Literal => (String::new(), false),
        Space::Far => (base(addr.base), false),
    }
}

/// An address as a value: `&seg1+0x10`.
fn address(address: &AddressRef) -> String {
    let (name, decimal) = address.addr.as_ref().map_or_else(|| (String::new(), false), place);
    let reached =
        if address.through == RegId::None { String::new() } else { format!("+{}", register(address.through)) };
    let indexed = if address.index_through == RegId::None {
        String::new()
    } else {
        format!("+{}*{}", register(address.index_through), address.scale)
    };
    let offset = address.addr.as_ref().map_or(address.offset, |addr| addr.disp);
    format!("&{name}{reached}{indexed}{}", displacement(offset, decimal))
}

/// A memory operand.
pub fn memory(
    cell: &Mem,
    syntax: &Syntax<'_>,
) -> String {
    let slot = cell.addr.as_ref().filter(|addr| addr.space == Space::Frame).and_then(|addr| (syntax.frame)(addr));
    let (name, decimal) = match (&slot, cell.addr.as_ref()) {
        (Some(slot), _) => (slot.clone(), true),
        (None, Some(addr)) => place(addr),
        (None, None) => (String::new(), true),
    };
    let segment = match (&cell.selector, cell.addr.as_ref()) {
        (Some(selector), _) => format!("{}:", held(*selector, RegId::None, syntax.value)),
        (None, Some(addr)) if addr.segment != iced_x86::Register::None => {
            format!("{}:", register(RegId::new(addr.segment)))
        }
        _ => String::new(),
    };
    let mut terms: Vec<String> = Vec::new();
    if !name.is_empty() {
        terms.push(name);
    }
    let framed = cell.addr.as_ref().is_some_and(|addr| addr.space == Space::Frame);
    match (&cell.base, cell.through) {
        (Some(base), through) => terms.push(held(*base, through, syntax.value)),
        (None, through) if through != RegId::None && !framed => terms.push(register(through)),
        _ => {}
    }
    let scaled = |text: String| if cell.scale == 1 { text } else { format!("{text}*{}", cell.scale) };
    match (&cell.index, cell.index_through) {
        (Some(index), through) => terms.push(scaled(held(*index, through, syntax.value))),
        (None, through) if through != RegId::None => terms.push(scaled(register(through))),
        _ => {}
    }
    // A slot name already says where the cell is; its offset is in the frame
    // line.
    let offset = if slot.is_some() { 0 } else { cell.addr.as_ref().map_or(cell.offset, |addr| addr.disp) };
    let mut inside = terms.join("+");
    let disp = displacement(offset, decimal);
    if inside.is_empty() && offset != 0 {
        inside = disp.trim_start_matches('+').to_owned();
    } else {
        inside += &disp;
    }
    if inside.is_empty() {
        inside.push('0');
    }
    format!("[{segment}{inside}]")
}

/// An immediate: decimal, or hex at its width where `mask`.
pub fn immediate(
    value: &Imm,
    mask: bool,
) -> String {
    if let Some(addr) = &value.address {
        return address(&AddressRef { addr: Some(*addr), ..AddressRef::new(None) });
    }
    if !mask || value.value.unsigned_abs() < 10 {
        return value.value.to_string();
    }
    let width = value.width * 8;
    let kept = if (1..64).contains(&width) { value.value & ((1_i64 << width) - 1) } else { value.value };
    format!("{kept:#x}")
}

/// The width an operand has, where it has one.
fn width_of(operand: &Loc) -> Option<u32> {
    match operand {
        Loc::Reg(Reg { width, .. }) | Loc::Held(Held { width, .. }) | Loc::Imm(Imm { width, .. }) => {
            Some(*width).filter(|width| *width != 0)
        }
        Loc::Mem(cell) => Some(cell.width).filter(|width| *width != 0),
        Loc::Address(_) => None,
    }
}

/// A destination.
fn defined(
    operand: &Loc,
    syntax: &Syntax<'_>,
) -> String {
    used(operand, None, false, syntax)
}

/// A source. Its width follows only where it is not the instruction's `own`.
fn used(
    operand: &Loc,
    own: Option<u32>,
    mask: bool,
    syntax: &Syntax<'_>,
) -> String {
    let differs = || width_of(operand).is_some() && width_of(operand) != own && own.is_some();
    let width = || format!(":{}", bits(width_of(operand).unwrap_or_default(), false));
    match operand {
        Loc::Reg(one) => match one.st_index() {
            Some(index) => format!("st{index}"),
            None => register(one.register),
        },
        Loc::Held(value) if differs() => format!("{}{}", (syntax.value)(value.value), width()),
        Loc::Held(value) => (syntax.value)(value.value),
        Loc::Imm(value) if differs() => format!("{}{}", immediate(value, mask), width()),
        Loc::Imm(value) => immediate(value, mask),
        Loc::Mem(cell) if differs() => format!("{}{}", memory(cell, syntax), width()),
        Loc::Mem(cell) => memory(cell, syntax),
        Loc::Address(one) => address(one),
    }
}

/// Operations whose immediate is a bit pattern.
fn masks(name: &str) -> bool {
    matches!(name, "and" | "or" | "xor" | "test" | "not")
}

/// The blocks of a jump table as `0..2->B5, 3->B6`, runs of one block joined.
fn table(
    blocks: &[i64],
    block: &dyn Fn(i64) -> String,
) -> String {
    let mut runs: Vec<(usize, usize, i64)> = Vec::new();
    for (index, at) in blocks.iter().enumerate() {
        match runs.last_mut() {
            Some((_, last, to)) if *to == *at && *last + 1 == index => *last = index,
            _ => runs.push((index, index, *at)),
        }
    }
    runs.iter()
        .map(|(first, last, at)| {
            let name = block(*at);
            if first == last { format!("{first}->{name}") } else { format!("{first}..{last}->{name}") }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Whether an operation's opcode names a width: not a transfer of control.
fn sized(op: Operation) -> bool {
    !matches!(
        op,
        Operation::Jump
            | Operation::Branch
            | Operation::Call
            | Operation::Return
            | Operation::Nothing
            | Operation::Leave
            | Operation::Restore
            | Operation::Data
            | Operation::Barrier
            | Operation::Escape
    )
}

/// `what` as a destination, an opcode and operands.
pub fn render(
    what: &Semantics,
    syntax: &Syntax<'_>,
) -> Rendered {
    let float = matches!(
        what.op,
        Operation::FloatLoad
            | Operation::FloatStore
            | Operation::FloatArith
            | Operation::FloatArithPop
            | Operation::FloatUnary
    ) && !what.name.as_deref().is_some_and(|name| name.starts_with("fi"));
    let stored = what.op == Operation::Move && matches!(what.dests.first(), Some(Loc::Mem(_)));
    let loaded = what.op == Operation::Move && what.sources.iter().any(|one| matches!(one, Loc::Mem(_)));
    let name = match (what.name.as_deref().filter(|name| !name.is_empty()), stored, loaded) {
        (Some("mov"), true, _) => "store",
        (Some("mov"), false, true) => "load",
        (Some(name), ..) => name,
        (None, ..) => what.op.as_str(),
    };
    let wide = what
        .dests
        .iter()
        .chain(&what.sources)
        .find_map(
            |one| match one {
                Loc::Mem(cell) if float => Some(cell.width),
                _ => None,
            },
        );
    let own = wide.or_else(|| what.dests.first().or(what.sources.first()).and_then(width_of));
    let mask = masks(name);
    let (memory_dests, register_dests): (Vec<&Loc>, Vec<&Loc>) =
        what.dests.iter().partition(|one| matches!(one, Loc::Mem(_)));
    let mut operands: Vec<String> = memory_dests.iter().map(|one| used(one, own, false, syntax)).collect();
    operands
        .extend(what.sources.iter().filter(|one| !memory_dests.contains(one)).map(|one| used(one, own, mask, syntax)));
    let dest = register_dests.iter().map(|one| defined(one, syntax)).collect::<Vec<_>>().join(", ");
    let opcode = |name: &str| match own.filter(|_| sized(what.op)) {
        Some(width) => format!("{name}.{}", bits(width, float)),
        None => name.to_owned(),
    };
    let target = |at: Option<i64>| at.map_or_else(|| "?".to_owned(), |at| (syntax.block)(at));
    let (op, operands) = match what.op {
        Operation::Branch => {
            let to = target(what.target);
            let text = match &syntax.otherwise {
                Some(otherwise) => format!("{to} else {otherwise}"),
                None => to,
            };
            (name.to_owned(), text)
        }
        Operation::Jump if what.indirect && !what.table.is_empty() => {
            ("switch".to_owned(), format!("{} [{}]", operands.join(", "), table(&what.table, syntax.block)))
        }
        Operation::Jump if what.indirect => (name.to_owned(), operands.join(", ")),
        Operation::Jump => (name.to_owned(), target(what.target)),
        Operation::Call => {
            let (callee, far) = match (&syntax.callee, what.indirect) {
                (_, true) => (operands.join(", "), false),
                (Some((callee, far)), false) => ((*callee).to_owned(), *far),
                (None, false) => ("?".to_owned(), false),
            };
            let far = if far { "far " } else { "" };
            (name.to_owned(), format!("{far}{callee}({})", syntax.arguments.join(", ")))
        }
        Operation::Return => {
            let all = operands.iter().chain(syntax.arguments).cloned().collect::<Vec<_>>();
            (name.to_owned(), all.join(", "))
        }
        _ => (opcode(name), operands.join(", ")),
    };
    Rendered { dest, op, operands }
}

impl fmt::Display for Loc {
    fn fmt(
        &self,
        formatter: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        formatter.write_str(&defined(self, &Syntax::plain()))
    }
}

impl fmt::Display for Semantics {
    fn fmt(
        &self,
        formatter: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        write!(formatter, "{}", render(self, &Syntax::plain()))
    }
}
