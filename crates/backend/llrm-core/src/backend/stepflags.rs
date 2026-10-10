//! A compare of a value with a constant that a step of the same value already
//! subtracts: the step's flags are the compare's.
//!
//! `j1 := sub j, 1` and `cmp j, 1; jl` set the same flags, so the compare goes
//! and the step takes its place before the branch (X86's optimizeCompareInstr,
//! run before allocation as LLVM runs it). Left in, the compare reads `j`
//! after the step defined `j1`, so the two cannot share a register and the
//! counter is stepped through a `lea` and copied back.

use std::sync::Arc;

use crate::model::ir::{Held, Loc, Operation};
use crate::model::lir::Insn;

/// The conditions that read only the flags `add v, -c` and `sub v, c` agree
/// on: not the carry, which an add leaves as the opposite of a borrow.
const SIGNED: [&str; 10] = ["je", "jne", "jl", "jle", "jg", "jge", "js", "jns", "jo", "jno"];

/// `insns` with each compare a step of its operand makes needless gone.
pub fn selected(insns: &[Arc<Insn>]) -> Vec<Arc<Insn>> {
    let mut out = insns.to_vec();
    let mut at = 0;
    while at < out.len() {
        if let Some(step) = _step_of(&out, at) {
            // The step goes where the compare was, last before the branch;
            // removing it shifts the compare down one.
            let moved = out.remove(step);
            out[at - 1] = moved;
        } else {
            at += 1;
        }
    }
    out
}

/// The index of the step whose flags the compare at `at` repeats, if `at` is
/// such a compare.
fn _step_of(
    insns: &[Arc<Insn>],
    at: usize,
) -> Option<usize> {
    let compare = insns[at].what.as_ref()?;
    if compare.op != Operation::Compare || compare.name.as_deref() != Some("cmp") {
        return None;
    }
    let [Loc::Held(Held { value, width }), Loc::Imm(constant)] = compare.sources.as_slice() else { return None };
    // Only the branch reads the flags, after anchors.
    let branch =
        insns[at + 1..].iter().find(|one| !one.what.as_ref().is_some_and(|what| what.op == Operation::Nothing))?;
    let name = branch.what.as_ref().filter(|what| what.op == Operation::Branch)?.name.as_deref()?;
    let bits = i64::from(*width) * 8;
    let wrapped = |number: i64| (number + (1 << (bits - 1))).rem_euclid(1 << bits) - (1 << (bits - 1));
    let wanted = wrapped(constant.value);
    for step in (0..at).rev() {
        let one = &insns[step];
        let Some(what) = one.what.as_ref() else { continue };
        if let [Loc::Held(result)] = what.dests.as_slice()
            && what.op == Operation::Binary
            && result.width == *width
            && let [Loc::Held(source), Loc::Imm(amount)] = what.sources.as_slice()
            && source.value == *value
            && source.width == *width
            && constant.address.is_none()
            && amount.address.is_none()
        {
            let same = match what.name.as_deref() {
                Some("sub") => wrapped(amount.value) == wanted,
                // The signed conditions only; and not the one constant whose
                // negation is itself, which overflows the other way.
                Some("add") => {
                    SIGNED.contains(&name) && wanted != -(1 << (bits - 1)) && wrapped(amount.value) == wrapped(-wanted)
                }
                _ => false,
            };
            // Nothing between reads the result, which moves down past it.
            let read = insns[step + 1..at]
                .iter()
                .any(
                    |between| between.uses.contains(&result.value)
                        || between.requires.iter().any(|(held, _)| held.value == result.value),
                );
            return (same && !read).then_some(step);
        }
        // Any other definition of the operand's flags is no matter; stop at
        // the first instruction that defines the value (never, in SSA).
        if one.defines.contains(value) {
            return None;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::selected;
    use crate::model::ir::{Held, Imm, Loc, Operation, Semantics};
    use crate::model::lir::Insn;

    fn insn(
        at: i64,
        op: Operation,
        name: &str,
        dests: Vec<Loc>,
        sources: Vec<Loc>,
        defines: Vec<u32>,
        uses: Vec<u32>,
    ) -> Arc<Insn> {
        let what = Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(op) };
        Arc::new(Insn::new(at, Some((at, at)), Some(what), defines, uses))
    }

    fn held(value: u32) -> Held {
        Held { value, width: 4 }
    }

    fn imm(value: i64) -> Loc {
        Loc::Imm(Imm { value, width: 4, address: None })
    }

    /// `j1 := j op k; (what is between); cmp j, 1; branch`.
    fn block(
        op: &str,
        k: i64,
        branch: &str,
        reads_step: bool,
    ) -> Vec<Arc<Insn>> {
        let between = if reads_step {
            vec![insn(1, Operation::Move, "mov", vec![Loc::Held(held(3))], vec![Loc::Held(held(2))], vec![3], vec![2])]
        } else {
            Vec::new()
        };
        let mut out = vec![insn(
            0,
            Operation::Binary,
            op,
            vec![Loc::Held(held(2))],
            vec![Loc::Held(held(1)), imm(k)],
            vec![2],
            vec![1],
        )];
        out.extend(between);
        out.push(insn(2, Operation::Compare, "cmp", Vec::new(), vec![Loc::Held(held(1)), imm(1)], Vec::new(), vec![1]));
        out.push(insn(3, Operation::Branch, branch, Vec::new(), Vec::new(), Vec::new(), Vec::new()));
        out
    }

    fn names(insns: &[Arc<Insn>]) -> Vec<String> {
        insns.iter().map(|one| one.what.as_ref().and_then(|what| what.name.clone()).unwrap_or_default()).collect()
    }

    /// `n - 1` and `n <= 1` were a `lea` and a `cmp` of the same value, so the
    /// two could not share a register (x_insertion: `lea eax, [ecx-1]; cmp
    /// ecx, 1` where the step's own flags answer the branch).
    #[test]
    fn test_the_step_of_a_compared_value_takes_the_compares_place() {
        assert_eq!(names(&selected(&block("sub", 1, "jle", false))), ["sub", "jle"]);
        assert_eq!(names(&selected(&block("add", -1, "jl", false))), ["add", "jl"]);
    }

    #[test]
    fn test_a_step_is_not_the_compare_where_the_carry_or_a_reader_differs() {
        // An add leaves the opposite carry to a subtract's borrow.
        assert_eq!(names(&selected(&block("add", -1, "jb", false))), ["add", "cmp", "jb"]);
        // The result is read between: it does not move past its reader.
        assert_eq!(names(&selected(&block("sub", 1, "jle", true))), ["sub", "mov", "cmp", "jle"]);
        // A different constant is a different compare.
        assert_eq!(names(&selected(&block("sub", 2, "jle", false))), ["sub", "cmp", "jle"]);
    }
}
