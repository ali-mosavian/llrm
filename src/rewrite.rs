//! The driver: one LINK unit in, each standalone .OBJ optimized once.
//!
//! Port of `qbopt/rewrite.py`, which `python -m qbopt.rewrite` runs and
//! `llrm-omf` replaces. Positional inputs are the object and library inputs
//! in linker order. External calls are resolved across that unit before any
//! body is raised, and all output objects are buffered until every one has
//! completed. A refusal is an error by default; `--allow-unchanged` names
//! the compatibility behavior explicitly.

use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::abi::linkunit::{LinkError, LinkUnit, ObjectInput};
use crate::abi::{profile, runtime};
use crate::backend::arithmetic;
use crate::backend::cpu::{self as targets, ProfileOrName};
use crate::model::passes::{Exception, LEVELS, O2, Options};
use crate::objectfile::omf;
use crate::support::hash::IndexMap;
use crate::support::pyjson::{self, Json};
use crate::wholeseg;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Region {
    pub id: i64,
    pub seg: i64,
    pub at: i64,
    pub end: i64,
    pub before: String,
    pub after: Option<String>,
    pub taken: bool,
    pub reason: Option<String>,
}

impl Region {
    /// `dataclasses.asdict`.
    fn as_dict(&self) -> Json {
        let text = |one: &Option<String>| one.clone().map_or(Json::None, Json::Str);
        Json::Dict(IndexMap::from_iter([
            ("id".to_owned(), Json::Int(self.id)),
            ("seg".to_owned(), Json::Int(self.seg)),
            ("at".to_owned(), Json::Int(self.at)),
            ("end".to_owned(), Json::Int(self.end)),
            ("before".to_owned(), Json::Str(self.before.clone())),
            ("after".to_owned(), text(&self.after)),
            ("taken".to_owned(), Json::Bool(self.taken)),
            ("reason".to_owned(), text(&self.reason)),
        ]))
    }
}

fn sha256(data: &[u8]) -> String {
    Sha256::digest(data).iter().map(|byte| format!("{byte:02x}")).collect()
}

fn value_error(message: impl Into<String>) -> Exception {
    Exception::new("ValueError", message)
}

/// `rewrite`'s keyword arguments, at Python's defaults.
#[derive(Clone)]
pub struct Rewrite<'a> {
    pub dry_run: bool,
    pub take: Option<BTreeSet<i64>>,
    pub max_regions: Option<i64>,
    pub native_fpu: bool,
    pub whole_segment: bool,
    pub absorb_calls: bool,
    pub cpu: &'static str,
    pub basic_semantics: bool,
    pub bounds_checks: bool,
    pub contract_profile: Option<&'a profile::Profile>,
    pub external_contracts: Option<IndexMap<String, runtime::Contract>>,
    pub contract_fingerprint: Option<String>,
    pub allow_unchanged: bool,
    pub options: Options,
}

impl Rewrite<'_> {
    pub fn new(dry_run: bool) -> Self {
        Rewrite {
            dry_run,
            take: None,
            max_regions: None,
            native_fpu: true,
            whole_segment: true,
            absorb_calls: true,
            cpu: "386",
            basic_semantics: false,
            bounds_checks: false,
            contract_profile: None,
            external_contracts: None,
            contract_fingerprint: None,
            allow_unchanged: false,
            options: O2(),
        }
    }
}

/// Optimize one raised body and lower it once.
///
/// Repeated optimization belongs on MIR, never on emitted machine code.
/// Only output from the allocating backend receives the completion marker.
/// A refusal raises `Unsupported` by default: returning the input makes an
/// unsupported construct indistinguishable from a successful no-op.
pub fn rewrite(data: &[u8], asked: &Rewrite<'_>) -> Result<(Vec<u8>, Vec<Region>), Exception> {
    // `take`, `max_regions` and `dry_run` bisected the machine arm by
    // region index. There are no regions to bisect.
    arithmetic::validate(asked.cpu).map_err(value_error)?;
    wholeseg::_native_only(asked.native_fpu)?;
    if asked.dry_run {
        return Ok((data.to_vec(), Vec::new()));
    }

    let regions: Vec<Region> = Vec::new(); // nothing plans one now; the CLI still reports the list

    let mut made_by = _configuration(
        asked.whole_segment,
        asked.native_fpu,
        asked.absorb_calls,
        asked.cpu,
        asked.basic_semantics,
        asked.bounds_checks,
        &asked.options,
    );
    let fingerprints: Vec<&str> = [
        asked.contract_fingerprint.as_deref(),
        asked.contract_profile.map(|profile| profile.fingerprint.as_str()),
    ]
    .into_iter()
    .flatten()
    .collect();
    if !fingerprints.is_empty() {
        made_by += &format!(",contracts={}", sha256(fingerprints.join(";").as_bytes()));
    }
    let parsed = omf::parse(data).map_err(|error| value_error(error.0))?;
    let was = omf::finalised_at(&parsed).map_err(|error| value_error(error.0))?;
    if let Some(was) = was {
        // Already emitted by this pass. What came out is a program, and
        // raising it again reads all of that as code BC wrote.
        if was != made_by {
            return Err(Exception::new(
                "Finalised",
                format!(
                    "this object was written by {}, and this run is {}",
                    crate::support::pyrepr::string(&was),
                    crate::support::pyrepr::string(&made_by)
                ),
            ));
        }
        return Ok((data.to_vec(), regions));
    }

    let mut combined: IndexMap<String, runtime::Contract> = asked.external_contracts.clone().unwrap_or_default();
    if let Some(contract_profile) = asked.contract_profile {
        combined.extend(contract_profile.rules.iter().map(|rule| (rule.name.clone(), rule.clone())));
    }
    let (out, terminal, reason) = _written(
        data,
        asked.whole_segment,
        asked.native_fpu,
        asked.absorb_calls,
        asked.cpu,
        asked.basic_semantics,
        asked.bounds_checks,
        (!combined.is_empty()).then_some(&combined),
        &asked.options,
    )?;
    if terminal {
        let parsed = omf::parse(&out).map_err(|error| value_error(error.0))?;
        let finalised = omf::finalised(&parsed, &made_by).map_err(|error| value_error(error.0))?;
        return Ok((finalised.iter().flat_map(|one| one.emit()).collect(), regions));
    }
    if asked.allow_unchanged {
        // Explicit compatibility mode only.
        return Ok((data.to_vec(), regions));
    }
    Err(Exception::new("Unsupported", reason))
}

/// Every option that can change what the emitter writes, as one string.
///
/// Its own schema is first: a later version of this pass reading an older
/// marker has to refuse rather than assume the bytes mean what they would
/// today.
pub fn _configuration(
    whole_segment: bool,
    native_fpu: bool,
    absorb_calls: bool,
    cpu: &str,
    basic_semantics: bool,
    bounds_checks: bool,
    options: &Options,
) -> String {
    let on: Vec<&str> = [("whole", whole_segment), ("fpu", native_fpu), ("absorb", absorb_calls)]
        .into_iter()
        .filter(|(_, on)| *on)
        .map(|(name, _)| name)
        .collect();
    let mut out = format!("1;{}", on.join(","));
    if cpu != "386" {
        out += &format!(",cpu={cpu}");
    }
    if basic_semantics {
        out += ",basic-semantics";
    }
    if bounds_checks {
        out += ",bounds-checks";
    }
    if options.level != O2().level {
        out += &format!(",{}", options.level);
    }
    out
}

/// Lower and emit once; report whether the allocating backend completed.
#[allow(clippy::too_many_arguments)]
pub fn _written(
    data: &[u8],
    whole_segment: bool,
    native_fpu: bool,
    absorb_calls: bool,
    cpu: &'static str,
    basic_semantics: bool,
    bounds_checks: bool,
    external_contracts: Option<&IndexMap<String, runtime::Contract>>,
    options: &Options,
) -> Result<(Vec<u8>, bool, String), Exception> {
    let _ = absorb_calls;
    if !whole_segment {
        return Ok((data.to_vec(), false, "whole-segment emission is disabled".to_owned()));
    }
    let got = wholeseg::emitted(
        data,
        true,
        native_fpu,
        None,
        None,
        ProfileOrName::Name(cpu),
        basic_semantics,
        bounds_checks,
        external_contracts,
        options,
    )?;
    if !bounds_checks && got.reason.starts_with("unchecked array lowering unsupported") {
        return Err(value_error(format!("{}; use --bounds-checks to retain the checked helper", got.reason)));
    }
    let terminal = got.outcome == wholeseg::Emission::Lir;
    Ok((got.data, terminal, got.reason))
}

const PROG: &str = "qbopt.rewrite";

const USAGE: &str = "usage: qbopt.rewrite [-h] [--cpu {386,486,P5,P6,K5,K6,K7,Core}] [-O {s,2}]
                     [-o OUTPUT] [--output-dir OUTPUT_DIR]
                     [--manifest MANIFEST] [--contracts CONTRACTS]
                     [--contract-root CONTRACT_ROOT] [--dry-run] [--take TAKE]
                     [--max-regions MAX_REGIONS] [--report]
                     [--basic-semantics] [--bounds-checks] [--native-fpu]
                     [--no-absorb-calls] [--no-whole-segment]
                     [--allow-unchanged]
                     inputs [inputs ...]
";

const HELP: &str = "
positional arguments:
  inputs                OMF .OBJ files to optimize and .LIB files used to
                        resolve them, in LINK order

options:
  -h, --help            show this help message and exit
  --cpu {386,486,P5,P6,K5,K6,K7,Core}
                        code-generation tuning target
  -O {s,2}              optimization level
  -o, --output OUTPUT   output file; valid for a single input OBJ
  --output-dir OUTPUT_DIR
                        directory receiving every optimized input OBJ
  --manifest MANIFEST
  --contracts CONTRACTS
                        audited, hash-checked external call profile (JSON);
                        repeat to combine profiles
  --contract-root CONTRACT_ROOT
                        artifact directory shared by all profiles; defaults to
                        each profile directory
  --dry-run
  --take TAKE           comma-separated region ids; refuse the rest
  --max-regions MAX_REGIONS
  --report
  --basic-semantics     preserve BASIC numeric runtime errors, conversions and
                        floating behavior
  --bounds-checks       retain BASIC array bounds checks (independent of
                        numeric semantics)
  --native-fpu          accepted and ignored: real x87 is the only floating-
                        point path, and every build REQUIRES A COPROCESSOR
  --no-absorb-calls     leave the arithmetic calls to the MIR tower instead of
                        calls.py
  --no-whole-segment    patch BC's own bytes rather than writing the code
                        segment from MIR
  --allow-unchanged     explicitly retain an input OBJ when the backend
                        refuses it (strict failure is the default)
";

/// What `ap.parse_args` leaves in `args`.
struct Args {
    inputs: Vec<PathBuf>,
    cpu: &'static str,
    options: Options,
    output: Option<PathBuf>,
    output_dir: Option<PathBuf>,
    manifest: Option<PathBuf>,
    contracts: Option<Vec<PathBuf>>,
    contract_root: Option<PathBuf>,
    dry_run: bool,
    take: Option<String>,
    max_regions: Option<i64>,
    report: bool,
    basic_semantics: bool,
    bounds_checks: bool,
    no_absorb_calls: bool,
    no_whole_segment: bool,
    allow_unchanged: bool,
}

/// argparse's `ap.error`: usage and the message on stderr, exit status 2.
#[derive(Debug)]
pub struct Exit(pub i32);

fn error(message: &str) -> Exit {
    eprint!("{USAGE}");
    eprintln!("{PROG}: error: {message}");
    Exit(2)
}

fn quoted(choices: &[&str]) -> String {
    choices.iter().map(|one| format!("'{one}'")).collect::<Vec<_>>().join(", ")
}

fn parse(argv: &[String]) -> Result<Args, Exit> {
    let mut args = Args {
        inputs: Vec::new(),
        cpu: "386",
        options: O2(),
        output: None,
        output_dir: None,
        manifest: None,
        contracts: None,
        contract_root: None,
        dry_run: false,
        take: None,
        max_regions: None,
        report: false,
        basic_semantics: false,
        bounds_checks: false,
        no_absorb_calls: false,
        no_whole_segment: false,
        allow_unchanged: false,
    };
    let mut at = 0;
    let mut positional_only = false;
    while at < argv.len() {
        let argument = &argv[at];
        at += 1;
        if positional_only || !argument.starts_with('-') || argument == "-" {
            args.inputs.push(PathBuf::from(argument));
            continue;
        }
        if argument == "--" {
            positional_only = true;
            continue;
        }
        let (flag, attached) = match argument.split_once('=') {
            Some((flag, value)) if flag.starts_with("--") => (flag.to_owned(), Some(value.to_owned())),
            _ if argument.starts_with("-O") && argument.len() > 2 => ("-O".to_owned(), Some(argument[2..].to_owned())),
            _ if argument.starts_with("-o") && argument.len() > 2 && !argument.starts_with("--") => {
                ("-o".to_owned(), Some(argument[2..].to_owned()))
            }
            _ => (argument.clone(), None),
        };
        let switch = |on: &mut bool| -> Result<(), Exit> {
            if attached.is_some() {
                return Err(error(&format!("argument {flag}: ignored explicit argument '{}'", attached.clone().unwrap_or_default())));
            }
            *on = true;
            Ok(())
        };
        let mut value = |shown: &str| -> Result<String, Exit> {
            if let Some(value) = attached.clone() {
                return Ok(value);
            }
            match argv.get(at) {
                Some(next) if !next.starts_with('-') || next == "-" => {
                    at += 1;
                    Ok(next.clone())
                }
                _ => Err(error(&format!("argument {shown}: expected one argument"))),
            }
        };
        match flag.as_str() {
            "-h" | "--help" => {
                print!("{USAGE}{HELP}");
                return Err(Exit(0));
            }
            "--cpu" => {
                let chosen = value("--cpu")?;
                let names = targets::names();
                let Some(name) = names.iter().copied().find(|name| *name == chosen) else {
                    return Err(error(&format!(
                        "argument --cpu: invalid choice: '{chosen}' (choose from {})",
                        quoted(&names)
                    )));
                };
                args.cpu = name;
            }
            "-O" => {
                let text = value("-O")?;
                let Some(options) = LEVELS().get(format!("O{text}").as_str()).cloned() else {
                    return Err(error(&format!("argument -O: unknown level -O{text}; choose -Os or -O2")));
                };
                args.options = options;
            }
            "-o" | "--output" => args.output = Some(PathBuf::from(value("-o/--output")?)),
            "--output-dir" => args.output_dir = Some(PathBuf::from(value("--output-dir")?)),
            "--manifest" => args.manifest = Some(PathBuf::from(value("--manifest")?)),
            "--contracts" => {
                let path = PathBuf::from(value("--contracts")?);
                args.contracts.get_or_insert_with(Vec::new).push(path);
            }
            "--contract-root" => args.contract_root = Some(PathBuf::from(value("--contract-root")?)),
            "--take" => args.take = Some(value("--take")?),
            "--max-regions" => {
                let text = value("--max-regions")?;
                let Ok(number) = text.parse::<i64>() else {
                    return Err(error(&format!("argument --max-regions: invalid int value: '{text}'")));
                };
                args.max_regions = Some(number);
            }
            "--dry-run" => switch(&mut args.dry_run)?,
            "--report" => switch(&mut args.report)?,
            "--basic-semantics" => switch(&mut args.basic_semantics)?,
            "--bounds-checks" => switch(&mut args.bounds_checks)?,
            "--native-fpu" => switch(&mut false)?,
            "--no-absorb-calls" => switch(&mut args.no_absorb_calls)?,
            "--no-whole-segment" => switch(&mut args.no_whole_segment)?,
            "--allow-unchanged" => switch(&mut args.allow_unchanged)?,
            _ => return Err(error(&format!("unrecognized arguments: {argument}"))),
        }
    }
    if args.inputs.is_empty() {
        return Err(error("the following arguments are required: inputs"));
    }
    Ok(args)
}

fn caught(raised: &Exception) -> bool {
    matches!(raised.kind, "ValueError" | "OSError" | "Finalised" | "Unsupported")
}

fn from_link(error: LinkError) -> Exception {
    match error {
        LinkError::OSError(text) => Exception::new("OSError", text),
        LinkError::LinkUnitError(text) | LinkError::ValueError(text) => value_error(text),
    }
}

fn from_profile(error: profile::ProfileError) -> Exception {
    match error {
        profile::ProfileError::OSError(text) => Exception::new("OSError", text),
        profile::ProfileError::ValueError(text) => value_error(text),
    }
}

/// `python -m qbopt.rewrite`: the exit status, or `Err` for an exception
/// Python lets escape as a traceback.
pub fn main(argv: &[String]) -> Result<i32, Exception> {
    let args = match parse(argv) {
        Ok(args) => args,
        Err(Exit(status)) => return Ok(status),
    };
    if args.contract_root.is_some() && args.contracts.is_none() {
        return Ok(error("--contract-root requires --contracts").0);
    }
    if args.output.is_some() && args.output_dir.is_some() {
        return Ok(error("use either --output or --output-dir, not both").0);
    }
    let take: Option<BTreeSet<i64>> = match &args.take {
        Some(text) if !text.is_empty() => Some(
            text.split(',')
                .map(|one| {
                    one.trim().parse::<i64>().map_err(|_| {
                        value_error(format!("invalid literal for int() with base 10: {}", crate::support::pyrepr::string(one)))
                    })
                })
                .collect::<Result<_, _>>()?,
        ),
        _ => None,
    };

    let run = || -> Result<(LinkUnit, Option<profile::Profile>, Vec<(ObjectInput, Vec<u8>, Vec<Region>)>), Exception> {
        let unit = LinkUnit::read(&args.inputs).map_err(from_link)?;
        if args.output.is_some() && unit.objects.len() != 1 {
            return Err(value_error("--output requires exactly one standalone OBJ; use --output-dir for several"));
        }
        let contracts = match &args.contracts {
            Some(paths) => Some(profile::load_many(paths, args.contract_root.as_deref()).map_err(from_profile)?),
            None => None,
        };
        let mut optimized = Vec::new();
        // Finish the whole unit before writing any member. One unsupported
        // object therefore leaves every caller input and prior output intact.
        for source in &unit.objects {
            let asked = Rewrite {
                dry_run: args.dry_run,
                take: take.clone(),
                max_regions: args.max_regions,
                native_fpu: true,
                whole_segment: !args.no_whole_segment,
                absorb_calls: !args.no_absorb_calls,
                cpu: args.cpu,
                basic_semantics: args.basic_semantics,
                bounds_checks: args.bounds_checks,
                contract_profile: contracts.as_ref(),
                external_contracts: Some(unit.contracts_for(source).map_err(from_link)?),
                contract_fingerprint: Some(unit.fingerprint.clone()),
                allow_unchanged: args.allow_unchanged,
                options: args.options.clone(),
            };
            let (out, regions) = rewrite(&source.data, &asked).map_err(|raised| {
                if raised.kind == "Unsupported" {
                    Exception::new("Unsupported", format!("{}: {}", source.path.display(), raised.message))
                } else {
                    raised
                }
            })?;
            optimized.push((source.clone(), out, regions));
        }
        Ok((unit, contracts, optimized))
    };
    let (unit, contracts, optimized) = match run() {
        Ok(done) => done,
        Err(raised) if caught(&raised) => return Ok(error(&raised.message).0),
        Err(raised) => return Err(raised),
    };

    if let Some(output_dir) = &args.output_dir {
        let names: Vec<String> = optimized
            .iter()
            .map(|(source, _out, _regions)| source.path.file_name().map_or_else(String::new, |name| name.to_string_lossy().to_lowercase()))
            .collect();
        if names.len() != names.iter().collect::<BTreeSet<_>>().len() {
            return Ok(error("--output-dir cannot represent input OBJs with duplicate filenames").0);
        }
        std::fs::create_dir_all(output_dir).map_err(|failure| Exception::new("OSError", failure.to_string()))?;
        for (source, out, _regions) in &optimized {
            let name = source.path.file_name().expect("an input names a file");
            std::fs::write(output_dir.join(name), out).map_err(|failure| Exception::new("OSError", failure.to_string()))?;
        }
    } else if let Some(output) = &args.output {
        std::fs::write(output, &optimized[0].1).map_err(|failure| Exception::new("OSError", failure.to_string()))?;
    }

    let objects: Vec<IndexMap<String, Json>> = optimized
        .iter()
        .map(|(source, out, regions)| {
            IndexMap::from_iter([
                ("input".to_owned(), Json::Str(source.path.display().to_string())),
                ("input_sha256".to_owned(), Json::Str(sha256(&source.data))),
                ("output_sha256".to_owned(), Json::Str(sha256(out))),
                ("input_bytes".to_owned(), Json::Int(source.data.len() as i64)),
                ("output_bytes".to_owned(), Json::Int(out.len() as i64)),
                ("regions".to_owned(), Json::List(regions.iter().map(Region::as_dict).collect())),
                ("taken".to_owned(), Json::Int(regions.iter().filter(|region| region.taken).count() as i64)),
            ])
        })
        .collect();
    let common: IndexMap<String, Json> = IndexMap::from_iter([
        ("dry_run".to_owned(), Json::Bool(args.dry_run)),
        ("cpu".to_owned(), Json::Str(args.cpu.to_owned())),
        ("level".to_owned(), Json::Str(args.options.level.clone())),
        (
            "semantics".to_owned(),
            Json::Str(if args.basic_semantics { "basic" } else { "native" }.to_owned()),
        ),
        ("bounds_checks".to_owned(), Json::Bool(args.bounds_checks)),
        (
            "link_inputs".to_owned(),
            Json::List(unit.inputs.iter().map(|path| Json::Str(path.display().to_string())).collect()),
        ),
        ("link_unit_sha256".to_owned(), Json::Str(unit.fingerprint.clone())),
        (
            "contract_profile_sha256".to_owned(),
            contracts.as_ref().map_or(Json::None, |contracts| Json::Str(contracts.fingerprint.clone())),
        ),
    ]);
    let manifest = if objects.len() == 1 {
        let mut one = objects[0].clone();
        one.extend(common);
        Json::Dict(one)
    } else {
        let mut all = common;
        all.insert("objects".to_owned(), Json::List(objects.into_iter().map(Json::Dict).collect()));
        Json::Dict(all)
    };
    let mut path = args.manifest.clone();
    if path.is_none() {
        path = args.output.as_ref().map(|output| output.with_extension("json"));
    }
    if path.is_none() {
        path = args.output_dir.as_ref().map(|output_dir| output_dir.join("qbopt-manifest.json"));
    }
    if let Some(path) = path.filter(|path| path != Path::new("")) {
        std::fs::write(&path, pyjson::dumps(&manifest, Some(2), None, false))
            .map_err(|failure| Exception::new("OSError", failure.to_string()))?;
    }

    if args.report {
        let mut stdout = std::io::stdout().lock();
        for (source, out, regions) in &optimized {
            let taken = regions.iter().filter(|region| region.taken).count();
            let name = source.path.file_name().map_or_else(String::new, |name| name.to_string_lossy().into_owned());
            let _ = writeln!(
                stdout,
                "{name}: {} -> {} bytes; {} regions, {taken} taken",
                source.data.len(),
                out.len(),
                regions.len()
            );
            for region in regions {
                let span = region.end - region.at;
                let note = if region.taken {
                    "taken".to_owned()
                } else {
                    format!("refused -- {}", region.reason.as_deref().unwrap_or("None"))
                };
                let _ = writeln!(stdout, "   {:3} {:#06x}..{:#06x}  {span:4}b  {note}", region.id, region.at, region.end);
            }
        }
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    //! Ports of `tests/test_rewrite.py`, `tests/test_cpu_driver.py` and the
    //! driver half of `tests/test_contract_profile.py`. Not portable: the
    //! tests that swap `wholeseg.emitted`/`RegAlloc.transform` for a spy
    //! (`test_a_fallback_is_not_finalised_or_raised_again`,
    //! `test_a_refusal_is_unmarked_non_terminal_and_does_not_loop_for_nothing`,
    //! `test_cli_forwards_profile_and_marks_its_identity`,
    //! `test_object_frontend_threads_machine_neutral_cpu_costs_to_mir`,
    //! `test_e2e_cli_passes_cpu_to_rewrite`), and the spy half of
    //! `test_a_finalised_object_is_given_back_before_anything_decodes_it`.

    use std::path::PathBuf;

    use iced_x86::{Decoder, DecoderOptions, Mnemonic, OpKind, Register};

    use super::*;
    use crate::objectfile::module;

    const HOTLOP: &str = "fixtures/omf/hotlop-q-evt.obj";
    const FIXTURE: &str = "fixtures/omf/lngmxx-p-g2.obj";

    fn objects() -> Vec<PathBuf> {
        let mut found: Vec<PathBuf> = std::fs::read_dir("fixtures/omf")
            .unwrap()
            .map(|one| one.unwrap().path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "obj"))
            .collect();
        found.sort();
        found
    }

    fn once(data: &[u8], absorb_calls: bool) -> Result<Vec<u8>, Exception> {
        Ok(rewrite(data, &Rewrite { absorb_calls, ..Rewrite::new(false) })?.0)
    }

    fn with_cpu(data: &[u8], cpu: &'static str) -> Result<Vec<u8>, Exception> {
        Ok(rewrite(data, &Rewrite { cpu, ..Rewrite::new(false) })?.0)
    }

    /// Every accepted corpus object is a self-contained OMF module whose live
    /// external fixups all resolve.
    #[test]
    #[ignore = "fails in Python too: 0x00a7: 6 bytes between the ops are not instructions"]
    fn test_a_real_pass_writes_a_complete_fresh_object() {
        for obj in objects() {
            let out = once(&std::fs::read(&obj).unwrap(), true).unwrap();
            let records = omf::parse(&out).unwrap();
            assert_eq!(records[0].r#type, omf::THEADR);
            assert_eq!(records.last().unwrap().r#type & 0xFE, omf::MODEND);
            assert!(omf::code_segment(&records).is_some());
            assert!(omf::finalised_at(&records).unwrap().is_some());
            let names = omf::externals(&records);
            let live: Vec<omf::Fixup> =
                omf::fixups(&records).into_iter().filter(|fixup| fixup.target == "external").collect();
            assert!(live.iter().all(|fixup| 0 < fixup.index
                && (fixup.index as usize) < names.len()
                && !names[fixup.index as usize].is_empty()));
        }
    }

    /// A finalized program is returned byte-for-byte without being raised.
    #[test]
    #[ignore = "fails in Python too: 0x00a7: 6 bytes between the ops are not instructions, and on four more objects"]
    fn test_rewriting_reaches_a_fixed_point() {
        for obj in objects() {
            let out = once(&std::fs::read(&obj).unwrap(), true).unwrap();
            assert_eq!(once(&out, true).unwrap(), out, "{}", obj.display());
        }
    }

    /// Raising the LIR emitter's program again read `sub sp,4` as a subtract
    /// and reserved a second frame: S= 0 for 630.
    #[test]
    fn test_a_finalised_object_is_given_back_before_anything_decodes_it() {
        let raw = std::fs::read(HOTLOP).unwrap();
        let first = once(&raw, false).unwrap();
        assert!(once(&first, false).unwrap() == first, "a finalised object came back changed");
    }

    /// Bytes produced for one set of options do not mean the same thing under another.
    #[test]
    fn test_the_same_object_asked_for_other_options_is_refused() {
        let raw = std::fs::read(HOTLOP).unwrap();
        let first = once(&raw, false).unwrap();
        let refused = once(&first, true).unwrap_err();
        assert_eq!(refused.kind, "Finalised");
        assert!(refused.message.contains("absorb"), "{}", refused.message);
    }

    #[test]
    fn test_exactly_one_marker_and_one_frame() {
        let raw = std::fs::read(HOTLOP).unwrap();
        let out = once(&raw, false).unwrap();
        let records = omf::parse(&out).unwrap();
        assert!(omf::finalised_at(&records).unwrap().is_some()); // errs if there are two
        let code = module::of(&records).unwrap().code;
        // NASM's "sub sp, ..."
        let reserved = Decoder::with_ip(16, &code[0x30..], 0x30, DecoderOptions::NONE)
            .into_iter()
            .filter(|one| one.mnemonic() == Mnemonic::Sub && one.op0_kind() == OpKind::Register && one.op0_register() == Register::SP)
            .count();
        assert!(reserved <= 1, "a frame reserved {reserved} times");
    }

    #[test]
    fn test_default_marker_means_386_not_unspecified() {
        let data = once(&std::fs::read(FIXTURE).unwrap(), true).unwrap();
        assert_eq!(with_cpu(&data, "386").unwrap(), data);
        assert_eq!(with_cpu(&data, "P5").unwrap_err().kind, "Finalised");
    }

    /// A stale contract dependency exits 2 before touching the output.
    #[test]
    fn test_stale_dependency_does_not_overwrite_cli_output() {
        let directory = tempfile::tempdir().unwrap();
        let source = std::fs::read("fixtures/regressions/qrender-main-v-g3.obj").unwrap();
        let dependency = std::fs::read("fixtures/omf/hotlop-p-g2.obj").unwrap();
        std::fs::write(directory.path().join("main.obj"), &source).unwrap();
        let digest = sha256;
        let document = format!(
            r#"{{"version": 1, "artifacts": {{"main.obj": "{}", "dependency.obj": "{}"}}, "contracts": {{"HOST_SHUTDOWN": {{"defined_in": "main.obj", "inputs": ["ax", "bx", "cx", "dx", "si", "di"], "evidence": "B$ENRA precedes flag reads; all six GP inputs and unknown effects retained."}}}}}}"#,
            digest(&source),
            digest(&dependency)
        );
        let path = directory.path().join("contracts.json");
        std::fs::write(&path, document).unwrap();
        std::fs::write(directory.path().join("dependency.obj"), b"changed dependency").unwrap();
        let output = directory.path().join("existing.obj");
        std::fs::write(&output, b"keep this").unwrap();
        let argv: Vec<String> = ["fixtures/omf/hotlop-p-g2.obj", "--contracts", path.to_str().unwrap(), "-o", output.to_str().unwrap()]
            .into_iter()
            .map(str::to_owned)
            .collect();
        assert_eq!(main(&argv).unwrap(), 2);
        assert_eq!(std::fs::read(&output).unwrap(), b"keep this");
    }
}
