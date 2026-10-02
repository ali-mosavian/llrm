//! What a language promises of a program, as MIR carries it, and the one way
//! to read it.
//!
//! A fact is a promise a pass may use and may also ignore: dropping one never
//! changes what the program means. What changes meaning (volatile, a callee
//! that returns twice) is part of the IR proper, not a fact.
//!
//! Each fact is declared once, in `facts!`: its attribute or flag and the
//! kinds of subject it can be stated of. What a pass that merges or moves
//! instructions does with a fact is not declared here until a pass does it
//! and that is measured to pay. A frontend states facts through `llrm_hir::facts`; a pass reads them
//! through [`Facts`] and never parses an attribute by name.

use crate::module::Function;
use crate::opcode::{Attribute, Flags};
use crate::types::TypeId;

/// The kind of thing a fact is stated of: a routine, one of its parameters,
/// or one of its instructions. HIR names them; MIR has the carriers alone.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Callable,
    Param,
    Instruction,
    /// One operand of an instruction: a call argument, or a place.
    Operand,
    Object,
    /// The terminator of a block: a loop's back edge carries what the language says of the loop.
    Terminator,
}

macro_rules! facts {
    (
        flags { $($flag:ident $fmethod:ident $fkey:literal on [$($fkind:ident),+];)* }
        valued { $($valued:ident($vty:ty) $vmethod:ident $vkey:literal on [$($vkind:ident),+];)* }
        custom { $($custom:ident($cty:ty) $cmethod:ident $ckey:literal on [$($ckind:ident),+];)* }
        bits { $($bit:ident $bmethod:ident $bkey:literal $bflag:expr, on [$($bkind:ident),+];)* }
    ) => {
        /// A promise of the language, stated once per subject.
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        pub enum Fact {
            $($flag,)*
            $($valued($vty),)*
            $($custom($cty),)*
            $($bit,)*
        }

        impl Fact {
            /// The name the codec and diagnostics use, and, for a flag or a
            /// valued fact, its MIR attribute's.
            pub fn key(self) -> &'static str {
                match self {
                    $(Fact::$flag => $fkey,)*
                    $(Fact::$valued(_) => $vkey,)*
                    $(Fact::$custom(_) => $ckey,)*
                    $(Fact::$bit => $bkey,)*
                }
            }

            /// The kinds of subject it can be stated of.
            pub fn kinds(self) -> &'static [Kind] {
                match self {
                    $(Fact::$flag => &[$(Kind::$fkind),+],)*
                    $(Fact::$valued(_) => &[$(Kind::$vkind),+],)*
                    $(Fact::$custom(_) => &[$(Kind::$ckind),+],)*
                    $(Fact::$bit => &[$(Kind::$bkind),+],)*
                }
            }

            /// Whether the name is a fact's.
            pub fn is_named(key: &str) -> bool {
                [$($fkey,)* $($vkey,)* $($ckey,)* $($bkey,)*].contains(&key)
            }

            /// The fact of a flag's name.
            pub fn flag(key: &str) -> Option<Fact> {
                match key { $($fkey => Some(Fact::$flag),)* _ => None }
            }

            /// The fact of a valued fact's name and value.
            pub fn valued(key: &str, value: u64) -> Option<Fact> {
                match key { $($vkey => Some(Fact::$valued(value as $vty)),)* _ => None }
            }

            /// The value a fact carries on the wire, and the second one a pair
            /// carries, if it carries any.
            pub fn wire_value(self) -> Option<(i64, Option<i64>)> {
                match self {
                    $(Fact::$flag => None,)*
                    $(Fact::$bit => None,)*
                    $(Fact::$valued(value) => Some((value as i64, None)),)*
                    $(Fact::$custom(value) => Some(Wire::wire(value)),)*
                }
            }

            /// The fact of a wire name and values; none where either is not one's.
            pub fn from_wire(key: &str, value: Option<i64>, second: Option<i64>) -> Option<Fact> {
                match (key, value, second) {
                    $(($fkey, None, None) => Some(Fact::$flag),)*
                    $(($bkey, None, None) => Some(Fact::$bit),)*
                    $(($vkey, Some(value), None) => Some(Fact::$valued(value as $vty)),)*
                    $(($ckey, Some(value), second) => <$cty as Wire>::unwire(value, second).map(Fact::$custom),)*
                    _ => None,
                }
            }

            /// The instruction flags a fact is, none where it is an attribute.
            pub fn flags(self) -> Flags {
                match self {
                    $(Fact::$bit => $bflag,)*
                    _ => Flags::default(),
                }
            }

            /// The facts that flags state.
            pub fn of_flags(flags: Flags) -> Vec<Fact> {
                let mut facts = Vec::new();
                $(if flags.contains($bflag) { facts.push(Fact::$bit); })*
                facts
            }

            /// One of each fact, for tests that must name every one.
            pub fn examples() -> Vec<Fact> {
                vec![$(Fact::$flag,)* $(Fact::$valued(Default::default()),)* $(Fact::$custom(Default::default()),)* $(Fact::$bit,)*]
            }
        }

        impl Facts {
            $(pub fn $fmethod(&self) -> bool {
                self.contains(Fact::$flag)
            })*
            $(pub fn $vmethod(&self) -> Option<$vty> {
                self.0.iter().find_map(|fact| if let Fact::$valued(value) = fact { Some(*value) } else { None })
            })*
            $(pub fn $bmethod(&self) -> bool {
                self.contains(Fact::$bit)
            })*
            $(pub fn $cmethod(&self) -> Option<$cty> {
                self.0.iter().find_map(|fact| if let Fact::$custom(value) = fact { Some(*value) } else { None })
            })*
        }
    };
}

/// A fact's value as one number on the wire, or a pair.
pub trait Wire: Sized {
    fn wire(self) -> (i64, Option<i64>);
    fn unwire(value: i64, second: Option<i64>) -> Option<Self>;
}

/// What a call may do to memory, as `memory(...)` states it.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum Effect {
    /// It neither reads nor writes.
    #[default]
    None,
    /// It may read, not write.
    Read,
    /// It may write, not read.
    Write,
    /// It may read and write only memory the program cannot name: a routine
    /// that ends the program touches nothing a caller's loop could read back.
    Inaccessible,
}

/// What the language says of inlining a routine; `Never` outranks the rest.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Inlining {
    /// Not at any call (`noinline`).
    Never,
    /// Worth a larger body than usual (`inlinehint`, C's `inline`).
    #[default]
    Hint,
    /// At every call it can be (`alwaysinline`).
    Always,
}

impl Inlining {
    fn flag(self) -> &'static str {
        match self {
            Inlining::Never => "noinline",
            Inlining::Hint => "inlinehint",
            Inlining::Always => "alwaysinline",
        }
    }

    fn of_flag(name: &str) -> Option<Inlining> {
        [Inlining::Never, Inlining::Hint, Inlining::Always].into_iter().find(|one| one.flag() == name)
    }
}

impl Wire for Inlining {
    fn wire(self) -> (i64, Option<i64>) {
        (self as i64, None)
    }

    fn unwire(value: i64, second: Option<i64>) -> Option<Inlining> {
        [Inlining::Never, Inlining::Hint, Inlining::Always].into_iter().find(|one| *one as i64 == value && second.is_none())
    }
}

facts! {
    flags {
        // Of a routine, its result: a pointer to memory nothing else names.
        NoAlias no_alias "noalias" on [Param, Callable];
        // The load reads what nothing writes after it is initialised.
        Invariant invariant "invariant" on [Instruction];
        ReadOnly read_only "readonly" on [Param];
        // Touches no memory through the pointer, or at all, of a routine.
        ReadNone read_none "readnone" on [Param, Callable];
        NonNull non_null "nonnull" on [Param];
        NoCapture no_capture "nocapture" on [Param, Operand];
        WriteOnly write_only "writeonly" on [Operand];
        NoReturn no_return "noreturn" on [Callable];
        // Every loop of it that does nothing observable ends: the language says
        // so of every loop, as LLVM's function-level `mustprogress`. C's promise is
        // per loop (C11 6.8.5p6), as `llvm.loop.mustprogress`, not this.
        MustProgress must_progress "mustprogress" on [Callable];
        // Of a routine: it raises nothing, comes back, calls nothing of the module, is rare.
        NoUnwind no_unwind "nounwind" on [Callable];
        WillReturn will_return "willreturn" on [Callable];
        NoCallback no_callback "nocallback" on [Callable];
        Cold cold "cold" on [Callable];
        // The result is a three-way compare of the data: its sign says which
        // is greater, nothing about how often. LLVM knows strcmp's by name
        // (LibFunc); here the language states it of the routine.
        ThreeWayCompare three_way_compare "threeway" on [Callable];
    }
    valued {
        Dereferenceable(u64) dereferenceable "dereferenceable" on [Param];
        Align(u64) align "align" on [Param, Object, Instruction];
        Initializes(u64) initializes "initializes" on [Operand];
        // Of a loop's back edge: most copies the language lets be made. 0 forbids, `u32::MAX` is all.
        Unroll(u32) unroll "unroll" on [Terminator];
    }
    custom {
        Memory(Effect) memory "memory" on [Callable];
        // Of a routine, its result.
        Range(Bounds) range "range" on [Param, Callable, Instruction];
        Inline(Inlining) inline "inline" on [Callable];
    }
    bits {
        NoSignedWrap no_signed_wrap "nsw" Flags::NSW, on [Instruction];
        NoUnsignedWrap no_unsigned_wrap "nuw" Flags::NUW, on [Instruction];
        InBounds in_bounds "inbounds" Flags::INBOUNDS, on [Instruction, Operand];
        // What the language lets a pass do to a floating operation: sums and
        // products regroup, no operand is NaN or infinite, a zero's sign is
        // not observed, a division is a multiply by the reciprocal.
        Reassoc reassoc "reassoc" Flags::REASSOC, on [Instruction];
        NoNaNs no_nans "nnan" Flags::NNAN, on [Instruction];
        NoInfs no_infs "ninf" Flags::NINF, on [Instruction];
        NoSignedZeros no_signed_zeros "nsz" Flags::NSZ, on [Instruction];
        AllowReciprocal allow_reciprocal "arcp" Flags::ARCP, on [Instruction];
    }
}

impl Fact {
    /// The MIR attribute that carries the fact, none where it is an
    /// instruction flag.
    pub fn attribute(self) -> Option<Attribute> {
        match self {
            Fact::Memory(Effect::Inaccessible) => Some(Attribute::Memory(vec![(Some("inaccessiblemem".to_owned()), "readwrite".to_owned())])),
            Fact::Dereferenceable(value) | Fact::Align(value) => Some(Attribute::Int(self.key().to_owned(), value)),
            Fact::Memory(effect) => Some(Attribute::Memory(vec![(None, effect.spelled().to_owned())])),
            Fact::Initializes(bytes) => Some(Attribute::Initializes(vec![(0, bytes as i64)])),
            Fact::Inline(how) => Some(Attribute::Flag(how.flag().to_owned())),
            // A range wants the width of what it bounds: `typed_attribute`.
            Fact::Invariant | Fact::Unroll(_) | Fact::Range(_) | Fact::NoSignedWrap | Fact::NoUnsignedWrap | Fact::InBounds | Fact::Reassoc | Fact::NoNaNs | Fact::NoInfs | Fact::NoSignedZeros | Fact::AllowReciprocal => None,
            _ => Some(Attribute::Flag(self.key().to_owned())),
        }
    }

    /// Whether it is carried as metadata (`!range`, `!llvm.loop`) or by the width of
    /// what it bounds, where it is neither an attribute nor an instruction flag.
    pub fn is_metadata(self) -> bool {
        matches!(self, Fact::Range(_) | Fact::Invariant | Fact::Unroll(_))
    }

    /// Whether, stated of a routine, it is of the routine's result.
    pub fn of_result(self) -> bool {
        matches!(self, Fact::NoAlias | Fact::Range(_))
    }

    /// The attribute of an integer of `bits` that carries the fact: as
    /// `attribute`, and a range as LLVM's half-open `[lo, hi + 1)` modulo 2^bits.
    pub fn typed_attribute(self, ty: TypeId, bits: u32) -> Option<Attribute> {
        match self {
            Fact::Range(Bounds { lo, hi }) => {
                let wrap = |value: i128| (value as u128) & (u128::MAX >> (128 - bits.clamp(1, 128)));
                Some(Attribute::Range { ty, lower: wrap(lo as i128), upper: wrap(hi as i128 + 1) })
            }
            other => other.attribute(),
        }
    }

    /// The fact a range attribute of an integer of `bits` states. `signed`
    /// reads its bounds as signed, as a frontend that stated `-1..=1` meant them.
    pub fn of_range(lower: u128, upper: u128, bits: u32, signed: bool) -> Option<Fact> {
        let mask = u128::MAX >> (128 - bits.clamp(1, 128));
        let value = |one: u128| {
            let one = one & mask;
            if signed && bits < 128 && one >> (bits - 1) & 1 == 1 { one as i128 - (1i128 << bits) } else { one as i128 }
        };
        let hi = value(upper.wrapping_sub(1));
        let lo = value(lower);
        (lo <= hi).then(|| Fact::Range(Bounds { lo: lo as i64, hi: hi as i64 }))
    }

    /// The attribute that carries a flag or valued fact; a fact that is carried otherwise has none.
    pub fn carrier(self) -> Attribute {
        self.attribute().unwrap_or_else(|| panic!("{} has no attribute", self.key()))
    }

    /// The fact a carrier states, if it states one.
    pub fn of_attribute(attribute: &Attribute) -> Option<Fact> {
        match attribute {
            Attribute::Flag(name) => Fact::flag(name).or_else(|| Inlining::of_flag(name).map(Fact::Inline)),
            Attribute::Int(name, value) => Fact::valued(name, *value),
            Attribute::Initializes(ranges) => match ranges[..] {
                [(0, bytes)] => Some(Fact::Initializes(bytes as u64)),
                _ => None,
            },
            Attribute::Memory(locations) => match locations[..] {
                [(None, ref access)] => Effect::of_spelling(access).map(Fact::Memory),
                [(Some(ref location), ref access)] if location == "inaccessiblemem" && access == "readwrite" => Some(Fact::Memory(Effect::Inaccessible)),
                _ => None,
            },
            _ => None,
        }
    }
}

impl Wire for Effect {
    fn wire(self) -> (i64, Option<i64>) {
        (self as i64, None)
    }

    fn unwire(value: i64, second: Option<i64>) -> Option<Effect> {
        [Effect::None, Effect::Read, Effect::Write, Effect::Inaccessible].into_iter().find(|one| *one as i64 == value && second.is_none())
    }
}

/// The values an integer is within, both included, as the number it is: a
/// byte read as unsigned is 0..=255, not -128..=127.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct Bounds {
    pub lo: i64,
    pub hi: i64,
}

impl Wire for Bounds {
    fn wire(self) -> (i64, Option<i64>) {
        (self.lo, Some(self.hi))
    }

    fn unwire(value: i64, second: Option<i64>) -> Option<Bounds> {
        second.filter(|&hi| value <= hi).map(|hi| Bounds { lo: value, hi })
    }
}

impl Effect {
    fn spelled(self) -> &'static str {
        match self {
            Effect::None => "none",
            Effect::Read => "read",
            Effect::Write => "write",
            Effect::Inaccessible => "inaccessible",
        }
    }

    fn of_spelling(access: &str) -> Option<Effect> {
        [Effect::None, Effect::Read, Effect::Write].into_iter().find(|one| one.spelled() == access)
    }
}

/// The facts stated of one thing, read from its attributes.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Facts(Vec<Fact>);

impl Facts {
    /// The facts listed, each once.
    pub fn from_facts(list: Vec<Fact>) -> Facts {
        let mut facts = Vec::new();
        for fact in list {
            if !facts.contains(&fact) {
                facts.push(fact);
            }
        }
        Facts(facts)
    }

    /// The facts `attributes` state; any other attribute is not a fact.
    pub fn of(attributes: &[Attribute]) -> Facts {
        let mut facts = Vec::new();
        for fact in attributes.iter().filter_map(Fact::of_attribute) {
            if !facts.contains(&fact) {
                facts.push(fact);
            }
        }
        Facts(facts)
    }

    /// As `of`, and the ranges too, which need the width of the integer `bits_of` names;
    /// read as signed where `signed` says.
    pub fn of_typed(attributes: &[Attribute], bits_of: impl Fn(TypeId) -> Option<u32>, signed: bool) -> Facts {
        let mut facts = Facts::of(attributes);
        for attribute in attributes {
            if let Attribute::Range { ty, lower, upper } = attribute
                && let Some(fact) = bits_of(*ty).and_then(|bits| Fact::of_range(*lower, *upper, bits, signed))
                && !facts.0.contains(&fact)
            {
                facts.0.push(fact);
            }
        }
        facts
    }

    /// The facts an instruction's flags state.
    pub fn of_flags(flags: Flags) -> Facts {
        Facts(Fact::of_flags(flags))
    }

    /// Those of `function`'s `index`th parameter.
    pub fn param(function: &Function, index: usize) -> Facts {
        Facts::of(function.parameter_attrs.get(index).map(Vec::as_slice).unwrap_or_default())
    }

    pub fn contains(&self, fact: Fact) -> bool {
        self.0.contains(&fact)
    }

    pub fn iter(&self) -> impl Iterator<Item = Fact> + '_ {
        self.0.iter().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every fact reads back from its own carrier, and no carrier states two.
    #[test]
    fn every_fact_round_trips_through_its_attribute() {
        for fact in Fact::examples() {
            match fact.attribute() {
                Some(attribute) => assert_eq!(Fact::of_attribute(&attribute), Some(fact), "{}", fact.key()),
                None if fact.is_metadata() => {}
                None => assert!(Fact::of_flags(fact.flags()).contains(&fact), "{} is a flag", fact.key()),
            }
            assert!(Fact::is_named(fact.key()));
            assert_eq!(Fact::from_wire(fact.key(), fact.wire_value().map(|one| one.0), fact.wire_value().and_then(|one| one.1)), Some(fact), "{} on the wire", fact.key());
            assert!(!fact.kinds().is_empty(), "{} is of no subject", fact.key());
        }
    }

    /// A pass asks the fact, not the attribute's spelling.
    #[test]
    fn facts_are_read_from_attributes_and_others_are_ignored() {
        let attributes = vec![Attribute::Flag("noalias".to_owned()), Attribute::Flag("builtin".to_owned()), Attribute::Flag("noalias".to_owned())];
        let facts = Facts::of(&attributes);
        assert!(facts.no_alias());
        assert_eq!(facts.iter().count(), 1);
        assert!(!Facts::of(&[Attribute::Flag("readonly".to_owned())]).no_alias());
    }

    /// A routine that ends the program states `inaccessiblemem`, and a pass
    /// reads it back as that fact.
    #[test]
    fn an_inaccessible_effect_reads_back_from_its_attribute() {
        let fact = Fact::Memory(Effect::Inaccessible);
        let attribute = fact.attribute().expect("an attribute");
        assert_eq!(Fact::of_attribute(&attribute), Some(fact));
        assert_eq!(Fact::from_wire(fact.key(), fact.wire_value().map(|one| one.0), fact.wire_value().and_then(|one| one.1)), Some(fact));
    }

    /// A range reaches LLVM half-open and at its integer's width, and comes
    /// back the bounds it was stated with; an empty one says nothing.
    #[test]
    fn a_range_is_its_attribute_at_the_width_of_what_it_bounds() {
        let ty = crate::types::Types::default().int(16);
        let bounds = |lo, hi| Fact::Range(Bounds { lo, hi });
        assert_eq!(bounds(0, 1).typed_attribute(ty, 16), Some(Attribute::Range { ty, lower: 0, upper: 2 }));
        assert_eq!(bounds(-1, 1).typed_attribute(ty, 16), Some(Attribute::Range { ty, lower: 0xffff, upper: 2 }));
        assert_eq!(Fact::of_range(0xffff, 2, 16, true), Some(bounds(-1, 1)));
        assert_eq!(Fact::of_range(0, 2, 16, false), Some(bounds(0, 1)));
        let attributes = [bounds(3, 9).typed_attribute(ty, 8).unwrap(), Attribute::Flag("noalias".to_owned())];
        let facts = Facts::of_typed(&attributes, |_| Some(8), false);
        assert_eq!((facts.range(), facts.no_alias()), (Some(Bounds { lo: 3, hi: 9 }), true));
        assert_eq!(Fact::from_wire("range", Some(5), Some(2)), None, "lo above hi is no range");
    }

    /// A pass asks the fact, never the attribute's spelling: no source outside
    /// the fact table, the parser and printer, and the tests names one as a
    /// flag. Every `Attribute::Flag("noalias")` or `has(attrs, "nocapture")`
    /// left behind is a second way to state a fact.
    #[test]
    fn no_pass_reads_a_fact_by_the_name_of_its_attribute() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let keys: Vec<&str> = Fact::examples().into_iter().filter(|fact| matches!(fact.attribute(), Some(Attribute::Flag(_)))).map(|fact| fact.key()).collect();
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
                } else if name.ends_with(".rs") && !name.contains("test") && !matches!(name.as_str(), "facts.rs" | "parse.rs" | "print.rs" | "opcode.rs") {
                    let text = std::fs::read_to_string(&path).expect("source");
                    // Test modules sit at the end of a file.
                    let source = text.split("#[cfg(test)]").next().unwrap_or_default();
                    for (at, line) in source.lines().enumerate() {
                        let reads = line.contains("Attribute::Flag(") || line.contains("has(") || line.contains("states(");
                        if reads && keys.iter().any(|key| line.contains(&format!("\"{key}\""))) {
                            found.push(format!("{}:{}: {}", path.display(), at + 1, line.trim()));
                        }
                    }
                }
            }
        }
        assert_eq!(found, Vec::<String>::new());
    }

    /// A pass asks the fact, never the attribute's spelling: no source outside
    /// the fact table, the parser and printer, and the tests names one as a
    /// flag. Every `Attribute::Flag("noalias")` or `has(attrs, "nocapture")`
    /// left behind is a second way to state a fact.
    #[test]
    fn no_pass_reads_a_fact_by_the_name_of_its_attribute() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let keys: Vec<&str> = Fact::examples().into_iter().filter(|fact| matches!(fact.attribute(), Some(Attribute::Flag(_)))).map(|fact| fact.key()).collect();
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
                } else if name.ends_with(".rs") && !name.contains("test") && !matches!(name.as_str(), "facts.rs" | "parse.rs" | "print.rs" | "opcode.rs") {
                    let text = std::fs::read_to_string(&path).expect("source");
                    // Test modules sit at the end of a file.
                    let source = text.split("#[cfg(test)]").next().unwrap_or_default();
                    for (at, line) in source.lines().enumerate() {
                        let reads = line.contains("Attribute::Flag(") || line.contains("has(") || line.contains("states(");
                        if reads && keys.iter().any(|key| line.contains(&format!("\"{key}\""))) {
                            found.push(format!("{}:{}: {}", path.display(), at + 1, line.trim()));
                        }
                    }
                }
            }
        }
        assert_eq!(found, Vec::<String>::new());
    }

    /// Each way to state inlining is its own attribute, reads back, and `Never` outranks.
    #[test]
    fn each_inlining_has_its_attribute() {
        for (how, name) in [(Inlining::Never, "noinline"), (Inlining::Hint, "inlinehint"), (Inlining::Always, "alwaysinline")] {
            assert_eq!(Fact::Inline(how).attribute(), Some(Attribute::Flag(name.to_owned())));
            assert_eq!(Facts::of(&[Attribute::Flag(name.to_owned())]).inline(), Some(how));
            assert_eq!(Fact::from_wire("inline", Fact::Inline(how).wire_value().map(|one| one.0), None), Some(Fact::Inline(how)));
        }
        assert!(Inlining::Never < Inlining::Hint && Inlining::Hint < Inlining::Always);
    }
}
