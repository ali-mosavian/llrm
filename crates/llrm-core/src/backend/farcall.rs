//! Port of `qbopt/backend/farcall.py`: give a far indirect call the memory
//! operand the 16-bit ISA requires.
//!
//! A 16:16 far call has only an `m16:16` form, while MIR carries the code
//! pointer as one four-byte value. Lower that target through one reusable
//! owned frame cell before allocation; this is target form selection, not
//! spilling.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use crate::backend::frame::{self as frames, Frame};
use crate::model::ir::{Held, Loc, Operation, Semantics};
use crate::model::lir::{Insn, LirBody};
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

    fn transform(&mut self, body: LirBody) -> Result<LirBody, String> {
        materialized(&body, &mut self.frame.borrow_mut()).map_err(|refused| refused.0)
    }
}

/// Materialize every packed far call target in one shared frame slot.
pub fn materialized(body: &LirBody, frame: &mut Frame) -> Result<LirBody, frames::Refused> {
    let mut slot = None;
    let mut out = body.clone();
    for block in &mut out.blocks {
        let mut insns = Vec::new();
        for one in &block.insns {
            match &one.what {
                Some(what)
                    if what.op == Operation::Call
                        && what.indirect
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
                            name: Some("mov".to_owned()),
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
                    call.what = Some(Semantics {
                        sources: vec![cell],
                        ..what.clone()
                    });
                    call.uses = Vec::new();
                    insns.push(Arc::new(call));
                }
                _ => insns.push(Arc::clone(one)),
            }
        }
        block.insns = insns;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::sync::Arc;

    use crate::support::hash::IndexMap;

    use super::FarIndirectCalls;
    use crate::backend::frame::{Frame, SlotKey};
    use crate::model::ir::{Held, Loc, Operation, Semantics};
    use crate::model::lir::{Insn, LirBlock, LirBody};
    use crate::model::passes::LIRTransform;
    use crate::support::pyrepr::Repr;

    #[test]
    fn far_indirect_calls_share_one_frame_cell() {
        // Expected lines printed by Python's FarIndirectCalls on the same body.
        let call = |at: i64, value: u32, width: u32| {
            Arc::new(Insn::new(
                at,
                Some((at, at + if width == 4 { 3 } else { 2 })),
                Some(Semantics {
                    name: Some("call".to_owned()),
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
        let body = LirBody::new("far", 0, vec![block], IndexMap::default(), IndexMap::default());
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
                format!("{} {:?} {} {:?} {:?} {:?}", one.at, one.covers, one.what.repr(), one.defines, one.uses, one.symbol)
            })
            .collect();
        assert_eq!(got, vec![store.clone(), called.clone(), near, store, called]);
        assert_eq!(frame.borrow().slots, IndexMap::from_iter([(SlotKey::from(("far-indirect-call", 0)), -20)]));
        assert_eq!(frame.borrow().size(), 4);
    }
}
