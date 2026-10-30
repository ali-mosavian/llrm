//! An instruction set: a bundle of features that extends another bundle, from
//! the family's `machine/isa/<family>/` files
//! (`docs/architecture/machine-description.md`).

use std::collections::BTreeSet;

/// One feature of the family's catalog (`features.toml`).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Feature {
    pub enables: Vec<String>,
    pub requires: Vec<String>,
    pub selects: Vec<String>,
    pub os_enables: bool,
}

/// A family's catalog and bundles, as text.
pub struct Family<'a> {
    pub catalog: &'a str,
    /// Bundle name and file text.
    pub bundles: &'a [(&'a str, &'a str)],
}

/// x86's files; the minimum is `i386`.
pub const X86: Family<'static> = Family {
    catalog: include_str!("../../../../machine/isa/x86/features.toml"),
    bundles: &[
        ("i386", include_str!("../../../../machine/isa/x86/i386.toml")),
        ("i486", include_str!("../../../../machine/isa/x86/i486.toml")),
        ("pentium", include_str!("../../../../machine/isa/x86/pentium.toml")),
        ("i686", include_str!("../../../../machine/isa/x86/i686.toml")),
        ("pentium3", include_str!("../../../../machine/isa/x86/pentium3.toml")),
    ],
};

fn strings(
    table: &toml::Table,
    key: &str,
    at: &str,
) -> Result<Vec<String>, String> {
    let Some(value) = table.get(key) else { return Ok(Vec::new()) };
    let list = value.as_array().ok_or_else(|| format!("{at}: `{key}` is no list"))?;
    list.iter()
        .map(|one| one.as_str().map(str::to_owned).ok_or_else(|| format!("{at}: `{key}` holds a non-string")))
        .collect()
}

impl Family<'_> {
    /// The catalog: feature name and feature, in name order.
    pub fn catalog(&self) -> Result<Vec<(String, Feature)>, String> {
        let doc: toml::Table = self.catalog.parse().map_err(|e| format!("features.toml: {e}"))?;
        let mut found = Vec::new();
        for (name, value) in &doc {
            let at = format!("features.toml [{name}]");
            let table = value.as_table().ok_or_else(|| format!("{at}: not a table"))?;
            let feature = Feature {
                enables: strings(table, "enables", &at)?,
                requires: strings(table, "requires", &at)?,
                selects: strings(table, "selects", &at)?,
                os_enables: table.get("os_enables").and_then(|one| one.as_bool()).unwrap_or(false),
            };
            found.push((name.clone(), feature));
        }
        Ok(found)
    }

    /// The features a bundle holds, its `extends` chain included. An unknown
    /// bundle or feature, a cycle, and a feature whose `requires` is not in the
    /// set are errors.
    pub fn features(
        &self,
        bundle: &str,
    ) -> Result<BTreeSet<String>, String> {
        let catalog = self.catalog()?;
        let mut held = BTreeSet::new();
        let mut seen = Vec::new();
        let mut next = Some(bundle.to_owned());
        while let Some(name) = next.take() {
            if seen.contains(&name) {
                return Err(format!("{name}: extends itself through {seen:?}"));
            }
            let text = self
                .bundles
                .iter()
                .find(|(one, _)| *one == name)
                .map(|(_, text)| *text)
                .ok_or_else(|| format!("no bundle `{name}`"))?;
            let doc: toml::Table = text.parse().map_err(|e| format!("{name}.toml: {e}"))?;
            for feature in strings(&doc, "features", &format!("{name}.toml"))? {
                if !catalog.iter().any(|(known, _)| *known == feature) {
                    return Err(format!("{name}.toml: no feature `{feature}` in the catalog"));
                }
                held.insert(feature);
            }
            next = doc.get("extends").and_then(|one| one.as_str()).map(str::to_owned);
            seen.push(name);
        }
        for (name, feature) in &catalog {
            if held.contains(name) {
                if let Some(missing) = feature.requires.iter().find(|one| !held.contains(*one)) {
                    return Err(format!("{bundle}: `{name}` requires `{missing}`"));
                }
            }
        }
        Ok(held)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|one| (*one).to_owned()).collect()
    }

    #[test]
    fn every_x86_bundle_resolves_and_each_extends_the_one_before() {
        let mut before = BTreeSet::new();
        for (name, _) in X86.bundles {
            let now = X86.features(name).unwrap();
            assert!(before.is_subset(&now), "{name} drops a feature of its parent");
            before = now;
        }
    }

    #[test]
    fn the_minimum_has_no_feature_and_the_486_has_the_fpu() {
        assert!(X86.features("i386").unwrap().is_empty());
        assert!(X86.features("i486").unwrap().is_superset(&set(&["x87", "bswap", "cmpxchg", "xadd"])));
    }

    #[test]
    fn a_cycle_a_missing_feature_and_an_unmet_requirement_are_errors() {
        let catalog = "[a]\nrequires = [\"b\"]\n[b]\n";
        let family = |bundles: &'static [(&'static str, &'static str)]| Family { catalog, bundles };
        let cycle = family(&[("x", "extends = \"y\"\n"), ("y", "extends = \"x\"\n")]);
        assert!(cycle.features("x").unwrap_err().contains("extends itself"));
        let unknown = family(&[("x", "features = [\"c\"]\n")]);
        assert!(unknown.features("x").unwrap_err().contains("no feature `c`"));
        let unmet = family(&[("x", "features = [\"a\"]\n")]);
        assert!(unmet.features("x").unwrap_err().contains("requires `b`"));
    }
}
