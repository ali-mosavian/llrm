//! Post-increment `(An)+` and pre-decrement `-(An)` accesses: the shared
//! `CombineToPostIndexedLoadStore` and `CombineToPreIndexedLoadStore` on, with
//! m68k's legality and offsets, and the writeback instructions they select to.
//!
//! # How it works
//! [`is_indexed_legal`] and [`indexed_address_parts`] say which accesses may be
//! indexed and which add or subtract updates the address (`isIndexedLoadLegal`,
//! `getPostIndexedAddressParts`). The combine then makes `LoadPost`, `LoadPre`,
//! `StorePost` or `StorePre` nodes, which [`select`] turns into [`Writeback`]
//! machine nodes: the pin's `MOV32do` and its relatives, plus a second result,
//! the updated base, tied to the base operand.
//!
//! DEVIATE: the pin models `(An)+` and `-(An)` with one result and no tied
//! base, its `SelectARIPI` and `SelectARIPD` never match, and it sets no
//! indexed action. Reason: best of gcc and clang is the goal, and only gcc
//! indexes. The writeback instructions take their flags, implicit defs and
//! classes from the pin's description of the instruction they print as. Exit:
//! none.
//!
//! # See also
//! `ARM::LDR_POST_IMM` and `AArch64::LDRXpost`, which are separate writeback
//! instructions in the same way.

use std::sync::LazyLock;

use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::mvt::EVT;
use llrm_codegen::backend::selectiondag::{MemInfo, NodeData, SDValue as Value, SelectionDAG};
use llrm_dagisel::dag::{Dag, SDValue};
use llrm_dagisel::instr::{InstrDesc, OperandDesc};
use llrm_dagisel::interp::Outcome;

use super::asm::{AsmOperand, asm};
use super::enc::{Piece, encoding};
use super::generated::instrinfo;
use super::generated::m68k_m68000::MACHINE_OPCODES;

/// Codes of the writeback instructions, above every code of the pin's table.
const FIRST: u16 = 0xF000;

/// A writeback instruction: the pin's instruction it prints as, and how.
struct Writeback {
    /// The name of the instruction in the machine function.
    name: &'static str,
    /// The pin's instruction it prints as once the base is allocated.
    prints_as: &'static str,
    load: bool,
    /// Pre-decrement, else post-increment.
    pre: bool,
    /// The operand of the printed instruction the updated base replaces: the
    /// one after the results it defines.
    drop: usize,
    /// The pin's instruction its description comes from.
    base: &'static str,
    /// A memory source of an arithmetic instruction (`ADD32dj`), not a move.
    source: bool,
}

macro_rules! writeback {
    ($($name:literal $load:literal $pre:literal),* $(,)?) => {
        &[$(Writeback {
            name: concat!($name, "_WB"),
            prints_as: $name,
            load: $load,
            pre: $pre,
            drop: if $load { 1 } else { 0 },
            base: $name,
            source: false,
        }),*]
    };
}

/// Byte, word and long into a data register, long into an address register;
/// from a data register, long from an address register.
const MOVES: &[Writeback] = writeback![
    "MOV8do" true false, "MOV16do" true false, "MOV32do" true false, "MOV32ao" true false,
    "MOV8de" true true, "MOV16de" true true, "MOV32de" true true, "MOV32ae" true true,
    "MOV8od" false false, "MOV16od" false false, "MOV32od" false false, "MOV32oa" false false,
    "MOV8ed" false true, "MOV16ed" false true, "MOV32ed" false true, "MOV32ea" false true,
];

/// Every writeback: the moves, and one for each pin instruction that reads its
/// source through an address register (`ADD32dj`: `add.l (%a0), %d0`), which
/// the pin has no post-increment twin of. The encoding has the mode field of
/// `(An)` and the twin has that of `(An)+`; nothing else differs.
static WRITEBACKS: LazyLock<Vec<Writeback>> = LazyLock::new(|| {
    let mut all: Vec<Writeback> =
        MOVES.iter().map(|one| Writeback { name: one.name, prints_as: one.prints_as, ..*one }).collect();
    for (_, base) in MACHINE_OPCODES.iter() {
        let Some(stem) = base.strip_suffix("dj").filter(|stem| !stem.starts_with("MOV")) else { continue };
        let twin = leak(format!("{stem}do"));
        if code_of_pin(twin).is_some() || !has_register_indirect_source(base) {
            continue;
        }
        let defs = instrinfo::desc(code_of_pin(base).expect("listed")).map_or(0, |desc| usize::from(desc.num_defs));
        let base: &'static str = base;
        all.push(Writeback {
            name: leak(format!("{twin}_WB")),
            prints_as: twin,
            load: true,
            pre: false,
            drop: defs,
            base,
            source: true,
        });
    }
    all
});

fn leak(text: String) -> &'static str {
    Box::leak(text.into_boxed_str())
}

/// Whether the pin's `name` reads its last operand as `(An)` and encodes that
/// with the mode field `(An)` has.
fn has_register_indirect_source(name: &str) -> bool {
    let printed = asm(name).is_some_and(|(_, _, operands)| {
        operands
            .iter()
            .any(
                |one| one.method.starts_with("printARI")
                    && one.method.ends_with("Mem")
                    && !one.method.starts_with("printARID")
                    && !one.method.starts_with("printARII")
                    && !one.method.starts_with("printARIP"),
            )
    });
    printed && indirect_mode(name).is_some() && instrinfo::desc(code_of_pin(name).unwrap_or(0)).is_some()
}

/// The piece of `name`'s encoding that holds the effective address mode of
/// `(An)`: 3 bits at 3 with value 2.
fn indirect_mode(name: &str) -> Option<usize> {
    encoding(name)?.2.iter().position(|piece| matches!(piece, Piece::Const { lo: 3, width: 3, value: 2 }))
}

fn code_of_pin(name: &str) -> Option<u16> {
    MACHINE_OPCODES.iter().find(|(_, one)| *one == name).map(|(code, _)| *code)
}

/// The machine code of the instruction named `name`, the pin's or a writeback.
pub fn code_of(name: &str) -> Option<u16> {
    WRITEBACKS.iter().position(|one| one.name == name).map(|at| FIRST + at as u16).or_else(|| code_of_pin(name))
}

/// The name of machine code `code`.
pub fn name_of(code: u32) -> Option<&'static str> {
    match u16::try_from(code).ok()?.checked_sub(FIRST) {
        Some(at) => WRITEBACKS.get(usize::from(at)).map(|one| one.name),
        None => MACHINE_OPCODES.iter().find(|(value, _)| u32::from(*value) == code).map(|(_, name)| *name),
    }
}

/// The writeback that takes a memory source through `(An)+` in place of the
/// register source of `user` (`ADD32dr` is `ADD32do_WB`).
pub fn source_twin(user: &str) -> Option<&'static str> {
    // the register source is any register (`r`) or a data register (`d`)
    let stem = user.strip_suffix('r').or_else(|| user.strip_suffix('d'))?;
    WRITEBACKS.iter().find(|one| one.source && one.prints_as.strip_suffix('o') == Some(stem)).map(|one| one.name)
}

/// The instructions that take a source through `(An)+`, by the name they print
/// as.
pub fn source_rows() -> impl Iterator<Item = &'static str> {
    WRITEBACKS.iter().filter(|one| one.source).map(|one| one.prints_as)
}

/// The width in bits of a post-increment load into a data register.
pub fn post_load_bits(name: &str) -> Option<&'static str> {
    let one = WRITEBACKS.iter().find(|one| one.name == name)?;
    let move_to_data = !one.source && one.load && !one.pre && one.prints_as.ends_with("do");
    move_to_data.then(|| one.prints_as.trim_start_matches("MOV").trim_end_matches("do"))
}

/// The instruction a writeback prints as, with the operand to drop (the
/// updated base, which the tied base operand already names once allocated).
pub fn printed_as(name: &str) -> Option<(&'static str, usize)> {
    let one = WRITEBACKS.iter().find(|one| one.name == name)?;
    Some((one.prints_as, one.drop))
}

static DESCS: LazyLock<Vec<InstrDesc>> = LazyLock::new(|| {
    WRITEBACKS
        .iter()
        .map(|one| {
            let base = instrinfo::desc(code_of_pin(one.base).expect("a writeback of a known instruction"))
                .expect("a described instruction");
            let mut operands = Vec::new();
            if one.source {
                // the results, the updated base, the sources, then the base:
                // the base is the pin's last operand
                let last = base.operands.len() - 1;
                let register = OperandDesc { kind: 2, tied_to: -1, ..base.operands[last] };
                let used = OperandDesc { tied_to: one.drop as i8, ..register };
                operands.extend_from_slice(&base.operands[..one.drop]);
                operands.push(register);
                operands.extend_from_slice(&base.operands[one.drop..last]);
                operands.push(used);
            } else {
                // the memory operand's register is the base: a use tied to the
                // new base, which is a result of the same class
                let memory = if one.load { 1 } else { 0 };
                let register = OperandDesc { kind: 2, tied_to: -1, ..base.operands[memory] };
                let new_base = (if one.load { 1 } else { 0 }) as i8;
                let used = OperandDesc { tied_to: new_base, ..register };
                if one.load {
                    operands.extend([base.operands[0], register, used]);
                } else {
                    operands.extend([register, used, base.operands[1]]);
                }
            }
            InstrDesc {
                num_operands: operands.len() as u8,
                num_defs: base.num_defs + 1,
                operands: Box::leak(operands.into_boxed_slice()),
                ..*base
            }
        })
        .collect()
});

/// The text of a source twin (`ADD32do`): the pin's row with `(An)+` where it
/// has `(An)`.
pub fn source_asm(name: &str) -> Option<&'static (&'static str, &'static str, &'static [AsmOperand])> {
    static ROWS: LazyLock<Vec<(&'static str, &'static str, &'static [AsmOperand])>> = LazyLock::new(|| {
        let mut rows: Vec<_> = WRITEBACKS
            .iter()
            .filter(|one| one.source)
            .map(|one| {
                let &(_, text, operands) = asm(one.base).expect("a printed row");
                let kept: Vec<AsmOperand> = operands
                    .iter()
                    .map(|one| AsmOperand {
                        name: one.name,
                        at: one.at,
                        method: if one.method.starts_with("printARI") && one.method.ends_with("Mem") {
                            leak(one.method.replacen("printARI", "printARIPI", 1))
                        } else {
                            one.method
                        },
                    })
                    .collect();
                (one.prints_as, text, &*Box::leak(kept.into_boxed_slice()))
            })
            .collect();
        rows.sort_by_key(|row| row.0);
        rows
    });
    ROWS.binary_search_by_key(&name, |row| row.0).ok().map(|at| &ROWS[at])
}

/// The encoding of a source twin: the pin's with the mode of `(An)+`.
pub fn source_encoding(name: &str) -> Option<(u16, &'static [Piece])> {
    static ROWS: LazyLock<Vec<(&'static str, u16, &'static [Piece])>> = LazyLock::new(|| {
        let mut rows: Vec<_> = WRITEBACKS
            .iter()
            .filter(|one| one.source)
            .map(|one| {
                let &(_, width, pieces) = encoding(one.base).expect("an encoded row");
                let mut pieces = pieces.to_vec();
                let at = indirect_mode(one.base).expect("the mode of (An)");
                pieces[at] = Piece::Const { lo: 3, width: 3, value: 3 };
                (one.prints_as, width, &*Box::leak(pieces.into_boxed_slice()))
            })
            .collect();
        rows.sort_by_key(|row| row.0);
        rows
    });
    ROWS.binary_search_by_key(&name, |row| row.0).ok().map(|at| (ROWS[at].1, ROWS[at].2))
}

/// The description of machine code `code`: the pin's, or a writeback's.
pub fn desc(code: u16) -> Option<&'static InstrDesc> {
    match code.checked_sub(FIRST) {
        Some(at) => DESCS.get(usize::from(at)),
        None => instrinfo::desc(code),
    }
}

/// The bytes an access moves, when it is one the writeback forms cover: a plain
/// load, or a store of a whole value, of a byte, word or long.
fn width(
    dag: &SelectionDAG,
    access: Value,
) -> Option<i64> {
    let NodeData::Mem(MemInfo { mem_vt, ext: None, .. }) = dag.node(access).data else { return None };
    if dag.opcode(access) == ISD::Store && dag.value_type(dag.operand(access, 1)) != mem_vt {
        return None;
    }
    [(EVT::I8, 1), (EVT::I16, 2), (EVT::I32, 4)].into_iter().find(|(vt, _)| *vt == mem_vt).map(|(_, bytes)| bytes)
}

/// `isIndexedLoadLegal` and `isIndexedStoreLegal`.
pub fn is_indexed_legal(vt: EVT) -> bool {
    matches!(vt, EVT::I8 | EVT::I16 | EVT::I32)
}

/// `getPostIndexedAddressParts` and `getPreIndexedAddressParts`: the address
/// moves by the access width, up after the access or down before it.
pub fn indexed_address_parts(
    dag: &SelectionDAG,
    access: Value,
    address: Value,
    post: bool,
) -> Option<(Value, i64)> {
    let bytes = width(dag, access)?;
    let base = dag.operand(address, 0);
    let amount = dag.operand(address, 1);
    // the constant is the raw bits of its type
    let shift = 128 - dag.value_type(amount).size_in_bits();
    let step = (((dag.constant(amount)? << shift) as i128) >> shift) as i64;
    let signed = if dag.opcode(address) == ISD::Sub { -step } else { step };
    (signed == if post { bytes } else { -bytes }).then_some((base, signed))
}

fn code(
    bytes: i64,
    load: bool,
    pre: bool,
    address_register: bool,
) -> Option<u16> {
    let wanted = |one: &&Writeback| {
        let long = one.prints_as.starts_with("MOV32");
        let word = one.prints_as.starts_with("MOV16");
        let narrow = match bytes {
            1 => !long && !word,
            2 => word,
            _ => long,
        };
        let letters = &one.prints_as[one.prints_as.len() - 2..];
        let to_address = letters.starts_with('a') || letters.ends_with('a');
        !one.source && one.load == load && one.pre == pre && narrow && to_address == address_register
    };
    WRITEBACKS.iter().position(|one| wanted(&one)).map(|at| FIRST + at as u16)
}

/// Selects an indexed load or store to its writeback instruction.
pub fn select(
    adapter: &mut llrm_codegen::backend::selectiondag::dagisel_adapter::DagisAdapter<'_>,
    node: u32,
) -> Option<Outcome> {
    let here = Value { node, res: 0 };
    let (load, pre) = match adapter.dag.opcode(here) {
        ISD::LoadPost => (true, false),
        ISD::LoadPre => (true, true),
        ISD::StorePost => (false, false),
        ISD::StorePre => (false, true),
        _ => return None,
    };
    let NodeData::Mem(info) = adapter.dag.node(here).data else { return None };
    let bytes = [(EVT::I8, 1), (EVT::I16, 2), (EVT::I32, 4)].into_iter().find(|(vt, _)| *vt == info.mem_vt)?.1;
    let at = |index| SDValue { node, res: index };
    let operand = |index| adapter.operand(node, index);
    // loads read (chain, base, offset), stores (chain, value, base, offset)
    let (chain, base) = if load { (operand(0), operand(1)) } else { (operand(0), operand(2)) };
    let (selected, vts, operands) = if load {
        (
            code(bytes, true, pre, false)?,
            vec![adapter.value_type(at(0)), adapter.value_type(at(1)), adapter.value_type(at(2))],
            vec![base, chain],
        )
    } else {
        (
            code(bytes, false, pre, false)?,
            vec![adapter.value_type(at(0)), adapter.value_type(at(1))],
            vec![base, operand(1), chain],
        )
    };
    adapter.morph_node_to(node, selected, &vts, &operands);
    Some(Outcome::Selected)
}
