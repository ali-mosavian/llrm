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
use crate::opcode::Attribute;

/// The kind of thing a fact is stated of.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Function,
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
    ($($variant:ident $method:ident $key:literal on [$($kind:ident),+] $policy:expr;)*) => {
        /// A promise of the language, stated once per subject.
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        pub enum Fact {
            $($variant,)*
        }

        impl Fact {
            /// Every fact, for tests that must name each.
            pub const ALL: &'static [Fact] = &[$(Fact::$variant,)*];

            /// The name the codec and diagnostics use.
            pub fn key(self) -> &'static str {
                match self { $(Fact::$variant => $key,)* }
            }

            /// The fact of a codec name.
            pub fn named(key: &str) -> Option<Fact> {
                match key { $($key => Some(Fact::$variant),)* _ => None }
            }

            /// The kinds of subject it can be stated of.
            pub fn kinds(self) -> &'static [Kind] {
                match self { $(Fact::$variant => &[$(Kind::$kind),+],)* }
            }

            /// What a rewriting pass does with it.
            pub fn policy(self) -> Policy {
                match self { $(Fact::$variant => $policy,)* }
            }
        }

        impl Facts {
            $(
                pub fn $method(&self) -> bool {
                    self.contains(Fact::$variant)
                }
            )*
        }
    };
}

facts! {
    NoAlias no_alias "noalias" on [Param] Policy::DECLARED;
}

impl Fact {
    /// The MIR carrier of the fact.
    pub fn attribute(self) -> Attribute {
        match self {
            Fact::NoAlias => Attribute::Flag("noalias".to_owned()),
        }
    }

    /// The fact a carrier states, if it states one.
    pub fn of_attribute(attribute: &Attribute) -> Option<Fact> {
        match attribute {
            Attribute::Flag(name) if name == "noalias" => Some(Fact::NoAlias),
            _ => None,
        }
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
        for &fact in Fact::ALL {
            assert_eq!(Fact::of_attribute(&fact.attribute()), Some(fact), "{}", fact.key());
            assert_eq!(Fact::named(fact.key()), Some(fact));
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
        let facts = Facts::of(&[Fact::NoAlias.attribute()]);
        assert!(facts.merged(&Facts::default()).no_alias());
        assert!(facts.speculated().no_alias());
    }
}
