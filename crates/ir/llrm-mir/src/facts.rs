//! What a language promises of a program, as MIR carries it, and the one way
//! to read it.
//!
//! A fact is a promise a pass may use and may also ignore: dropping one never
//! changes what the program means. What changes meaning (volatile, a callee
//! that returns twice) is part of the IR proper, not a fact.
//!
//! Each fact is declared once, in `facts!`: its attribute, the kinds of
//! subject it can be stated of, and what a pass that rewrites code does with
//! it. A frontend states facts through `llrm_hir::facts`; a pass reads them
//! through [`Facts`] and never parses an attribute by name.

use crate::module::Function;
use crate::opcode::{Attribute, Flags};

/// The kind of thing a fact is stated of.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Function,
    Callable,
    Param,
    Return,
    Instruction,
    Block,
    Place,
    Object,
    Program,
}

/// What becomes of a fact when two instructions that state it become one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Merge {
    /// Both state it, so it holds for the one.
    Intersect,
    /// It is a property of the declaration, which a merge does not touch.
    Keep,
}

/// What becomes of a fact when its instruction is moved where it may run
/// without the path that made the fact true.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Hoist {
    Keep,
    /// It holds only on that path, so it is dropped.
    Drop,
}

/// What a pass that rewrites code does with a fact; a clone keeps all.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Policy {
    pub merge: Merge,
    pub hoist: Hoist,
}

impl Policy {
    /// A property of a declaration: no merge or move changes it.
    pub const DECLARED: Policy = Policy { merge: Merge::Keep, hoist: Hoist::Keep };
}

macro_rules! facts {
    (
        flags { $($flag:ident $fmethod:ident $fkey:literal on [$($fkind:ident),+] $fpolicy:expr;)* }
        valued { $($valued:ident($vty:ty) $vmethod:ident $vkey:literal on [$($vkind:ident),+] $vpolicy:expr;)* }
        custom { $($custom:ident($cty:ty) $cmethod:ident $ckey:literal on [$($ckind:ident),+] $cpolicy:expr;)* }
        bits { $($bit:ident $bmethod:ident $bkey:literal $bflag:expr, on [$($bkind:ident),+] $bpolicy:expr;)* }
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

            /// What a rewriting pass does with it.
            pub fn policy(self) -> Policy {
                match self {
                    $(Fact::$flag => $fpolicy,)*
                    $(Fact::$valued(_) => $vpolicy,)*
                    $(Fact::$custom(_) => $cpolicy,)*
                    $(Fact::$bit => $bpolicy,)*
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
        NoAlias no_alias "noalias" on [Param] Policy::DECLARED;
        ReadOnly read_only "readonly" on [Param] Policy::DECLARED;
        NonNull non_null "nonnull" on [Param] Policy::DECLARED;
        NoReturn no_return "noreturn" on [Function, Callable] Policy::DECLARED;
    }
    valued {
        Dereferenceable(u64) dereferenceable "dereferenceable" on [Param] Policy::DECLARED;
    }
    custom {
        Memory(Effect) memory "memory" on [Function, Callable] Policy::DECLARED;
    }
    bits {
        NoSignedWrap no_signed_wrap "nsw" Flags::NSW, on [Instruction] Policy { merge: Merge::Intersect, hoist: Hoist::Keep };
        NoUnsignedWrap no_unsigned_wrap "nuw" Flags::NUW, on [Instruction] Policy { merge: Merge::Intersect, hoist: Hoist::Keep };
    }
}

impl Fact {
    /// The MIR attribute that carries the fact, none where it is an
    /// instruction flag.
    pub fn attribute(self) -> Option<Attribute> {
        match self {
            Fact::Dereferenceable(bytes) => Some(Attribute::Int(self.key().to_owned(), bytes)),
            Fact::Memory(effect) => Some(Attribute::Memory(vec![(None, effect.spelled().to_owned())])),
            Fact::NoSignedWrap | Fact::NoUnsignedWrap => None,
            _ => Some(Attribute::Flag(self.key().to_owned())),
        }
    }

    /// The fact a carrier states, if it states one.
    pub fn of_attribute(attribute: &Attribute) -> Option<Fact> {
        match attribute {
            Attribute::Flag(name) => Fact::flag(name),
            Attribute::Int(name, value) => Fact::valued(name, *value),
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

    /// What holds of the one instruction two instructions become.
    pub fn merged(&self, other: &Facts) -> Facts {
        let keeps = |fact: &Fact| match fact.policy().merge {
            Merge::Intersect => other.contains(*fact),
            Merge::Keep => true,
        };
        Facts(self.0.iter().copied().filter(keeps).collect())
    }

    /// What holds of an instruction moved where its path no longer guards it.
    pub fn speculated(&self) -> Facts {
        Facts(self.0.iter().copied().filter(|fact| fact.policy().hoist == Hoist::Keep).collect())
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

    /// A declared fact survives a merge and a move.
    #[test]
    fn a_declared_fact_is_not_changed_by_merge_or_speculation() {
        let facts = Facts::of(&[Fact::NoAlias.attribute().unwrap()]);
        assert!(facts.merged(&Facts::default()).no_alias());
        assert!(facts.speculated().no_alias());
    }
}
