//! Read-only tables kept in the code segment of the procedure that reads them.
//!
//! # How it works
//! Where a target's data segment is scarce and its code segments are not
//! (real mode: DS is DGROUP), `TargetLoweringObjectFile::getSectionForConstant`
//! puts a constant in the text section. [`placed`] does that for the tables of
//! a module: a table moves into the code, after the one procedure that reads
//! it, when
//!
//! - every reference to it is a load (an operand read, a table jump's cell): no
//!   store, no `lea`, no `offset`, nothing inline or in any data names it, so
//!   no pointer to it exists and a pointer into DS could never reach code;
//! - one procedure holds all the references, so the table is in the same
//!   segment as each reader whether the object has one code segment or one per
//!   procedure;
//! - its datums are bytes, labels, alignment and near pointers.
//!
//! Each reference then reads through `cs:`.
//!
//! ```text
//!   mov al, byte ptr T[di]          mov al, byte ptr cs:T[di]
//!   (T in _DATA, 12 B)       =>     (T after the procedure's last byte)
//! ```
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use llrm_lir::registers::RegId;

use crate::backend::masm::{self, Datum, InlinePart, Procedure};
use crate::model::ir::{Addr, Loc, Semantics, Space};
use crate::support::hash::IndexMap;

/// A table's name in the operands that reach it.
pub type Key = (Space, i64);

/// A table and the data it holds: from its first label to its last byte.
pub struct Table {
    pub key: Key,
    pub datums: Vec<Datum>,
}

/// The tables of `tables` that move into code: for each procedure by name, the
/// data that follows it. The rest come back as they were, in their order.
/// `data` is the module's other data, whose pointers name what is not a load,
/// and `pinned` the tables something else names (an alias).
pub fn placed(
    procedures: &mut [Procedure],
    tables: Vec<Table>,
    names: &IndexMap<(Space, i64), String>,
    data: &[&[Datum]],
    pinned: &BTreeSet<Key>,
) -> (BTreeMap<String, Vec<Datum>>, Vec<Table>) {
    let by_key: HashMap<Key, usize> = tables.iter().enumerate().map(|(at, one)| (one.key, at)).collect();
    let by_name: HashMap<&str, usize> =
        tables.iter().enumerate().filter_map(|(at, one)| Some((names.get(&one.key)?.as_str(), at))).collect();
    let mut readers: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); tables.len()];
    let mut escapes: Vec<bool> = tables.iter().map(|one| pinned.contains(&one.key)).collect();
    let others = data.iter().copied().chain(tables.iter().map(|one| one.datums.as_slice()));
    for datum in others.flatten() {
        if let Datum::Pointer(masm::Pointer { name, .. })
        | Datum::SegmentWord(name)
        | Datum::Difference(masm::Difference { name, .. }) = datum
            && let Some(&table) = by_name.get(name.as_str())
        {
            escapes[table] = true;
        }
    }
    for (number, procedure) in procedures.iter().enumerate() {
        for table in procedure_tables(procedure, &by_key, &by_name, &mut escapes) {
            readers[table].insert(number);
        }
    }
    let mut kept = Vec::new();
    let mut moved: BTreeMap<usize, Vec<Table>> = BTreeMap::new();
    for (at, table) in tables.into_iter().enumerate() {
        match readers[at].iter().next() {
            Some(&reader) if readers[at].len() == 1 && !escapes[at] && movable(&table.datums) => {
                moved.entry(reader).or_default().push(table);
            }
            _ => kept.push(table),
        }
    }
    let keys: BTreeSet<Key> = moved.values().flatten().map(|one| one.key).collect();
    let mut code = BTreeMap::new();
    for (number, tables) in moved {
        procedures[number] = through_cs(&procedures[number], &keys, RegId::CS);
        code.insert(procedures[number].name.clone(), tables.into_iter().flat_map(|one| one.datums).collect());
    }
    (code, kept)
}

/// The data a code segment can hold in place.
pub fn movable(datums: &[Datum]) -> bool {
    !datums.is_empty()
        && datums.iter().all(|one| match one {
            Datum::Label(_) | Datum::Object(_) | Datum::Bytes(_) | Datum::Align(_) => true,
            Datum::Pointer(pointer) => !pointer.far,
            _ => false,
        })
}

/// The tables `procedure` reads, and into `escapes` the ones it does anything
/// else with.
fn procedure_tables(
    procedure: &Procedure,
    by_key: &HashMap<Key, usize>,
    by_name: &HashMap<&str, usize>,
    escapes: &mut [bool],
) -> BTreeSet<usize> {
    let mut read = BTreeSet::new();
    let key_of = |addr: &Addr| by_key.get(&(addr.space, addr.index)).copied();
    for what in procedure.body.blocks.iter().flat_map(|block| block.insns.iter()).filter_map(|one| one.what.as_ref()) {
        for place in &what.dests {
            if let Some(table) = reference(place).and_then(|addr| key_of(&addr)) {
                escapes[table] = true;
            }
        }
        for place in &what.sources {
            let Some(addr) = reference(place) else { continue };
            let Some(table) = key_of(&addr) else { continue };
            match place {
                Loc::Mem(cell) if addr.segment == RegId::None.iced() && cell.selector.is_none() => {
                    read.insert(table);
                }
                _ => escapes[table] = true,
            }
        }
    }
    for callee in procedure.callees.values() {
        for part in &callee.code {
            if let InlinePart::Fixup(_, name, _) | InlinePart::Near(_, name) = part
                && let Some(&table) = by_name.get(name.as_str())
            {
                escapes[table] = true;
            }
        }
    }
    read
}

/// The symbolic address an operand names, whether it reads or takes it.
fn reference(place: &Loc) -> Option<Addr> {
    match place {
        Loc::Mem(cell) => cell.addr,
        Loc::Address(address) => address.addr,
        Loc::Imm(imm) => imm.address,
        _ => None,
    }
    .filter(|addr| matches!(addr.space, Space::Segment | Space::External))
}

/// `procedure` with every load of a table of `keys` read through `segment`.
fn through_cs(
    procedure: &Procedure,
    keys: &BTreeSet<Key>,
    segment: RegId,
) -> Procedure {
    let moved = |place: &Loc| match place {
        Loc::Mem(cell) => {
            let addr = cell.addr.filter(|addr| keys.contains(&(addr.space, addr.index)))?;
            Some(Loc::Mem(crate::model::ir::Mem {
                addr: Some(Addr { segment: segment.iced(), ..addr }),
                ..cell.clone()
            }))
        }
        _ => None,
    };
    let mut body = procedure.body.clone();
    for block in &mut body.blocks {
        let insns: Vec<_> = block
            .insns
            .iter()
            .map(|one| {
                let Some(what) = &one.what else { return Arc::clone(one) };
                if !what.sources.iter().any(|place| moved(place).is_some()) {
                    return Arc::clone(one);
                }
                let sources = what.sources.iter().map(|place| moved(place).unwrap_or_else(|| place.clone())).collect();
                let mut made = (**one).clone();
                made.what = Some(Semantics { sources, ..what.clone() });
                Arc::new(made)
            })
            .collect();
        *block = block.with_insns(insns);
    }
    Procedure { body, ..procedure.clone() }
}

/// The jump tables `data` holds (from [`masm::jump_tables`]), one each: a
/// table is its `Object` label and the pointers up to the next.
pub fn jump_tables(
    data: Vec<Datum>,
    names: &IndexMap<(Space, i64), String>,
) -> Vec<Table> {
    let keys: HashMap<&str, Key> = names.iter().map(|(key, name)| (name.as_str(), *key)).collect();
    let mut out: Vec<Table> = Vec::new();
    for datum in data {
        if let Datum::Object(masm::Label { name }) = &datum {
            out.push(Table { key: keys[name.as_str()], datums: Vec::new() });
        }
        out.last_mut().expect("a table opens with its label").datums.push(datum);
    }
    out
}

/// The labels `datums` define.
pub fn labels(datums: &[Datum]) -> impl Iterator<Item = &str> {
    datums
        .iter()
        .filter_map(
            |one| match one {
                Datum::Label(masm::Label { name }) | Datum::Object(masm::Label { name }) => Some(name.as_str()),
                _ => None,
            },
        )
}
