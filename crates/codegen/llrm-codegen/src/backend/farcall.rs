//! Give a far indirect call the memory operand the 16-bit ISA requires.
//!
//! # How it works
//! A 16:16 far call has only an `m16:16` form, while MIR carries the code
//! pointer as one four-byte value in a register. The pass stores that value
//! into one frame cell and calls through the cell.
//!
//! ```text
//!   call  v5                 mov  [bp-0x14], v5
//!   (indirect, 4 bytes)  =>  call [bp-0x14]
//! ```
//!
//! Only an indirect call whose single source is a four-byte value counts,
//! and only on a target with a far segment register; elsewhere a dword is a
//! near pointer and the call is left alone. All such calls in a body share
//! the one cell (`("far-indirect-call", 0)`, four bytes), made on first use.
//! The call stops reading the pointer's value but keeps reading
//! its register arguments. This is form selection, not spilling: the store
//! is not a spill and the allocator treats the cell as any frame slot.
//!
//! # Example
//! `int apply(int (__far *f)(int), int x) { return f(x) + 1; }` on `-m16`. No
//! dump prints the body before this phase, so this is what it leaves: the
//! pointer is stored to the frame cell `[bp-0x4]` and the call reads the cell.
//!
//! ```lir
//! _apply@3: 8 instructions
//!   block 0x0000
//!     0x0000     ; inserted; delivers v3@AX
//!     0x0000  v1 := mov [[bp+0x6]]  ; inserted
//!     0x0000  v2 := mov [[bp+0x8]]  ; inserted
//!     0x0000  [[bp-0x4]] := mov v1  ; inserted
//!     0x0000  [[bp-0x2]] := mov v2  ; inserted
//!     0x0000  call [[bp-0x4]]  ; inserted; requires v3@AX; delivers v4@EAX; ...
//!     0x0001  v5 := add v4, 0x1  ; inserted
//!     0x0002     ; inserted; requires v5@EAX
//! ```
//!
//! From `ISEL_DUMP=1` of that function, stage `FarIndirectCalls`, at -O2.
//!
//! # Runs
//! First machine phase (`far-indirect-calls`), before `ssaspill`, once per
//! body in `machine_with` (`flow.rs`). Every optimization level.
//!
//! # See also
//! The phase map: [`crate::backend`]

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

use crate::backend::farload::{_next_word, _plain};
use crate::backend::frame::{self as frames, Frame};
use crate::model::ir::{self, Held, Loc, Mem, Operation, Semantics};
use crate::model::lir::{self, Insn, LirBody};
use crate::model::passes::LIRTransform;

/// Python holds the one mutable frame every machine phase shares.
pub struct FarIndirectCalls {
    pub frame: Rc<RefCell<Frame>>,
}

impl FarIndirectCalls {
    #[must_use]
    pub const fn new(frame: Rc<RefCell<Frame>>) -> Self {
        Self { frame }
    }
}

impl LIRTransform for FarIndirectCalls {
    fn class_name(&self) -> &'static str {
        "FarIndirectCalls"
    }

    fn name(&self) -> &str {
        "far-indirect-calls"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        let body = materialized(&body, &mut self.frame.borrow_mut()).map_err(|refused| refused.0)?;
        Ok(loads_folded(&body))
    }
}

/// Materialize every packed far call target in one shared frame slot.
pub fn materialized(
    body: &LirBody,
    frame: &mut Frame,
) -> Result<LirBody, frames::Refused> {
    let far = body.regs().far_segment.is_some();
    let mut slot = None;
    let mut out = body.clone();
    for block in &mut out.blocks {
        let mut insns = Vec::new();
        for one in &block.insns {
            match &one.what {
                Some(what)
                    if what.op == Operation::Call
                        && what.indirect
                        // A far pointer packs selector and offset in four
                        // bytes; where the target has no far segment a dword is
                        // a near pointer.
                        && far
                        && matches!(what.sources.as_slice(), [Loc::Held(Held { width: 4, .. })]) =>
                {
                    let Loc::Held(target) = what.sources[0] else { unreachable!() };
                    if slot.is_none() {
                        slot = Some(frame.cell(("far-indirect-call", 0), 4)?);
                    }
                    let cell = Loc::Mem(slot.clone().expect("made above"));
                    let mut store = Insn::new(
                        one.at,
                        Some((one.at, one.at)),
                        Some(Semantics {
                            name: Some("mov".into()),
                            dests: vec![cell.clone()],
                            sources: vec![Loc::Held(target)],
                            ..Semantics::new(Operation::Move)
                        }),
                        Vec::new(),
                        vec![target.value],
                    );
                    store.symbol = Some(false);
                    insns.push(Arc::new(store));
                    let mut call = (**one).clone();
                    call.what = Some(Semantics { sources: vec![cell], ..what.clone() });
                    // The target now comes from the cell; what the call still
                    // reads (its register arguments) stays
                    // read.
                    call.uses.retain(|&value| value != target.value);
                    insns.push(Arc::new(call));
                }
                _ => insns.push(Arc::clone(one)),
            }
        }
        block.insns = insns.into();
    }
    Ok(out)
}

/// A far call through a cell that two word loads (or one dword load) of the
/// same place were stored to calls through that place instead: `call dword
/// ptr table[bx]` for `mov cx,table[bx]; mov dx,table+2[bx]; mov [bp-2],dx;
/// mov [bp-4],cx; call [bp-4]`. The loads and the stores read nothing else.
pub fn loads_folded(body: &LirBody) -> LirBody {
    if body.regs().far_segment.is_none() {
        return body.clone();
    }
    let mut reads: BTreeMap<u32, usize> = BTreeMap::new();
    for block in &body.blocks {
        for phi in block.phis.iter() {
            for (_from, value) in &phi.incoming {
                *reads.entry(*value).or_default() += 1;
            }
        }
        for one in block.insns.iter() {
            for value in one.uses.iter().copied().chain(one.requires.iter().map(|(held, _)| held.value)) {
                *reads.entry(value).or_default() += 1;
            }
        }
    }
    let mut out = body.clone();
    for block in &mut out.blocks {
        let mut insns: Vec<Arc<Insn>> = block.insns.iter().cloned().collect();
        let mut at = 0;
        while at < insns.len() {
            if let Some(folded) = _fold_call(&insns, at, &reads) {
                insns = folded;
            }
            at += 1;
        }
        block.insns = insns.into();
    }
    out
}

/// `insns` with the call at `call` made to read the place its pointer was
/// loaded from; `None` when it does not have that shape, or the place is no
/// symbol's.
fn _fold_call(
    insns: &[Arc<Insn>],
    call: usize,
    reads: &BTreeMap<u32, usize>,
) -> Option<Vec<Arc<Insn>>> {
    let what = insns[call].what.as_ref()?;
    let [Loc::Mem(cell)] = what.sources.as_slice() else { return None };
    if what.op != Operation::Call || !what.indirect || cell.width != 4 || cell.selector.is_some() || cell.base.is_some()
    {
        return None;
    }
    let high_cell = Mem { width: 2, ..cell.clone() };
    let low_cell = Mem { width: 2, ..cell.clone() };
    let (mut low, mut high, mut whole) = (None, None, None);
    for (at, one) in insns[..call].iter().enumerate().rev() {
        let Some(one_what) = one.what.as_ref() else { continue };
        let stored = match one_what.dests.as_slice() {
            [Loc::Mem(dest)] if one_what.op == Operation::Move && one_what.name.as_deref() == Some("mov") => dest,
            [] => continue,
            _ if one_what.dests.iter().all(|place| !matches!(place, Loc::Mem(_))) && one_what.op != Operation::Call => {
                continue;
            }
            _ => break,
        };
        let [Loc::Held(source)] = one_what.sources.as_slice() else { break };
        if stored.same_place(cell) && stored.width == 4 {
            whole = Some((at, source.value));
        } else if stored.same_place(&low_cell) {
            low = Some((at, source.value));
        } else if _next_word(&low_cell, stored) && stored.width == 2 {
            high = Some((at, source.value));
        } else {
            break;
        }
        if whole.is_some() || (low.is_some() && high.is_some()) {
            break;
        }
    }
    let _ = high_cell;
    let loaded = |value: u32, before: usize| -> Option<(usize, Mem)> {
        let at = insns[..before].iter().rposition(|one| one.defines.contains(&value))?;
        let one = &insns[at];
        let what = one.what.as_ref()?;
        let (true, [Loc::Mem(place)]) =
            (what.op == Operation::Move && what.name.as_deref() == Some("mov"), what.sources.as_slice())
        else {
            return None;
        };
        (reads.get(&value) == Some(&1) && !one.volatile() && _plain(one) && one.defines.len() == 1)
            .then(|| (at, place.clone()))
    };
    let (sources, stores, first_load, place) = if let Some((store, value)) = whole {
        let (at, place) = loaded(value, store)?;
        if place.width != 4 {
            return None;
        }
        (vec![at], vec![store], at, place)
    } else {
        let ((low_store, low_value), (high_store, high_value)) = (low?, high?);
        let (low_at, low_place) = loaded(low_value, low_store)?;
        let (high_at, high_place) = loaded(high_value, high_store)?;
        if !_next_word(&low_place, &high_place) || low_place.width != 2 || high_place.width != 2 {
            return None;
        }
        (vec![low_at, high_at], vec![low_store, high_store], low_at.min(high_at), Mem { width: 4, ..low_place })
    };
    // A call reads only a place a symbol names (a table entry): one reached
    // by a register and an offset alone (`fill->line`) has no encoding the
    // call can take, and `call dword ptr [ax+4]` was all the allocator left.
    if place.addr.is_none() {
        return None;
    }
    // Nothing between the first load and the call may write memory, or the
    // call would read what the load did not.
    for (at, one) in insns.iter().enumerate().take(call).skip(first_load) {
        if sources.contains(&at) || stores.contains(&at) {
            continue;
        }
        let one_what = one.what.as_ref()?;
        if one.volatile()
            || one_what.op == Operation::Call
            || one_what.dests.iter().any(|place| matches!(place, Loc::Mem(_)))
            || !one.clobbers.is_empty()
        {
            return None;
        }
    }
    // A value the call needs in a fixed register (the first argument) cannot
    // also be the address base: it would be read from that register.
    let pinned = &insns[call].requires;
    if ir::values(&Loc::Mem(place.clone())).iter().any(|held| pinned.iter().any(|(fixed, _)| fixed.value == held.value))
    {
        return None;
    }
    let mut folded: Vec<Arc<Insn>> = insns.to_vec();
    for at in sources.iter().chain(&stores) {
        folded[*at] = lir::bytes_only(&insns[*at]);
    }
    let mut called = (*insns[call]).clone();
    called.uses.extend(ir::values(&Loc::Mem(place.clone())).iter().map(|held| held.value));
    called.what = Some(Semantics { sources: vec![Loc::Mem(place)], ..what.clone() });
    folded[call] = Arc::new(called);
    Some(folded)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::sync::Arc;

    use llrm_lir::registers::RegId;

    use super::FarIndirectCalls;
    use crate::backend::frame::{Frame, SlotKey};
    use crate::model::ir::{self, Addr, Held, Loc, Mem, Operation, Semantics, Space};
    use crate::model::lir::{Insn, LirBlock, LirBody};
    use crate::model::passes::LIRTransform;
    use crate::support::hash::IndexMap;
    use crate::support::pyrepr::Repr;

    #[test]
    fn far_indirect_calls_share_one_frame_cell() {
        // Expected lines printed by Python's FarIndirectCalls on the same body.
        let call = |at: i64, value: u32, width: u32| {
            Arc::new(Insn::new(
                at,
                Some((at, at + if width == 4 { 3 } else { 2 })),
                Some(Semantics {
                    name: Some("call".into()),
                    sources: vec![Loc::Held(Held { value, width })],
                    indirect: true,
                    ..Semantics::new(Operation::Call)
                }),
                vec![],
                vec![value],
            ))
        };
        let far = call(4, 5, 4);
        let block = LirBlock::new(0, vec![Arc::clone(&far), call(8, 6, 2), far]);
        let body = LirBody::new("far", 0, vec![block.into()], IndexMap::default(), IndexMap::default());
        let frame = Rc::new(RefCell::new(Frame::new(-16)));
        let out = FarIndirectCalls::new(Rc::clone(&frame)).transform(body).unwrap();
        let cell = "Mem(addr=[bp-0x14], width=4, through=26, offset=0, disp_width=2, base=None, \
                    stack_argument=False, selector=None, index=None, scale=1, index_through=0)";
        let store = format!(
            "4 Some((4, 4)) Semantics(op=<Operation.MOVE: 'move'>, name='mov', dests=({cell},), \
             sources=(Held(value=5, width=4),), target=None, indirect=False) [] [5] Some(false)"
        );
        let called = format!(
            "4 Some((4, 7)) Semantics(op=<Operation.CALL: 'call'>, name='call', dests=(), \
             sources=({cell},), target=None, indirect=True) [] [] None"
        );
        let near = "8 Some((8, 10)) Semantics(op=<Operation.CALL: 'call'>, name='call', dests=(), \
                    sources=(Held(value=6, width=2),), target=None, indirect=True) [] [6] None"
            .to_owned();
        let got: Vec<String> = out
            .insns()
            .iter()
            .map(|one| {
                format!(
                    "{} {:?} {} {:?} {:?} {:?}",
                    one.at,
                    one.covers,
                    one.what.repr(),
                    one.defines,
                    one.uses,
                    one.symbol
                )
            })
            .collect();
        assert_eq!(got, vec![store.clone(), called.clone(), near, store, called]);
        assert_eq!(frame.borrow().slots, IndexMap::from_iter([(SlotKey::from(("far-indirect-call", 0)), -20)]));
        assert_eq!(frame.borrow().size(), 4);
    }

    /// A far indirect call that takes a register argument still reads it: the
    /// pass emptied the call's uses, so an instruction ahead of it was free
    /// to write the argument's register (`mov eax, ecx` passed the wrong
    /// value).
    #[test]
    fn a_far_call_keeps_reading_its_register_arguments() {
        let mut call = Insn::new(
            4,
            Some((4, 7)),
            Some(Semantics {
                name: Some("call".into()),
                sources: vec![Loc::Held(Held { value: 5, width: 4 })],
                indirect: true,
                ..Semantics::new(Operation::Call)
            }),
            vec![],
            vec![7, 5],
        );
        call.requires = vec![(Held { value: 7, width: 4 }, RegId::EAX)];
        let body = LirBody::new(
            "far",
            0,
            vec![LirBlock::new(0, vec![Arc::new(call)]).into()],
            IndexMap::default(),
            IndexMap::default(),
        );
        let out = FarIndirectCalls::new(Rc::new(RefCell::new(Frame::new(-16)))).transform(body).unwrap();
        let called = out
            .insns()
            .into_iter()
            .find(|one| one.what.as_ref().is_some_and(|what| what.op == Operation::Call))
            .unwrap();
        assert_eq!(called.uses, vec![7]);
    }

    /// Where the target has no far segment a dword target is a near pointer:
    /// Compiles for two targets, taken turns in one thread, each ask the
    /// register file of their own body: a dword call target is a far pointer
    /// in m16 and a near one in m32. A process-wide file answered both alike
    /// (29 of llrm-c's m16 tests failed when an m32 compile bound first).
    #[test]
    fn a_dword_call_target_is_far_or_near_by_the_register_file_of_its_own_body() {
        let call = Arc::new(Insn::new(
            4,
            Some((4, 7)),
            Some(Semantics {
                name: Some("call".into()),
                sources: vec![Loc::Held(Held { value: 5, width: 4 })],
                indirect: true,
                ..Semantics::new(Operation::Call)
            }),
            vec![],
            vec![5],
        ));
        for (info, calls_made) in [
            (&llrm_x86_m16::REGISTER_INFO, 2),
            (&llrm_x86_m32::REGISTER_INFO, 1),
            (&llrm_x86_m16::REGISTER_INFO, 2),
            (&llrm_x86_m32::REGISTER_INFO, 1),
        ] {
            let mut body = LirBody::new(
                "f",
                0,
                vec![LirBlock::new(0, vec![Arc::clone(&call)]).into()],
                IndexMap::default(),
                IndexMap::default(),
            );
            body.registers = Some(llrm_lir::registers::Regs(info));
            let out = super::materialized(&body, &mut Frame::new(-16)).unwrap();
            assert_eq!(out.insns().len(), calls_made, "{:?}", out.insns());
        }
    }

    fn word_load(
        at: i64,
        value: u32,
        cell: Mem,
    ) -> Arc<Insn> {
        let uses = ir::values(&Loc::Mem(cell.clone())).iter().map(|held| held.value).collect();
        Arc::new(Insn::new(
            at,
            Some((at, at)),
            Some(Semantics {
                name: Some("mov".into()),
                dests: vec![Loc::Held(Held { value, width: 2 })],
                sources: vec![Loc::Mem(cell)],
                ..Semantics::new(Operation::Move)
            }),
            vec![value],
            uses,
        ))
    }

    fn word_store(
        at: i64,
        cell: Mem,
        value: u32,
    ) -> Arc<Insn> {
        Arc::new(Insn::new(
            at,
            Some((at, at)),
            Some(Semantics {
                name: Some("mov".into()),
                dests: vec![Loc::Mem(cell)],
                sources: vec![Loc::Held(Held { value, width: 2 })],
                ..Semantics::new(Operation::Move)
            }),
            vec![],
            vec![value],
        ))
    }

    fn far_call_after(between: Option<Arc<Insn>>) -> (Vec<Arc<Insn>>, Mem) {
        let table = |disp: i64| Mem {
            base: Some(Held { value: 10, width: 2 }),
            ..Mem::new(Some(Addr::new(Space::Far, disp)), 2)
        };
        let slot = |disp: i64, width: u32| Mem::new(Some(Addr::new(Space::Frame, disp)), width);
        let mut insns = vec![
            word_load(1, 1, table(0)),
            word_load(2, 2, table(2)),
            word_store(3, slot(-2, 2), 2),
            word_store(4, slot(-4, 2), 1),
        ];
        insns.extend(between);
        insns.push(Arc::new(Insn::new(
            5,
            Some((5, 5)),
            Some(Semantics {
                name: Some("call".into()),
                sources: vec![Loc::Mem(slot(-4, 4))],
                indirect: true,
                ..Semantics::new(Operation::Call)
            }),
            vec![],
            vec![],
        )));
        (insns, Mem { width: 4, ..table(0) })
    }

    fn folded_calls(insns: Vec<Arc<Insn>>) -> Vec<Arc<Insn>> {
        let block = LirBlock::new(0, insns);
        let body = LirBody::new("far", 0, vec![block.into()], IndexMap::default(), IndexMap::default());
        super::loads_folded(&body).insns().to_vec()
    }

    /// `x_funcptr` at m16 stored the table entry's two words to a frame cell
    /// and called through the cell (`mov cx,_table[bx]; mov
    /// dx,_table+2[bx]; mov [bp-2],dx; mov [bp-4],cx; call dword ptr
    /// [bp-4]`): four instructions and a cell where `call dword ptr
    /// _table[bx]` is one.
    #[test]
    fn a_far_call_reads_the_table_entry_its_pointer_was_loaded_from() {
        let (insns, entry) = far_call_after(None);
        let out = folded_calls(insns);
        let call = out.last().unwrap().what.as_ref().unwrap();
        assert_eq!(call.sources, vec![Loc::Mem(entry)]);
        assert!(
            out[..4].iter().all(|one| one.what.as_ref().is_some_and(|what| what.op == Operation::Nothing)),
            "loads and stores left: {out:?}"
        );
    }

    /// `s->line(s, ..)` at m16 folded the call onto the table entry through the
    /// argument's value, which the call needs in AX: the call read `[ax+4]`, no
    /// address register, and `llrm-c` refused `gline.c`. A base the call
    /// requires in a fixed register keeps the cell.
    #[test]
    fn a_far_call_keeps_its_cell_when_the_entry_base_is_an_argument() {
        let (mut insns, _entry) = far_call_after(None);
        let last = insns.len() - 1;
        let mut call = (*insns[last]).clone();
        call.requires = vec![(Held { value: 10, width: 2 }, RegId::EAX)];
        insns[last] = Arc::new(call);
        let out = folded_calls(insns.clone());
        assert_eq!(out, insns);
    }

    /// A store between the loads and the call may change the entry: the call
    /// keeps its cell.
    #[test]
    fn a_far_call_after_a_store_to_memory_keeps_its_cell() {
        let blocker = word_store(9, Mem::new(Some(Addr::new(Space::Far, 8)), 2), 7);
        let (insns, _entry) = far_call_after(Some(blocker));
        let out = folded_calls(insns.clone());
        assert_eq!(out, insns);
    }

    /// `fill->line(...)` in gline.c (`call dword ptr [bx+8]`) was folded to a
    /// call through a register and an offset, which no symbol names: the
    /// allocator left it `call dword ptr [ax+4]` and the object was refused
    /// ("_line@3: Semantics(op=CALL ...)", every QB link). Such a call keeps
    /// its cell.
    #[test]
    fn a_far_call_through_a_register_and_an_offset_keeps_its_cell() {
        let field = |offset: i64, width: u32| Mem {
            base: Some(Held { value: 10, width: 2 }),
            through: iced_x86::Register::DS.into(),
            offset,
            ..Mem::new(None, width)
        };
        let slot = |disp: i64, width: u32| Mem::new(Some(Addr::new(Space::Frame, disp)), width);
        let insns = vec![
            word_load(1, 1, field(8, 2)),
            word_load(2, 2, field(10, 2)),
            word_store(3, slot(-2, 2), 2),
            word_store(4, slot(-4, 2), 1),
            Arc::new(Insn::new(
                5,
                Some((5, 5)),
                Some(Semantics {
                    name: Some("call".into()),
                    sources: vec![Loc::Mem(slot(-4, 4))],
                    indirect: true,
                    ..Semantics::new(Operation::Call)
                }),
                vec![],
                vec![],
            )),
        ];
        let out = folded_calls(insns.clone());
        assert_eq!(out, insns);
    }
}
