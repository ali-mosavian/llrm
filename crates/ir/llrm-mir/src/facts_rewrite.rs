//! What a pass that rewrites code does to the facts on it: when it folds two
//! instructions into one, and when it runs one where it did not run.
//!
//! One `match` per question over every `Fact`, with no catch-all: a fact
//! added without an answer does not compile.
//!
//! **Merge** (GVN, EarlyCSE: the one instruction runs wherever either did).
//! A promise survives only if both made it, at the weaker of the two values.
//! Today no pass merges across a promise: flags are in every value-numbering
//! key, so `add nsw` and a plain `add` stay two instructions, and no pass
//! copies metadata between instructions. Whoever lets one do either, calls
//! this.
//!
//! **Speculate** (LICM, hoist, PRE insertion: the instruction runs where it
//! did not). A fact stated of an instruction is a poison-generating promise,
//! and LLVM's poison is lazy: it matters only where a user reads it, and the
//! users of a moved instruction are the ones its original had, which only ran
//! after the original did. So every fact here survives. A fact that made the
//! instruction undefined behaviour outright (LLVM's `noundef`, `!dereferenceable`
//! on a load) would not, and answers `false` below. Checked when this was
//! written, on the passes `llrm_transforms::pipeline` runs: `hoist`,
//! `loopmotion`, `lsr` (induction steps), `exitsink` (through `indvars`),
//! `gvn::joined` (insertion on an edge) and `loadjoins` (a load cloned into
//! predecessors) move or insert an instruction with its flags, and are sound
//! by the argument above; `algebraic` reassociation changes operands and
//! already clears the flags (`set_flags(.., Flags::default())`). The optimiser passes
//! `llrm-mir` once had were never run by the compile route (#237) and are gone;
//! a pass added to the route comes under this audit.
//!
//! The argument needs one more thing: no reader takes a flag or `!range` off
//! an instruction to conclude something about its operands, or about another
//! point, without the instruction executing wherever it concludes. LLVM drops
//! flags when it hoists for exactly that. Checked: `induction::_promised`
//! reads a counter's step, which flows into the header phi along the back
//! edge and so ran on every trip it matters to (and depends on the phi, so is
//! never hoisted); `induction::inbounds_backedges` requires the access to
//! dominate the latch; `memory` and `pointerfacts` conclude only that a GEP's
//! result is in its base's object; `gepoffset` rewrites `sext (add nsw ..)` at
//! the add; `isel` prices a wrapping add. `ranges`, `guards`, `consts` and
//! value tracking read no flag. The test below lists the files that name a
//! no-wrap or inbounds flag, so a new reader is checked before it is added.

use crate::facts::{Bounds, Effect, Fact, Facts};

impl Fact {
    /// The fact that holds of the one instruction two stated instructions
    /// are folded into, where `self` was stated of one and `other` of the
    /// other. `None` where nothing is promised of both.
    pub fn merged(self, other: Fact) -> Option<Fact> {
        match (self, other) {
            (Fact::Dereferenceable(a), Fact::Dereferenceable(b)) => Some(Fact::Dereferenceable(a.min(b))),
            (Fact::Align(a), Fact::Align(b)) => Some(Fact::Align(a.min(b))),
            (Fact::Initializes(a), Fact::Initializes(b)) => Some(Fact::Initializes(a.min(b))),
            (Fact::Memory(a), Fact::Memory(b)) => Self::effects(a, b).map(Fact::Memory),
            // Both bounds hold of the one value that is either's: the hull.
            (Fact::Range(a), Fact::Range(b)) => Some(Fact::Range(Bounds { lo: a.lo.min(b.lo), hi: a.hi.max(b.hi) })),
            (Fact::Unroll(a), Fact::Unroll(b)) => Some(Fact::Unroll(a.min(b))),
            (a, b) if a == b => Some(a),
            _ => None,
        }
    }

    /// The weaker of two memory effects: what either may do, if that is one of them.
    fn effects(a: Effect, b: Effect) -> Option<Effect> {
        match (a, b) {
            (a, b) if a == b => Some(a),
            (Effect::None, other) | (other, Effect::None) => Some(other),
            _ => None,
        }
    }

    /// Whether the fact still holds of an instruction run where it did not
    /// run. Every variant answers; see the module documentation.
    pub fn survives_speculation(self) -> bool {
        match self {
            Fact::NoAlias
            | Fact::ReadOnly
            | Fact::ReadNone
            | Fact::NonNull
            | Fact::NoCapture
            | Fact::NoRetain
            | Fact::Releases
            | Fact::WriteOnly
            | Fact::NoReturn
            | Fact::NoUnwind
            | Fact::WillReturn
            | Fact::NoCallback
            | Fact::Cold
            | Fact::ThreeWayCompare
            | Fact::Dereferenceable(_)
            | Fact::Align(_)
            | Fact::Initializes(_)
            | Fact::Memory(_)
            | Fact::NoSignedWrap
            | Fact::NoUnsignedWrap
            | Fact::InBounds
            | Fact::Inline(_)
            | Fact::Reassoc
            | Fact::NoNaNs
            | Fact::NoInfs
            | Fact::NoSignedZeros
            | Fact::AllowReciprocal
            | Fact::Invariant
            | Fact::Unroll(_)
            | Fact::MustProgress
            | Fact::NoRecurse
            | Fact::Range(_) => true,
        }
    }
}

impl Facts {
    /// What two stated sets both promise, at the weaker value of each.
    pub fn merged(&self, other: &Facts) -> Facts {
        let mut kept = Vec::new();
        for fact in self.iter() {
            let shared = other.iter().filter_map(|one| fact.merged(one)).next();
            kept.extend(shared.filter(|one| !kept.contains(one)));
        }
        Facts::from_facts(kept)
    }

    /// What still holds of an instruction run where it did not run.
    pub fn speculated(&self) -> Facts {
        Facts::from_facts(self.iter().filter(|fact| fact.survives_speculation()).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(list: &[Fact]) -> Facts {
        Facts::from_facts(list.to_vec())
    }

    /// Two instructions folded keep what both promised, at the weaker value:
    /// `add nsw` with a plain `add` is a plain `add`.
    #[test]
    fn a_merge_keeps_only_what_both_promised_at_the_weaker_value() {
        let both = facts(&[Fact::NoSignedWrap, Fact::NoUnsignedWrap, Fact::Dereferenceable(8), Fact::Memory(Effect::None)]);
        let other = facts(&[Fact::NoSignedWrap, Fact::Dereferenceable(4), Fact::Memory(Effect::Read)]);
        let merged = both.merged(&other);
        assert!(merged.no_signed_wrap() && !merged.no_unsigned_wrap());
        assert_eq!(merged.dereferenceable(), Some(4));
        assert_eq!(merged.memory(), Some(Effect::Read));
        assert_eq!(facts(&[Fact::NoSignedWrap]).merged(&facts(&[])).iter().count(), 0, "one side's promise alone is not kept");
        assert_eq!(facts(&[Fact::Memory(Effect::Read)]).merged(&facts(&[Fact::Memory(Effect::Write)])).memory(), None);
        let hull = facts(&[Fact::Range(Bounds { lo: 0, hi: 3 })]).merged(&facts(&[Fact::Range(Bounds { lo: 2, hi: 9 })]));
        assert_eq!(hull.range(), Some(Bounds { lo: 0, hi: 9 }), "a merged range is the hull");
    }

    /// Every fact survives being run where it did not run, today: the users
    /// of the moved instruction are the original's.
    #[test]
    fn speculation_keeps_every_fact_today() {
        let all = Facts::from_facts(Fact::examples());
        assert_eq!(all.speculated().iter().count(), all.iter().count());
    }

    /// Every file that reads a no-wrap or inbounds promise off an
    /// instruction is one whose conclusion was checked (see the module
    /// documentation); a new one fails here until it is, and is listed.
    #[test]
    fn only_audited_readers_take_a_promise_off_an_instruction() {
        const CHECKED: [&str; 7] = ["induction.rs", "memory.rs", "gepoffset.rs", "isel.rs", "interpret.rs", "mir.rs", "pointerfacts.rs"];
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let reads = ["no_signed_wrap()", "no_unsigned_wrap()", "in_bounds()", "Flags::NSW", "Flags::NUW", "Flags::INBOUNDS"];
        let mut found = Vec::new();
        let mut directories = vec![root];
        while let Some(directory) = directories.pop() {
            for entry in std::fs::read_dir(&directory).expect("a directory").flatten() {
                let path = entry.path();
                let name = path.file_name().and_then(|one| one.to_str()).unwrap_or_default().to_owned();
                if path.is_dir() {
                    if !matches!(name.as_str(), "target" | "tests" | "fixtures") {
                        directories.push(path);
                    }
                } else if name.ends_with(".rs") && !name.contains("test") && !CHECKED.contains(&name.as_str()) && !matches!(name.as_str(), "facts.rs" | "facts_rewrite.rs" | "parse.rs" | "print.rs" | "opcode.rs" | "edit.rs") {
                    let text = std::fs::read_to_string(&path).expect("source");
                    let source = text.split("#[cfg(test)]").next().unwrap_or_default();
                    if reads.iter().any(|read| source.contains(read)) {
                        found.push(name);
                    }
                }
            }
        }
        found.sort();
        assert_eq!(found, Vec::<String>::new(), "a reader of nsw/nuw/inbounds that was not audited");
    }
}
