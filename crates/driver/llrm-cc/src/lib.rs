//! `llrm-cc`: a cc-compatible driver that runs the pinned, unmodified clang for
//! the front end (`-S -emit-llvm -Xclang -disable-llvm-passes`) and `llrm-llc`
//! for the rest. Only phase flags are understood; every other flag goes to
//! clang. Preprocess, dependency and query jobs run clang as is.

mod inline_asm;
mod lifetimes;

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
    /// An environment variable naming a clang for this target (a patched
    /// build), ahead of the pinned one.
    pub clang_env: Option<String>,
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
const PIN_FILE: &str = include_str!("../../../../toolchain/clang/pin");

/// The pin file's first line. The lines after it are builds of the pin that
/// carry patches, and report their own commit.
fn pin() -> &'static str {
    PIN_FILE.lines().next().unwrap_or_default().trim()
}
/// The llrm checkout this driver was built in, for the tools the recipes name.
const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../..");
const DEFAULT_TARGET: &str = "x86_64-linux";

/// `text` with each `{datalayout:NAME}` replaced by the `datalayout` of
/// crates/target/llrm-NAME's description.
fn expanded(text: &str) -> Result<String, String> {
    let mut out = text.to_owned();
    while let Some(start) = out.find("{datalayout:") {
        let end = start + out[start..].find('}').ok_or("targets.toml: unclosed {datalayout:")?;
        let name = &out[start + "{datalayout:".len()..end];
        let path = format!("{ROOT}/crates/target/llrm-{name}/src/machines/datalayout.toml");
        let description = std::fs::read_to_string(&path).map_err(|error| format!("{path}: {error}"))?;
        let table: toml::Table = description.parse().map_err(|error| format!("{path}: {error}"))?;
        let layout = table.get("datalayout").and_then(toml::Value::as_str).ok_or(format!("{path}: no datalayout"))?;
        out.replace_range(start..=end, layout);
    }
    Ok(out)
}

pub fn targets(text: &str) -> Result<BTreeMap<String, Target>, String> {
    let mut table: BTreeMap<String, Target> = toml::from_str(text).map_err(|error| format!("targets.toml: {error}"))?;
    for target in table.values_mut() {
        target.datalayout = expanded(&target.datalayout)?;
        target.clang_datalayout = target.clang_datalayout.take().map(|one| expanded(&one)).transpose()?;
        for word in target.clang.iter_mut().chain(&mut target.llc) {
            *word = word.replace("{root}", ROOT);
        }
        for word in target.clang.iter_mut().chain(&mut target.llc).chain(&mut target.link) {
            *word = with_environment(word);
        }
    }
    Ok(table)
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
/// The backend's own flags (llrm-llc's `Flags`), which clang does not know.
const LLC_FLAGS: &[&str] = &[
    "-mstack-is-data",
    "-mno-stack-is-data",
    "-mfar-bss",
    "-mno-far-bss",
    "-mregcall-default",
    "-mc-library",
    "-mbss-begin",
    "-mno-bss-begin",
    "-fstack-usage",
    "-ffloat-store",
    "-fverify-ir",
    "-fno-float-store",
];
const LLC_PREFIXES: &[&str] =
    &["--isel=", "-fobject-format=", "-mcode-segment=", "-fno-inline=", "-Wstack-usage=", "-fexcess-precision="];
/// The passes `-f[no-]PASS` switches in the backend's pipeline.
const BACKEND_PASSES: &[&str] = &[
    "allocation-search",
    "allocation-search-all",
    "peel-loops",
    "inline-functions-called-once",
    "inline-functions",
    "strength-reduce",
    "lsr",
    "unswitch-loops",
    "loop-interchange",
    "loop-unroll-and-jam",
    "tree-ch",
    "ipa-cp-clone",
    "inline-recursive",
    "gcse",
    "tree-pre",
    "tree-switch-conversion",
    "jump-tables",
    "tree-dse",
    "tree-dce",
    "tree-sink",
    "llrm-spares",
    "llrm-homes",
    "llrm-stable-loads",
    "tree-sra",
    "move-loop-invariants",
    "tree-loop-distribute-patterns",
    "optimize-sibling-calls",
];

fn backend_pass(flag: &str) -> Option<&'static str> {
    let name = flag.strip_prefix("-f")?;
    let name = name.strip_prefix("no-").unwrap_or(name);
    BACKEND_PASSES.iter().copied().find(|pass| *pass == name)
}

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
        } else if let Some(list) = flag.strip_prefix("-fsanitize=").or_else(|| flag.strip_prefix("-fno-sanitize="))
            && list.split(',').all(|name| matches!(name, "bounds" | "stack"))
        {
            // The checks are the backend's (gcc's names, as llrm-c took them):
            // `stack` checks SP on entry; `bounds` checks nothing in IR, which
            // carries no subscripts.
            build.llc.push(flag.to_owned());
        } else if REFUSED.iter().any(|one| flag.starts_with(one)) {
            return Err(format!("{flag}: not translated yet"));
        } else if flag.starts_with("-g") && flag != "-g0" {
            // clang writes the debug metadata, the backend the format `flag`
            // names.
            if !build.clang.iter().any(|one| one == "-g") {
                // No `#dbg_assign`: the importer reads `#dbg_value` and
                // `#dbg_declare` only.
                build
                    .clang
                    .extend(
                        ["-g", "-fdebug-compilation-dir=.", "-Xclang", "-fexperimental-assignment-tracking=disabled"]
                            .map(str::to_owned),
                    );
            }
            build.llc.push(flag.to_owned());
        } else if let Some(level) = flag.strip_prefix("-O") {
            build.level = match level {
                "" => "1".into(),
                "fast" => "3".into(),
                "g" => "1".into(),
                other => other.to_owned(),
            };
            // clang has no -Omax: its top level is -O3.
            build.clang.push(if level == "max" { "-O3".to_owned() } else { flag.to_owned() });
        } else if flag.starts_with("-mabi=") {
            // The backend's convention (clang's own -mabi is for other
            // targets); it follows the table's. Open Watcom's `char` is
            // unsigned, whatever the target's clang default.
            if flag == "-mabi=watcom" && !argv.iter().any(|one| one == "-fsigned-char") {
                build.clang.push("-funsigned-char".to_owned());
            }
            build.llc.push(flag.to_owned());
            if flag == "-mabi=watcom" {
                build.clang.push("-funsigned-char".to_owned());
            }
        } else if flag == "-funroll-loops" || flag == "-fno-unroll-loops" {
            build.clang.push(flag.to_owned());
            build.llc.push(flag.to_owned());
        } else if flag.starts_with("-march=") || flag.starts_with("-mtune=") {
            // The backend prices and selects for the CPU as clang does.
            build.clang.push(flag.to_owned());
            build.llc.push(flag.to_owned());
        } else if let Some(pass) = backend_pass(flag) {
            // The pipeline's `-f[no-]PASS` switches are the backend's; clang
            // knows only the inliner's.
            if pass == "inline-functions" {
                build.clang.push(flag.to_owned());
            }
            build.llc.push(flag.to_owned());
        } else if LLC_FLAGS.contains(&flag) || LLC_PREFIXES.iter().any(|one| flag.starts_with(one)) {
            build.llc.push(flag.to_owned());
        } else if flag == "--dump" {
            // llrm-c's switch: each backend stage written to the directory.
            build.llc.push(flag.to_owned());
            build.llc.push(value(&mut at)?);
        } else if flag.starts_with("--dump=") {
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

/// `word` with each `{env:NAME}` that is set replaced by its value; one that is
/// not stays, and `unset` names it.
fn with_environment(word: &str) -> String {
    let mut out = word.to_owned();
    while let Some(open) = out.find("{env:") {
        let Some(close) = out[open..].find('}') else { break };
        let name = &out[open + 5..open + close];
        let Some(value) = std::env::var_os(name) else { break };
        out.replace_range(open..=open + close, &value.to_string_lossy());
    }
    out
}

fn unset(word: &str) -> String {
    let name = word.split("{env:").nth(1).and_then(|rest| rest.split('}').next()).unwrap_or(word);
    format!("llrm-cc: the target needs the environment variable {name}")
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
    if let Some(open) = command.iter().find(|word| word.contains("{env:")) {
        return Err(unset(open));
    }
    if command.is_empty() {
        return Err("empty link recipe".into());
    }
    Ok(command)
}

fn clang_path(target: &Target) -> PathBuf {
    let own = target.clang_env.as_deref().and_then(std::env::var_os);
    own.or_else(|| std::env::var_os("LLRM_PIN_CLANG"))
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
    let said = String::from_utf8_lossy(&out.stdout);
    if PIN_FILE.lines().filter_map(|line| line.split_whitespace().next()).any(|sha| said.contains(sha)) {
        Ok(())
    } else {
        Err(format!("{} is not the pinned clang {}", clang.display(), pin()))
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
    let clang = clang_path(target);
    let fail = |why: String| {
        eprintln!("llrm-cc: {why}");
        1
    };
    if let Some(open) = target.clang.iter().chain(&target.llc).find(|word| word.contains("{env:")) {
        return Err(fail(unset(open)));
    }
    if build.output.is_some() && build.stop != Stop::Executable && build.inputs.len() > 1 {
        return Err(fail("cannot specify -o with -c or -S and several inputs".into()));
    }
    // `-nostdinc` drops clang's own freestanding headers (`stdint.h`,
    // `stdarg.h`); they go back after the target's.
    let resource: Vec<String> = if target.clang.iter().any(|word| word == "-nostdinc") {
        Command::new(&clang)
            .arg("-print-resource-dir")
            .output()
            .ok()
            .and_then(|out| String::from_utf8(out.stdout).ok())
            .map(|dir| vec!["-idirafter".to_owned(), format!("{}/include", dir.trim())])
            .unwrap_or_default()
    } else {
        Vec::new()
    };
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
                let ir = scratch.join(format!("{}.ll", stem(path)));
                let mut args = target.clang.clone();
                args.extend(resource.iter().cloned());
                args.extend(build.clang.iter().cloned());
                args.extend(
                    ["-S", "-emit-llvm", "-Xclang", "-disable-llvm-passes", "-Xclang", "-disable-O0-optnone", "-o"]
                        .map(String::from),
                );
                args.extend([ir.display().to_string(), path.clone()]);
                run(&clang, &args)?;
                let text = std::fs::read_to_string(&ir).map_err(|error| fail(format!("{}: {error}", ir.display())))?;
                let text = lifetimes::upgraded(&text);
                std::fs::write(&ir, &text).map_err(|error| fail(format!("{}: {error}", ir.display())))?;
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
                if text.contains(" asm sideeffect ") {
                    let mut scratch_number = 0;
                    let mut assemble = |source: &str| -> Result<Vec<u8>, String> {
                        scratch_number += 1;
                        let (s, o, b) = (
                            scratch.join(format!("{number}.{scratch_number}.s")),
                            scratch.join(format!("{number}.{scratch_number}.o")),
                            scratch.join(format!("{number}.{scratch_number}.bin")),
                        );
                        std::fs::write(&s, source).map_err(|error| error.to_string())?;
                        let ran = |program: &Path, args: &[&str]| {
                            Command::new(program).args(args).status().map(|one| one.success()).unwrap_or(false)
                        };
                        let (sp, op, bp) = (s.display().to_string(), o.display().to_string(), b.display().to_string());
                        if !ran(&clang, &["-target", "i386-unknown-none", "-x", "assembler", "-c", &sp, "-o", &op])
                            || !ran(Path::new("objcopy"), &["-O", "binary", "--only-section=.text", &op, &bp])
                        {
                            return Err(format!("asm: could not assemble\n{source}"));
                        }
                        std::fs::read(&b).map_err(|error| error.to_string())
                    };
                    let translated = inline_asm::translate(
                        &std::fs::read_to_string(&ir).map_err(|e| fail(e.to_string()))?,
                        &mut assemble,
                    )
                    .map_err(|why| fail(format!("{path}: {why}")))?;
                    std::fs::write(&ir, translated).map_err(|error| fail(format!("{}: {error}", ir.display())))?;
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
        // The start-up and the externals are linked near where the model's code
        // is near.
        let near_code = build.clang.iter().any(|flag| matches!(flag.as_str(), "-mcmodel=small" | "-mcmodel=compact"));
        let mut link = Command::new(&command[0]);
        link.args(&command[1..]);
        // The link script compiles the float helpers with this same driver,
        // wherever cargo put it.
        if let Ok(own) = std::env::current_exe() {
            link.env("LLRM_CC", own);
        }
        if near_code {
            link.env("LLRM_NEAR_CODE", "1");
        }
        // The recipes' tools build the float support with this driver.
        if let Ok(me) = std::env::current_exe() {
            link.env("LLRM_CC_BIN", me);
        }
        match link.status() {
            Ok(status) if status.success() => {}
            Ok(status) => return Err(status.code().unwrap_or(1)),
            Err(error) => {
                eprintln!("llrm-cc: {}: {error}", command[0]);
                return Err(127);
            }
        }
    }
    Ok(())
}

/// Exit status: the failing tool's, 1 for a refusal of the driver's own.
/// `--os-layer FIELD -m16|-m32`: what the target's OS layer says of C's runtime
/// (the tools that assemble the start-up ask; no clang runs).
pub fn os_layer(argv: &[String]) -> Option<Result<String, String>> {
    let at = argv.iter().position(|one| one == "--os-layer")?;
    let field = argv.get(at + 1).ok_or("--os-layer: expected one argument");
    let mode = argv.iter().find_map(|one| one.strip_prefix("-m").filter(|bits| matches!(*bits, "16" | "32")));
    Some(field.map_err(String::from).and_then(|field| {
        let name = format!("x86-m{}", mode.ok_or("--os-layer needs -m16 or -m32")?);
        let target = llrm_driver::all().into_iter().find(|one| one.name() == name).ok_or("no such target")?;
        let layer = target.os_layer().ok_or("this target has no OS layer")?;
        layer.report(&target.runtime("c").ok_or("this target has no C runtime")?, field)
    }))
}

pub fn run_main(argv: &[String]) -> i32 {
    if let Some(told) = os_layer(argv) {
        return match told {
            Ok(text) => {
                println!("{}", text.trim_end());
                0
            }
            Err(why) => {
                eprintln!("llrm-cc: {why}");
                2
            }
        };
    }
    let go = || -> Result<(), i32> {
        llrm_mir::dumpdir::refuse(argv).map_err(|why| {
            eprintln!("llrm-cc: {why}");
            2
        })?;
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
        check_pin(&clang_path(target)).map_err(|why| {
            eprintln!("llrm-cc: {why}");
            1
        })?;
        match job {
            Job::Passthrough(args) => {
                let mut all = target.clang.clone();
                all.extend(args);
                run(&clang_path(target), &all)
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
