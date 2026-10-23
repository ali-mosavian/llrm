//! `features.toml`: what a CPU says of each pattern-predicate atom.

use std::collections::{BTreeMap, BTreeSet};

use crate::predicate::Verdict;

pub struct Cpu {
    features: BTreeSet<String>,
    atoms: BTreeMap<String, AtomRule>,
    dynamic: BTreeMap<String, String>,
    dead_type_prefixes: Vec<String>,
    node_predicates: BTreeMap<String, String>,
}

enum AtomRule {
    Constant(bool),
    AnyOf(Vec<String>),
}

impl Cpu {
    pub fn load(
        text: &str,
        name: &str,
    ) -> Result<Cpu, String> {
        let doc: toml::Table = text.parse().map_err(|e| format!("features.toml: {e}"))?;
        let features = doc
            .get("cpu")
            .and_then(|c| c.get(name))
            .and_then(|c| c.get("features"))
            .and_then(|f| f.as_array())
            .ok_or_else(|| format!("features.toml has no cpu `{name}`"))?
            .iter()
            .map(|f| f.as_str().map(str::to_string).ok_or("a feature is not a string"))
            .collect::<Result<_, _>>()?;
        let mut atoms = BTreeMap::new();
        for (atom, rule) in doc.get("atom").and_then(|a| a.as_table()).into_iter().flatten() {
            let rule = match rule {
                toml::Value::Boolean(b) => AtomRule::Constant(*b),
                toml::Value::Array(list) => {
                    AtomRule::AnyOf(list.iter().map(|f| f.as_str().unwrap_or_default().to_string()).collect())
                }
                _ => return Err(format!("atom `{atom}`: expected a boolean or a feature list")),
            };
            atoms.insert(atom.clone(), rule);
        }
        // A CPU's own `[cpu.NAME.atom]` rows win over the shared `[atom]` rows.
        for (atom, rule) in doc
            .get("cpu")
            .and_then(|c| c.get(name))
            .and_then(|c| c.get("atom"))
            .and_then(|a| a.as_table())
            .into_iter()
            .flatten()
        {
            let toml::Value::Boolean(b) = rule else { return Err(format!("atom `{atom}`: expected a boolean")) };
            atoms.insert(atom.clone(), AtomRule::Constant(*b));
        }
        let dynamic = doc
            .get("dynamic")
            .and_then(|d| d.as_table())
            .into_iter()
            .flatten()
            .map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_string()))
            .collect();
        let dead_type_prefixes = doc
            .get("types")
            .and_then(|t| t.get("dead_prefixes"))
            .and_then(|p| p.as_array())
            .into_iter()
            .flatten()
            .filter_map(|p| p.as_str().map(str::to_string))
            .collect();
        let node_predicates = doc
            .get("node_predicate")
            .and_then(|d| d.as_table())
            .into_iter()
            .flatten()
            .map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_string()))
            .collect();
        Ok(Cpu { features, atoms, dynamic, dead_type_prefixes, node_predicates })
    }

    /// The verdict of one atom; an atom nothing states is an error, not a
    /// guess.
    pub fn verdict(
        &self,
        atom: &str,
    ) -> Result<Verdict, String> {
        if let Some(rule) = self.atoms.get(atom) {
            return Ok(match rule {
                AtomRule::Constant(true) => Verdict::Yes,
                AtomRule::Constant(false) => Verdict::No,
                AtomRule::AnyOf(list) => {
                    if list.iter().any(|f| self.features.contains(f)) {
                        Verdict::Yes
                    } else {
                        Verdict::No
                    }
                }
            });
        }
        if self.dynamic.contains_key(atom) {
            return Ok(Verdict::Dynamic);
        }
        if let Some(feature) = atom.strip_prefix("Subtarget->has").and_then(|a| a.strip_suffix("()")) {
            return Ok(if self.features.contains(feature) { Verdict::Yes } else { Verdict::No });
        }
        Err(format!("features.toml says nothing of the predicate atom `{atom}`"))
    }

    /// The `llrm_dagisel::predicates` function for a PatFrag name.
    pub fn node_predicate_fn(
        &self,
        name: &str,
    ) -> Option<&str> {
        self.node_predicates.get(name).map(String::as_str)
    }

    /// The `PatternContext` method that decides a dynamic atom.
    pub fn dynamic_method(
        &self,
        atom: &str,
    ) -> Option<&str> {
        self.dynamic.get(atom).map(String::as_str)
    }

    /// Whether `MVT::<name>` is a type no selection here produces.
    pub fn dead_type(
        &self,
        mvt_name: &str,
    ) -> bool {
        self.dead_type_prefixes.iter().any(|p| {
            mvt_name.strip_prefix(p.as_str()).and_then(|rest| rest.bytes().next()).is_some_and(|b| b.is_ascii_digit())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = include_str!("../features.toml");

    #[test]
    fn i486_has_no_cmov_and_p5_has_cx8() {
        let i486 = Cpu::load(TEXT, "i486").unwrap();
        let p5 = Cpu::load(TEXT, "p5").unwrap();
        assert_eq!(i486.verdict("Subtarget->canUseCMOV()").unwrap(), Verdict::No);
        assert_eq!(i486.verdict("Subtarget->hasCX8()").unwrap(), Verdict::No);
        assert_eq!(p5.verdict("Subtarget->hasCX8()").unwrap(), Verdict::Yes);
        assert_eq!(p5.verdict("Subtarget->hasX87()").unwrap(), Verdict::Yes);
    }

    #[test]
    fn vector_types_are_dead_and_scalars_are_not() {
        let cpu = Cpu::load(TEXT, "i486").unwrap();
        assert!(cpu.dead_type("v4i32") && cpu.dead_type("nxv2f32"));
        assert!(!cpu.dead_type("i32") && !cpu.dead_type("f80"));
    }

    #[test]
    fn an_unstated_atom_is_refused() {
        assert!(Cpu::load(TEXT, "i486").unwrap().verdict("Subtarget->isTargetFoo()").is_err());
    }
}
