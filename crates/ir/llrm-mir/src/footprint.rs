//! What a change reaches: the parts of a function an edit touched, found once
//! for every pass that asks (`FunctionPass::reads` says which parts a pass
//! looks at). A pass that last found nothing to do, and reads nothing an edit
//! since reached, would find nothing again; gcc's `TODO_*` flags and LLVM's
//! preserved analyses say the same of what a pass needs to look at again.

use crate::dense::IdSet;
use crate::module::{Change, Function, InstId};
use crate::opcode::Opcode;

/// A set of the parts of a function an edit reached.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct Footprint(u8);

impl Footprint {
    pub const NONE: Self = Self(0);
    /// A block made or removed; a terminator or a phi made, removed, changed or
    /// moved: the CFG and with it the loops' shape.
    pub const STRUCTURE: Self = Self(1);
    /// A branch, a switch or an invoke made, removed, changed or moved; or an
    /// instruction that a branch's, a switch's or a select's condition is
    /// made of, directly or through others (a phi passes a value on).
    pub const CONTROL: Self = Self(2);
    /// A load, a store, a call or an allocation, or what one reads.
    pub const MEMORY: Self = Self(4);
    /// Any instruction that is none of those: a value of arithmetic that
    /// nothing above reads.
    pub const VALUES: Self = Self(8);
    pub const ALL: Self = Self(15);

    pub fn meets(
        self,
        other: Self,
    ) -> bool {
        self.0 & other.0 != 0
    }

    pub fn with(
        self,
        other: Self,
    ) -> Self {
        Self(self.0 | other.0)
    }

    pub fn is_none(self) -> bool {
        self.0 == 0
    }
}

impl std::ops::BitOr for Footprint {
    type Output = Self;

    fn bitor(
        self,
        other: Self,
    ) -> Self {
        self.with(other)
    }
}

/// The parts of `function` that `changes` reached. The function is as the
/// changes left it: an instruction taken out reaches what read it,
/// whose operands are changes of their own.
pub fn of(
    function: &Function,
    changes: &[Change],
) -> Footprint {
    let mut found = Footprint::NONE;
    let mut seen: IdSet<u32> = IdSet::new();
    for change in changes {
        let inst = match *change {
            Change::BlockCreated(_) | Change::BlockErased(_) => {
                found = found | Footprint::STRUCTURE;
                continue;
            }
            Change::Inserted { inst, .. }
            | Change::Moved { inst, .. }
            | Change::Rewritten(inst)
            | Change::Erased { inst, .. } => inst,
            Change::Cloned { to, .. } => to,
        };
        found = found | node(function, inst, &mut seen);
        if found == Footprint::ALL {
            break;
        }
    }
    found
}

fn node(
    function: &Function,
    inst: InstId,
    seen: &mut IdSet<u32>,
) -> Footprint {
    let opcode = &function.instruction(inst).opcode;
    // What the instruction is, and then where its value goes.
    let mut found = if opcode.is_terminator() {
        // A condition's change is the branch's own.
        if matches!(opcode, Opcode::Br | Opcode::Switch | Opcode::Invoke(_)) {
            Footprint::STRUCTURE | Footprint::CONTROL
        } else {
            Footprint::STRUCTURE
        }
    } else if matches!(opcode, Opcode::Phi) {
        Footprint::STRUCTURE | Footprint::VALUES
    } else if is_memory(opcode) {
        Footprint::MEMORY
    } else {
        Footprint::VALUES
    };
    if opcode.is_terminator() || is_memory(opcode) {
        return found;
    }
    // A value is read by what uses it, a phi passing it on: the control it
    // decides and the memory it addresses or stores are reached through
    // them. A value returned, or only fed round a loop, reaches neither.
    let mut work = vec![inst];
    while let Some(at) = work.pop() {
        if !seen.insert(at.0) {
            continue;
        }
        let Some(result) = function.instruction(at).result.filter(|_| !function.is_erased(at)) else { continue };
        for user in function.users(result) {
            let opcode = &function.instruction(user.user).opcode;
            match opcode {
                Opcode::Br | Opcode::Switch if user.index == 0 => found = found | Footprint::CONTROL,
                Opcode::Select if user.index == 0 => found = found | Footprint::CONTROL,
                _ if is_memory(opcode) => found = found | Footprint::MEMORY,
                _ if opcode.is_terminator() => {}
                _ => work.push(user.user),
            }
            if found.meets(Footprint::CONTROL) && found.meets(Footprint::MEMORY) {
                return found;
            }
        }
    }
    found
}

fn is_memory(opcode: &Opcode) -> bool {
    matches!(
        opcode,
        Opcode::Load { .. }
            | Opcode::Store { .. }
            | Opcode::Call(_)
            | Opcode::Invoke(_)
            | Opcode::Alloca { .. }
            | Opcode::LandingPad { .. }
            | Opcode::Resume
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GlobalKind, parse};

    /// The footprint of a change to the instruction defining `%name` in:
    /// a loop summing `%v` into `%s` (returned), counting `%i` to `%n`, and
    /// storing `%i`.
    fn reached(name: &str) -> Footprint {
        let text = "define i32 @f(i32 %n, i32 %v, ptr %p) {
b0:
  br label %b1

b1:
  %i = phi i32 [0, %b0], [%j, %b1]
  %s = phi i32 [0, %b0], [%t, %b1]
  %t = add i32 %s, %v
  %j = add i32 %i, 1
  store i32 %i, ptr %p
  %c = icmp ult i32 %j, %n
  br i1 %c, label %b1, label %b2

b2:
  ret i32 %t
}
";
        let module = parse::module(text).expect("parses");
        let GlobalKind::Function(function) = &module.global(module.named("f").expect("@f")).kind else {
            unreachable!()
        };
        let inst = function
            .walk()
            .map(|(_, inst)| inst)
            .find(|&inst| {
                function
                    .instruction(inst)
                    .result
                    .is_some_and(|result| function.value(result).name.as_deref() == Some(name))
            })
            .expect("the instruction");
        of(function, &[Change::Rewritten(inst)])
    }

    /// A sum fed round a loop and returned is read by a phi, an add and a
    /// `ret`: no condition and no memory. The counter that a compare reads
    /// is a condition; what a store writes is memory.
    #[test]
    fn a_value_reaches_the_conditions_and_the_memory_that_read_it_and_no_more() {
        assert_eq!(reached("t"), Footprint::VALUES);
        assert_eq!(reached("j"), Footprint::VALUES | Footprint::CONTROL | Footprint::MEMORY);
        assert_eq!(reached("c"), Footprint::VALUES | Footprint::CONTROL);
        assert_eq!(reached("s"), Footprint::STRUCTURE | Footprint::VALUES);
        assert!(reached("i").meets(Footprint::MEMORY));
    }
}
