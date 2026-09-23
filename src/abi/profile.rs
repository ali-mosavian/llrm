//! Port of `qbopt/abi/profile.py`: hash-checked external call profiles.

use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::abi::runtime::{self, Contract, Reg};
use crate::objectfile::omf;
use crate::support::hash::IndexMap;
use crate::support::pyjson::{self, Json};
use crate::support::pypath;
use crate::support::pyrepr;

/// What `load` raises: `ValueError` (with its subclasses) or `OSError`,
/// each carrying Python's `str(error)`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProfileError {
    ValueError(String),
    OSError(String),
}

impl fmt::Display for ProfileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProfileError::ValueError(text) | ProfileError::OSError(text) => formatter.write_str(text),
        }
    }
}

impl std::error::Error for ProfileError {}

impl From<omf::ValueError> for ProfileError {
    fn from(error: omf::ValueError) -> Self {
        ProfileError::ValueError(error.0)
    }
}

fn value_error(text: String) -> ProfileError {
    ProfileError::ValueError(text)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Profile {
    pub rules: Vec<Contract>,
    pub fingerprint: String,
}

/// `sha256(json.dumps(document, sort_keys=True, separators=(",", ":")).encode()).hexdigest()`.
fn digest(document: &Json) -> String {
    let text = pyjson::dumps(document, None, Some((",", ":")), true);
    Sha256::digest(text.as_bytes()).iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Combine independently audited profiles without an order-dependent override.
pub fn combined(profiles: Vec<Profile>) -> Result<Profile, ProfileError> {
    if profiles.is_empty() {
        return Err(value_error("at least one contract profile is required".to_owned()));
    }
    if profiles.len() == 1 {
        return Ok(profiles.into_iter().next().unwrap());
    }
    let mut rules: IndexMap<String, Contract> = IndexMap::default();
    for one in &profiles {
        for rule in &one.rules {
            if rules.contains_key(&rule.name) {
                return Err(value_error(format!(
                    "external contract is declared by multiple profiles: {}",
                    rule.name
                )));
            }
            rules.insert(rule.name.clone(), rule.clone());
        }
    }
    let mut fingerprints: Vec<String> = profiles.iter().map(|one| one.fingerprint.clone()).collect();
    fingerprints.sort();
    let document = Json::Dict(IndexMap::from_iter([
        ("version".to_owned(), Json::Int(1)),
        ("profiles".to_owned(), Json::List(fingerprints.into_iter().map(Json::Str).collect())),
    ]));
    let fingerprint = digest(&document);
    let mut names: Vec<&String> = rules.keys().collect();
    names.sort();
    Ok(Profile { rules: names.into_iter().map(|name| rules[name].clone()).collect(), fingerprint })
}

pub fn load_many(paths: &[PathBuf], root: Option<&Path>) -> Result<Profile, ProfileError> {
    combined(paths.iter().map(|path| load(path, root)).collect::<Result<Vec<_>, _>>()?)
}

pub fn _unique(pairs: Vec<(String, Json)>) -> Result<Json, String> {
    let mut result: IndexMap<String, Json> = IndexMap::default();
    for (name, value) in pairs {
        if result.contains_key(&name) {
            return Err(format!("duplicate contract profile key: {name}"));
        }
        result.insert(name, value);
    }
    Ok(Json::Dict(result))
}

/// `repr(sorted(names))`.
fn sorted_repr(names: &[&str]) -> String {
    let mut names = names.to_vec();
    names.sort();
    let items: Vec<String> = names.iter().map(|name| pyrepr::string(name)).collect();
    format!("[{}]", items.join(", "))
}

pub fn _fields<'a>(
    value: &'a Json,
    required: &[&str],
    optional: &[&str],
) -> Result<&'a IndexMap<String, Json>, ProfileError> {
    let fits = match value {
        Json::Dict(fields) => {
            required.iter().all(|name| fields.contains_key(*name))
                && fields.keys().all(|key| required.contains(&key.as_str()) || optional.contains(&key.as_str()))
        }
        _ => false,
    };
    match value {
        Json::Dict(fields) if fits => Ok(fields),
        _ => Err(value_error(format!(
            "expected profile fields {}, optional {}",
            sorted_repr(required),
            sorted_repr(optional)
        ))),
    }
}

/// `None` for a missing key and for JSON null alike, as `dict.get` reads both.
fn get<'a>(row: &'a IndexMap<String, Json>, name: &str) -> Option<&'a Json> {
    row.get(name).filter(|value| **value != Json::None)
}

pub fn load(path: &Path, root: Option<&Path>) -> Result<Profile, ProfileError> {
    let data = std::fs::read(path).map_err(|error| ProfileError::OSError(pypath::os_error(&error, path)))?;
    let text = pypath::decode_utf8(&data).map_err(value_error)?;
    let parsed = pyjson::loads_with(&text, Some(&_unique)).map_err(value_error)?;
    let document = _fields(&parsed, &["version", "artifacts", "contracts"], &[])?;
    if !matches!(document["version"], Json::Int(1)) {
        return Err(value_error("unsupported contract profile version".to_owned()));
    }
    let (artifacts, declarations) = match (&document["artifacts"], &document["contracts"]) {
        (Json::Dict(artifacts), Json::Dict(declarations)) if !artifacts.is_empty() && !declarations.is_empty() => {
            (artifacts, declarations)
        }
        _ => return Err(value_error("contract profile needs nonempty artifacts and contracts maps".to_owned())),
    };
    let directory = pypath::resolve(root.unwrap_or_else(|| path.parent().unwrap_or(Path::new(""))));
    let mut contents: IndexMap<String, Vec<u8>> = IndexMap::default();
    for (name, expected) in artifacts {
        let relative = Path::new(name);
        let location = pypath::resolve(&directory.join(relative));
        if relative.is_absolute()
            || relative.components().any(|part| part == std::path::Component::ParentDir)
            || !location.starts_with(&directory)
        {
            return Err(value_error(format!("profile artifact must stay inside the contract root: {name}")));
        }
        let valid = match expected {
            Json::Str(expected) => {
                expected.chars().count() == 64 && expected.chars().all(|c| "0123456789abcdef".contains(c))
            }
            _ => false,
        };
        if !valid {
            return Err(value_error(format!("invalid SHA-256 for profile artifact: {name}")));
        }
        let data =
            std::fs::read(&location).map_err(|error| ProfileError::OSError(pypath::os_error(&error, &location)))?;
        let found: String = Sha256::digest(&data).iter().map(|byte| format!("{byte:02x}")).collect();
        if Json::Str(found) != *expected {
            return Err(value_error(format!("contract profile artifact hash mismatch: {name}")));
        }
        contents.insert(name.clone(), data);
    }
    let mut symbols: IndexMap<String, IndexMap<String, BTreeSet<Option<String>>>> = IndexMap::default();
    let mut rules = Vec::new();
    let mut sorted: Vec<(&String, &Json)> = declarations.iter().collect();
    sorted.sort_by(|one, other| one.0.cmp(other.0));
    for (name, declaration) in sorted {
        let row = _fields(declaration, &["defined_in", "inputs", "evidence"], &["cleanup", "member"])?;
        let defining = match &row["defined_in"] {
            Json::Str(defining) if contents.contains_key(defining) => defining,
            _ => return Err(value_error(format!("{name}: defining file is not a verified artifact"))),
        };
        if !symbols.contains_key(defining) {
            let archived = omf::library_modules(&contents[defining])?;
            let record_sets: Vec<(Option<String>, Vec<_>)> = if archived.is_empty() {
                vec![(None, omf::parse(&contents[defining])?)]
            } else {
                archived.into_iter().map(|(member, records)| (Some(member), records)).collect()
            };
            let mut definitions: IndexMap<String, BTreeSet<Option<String>>> = IndexMap::default();
            for (member, records) in &record_sets {
                for seg in 1..omf::segments(records).len() as i64 {
                    for symbol in omf::pubdef_names(records, seg)?.into_values() {
                        definitions.entry(symbol).or_default().insert(member.clone());
                    }
                }
            }
            symbols.insert(defining.clone(), definitions);
        }
        let definitions = symbols[defining].get(name).cloned().unwrap_or_default();
        let member = get(row, "member");
        let member = match member {
            None => None,
            Some(Json::Str(member)) if !member.is_empty() => Some(member),
            Some(_) => return Err(value_error(format!("{name}: member must be a nonempty archive module name"))),
        };
        if name.is_empty() || definitions.is_empty() {
            return Err(value_error(format!("{name}: symbol is not defined in {defining}")));
        }
        if let Some(member) = member {
            if !definitions.contains(&Some(member.clone())) {
                return Err(value_error(format!("{name}: symbol is not defined by {member} in {defining}")));
            }
        }
        if member.is_none() && definitions.len() != 1 {
            return Err(value_error(format!(
                "{name}: symbol is defined by multiple members of {defining}; specify member"
            )));
        }
        let inputs: Vec<&String> = match &row["inputs"] {
            Json::List(items) if items.iter().all(|item| matches!(item, Json::Str(_))) => items
                .iter()
                .map(|item| match item {
                    Json::Str(item) => item,
                    _ => unreachable!(),
                })
                .collect(),
            _ => return Err(value_error(format!("{name}: inputs must be a list of register names"))),
        };
        let registers: BTreeSet<Reg> =
            inputs.iter().map(|item| Reg::from_value(item)).collect::<Result<_, _>>().map_err(value_error)?;
        if registers.len() != inputs.len() {
            return Err(value_error(format!("{name}: duplicate input register")));
        }
        let evidence = match &row["evidence"] {
            Json::Str(evidence) if !evidence.trim().is_empty() => evidence,
            _ => return Err(value_error(format!("{name}: audit evidence is required"))),
        };
        let cleanup = match get(row, "cleanup") {
            None => None,
            Some(Json::Int(cleanup)) if (0..=65535).contains(cleanup) && cleanup % 2 == 0 => Some(*cleanup),
            Some(_) => return Err(value_error(format!("{name}: cleanup must be an even byte count or null"))),
        };
        rules.push(Contract {
            inputs: Some(registers),
            cleanup,
            evidence: evidence.clone(),
            ..runtime::worst(name)
        });
    }
    let fingerprint = digest(&parsed);
    Ok(Profile { rules, fingerprint })
}

#[cfg(test)]
#[path = "profile_tests.rs"]
mod tests;
