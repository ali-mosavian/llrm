//! `llrm-cc`: a cc-compatible driver that runs the pinned, unmodified clang for
//! the front end (`-S -emit-llvm -Xclang -disable-llvm-passes`) and `llrm-llc`
//! for the rest. Only phase flags are understood; every other flag goes to
//! clang. Preprocess, dependency and query jobs run clang as is.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

/// The facts of a data layout that decide a struct's shape and a call's frame:
/// endianness, and each type's size and ABI alignment, with LLVM's defaults for
/// what the string leaves out. Preferred alignments, stack alignment, mangling,
/// integer widths past 64 and the address spaces clang adds (above 255) change
/// no layout.
pub fn layout_facts(datalayout: &str) -> BTreeMap<String, String> {
    let mut facts: BTreeMap<String, String> = [
        ("e", "big"),
        ("p", "64:64"),
        ("i8", "8"),
        ("i16", "16"),
        ("i32", "32"),
        ("i64", "32"),
        ("f32", "32"),
        ("f64", "64"),
        ("f80", "128"),
    ]
    .map(|(key, value)| (key.to_owned(), value.to_owned()))
    .into();
    for spec in datalayout.split('-') {
        let parts: Vec<&str> = spec.split(':').collect();
        let key = parts[0];
        match key {
            "e" => {
                facts.insert("e".into(), "little".into());
            }
            _ if key.starts_with('p') => {
                let space = key[1..].parse::<u32>().unwrap_or(0);
                if space < 256 && parts.len() > 2 {
                    facts
                        .insert(if space == 0 { "p".into() } else { key.into() }, format!("{}:{}", parts[1], parts[2]));
                }
            }
            _ if facts.contains_key(key) && parts.len() > 1 => {
                facts.insert(key.into(), parts[1].into());
            }
            _ => {}
        }
    }
    facts
}

/// Per-target facts; the file's header says what each field is.
#[derive(Clone, Debug, Deserialize)]
pub struct Target {
    pub triple: String,
    /// What llrm-llc is told the module's layout is (its target's
    /// `datalayout.toml`).
    pub datalayout: String,
    /// What clang prints for the triple where it spells the same layout
    /// differently; absent, `datalayout`.
    pub clang_datalayout: Option<String>,
    pub clang: Vec<String>,
    pub llc: Vec<String>,
    /// The backend program beside this driver; `llrm-llc` unless the target has
    /// its own.
    #[serde(default = "default_llc")]
    pub llc_program: String,
    /// Whether the backend takes `-O<n>`.
    #[serde(default = "yes")]
    pub llc_level: bool,
    pub object: String,
    pub link: Vec<String>,
    /// The command that runs an executable of the target, the executable last;
    /// absent where the host runs it.
    #[serde(default)]
    pub run: Vec<String>,
}

fn default_llc() -> String {
    "llrm-llc".into()
}

fn yes() -> bool {
    true
}

const TARGETS: &str = include_str!("../targets.toml");
/// The clang commit llrm's passes were translated from; the one place it is
/// written for this driver.
const PIN: &str = include_str!("../../../../toolchain/clang/pin");
/// The llrm checkout this driver was built in, for the tools the recipes name.
const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../..");
const DEFAULT_TARGET: &str = "x86_64-linux";

pub fn targets(text: &str) -> Result<BTreeMap<String, Target>, String> {
    toml::from_str(text).map_err(|error| format!("targets.toml: {error}"))
}

#[derive(Debug, PartialEq)]
pub enum Stop {
    Assembly,
    Object,
    Executable,
}

#[derive(Debug, PartialEq)]
pub enum Input {
    Source(String),
    Assemble(String),
    Object(String),
}

#[derive(Debug, PartialEq)]
pub struct Build {
    pub stop: Stop,
    pub output: Option<String>,
    /// `-O` argument: `0`, `1`, `2`, `3`, `s`, `z`.
    pub level: String,
    pub clang: Vec<String>,
    pub llc: Vec<String>,
    pub link: Vec<String>,
    pub inputs: Vec<Input>,
}

#[derive(Debug, PartialEq)]
pub enum Job {
    /// Clang as is.
    Passthrough(Vec<String>),
    Build(Build),
}

const TAKES_VALUE: &[&str] = &[
    "-o",
    "-x",
    "-I",
    "-D",
    "-U",
    "-include",
    "-isystem",
    "-iquote",
    "-idirafter",
    "-isysroot",
    "-MF",
    "-MT",
    "-MQ",
    "-Xclang",
    "-Xlinker",
    "-mllvm",
    "-L",
    "-l",
    "-T",
    "-z",
    "-framework",
    "-arch",
    "-target",
];
const REFUSED: &[&str] = &["-fsanitize", "-fprofile", "-fcs-profile", "-flto", "-fmemory-profile"];
const PASSTHROUGH: &[&str] = &["-E", "-M", "-MM", "-emit-llvm", "-fsyntax-only", "-###", "-dumpmachine"];

/// The target named by `-target`/`--target=`, and the job the rest of `argv`
/// asks for.
pub fn parse(argv: &[String]) -> Result<(Option<String>, Job), String> {
    let mut target = None;
    let mut build = Build {
        stop: Stop::Executable,
        output: None,
        level: "0".into(),
        clang: vec![],
        llc: vec![],
        link: vec![],
        inputs: vec![],
    };
    let mut passthrough = false;
    let mut at = 0;
    while at < argv.len() {
        let flag = argv[at].as_str();
        let value = |at: &mut usize| -> Result<String, String> {
            *at += 1;
            argv.get(*at).cloned().ok_or_else(|| format!("{flag}: missing value"))
        };
        if let Some(name) = flag.strip_prefix("--target=") {
            target = Some(name.to_owned());
        } else if flag == "-target" {
            target = Some(value(&mut at)?);
        } else if flag == "-o" {
            build.output = Some(value(&mut at)?);
        } else if flag == "-c" {
            build.stop = Stop::Object;
        } else if flag == "-S" {
            build.stop = Stop::Assembly;
        } else if PASSTHROUGH.contains(&flag)
            || flag.starts_with("-print-")
            || flag.starts_with("--version")
            || flag == "-v"
            || flag == "--help"
        {
            passthrough = true;
        } else if REFUSED.iter().any(|one| flag.starts_with(one)) {
            return Err(format!("{flag}: not translated yet"));
        } else if flag.starts_with("-g") && flag != "-g0" {
            return Err(format!("{flag}: debug info is not carried yet; use -g0"));
        } else if let Some(level) = flag.strip_prefix("-O") {
            build.level = match level {
                "" => "1".into(),
                "fast" => "3".into(),
                "g" => "1".into(),
                other => other.to_owned(),
            };
            build.clang.push(flag.to_owned());
        } else if flag == "-funroll-loops" || flag == "-fno-unroll-loops" {
            build.clang.push(flag.to_owned());
            build.llc.push(flag.to_owned());
        } else if flag == "-l" || flag == "-L" || flag == "-Xlinker" {
            build.link.push(flag.to_owned());
            build.link.push(value(&mut at)?);
        } else if flag.starts_with("-l")
            || flag.starts_with("-L")
            || flag.starts_with("-Wl,")
            || flag.starts_with("-fuse-ld=")
            || ["-shared", "-static", "-pthread", "-rdynamic", "-nostdlib", "-nostartfiles", "-s", "-pie", "-no-pie"]
                .contains(&flag)
        {
            build.link.push(flag.to_owned());
        } else if flag.starts_with('-') && flag.len() > 1 {
            build.clang.push(flag.to_owned());
            if TAKES_VALUE.contains(&flag) {
                build.clang.push(value(&mut at)?);
            }
        } else {
            let lower = flag.to_ascii_lowercase();
            let ext = Path::new(&lower).extension().and_then(|one| one.to_str()).unwrap_or_default().to_owned();
            build
                .inputs
                .push(
                    match ext.as_str() {
                        "c" | "cc" | "cpp" | "cxx" | "c++" | "i" | "ii" => Input::Source(flag.to_owned()),
                        "s" => Input::Assemble(flag.to_owned()),
                        _ => Input::Object(flag.to_owned()),
                    },
                );
        }
        at += 1;
    }
    if passthrough || build.inputs.is_empty() {
        let skip: Vec<usize> = argv
            .iter()
            .enumerate()
            .filter(|(_, one)| one.as_str() == "-target" || one.starts_with("--target="))
            .flat_map(|(at, one)| if one == "-target" { vec![at, at + 1] } else { vec![at] })
            .collect();
        let kept = argv.iter().enumerate().filter(|(at, _)| !skip.contains(at)).map(|(_, one)| one.clone()).collect();
        return Ok((target, Job::Passthrough(kept)));
    }
    Ok((target, Job::Build(build)))
}

pub fn target_named<'a>(
    table: &'a BTreeMap<String, Target>,
    name: &str,
) -> Result<&'a Target, String> {
    table
        .get(name)
        .or_else(|| table.values().find(|one| one.triple == name))
        .ok_or_else(
            || format!("no target `{name}`; the table has {}", table.keys().cloned().collect::<Vec<_>>().join(", ")),
        )
}

/// `llrm-llc`'s argument list for one translated unit.
pub fn llc_args(
    target: &Target,
    build: &Build,
    ir: &Path,
    output: &Path,
) -> Vec<String> {
    let mut args = vec![];
    if target.llc_level {
        args.push(format!("-O{}", build.level));
    }
    args.extend(target.llc.iter().cloned());
    args.extend(build.llc.iter().cloned());
    if build.stop == Stop::Assembly {
        args.push("-S".into());
    }
    args.extend(["-o".into(), output.display().to_string(), ir.display().to_string()]);
    args
}

/// The link command: the recipe with its placeholders expanded.
pub fn link_command(
    target: &Target,
    objects: &[String],
    flags: &[String],
    output: &str,
) -> Result<Vec<String>, String> {
    let mut command = vec![];
    for word in &target.link {
        match word.as_str() {
            "{objects}" => command.extend(objects.iter().cloned()),
            "{flags}" => command.extend(flags.iter().cloned()),
            "{output}" => command.push(output.to_owned()),
            other if other.contains("{root}") => command.push(other.replace("{root}", ROOT)),
            other => command.push(other.to_owned()),
        }
    }
    if command.is_empty() {
        return Err("empty link recipe".into());
    }
    Ok(command)
}

fn clang_path() -> PathBuf {
    std::env::var_os("LLRM_PIN_CLANG")
        .map(PathBuf::from)
        .unwrap_or_else(
            || PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join("scratch/llvm-build/bin/clang"),
        )
}

/// `LLRM_LLC` names the default backend; another program is looked for beside
/// this driver.
fn llc_path(program: &str) -> PathBuf {
    let beside = || std::env::current_exe().ok().map(|one| one.with_file_name(program));
    match std::env::var_os("LLRM_LLC").map(PathBuf::from).filter(|_| program == default_llc()) {
        Some(path) => path,
        None => beside().unwrap_or_else(|| program.into()),
    }
}

fn run(
    program: &Path,
    args: &[String],
) -> Result<(), i32> {
    match Command::new(program).args(args).status() {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(status.code().unwrap_or(1)),
        Err(error) => {
            eprintln!("llrm-cc: {}: {error}", program.display());
            Err(127)
        }
    }
}

fn check_pin(clang: &Path) -> Result<(), String> {
    let out = Command::new(clang).arg("--version").output().map_err(|error| format!("{}: {error}", clang.display()))?;
    if String::from_utf8_lossy(&out.stdout).contains(PIN.trim()) {
        Ok(())
    } else {
        Err(format!("{} is not the pinned clang {}", clang.display(), PIN.trim()))
    }
}

fn datalayout_of(ir: &str) -> Option<&str> {
    ir.lines().find_map(|line| line.strip_prefix("target datalayout = \"")?.split('"').next())
}

fn stem(input: &str) -> String {
    Path::new(input).file_stem().and_then(|one| one.to_str()).unwrap_or("a").to_owned()
}

fn build(
    target: &Target,
    build: &Build,
    scratch: &Path,
) -> Result<(), i32> {
    let clang = clang_path();
    let fail = |why: String| {
        eprintln!("llrm-cc: {why}");
        1
    };
    if build.output.is_some() && build.stop != Stop::Executable && build.inputs.len() > 1 {
        return Err(fail("cannot specify -o with -c or -S and several inputs".into()));
    }
    let mut objects = vec![];
    for (number, input) in build.inputs.iter().enumerate() {
        let single = build.inputs.len() == 1;
        let want = |extension: &str| -> String {
            match (&build.output, build.stop == Stop::Executable) {
                (Some(path), false) if single => path.clone(),
                (_, false) => format!(
                    "{}.{extension}",
                    stem(match input {
                        Input::Source(path) | Input::Assemble(path) | Input::Object(path) => path,
                    })
                ),
                (_, true) => scratch.join(format!("{number}.{extension}")).display().to_string(),
            }
        };
        match input {
            Input::Object(path) => objects.push(path.clone()),
            Input::Assemble(path) => {
                let object = want("o");
                let mut args = target.clang.clone();
                args.extend(build.clang.iter().cloned());
                args.extend(["-c".into(), "-o".into(), object.clone(), path.clone()]);
                run(&clang, &args)?;
                objects.push(object);
            }
            Input::Source(path) => {
                let ir = scratch.join(format!("{number}.ll"));
                let mut args = target.clang.clone();
                args.extend(build.clang.iter().cloned());
                args.extend(
                    ["-S", "-emit-llvm", "-Xclang", "-disable-llvm-passes", "-Xclang", "-disable-O0-optnone", "-o"]
                        .map(String::from),
                );
                args.extend([ir.display().to_string(), path.clone()]);
                run(&clang, &args)?;
                let text = std::fs::read_to_string(&ir).map_err(|error| fail(format!("{}: {error}", ir.display())))?;
                let printed = datalayout_of(&text).unwrap_or_default();
                let expected = target.clang_datalayout.as_deref().unwrap_or(&target.datalayout);
                if printed != expected && layout_facts(printed) != layout_facts(&target.datalayout) {
                    return Err(fail(format!(
                        "{path}: clang's datalayout `{printed}` lays types out differently from the table's `{}`",
                        target.datalayout
                    )));
                }
                if printed != target.datalayout {
                    let line = format!("target datalayout = \"{printed}\"");
                    let fixed = text.replacen(&line, &format!("target datalayout = \"{}\"", target.datalayout), 1);
                    std::fs::write(&ir, fixed).map_err(|error| fail(format!("{}: {error}", ir.display())))?;
                }
                let object = want(if build.stop == Stop::Assembly { "s" } else { "o" });
                run(&llc_path(&target.llc_program), &llc_args(target, build, &ir, Path::new(&object)))?;
                objects.push(object);
            }
        }
    }
    if build.stop == Stop::Executable {
        let output = build.output.clone().unwrap_or_else(|| "a.out".into());
        let command = link_command(target, &objects, &build.link, &output).map_err(fail)?;
        run(Path::new(&command[0]), &command[1..])?;
    }
    Ok(())
}

/// Exit status: the failing tool's, 1 for a refusal of the driver's own.
pub fn run_main(argv: &[String]) -> i32 {
    let go = || -> Result<(), i32> {
        let table = targets(TARGETS).map_err(|why| {
            eprintln!("llrm-cc: {why}");
            1
        })?;
        let (named, job) = parse(argv).map_err(|why| {
            eprintln!("llrm-cc: {why}");
            1
        })?;
        let name = named.or_else(|| std::env::var("LLRM_CC_TARGET").ok()).unwrap_or_else(|| DEFAULT_TARGET.into());
        let target = target_named(&table, &name).map_err(|why| {
            eprintln!("llrm-cc: {why}");
            1
        })?;
        check_pin(&clang_path()).map_err(|why| {
            eprintln!("llrm-cc: {why}");
            1
        })?;
        match job {
            Job::Passthrough(args) => {
                let mut all = target.clang.clone();
                all.extend(args);
                run(&clang_path(), &all)
            }
            Job::Build(one) => {
                let scratch = std::env::temp_dir().join(format!("llrm-cc-{}", std::process::id()));
                std::fs::create_dir_all(&scratch).map_err(|error| {
                    eprintln!("llrm-cc: {}: {error}", scratch.display());
                    1
                })?;
                let done = build(target, &one, &scratch);
                let _ = std::fs::remove_dir_all(&scratch);
                done
            }
        }
    };
    go().err().unwrap_or(0)
}

#[cfg(test)]
mod tests;
