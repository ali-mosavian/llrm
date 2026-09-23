//! Port of `qbopt/abi/linkunit.py`: the OMF objects and libraries one LINK
//! invocation sees, in order.
//!
//! An external call's definition may be in a sibling OBJ or an archive
//! member; this resolves those names before any object is changed and
//! supplies a deliberately conservative interface for a resolved definition
//! whose detailed runtime contract is not yet known. That interface retains
//! every allocatable GP input and keeps the worst memory, clobber, control
//! and error effects.

use std::cell::RefCell;
use std::fmt;
use std::path::PathBuf;
use std::rc::Rc;

use sha2::{Digest, Sha256};

use crate::abi::inputscan::{self, Scanner};
use crate::abi::runtime::{self, Contract};
use crate::objectfile::module;
use crate::objectfile::omf::{self, Record};
use crate::support::hash::IndexMap;
use crate::support::pypath;
use crate::support::pyrepr;

/// What `LinkUnit` raises, each with Python's `str(error)`:
/// `LinkUnitError(ValueError)`, any other `ValueError`, or `OSError`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LinkError {
    LinkUnitError(String),
    ValueError(String),
    OSError(String),
}

impl fmt::Display for LinkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LinkError::LinkUnitError(text) | LinkError::ValueError(text) | LinkError::OSError(text) => {
                formatter.write_str(text)
            }
        }
    }
}

impl std::error::Error for LinkError {}

impl From<omf::ValueError> for LinkError {
    fn from(error: omf::ValueError) -> Self {
        LinkError::ValueError(error.0)
    }
}

fn link_unit_error(text: String) -> LinkError {
    LinkError::LinkUnitError(text)
}

#[derive(Clone, Debug)]
pub struct ObjectInput {
    pub path: PathBuf,
    pub data: Vec<u8>,
    pub records: Vec<Rc<Record>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Definition {
    pub path: PathBuf,
    pub member: Option<String>,
    pub segment: i64,
    pub offset: i64,
}

impl Definition {
    pub fn label(&self) -> String {
        let within = self.member.as_ref().map_or(String::new(), |member| format!("({member})"));
        format!("{}{within}:{}:{:#x}", self.path.display(), self.segment, self.offset)
    }
}

/// Microsoft LINK's symbol identity, independent of OMF spelling case.
pub fn _symbol(name: &str) -> String {
    inputscan::_casefold(name)
}

pub struct LinkUnit {
    pub inputs: Vec<PathBuf>,
    pub objects: Vec<ObjectInput>,
    pub definitions: IndexMap<String, Vec<Definition>>,
    pub fingerprint: String,
    pub scanner: RefCell<Scanner>,
}

impl LinkUnit {
    pub fn read(paths: &[PathBuf]) -> Result<LinkUnit, LinkError> {
        if paths.is_empty() {
            return Err(link_unit_error("a link unit needs at least one OMF input".to_owned()));
        }
        let mut objects: Vec<ObjectInput> = Vec::new();
        let mut definitions: IndexMap<String, Vec<Definition>> = IndexMap::default();
        let mut scanned: Vec<(String, Vec<Rc<Record>>)> = Vec::new();
        let mut digest = Sha256::new();

        for (order, requested) in paths.iter().enumerate() {
            let path = pypath::resolve(requested);
            let data = std::fs::read(&path).map_err(|error| LinkError::OSError(pypath::os_error(&error, &path)))?;
            digest.update((order as u32).to_le_bytes());
            let encoded = path.to_string_lossy().into_owned().into_bytes();
            digest.update((encoded.len() as u32).to_le_bytes());
            digest.update(&encoded);
            digest.update((data.len() as u64).to_le_bytes());
            digest.update(&data);

            let archived = omf::library_modules(&data)?;
            let members: Vec<(Option<String>, Vec<Rc<Record>>)> = if archived.is_empty() {
                vec![(None, omf::parse(&data)?)]
            } else {
                archived.iter().map(|(name, records)| (Some(name.clone()), records.clone())).collect()
            };
            for (member_name, records) in &members {
                let label = match member_name {
                    Some(member) => format!("{}({member})", path.display()),
                    None => path.display().to_string(),
                };
                scanned.push((label, records.clone()));
                let mut unknown: Vec<u8> =
                    records.iter().map(|record| record.r#type).filter(|kind| !omf::NAMES.contains_key(kind)).collect();
                unknown.sort();
                unknown.dedup();
                if !unknown.is_empty() {
                    let kinds: Vec<String> = unknown.iter().map(|kind| format!("{kind:#04x}")).collect();
                    let r#where = match member_name {
                        Some(member) => format!("{}({member})", path.display()),
                        None => path.display().to_string(),
                    };
                    return Err(link_unit_error(format!(
                        "{where}: unrecognized OMF record type(s) {}",
                        kinds.join(", ")
                    )));
                }
                for (name, (segment, offset)) in omf::public_definitions(records)? {
                    definitions.entry(_symbol(&name)).or_default().push(Definition {
                        path: path.clone(),
                        member: member_name.clone(),
                        segment,
                        offset,
                    });
                }
            }
            if archived.is_empty() {
                let records = &members[0].1;
                if module::of(records).is_none() {
                    return Err(link_unit_error(format!(
                        "{}: standalone input has no recognized code module",
                        path.display()
                    )));
                }
                objects.push(ObjectInput { path: path.clone(), data, records: records.clone() });
            }
        }

        if objects.is_empty() {
            return Err(link_unit_error(
                "the link unit contains libraries but no standalone OBJ to optimize".to_owned(),
            ));
        }
        let fingerprint: String = digest.finalize().iter().map(|byte| format!("{byte:02x}")).collect();
        Ok(LinkUnit {
            inputs: paths.iter().map(|path| pypath::resolve(path)).collect(),
            objects,
            definitions,
            fingerprint,
            scanner: RefCell::new(Scanner::new(&scanned)?),
        })
    }

    pub fn definition(&self, name: &str) -> Result<&Definition, LinkError> {
        let found: &[Definition] = self.definitions.get(&_symbol(name)).map_or(&[], Vec::as_slice);
        if found.is_empty() {
            return Err(link_unit_error(format!("unresolved external {}", pyrepr::string(name))));
        }
        // Explicit objects all participate in the link. Two of them exporting
        // the same name is a multiply-defined public and must fail. Archive
        // members are demand-loaded, however: a definition already supplied
        // by an object prevents a library member from being selected, and in
        // an all-library search the first definition in command/member order
        // satisfies the unresolved symbol.
        let objects: Vec<&Definition> = found.iter().filter(|one| one.member.is_none()).collect();
        if objects.len() > 1 {
            let locations: Vec<String> = objects.iter().map(|one| one.label()).collect();
            return Err(link_unit_error(format!(
                "multiply-defined public {}: {}",
                pyrepr::string(name),
                locations.join(", ")
            )));
        }
        Ok(objects.first().copied().unwrap_or(&found[0]))
    }

    /// Every external call's interface, after resolving the whole unit.
    pub fn contracts_for(&self, source: &ObjectInput) -> Result<IndexMap<String, Contract>, LinkError> {
        let records = &source.records;
        let externals = omf::externals(records);
        let mut referenced: Vec<i64> = omf::fixups(records).iter().flat_map(omf::names_externals).collect();
        referenced.sort();
        referenced.dedup();
        for index in referenced {
            if !(0 < index && (index as usize) < externals.len()) {
                return Err(link_unit_error(format!(
                    "{}: fixup names missing EXTDEF index {index}",
                    source.path.display()
                )));
            }
            // Calls need an ABI contract below; data and frame references do
            // not, but LINK must still be able to resolve them. Validating all
            // live fixups here fails before optimizing any object.
            self.definition(&externals[index as usize])?;
        }
        let Some(found) = module::of(records) else {
            return Err(link_unit_error(format!(
                "{}: code module disappeared while resolving contracts",
                source.path.display()
            )));
        };
        // Start with the module's complete per-site view, including event
        // entries, dynamic cleanup and language-ABI inference.
        let selected = runtime::for_module(&found, None)?;
        let mut contracts: IndexMap<String, Contract> = IndexMap::default();
        let mut calls: Vec<(&i64, &String)> = found.calls.iter().collect();
        calls.sort();
        for (at, name) in calls {
            let definition = self.definition(name)?;
            let routine = &selected[at];
            if runtime::established_inputs(routine) {
                continue;
            }
            let label = match &definition.member {
                Some(member) => format!("{}({member})", definition.path.display()),
                None => definition.path.display().to_string(),
            };
            let chosen = Some((label.as_str(), definition.segment, definition.offset));
            let mut scanner = self.scanner.borrow_mut();
            let scanned = scanner.inputs(name, chosen).and_then(|discovered| {
                scanner.kept(name, chosen).map(|kept| (discovered, kept))
            });
            let (discovered, kept) = scanned.map_err(|error| {
                link_unit_error(format!("{}: input-contract discovery failed: {error}", definition.label()))
            })?;
            contracts.insert(
                name.clone(),
                Contract {
                    inputs: Some(discovered.registers.clone()),
                    clobbers: routine.clobbers.difference(&kept.registers).copied().collect(),
                    // The scan keeps nothing past an unresolved edge, and the
                    // error funnel never returns to the caller.
                    clobbers_reached: true,
                    evidence: format!(
                        "Link-unit definition {}. {}. {}. \
                         The callee remains an opaque barrier: no memory, control, cleanup or error effect is \
                         relaxed. \
                         Incoming arithmetic flags are excluded by the compiler calling convention; a \
                         hand-written flag-taking entry needs an explicit audited contract.",
                        definition.label(),
                        discovered.evidence,
                        kept.evidence
                    ),
                    ..routine.clone()
                },
            );
        }
        Ok(contracts)
    }
}
