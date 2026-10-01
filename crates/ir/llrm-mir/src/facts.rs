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

            /// The value a fact carries on the wire, if it carries one.
            pub fn wire_value(self) -> Option<i64> {
                match self {
                    $(Fact::$flag => None,)*
                    $(Fact::$bit => None,)*
                    $(Fact::$valued(value) => Some(value as i64),)*
                    $(Fact::$custom(value) => Some(Wire::wire(value)),)*
                }
            }

            /// The fact of a wire name and value; none where either is not one's.
            pub fn from_wire(key: &str, value: Option<i64>) -> Option<Fact> {
                match (key, value) {
                    $(($fkey, None) => Some(Fact::$flag),)*
                    $(($bkey, None) => Some(Fact::$bit),)*
                    $(($vkey, Some(value)) => Some(Fact::$valued(value as $vty)),)*
                    $(($ckey, Some(value)) => <$cty as Wire>::unwire(value).map(Fact::$custom),)*
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

/// A fact's value as a number on the wire.
pub trait Wire: Sized {
    fn wire(self) -> i64;
    fn unwire(value: i64) -> Option<Self>;
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
}

facts! {
    flags {
        NoAlias no_alias "noalias" on [Param];
        ReadOnly read_only "readonly" on [Param];
        NonNull non_null "nonnull" on [Param];
        NoCapture no_capture "nocapture" on [Param, Operand];
        WriteOnly write_only "writeonly" on [Operand];
        NoReturn no_return "noreturn" on [Callable];
    }
    valued {
        Dereferenceable(u64) dereferenceable "dereferenceable" on [Param];
        Align(u64) align "align" on [Param, Object];
        Initializes(u64) initializes "initializes" on [Operand];
    }
    custom {
        Memory(Effect) memory "memory" on [Callable];
    }
    bits {
        NoSignedWrap no_signed_wrap "nsw" Flags::NSW, on [Instruction];
        NoUnsignedWrap no_unsigned_wrap "nuw" Flags::NUW, on [Instruction];
        InBounds in_bounds "inbounds" Flags::INBOUNDS, on [Instruction, Operand];
    }
}

impl Fact {
    /// The MIR attribute that carries the fact, none where it is an
    /// instruction flag.
    pub fn attribute(self) -> Option<Attribute> {
        match self {
            Fact::Dereferenceable(bytes) => Some(Attribute::Int(self.key().to_owned(), bytes)),
            Fact::Memory(effect) => Some(Attribute::Memory(vec![(None, effect.spelled().to_owned())])),
            Fact::Initializes(bytes) => Some(Attribute::Initializes(vec![(0, bytes as i64)])),
            Fact::NoSignedWrap | Fact::NoUnsignedWrap | Fact::InBounds => None,
            _ => Some(Attribute::Flag(self.key().to_owned())),
        }
    }

    /// The fact a carrier states, if it states one.
    pub fn of_attribute(attribute: &Attribute) -> Option<Fact> {
        match attribute {
            Attribute::Flag(name) => Fact::flag(name),
            Attribute::Int(name, value) => Fact::valued(name, *value),
            Attribute::Initializes(ranges) => match ranges[..] {
                [(0, bytes)] if bytes > 0 => Some(Fact::Initializes(bytes as u64)),
                _ => None,
            },
            Attribute::Memory(locations) => match locations[..] {
                [(None, ref access)] => Effect::of_spelling(access).map(Fact::Memory),
                _ => None,
            },
            _ => None,
        }
    }
}

impl Wire for Effect {
    fn wire(self) -> i64 {
        self as i64
    }

    fn unwire(value: i64) -> Option<Effect> {
        [Effect::None, Effect::Read, Effect::Write].into_iter().find(|one| *one as i64 == value)
    }
}

impl Effect {
    fn spelled(self) -> &'static str {
        match self {
            Effect::None => "none",
            Effect::Read => "read",
            Effect::Write => "write",
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
                None => assert!(Fact::of_flags(fact.flags()).contains(&fact), "{} is a flag", fact.key()),
            }
            assert!(Fact::is_named(fact.key()));
            assert_eq!(Fact::from_wire(fact.key(), fact.wire_value()), Some(fact), "{} on the wire", fact.key());
            assert!(!fact.kinds().is_empty(), "{} is of no subject", fact.key());
        }
    }

    /// A pass asks the fact, not the attribute's spelling.
    #[test]
    fn facts_are_read_from_attributes_and_others_are_ignored() {
        let attributes = vec![Attribute::Flag("noalias".to_owned()), Attribute::Flag("nocallback".to_owned()), Attribute::Flag("noalias".to_owned())];
        let facts = Facts::of(&attributes);
        assert!(facts.no_alias());
        assert_eq!(facts.iter().count(), 1);
        assert!(!Facts::of(&[Attribute::Flag("readonly".to_owned())]).no_alias());
    }

}
