//! `difftest FIRST COUNT [JOBS]` (or `DIFF_SEEDS=first:count`, `DIFF_JOBS`,
//! `DIFF_FILE`), or `difftest DIR` for the reduced programs of
//! `tests/difftest/`: random C programs (`llrm-cgen`, 32-bit `int`) compiled by
//! the pin's clang to IR, by this compiler to a static m68k executable run
//! under `qemu-m68k`, against the same program built by m68k gcc -O2 for the
//! 68020 under qemu (the toolchain has no libgcc, so no 68000 divide)
//! and by the host's gcc at -m32 -O0 (ILP32, so `long` is as wide). The 8-bit
//! fold of the 16-bit checksum must agree.
//!
//! Each seed ends `ok`, `differs`, `hang` (ours ran past the time limit),
//! `reject` (clang, the compiler or the assembler said no: the reason is
//! printed) or `reference` (the host could not build it). A program that does
//! not agree is kept in `$DIFF_KEEP` (default `$TMPDIR/llrm-m68k-difftest`).
//!
//! `$DIFF_CPU=m68020` compiles for the 68020. `$DIFF_UBSAN=1` builds the host
//! program with `-fsanitize=undefined`: one that trips it has no reference.
//!
//! `$LLRM_PIN_CLANG` is the pin's clang; `$GCC_M68K` the unpacked cross
//! toolchain.

use std::path::{Path, PathBuf};
use std::process::Command;

use llrm_cgen::{DataModel, Options as Gen, program};
use llrm_m68k::compile::{Item, Options, items_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::kernels::{DATALAYOUT, source_in};
use llrm_m68k::mc::{Inst, Operand};
use llrm_m68k::object::{executable, write_executable};
use llrm_m68k::reg;

/// The data the generated programs declare: `buf` (16 bytes), `tab` (8) and the
/// volatile `zero`.
const BUF: [u8; 16] = [3, 1, 4, 1, 5, 9, 2, 6, 5, 3, 5, 8, 9, 7, 9, 3];
const TAB: [u8; 8] = [2, 5, 8, 11, 14, 17, 20, 23];

/// What the harness adds to a program: the function the image calls, folding
/// `test` to a byte.
const FOLD: &str = "\nunsigned char k(void) { unsigned short r = test(); return (unsigned char)(r ^ (r >> 8)); }\n";
const HOST_MAIN: &str = "#include <stdio.h>\nconst unsigned char buf[16] = {3,1,4,1,5,9,2,6,5,3,5,8,9,7,9,3};\nconst unsigned char tab[8] = {2,5,8,11,14,17,20,23};\nconst volatile unsigned char zero = 0;\nunsigned short test(void);\nint main(void) { unsigned r = test(); printf(\"%u\\n\", (r ^ (r >> 8)) & 255); return 0; }\n";

#[derive(Debug)]
enum Outcome {
    Ok,
    Differs {
        ours: i32,
        reference: i32,
        host: i32,
    },
    /// Right once and different on a stack that starts elsewhere: a read of
    /// what was never written.
    Unstable {
        first: i32,
        second: i32,
    },
    Hang,
    Reject(String),
    Reference(String),
}

/// `$DIFF_OPTIONS`: `raw` (no mid-end), `size`, `pinned` (neither);
/// which layer a failure is in.
fn options() -> Options {
    match std::env::var("DIFF_OPTIONS").as_deref() {
        Ok("raw") => Options { mid_end: false, ..Options::default() },
        Ok("size") => Options { size: true, ..Options::default() },
        Ok("pinned") => Options::PINNED,
        _ => Options::default(),
    }
}

/// `$DIFF_CPU`: `m68020` selects 68020 code (the instructions the 68000 table
/// hides: `mulu.l`, `divs.l`).
fn cpu() -> Cpu {
    match std::env::var("DIFF_CPU").as_deref() {
        Ok("m68020") => Cpu::M68020,
        _ => Cpu::M68000,
    }
}

/// The qemu model that runs what this compiler made.
fn qemu_cpu() -> &'static str {
    match cpu() {
        Cpu::M68020 => "m68020",
        _ => "m68000",
    }
}

fn clang() -> PathBuf {
    std::env::var_os("LLRM_PIN_CLANG").map(PathBuf::from).expect("LLRM_PIN_CLANG: the pin's clang")
}

/// A guarded child ended: its exit code and stdout, or the deadline passed.
enum Guarded {
    Exited(Option<i32>, String),
    TimedOut,
}

/// Runs `program` in its own process group under a CPU limit (`ulimit -t`,
/// which holds even if this driver is gone) and a wall deadline, and kills the
/// group on every way out. Plain `Command::output` left looping programs at 97%
/// CPU for an hour, and uutils `timeout -s KILL` never fired on this box.
/// Ported from the 6502 harness (ce5f75ba59).
fn guarded(
    program: &Path,
    args: &[&str],
    seconds: u32,
    env: &[(&str, &str)],
) -> Option<Guarded> {
    use std::io::Read;
    use std::os::unix::process::CommandExt;
    use std::process::Stdio;

    struct Group(u32);
    impl Drop for Group {
        fn drop(&mut self) {
            let _ = Command::new("kill").args(["-KILL", "--", &format!("-{}", self.0)]).stderr(Stdio::null()).status();
        }
    }

    let mut child = Command::new("sh")
        .args(["-c", &format!("ulimit -t {seconds}; exec \"$0\" \"$@\"")])
        .arg(program)
        .args(args)
        .envs(env.iter().copied())
        .process_group(0)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let _group = Group(child.id());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(seconds.into());
    let status = loop {
        if let Some(status) = child.try_wait().ok()? {
            break status;
        }
        if std::time::Instant::now() > deadline {
            return Some(Guarded::TimedOut);
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    let mut text = String::new();
    child.stdout.take()?.read_to_string(&mut text).ok()?;
    // killed by the CPU limit: SIGXCPU (24) or SIGKILL (9)
    use std::os::unix::process::ExitStatusExt;
    if matches!(status.signal(), Some(9 | 24)) {
        return Some(Guarded::TimedOut);
    }
    Some(Guarded::Exited(status.code(), text))
}

fn run(
    command: &mut Command,
    what: &str,
) -> Result<String, String> {
    let out = command.output().map_err(|error| format!("{what}: {error}"))?;
    if out.status.success() {
        return Ok(String::from_utf8_lossy(&out.stdout).into_owned());
    }
    let why = String::from_utf8_lossy(&out.stderr);
    Err(format!("{what}: {}", why.lines().next().unwrap_or("failed")))
}

/// clang's IR in MIR's subset: no attributes, linkage or metadata decorations.
fn sanitize(ir: &str) -> String {
    const DROP: [&str; 14] = [
        " dso_local",
        " local_unnamed_addr",
        " noundef",
        " zeroext",
        " signext",
        " inbounds",
        " nuw",
        " nsw",
        " tail",
        " unnamed_addr",
        " nonnull",
        " noalias",
        " nocapture",
        " readonly",
    ];
    let mut out = Vec::new();
    for line in ir.lines() {
        if line.starts_with("target triple")
            || line.starts_with("attributes ")
            || line.starts_with('!')
            || line.starts_with("source_filename")
            || line.starts_with("; ModuleID")
        {
            continue;
        }
        if line.starts_with("target datalayout") {
            out.push(format!("target datalayout = \"{DATALAYOUT}\""));
            continue;
        }
        let mut line = line.to_owned();
        for word in DROP {
            line = line.replace(word, "");
        }
        // trailing `, !tbaa !2`, `, !llvm.loop !5` and ` #0` on a define
        if let Some(at) = line.find(", !") {
            line.truncate(at);
        }
        if let Some(at) = line.find(" #") {
            let rest = line[at + 2..].trim_start_matches(|c: char| c.is_ascii_digit()).to_owned();
            line.truncate(at);
            line.push_str(&rest);
        }
        out.push(line);
    }
    out.join("\n")
}

fn defined(ir: &str) -> Vec<String> {
    ir.lines()
        .filter(|line| line.starts_with("define "))
        .filter_map(|line| line.split_once('@').map(|(_, rest)| rest.split('(').next().unwrap_or("").to_owned()))
        .collect()
}

fn inst(
    opcode: &'static str,
    operands: Vec<Operand>,
) -> Item {
    Item::Inst(Inst { opcode, operands })
}

/// `_start`, the compiled functions, padding to a long and the data with its
/// labels.
fn image(functions: Vec<Item>) -> Vec<Item> {
    let mut all = vec![
        Item::Label("_start".to_owned()),
        inst("CALLb", vec![Operand::Symbol("k".to_owned(), 0)]),
        inst("MOV32rr", vec![Operand::Reg(reg::D1), Operand::Reg(reg::D0)]),
        inst("MOVQ", vec![Operand::Reg(reg::D0), Operand::Imm(1)]),
        inst("TRAP", vec![Operand::Imm(0)]),
    ];
    all.extend(functions);
    let size = llrm_m68k::object::assemble(&all, 0).map(|one| one.code.len()).unwrap_or(0);
    all.push(Item::Bytes(vec![0; (4 - size % 4) % 4]));
    for (label, bytes) in [("buf", BUF.to_vec()), ("tab", TAB.to_vec()), ("zero", vec![0, 0])] {
        all.push(Item::Label(label.to_owned()));
        all.push(Item::Bytes(bytes));
    }
    all
}

/// `exe` under qemu, with `pad` bytes more of environment: the stack it starts
/// with is different.
fn qemu(
    exe: &Path,
    limit: &str,
    cpu: &str,
    pad: usize,
) -> Result<i32, bool> {
    let seconds: u32 = limit.trim_end_matches('s').parse().unwrap_or(10);
    let padding = "x".repeat(pad);
    let env: Vec<(&str, &str)> = if pad == 0 { Vec::new() } else { vec![("LLRM_PAD", &padding)] };
    match guarded(Path::new("qemu-m68k"), &["-cpu", cpu, &exe.display().to_string()], seconds, &env) {
        Some(Guarded::TimedOut) => Err(true),
        Some(Guarded::Exited(Some(code), _)) => Ok(code),
        _ => Err(false),
    }
}

fn gcc_m68k(
    dir: &Path,
    source: &str,
) -> Option<i32> {
    let root = PathBuf::from(std::env::var("GCC_M68K").ok()?);
    let driver = root.join("usr/bin/m68k-linux-gnu-gcc");
    let start = "void _start(void) { extern unsigned char k(void); register int s asm(\"d1\") = k(); register int n asm(\"d0\") = 1; asm volatile(\"trap #0\" :: \"r\"(s), \"r\"(n)); for (;;); }\nconst unsigned char buf[16] = {3,1,4,1,5,9,2,6,5,3,5,8,9,7,9,3};\nconst unsigned char tab[8] = {2,5,8,11,14,17,20,23};\nconst volatile unsigned char zero = 0;\n";
    std::fs::write(dir.join("m.c"), format!("{start}{source}")).ok()?;
    let exe = dir.join("m68k-gcc");
    let ok = Command::new(&driver)
        .env("LD_LIBRARY_PATH", root.join("usr/lib/x86_64-linux-gnu"))
        .arg(format!("-B{}/usr/lib/gcc-cross/m68k-linux-gnu/15/", root.display()))
        .arg(format!("-B{}/usr/m68k-linux-gnu/bin/", root.display()))
        .args(["-ffreestanding", "-nostdlib", "-static", "-fno-asynchronous-unwind-tables", "-mcpu=68020", "-O2", "-w"])
        .arg(dir.join("m.c"))
        .arg("-o")
        .arg(&exe)
        .output()
        .ok()?
        .status
        .success();
    if !ok {
        return None;
    }
    qemu(&exe, "10", "m68020", 0).ok()
}

fn check(
    seed: u64,
    work: &Path,
) -> Outcome {
    // `$DIFF_FILE` runs a hand-written program (with a `test`), the way
    // `llrm-cgen/reduce.py` cuts one down
    let source = match std::env::var("DIFF_FILE") {
        Ok(path) => std::fs::read_to_string(path).unwrap(),
        Err(_) => program(seed, &Gen { opaque: true, ..Gen::new(DataModel::INT32) }),
    };
    check_text(&seed.to_string(), &source, work)
}

fn check_text(
    name: &str,
    source: &str,
    work: &Path,
) -> Outcome {
    let dir = work.join(name);
    std::fs::create_dir_all(&dir).unwrap();
    let full = format!("{source}{FOLD}");
    std::fs::write(dir.join("p.c"), &full).unwrap();

    // a reduced program must stay defined: no missing return, no read of a
    // local before it is set
    if std::env::var_os("DIFF_STRICT").is_some() {
        let strict = run(
            Command::new(clang())
                .current_dir(&dir)
                .args(
                    [
                        "--target=m68k-linux-gnu",
                        "-fsyntax-only",
                        "-Werror=return-type",
                        "-Werror=uninitialized",
                        "-Werror=sometimes-uninitialized",
                        "-Werror=conditional-uninitialized",
                        "p.c",
                    ],
                ),
            "not a defined program",
        );
        if let Err(why) = strict {
            return Outcome::Reject(why);
        }
    }

    // the second opinion: the host's gcc at -O0, as ILP32 like the m68k data
    // model, so `long` is as wide on both; `$DIFF_UBSAN` adds the
    // undefined-behaviour check (a program that trips it has no reference)
    std::fs::write(dir.join("main.c"), HOST_MAIN).unwrap();
    let mut host_args = vec!["-m32", "-O0", "-w"];
    if std::env::var_os("DIFF_UBSAN").is_some() {
        host_args.extend(["-fsanitize=undefined", "-fno-sanitize-recover=all"]);
    }
    host_args.extend(["-o", "host", "main.c", "p.c"]);
    let built = run(Command::new("gcc").current_dir(&dir).args(&host_args), "host gcc");
    if let Err(why) = built {
        return Outcome::Reference(why);
    }
    let host: i32 = match guarded(&dir.join("host"), &[], 10, &[]) {
        Some(Guarded::Exited(Some(0), text)) => text.trim().parse().unwrap_or(-1),
        Some(Guarded::TimedOut) => return Outcome::Reference("host run: timed out".into()),
        _ => return Outcome::Reference("host run: failed".into()),
    };
    // m68k gcc is the reference; the host's answer is the second opinion
    let gcc_m68k = gcc_m68k(&dir, &full);
    let reference = gcc_m68k.unwrap_or(host);

    // ours
    let ir = match run(
        Command::new(clang())
            .current_dir(&dir)
            .args(
                [
                    "--target=m68k-linux-gnu",
                    "-mcpu=68000",
                    "-O1",
                    "-fno-unroll-loops",
                    "-S",
                    "-emit-llvm",
                    "p.c",
                    "-o",
                    "p.ll",
                ],
            ),
        "clang",
    ) {
        Ok(_) => std::fs::read_to_string(dir.join("p.ll")).unwrap(),
        Err(why) => return Outcome::Reject(why),
    };
    let mut ir = sanitize(&ir);
    ir.push_str(source_in("library", "rt").split_once('\n').map_or("", |(_, rest)| rest));
    std::fs::write(dir.join("p.san.ll"), &ir).unwrap();
    let module = match llrm_mir::parse::module(&ir) {
        Ok(module) => module,
        Err(why) => return Outcome::Reject(format!("parse: {why}")),
    };
    let mut items = Vec::new();
    for name in defined(&ir) {
        match items_with(&module, &name, cpu(), options()) {
            Ok(code) => items.extend(code),
            Err(why) => return Outcome::Reject(format!("{name}: {why}")),
        }
    }
    // an instrument that never sees the form it tests agrees with anything
    let mut forms = 0;
    for item in &items {
        let Item::Inst(inst) = item else { continue };
        let Ok(text) = llrm_m68k::mc::print(inst) else { continue };
        let mnemonic = text.split_whitespace().next().unwrap_or("");
        if mnemonic.starts_with("addq") || mnemonic.starts_with("subq") {
            *INDEXED_FORMS.lock().unwrap().entry(mnemonic.to_owned()).or_insert(0usize) += 1;
            continue;
        }
        let direction = if text.contains(")+") {
            "(An)+"
        } else if text.contains("-(%a") {
            "-(An)"
        } else {
            continue;
        };
        let width = text.split_whitespace().next().unwrap_or("").trim_start_matches("move");
        *INDEXED_FORMS.lock().unwrap().entry(format!("move{width} {direction}")).or_insert(0usize) += 1;
        forms += 1;
    }
    if std::env::var_os("DIFF_STATIC").is_some() {
        let total: usize = items
            .iter()
            .map(|item| match item {
                Item::Inst(inst) => llrm_m68k::encode::encode(inst).map_or(0, |one| one.bytes.len()),
                _ => 0,
            })
            .sum();
        eprintln!("STATIC {name} {total}");
    }
    if forms > 0 {
        INDEXED_PROGRAMS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    let bytes = match executable(&image(items)) {
        Ok(bytes) => bytes,
        Err(why) => return Outcome::Reject(format!("assemble: {why}")),
    };
    let exe = dir.join("ours");
    write_executable(&exe, &bytes).unwrap();
    match qemu(&exe, "10", qemu_cpu(), 0) {
        // the same program on a stack that starts elsewhere: an answer that
        // moves read garbage
        Ok(code) if code == reference => match qemu(&exe, "10", qemu_cpu(), 4001) {
            Ok(other) if other != code => Outcome::Unstable { first: code, second: other },
            _ => Outcome::Ok,
        },
        Ok(code) => Outcome::Differs { ours: code, reference, host },
        Err(true) => Outcome::Hang,
        Err(false) => Outcome::Reject("qemu did not run".to_owned()),
    }
}

static INDEXED_PROGRAMS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static INDEXED_FORMS: std::sync::Mutex<std::collections::BTreeMap<String, usize>> =
    std::sync::Mutex::new(std::collections::BTreeMap::new());

/// `difftest DIR`: every `DIR/*.c` (a reduced program that once failed, with a
/// `test`) must agree.
fn corpus(dir: &Path) -> bool {
    let work = std::env::temp_dir().join(format!("llrm-m68k-corpus-{}", std::process::id()));
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|entry| Some(entry.ok()?.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "c"))
        .collect();
    files.sort();
    let mut bad = 0;
    for file in &files {
        let name = file.file_stem().unwrap().to_string_lossy().into_owned();
        match check_text(&name, &std::fs::read_to_string(file).unwrap(), &work) {
            Outcome::Ok => println!("{name}: ok"),
            other => {
                bad += 1;
                println!("{name}: {other:?}");
            }
        }
    }
    println!("{} programs: {} not ok", files.len(), bad);
    println!("forms used: {:?}", INDEXED_FORMS.lock().unwrap());
    bad == 0
}

/// What a program's answer is compared with besides the host's gcc.
#[derive(Debug, Eq, PartialEq)]
enum Reference {
    /// m68k gcc under qemu: the driver in the unpacked toolchain.
    M68k(PathBuf),
    /// The host's gcc alone, asked for with `$DIFF_HOST_ONLY`.
    HostOnly,
}

/// The reference to use: the m68k gcc under `root` (`$GCC_M68K`), or, only when
/// `host_only` is asked for, none. A root with no driver is an error: the
/// host-only fallback was once taken silently from a wrong path.
fn reference(
    root: Option<&std::ffi::OsStr>,
    host_only: bool,
) -> Result<Reference, String> {
    if host_only {
        return Ok(Reference::HostOnly);
    }
    let root = root.ok_or("GCC_M68K is not set (DIFF_HOST_ONLY=1 compares with the host's gcc alone)")?;
    let driver = Path::new(root).join("usr/bin/m68k-linux-gnu-gcc");
    if driver.exists() {
        Ok(Reference::M68k(driver))
    } else {
        Err(format!("no m68k gcc at {} (DIFF_HOST_ONLY=1 compares with the host's gcc alone)", driver.display()))
    }
}

/// The first line of `gcc --version`.
fn version(driver: &Path) -> String {
    Command::new(driver)
        .arg("--version")
        .output()
        .ok()
        .and_then(|out| String::from_utf8_lossy(&out.stdout).lines().next().map(str::to_owned))
        .unwrap_or_else(|| "version unknown".to_owned())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match reference(std::env::var_os("GCC_M68K").as_deref(), std::env::var_os("DIFF_HOST_ONLY").is_some()) {
        Ok(Reference::M68k(driver)) => eprintln!("difftest: reference {} ({})", driver.display(), version(&driver)),
        Ok(Reference::HostOnly) => eprintln!("difftest: DIFF_HOST_ONLY: the reference is the host's gcc -m32 alone"),
        Err(why) => {
            eprintln!("difftest: {why}");
            std::process::exit(2);
        }
    }
    if let Some(dir) = args.get(1).map(Path::new).filter(|path| path.is_dir()) {
        std::process::exit(i32::from(!corpus(dir)));
    }
    // `$DIFF_SEEDS=first:count` and `$DIFF_JOBS` are what the reducer sets
    let (first, count) = std::env::var("DIFF_SEEDS")
        .ok()
        .and_then(|spec| spec.split_once(':').and_then(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?))))
        .unwrap_or_else(|| {
            (
                args.get(1).and_then(|a| a.parse().ok()).unwrap_or(0),
                args.get(2).and_then(|a| a.parse().ok()).unwrap_or(100),
            )
        });
    let jobs: usize = std::env::var("DIFF_JOBS")
        .ok()
        .and_then(|j| j.parse().ok())
        .or_else(|| args.get(3).and_then(|a| a.parse().ok()))
        .unwrap_or(2);
    let work = PathBuf::from(
        std::env::var("DIFF_KEEP").unwrap_or_else(|_| format!("{}/llrm-m68k-difftest", std::env::temp_dir().display())),
    );
    std::fs::create_dir_all(&work).unwrap();
    let seeds: Vec<u64> = (first..first + count).collect();
    let next = std::sync::atomic::AtomicUsize::new(0);
    let results = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..jobs {
            scope.spawn(|| {
                loop {
                    let at = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    let Some(&seed) = seeds.get(at) else { break };
                    let outcome = std::panic::catch_unwind(|| check(seed, &work))
                        .unwrap_or_else(|_| Outcome::Reject("the compiler panicked".to_owned()));
                    if matches!(outcome, Outcome::Ok) && std::env::var_os("DIFF_KEEP_OK").is_none() {
                        let _ = std::fs::remove_dir_all(work.join(seed.to_string()));
                    }
                    results.lock().unwrap().push((seed, outcome));
                }
            });
        }
    });
    let mut results = results.into_inner().unwrap();
    results.sort_by_key(|(seed, _)| *seed);
    let (mut ok, mut differs, mut hang, mut reject, mut reference) = (0, 0, 0, 0, 0);
    for (seed, outcome) in &results {
        match outcome {
            Outcome::Ok => ok += 1,
            Outcome::Unstable { first, second } => {
                differs += 1;
                println!("seed {seed}: MISMATCH ours {first}, then {second} on another stack (reads garbage)");
            }
            Outcome::Differs { ours, reference, host } => {
                differs += 1;
                println!("seed {seed}: MISMATCH ours {ours}, m68k gcc -O2 {reference}, host gcc -m32 -O0 {host}");
            }
            Outcome::Hang => {
                hang += 1;
                println!("seed {seed}: HANG");
            }
            Outcome::Reject(why) => {
                reject += 1;
                println!("seed {seed}: reject: {why}");
            }
            Outcome::Reference(why) => {
                reference += 1;
                println!("seed {seed}: no reference: {why}");
            }
        }
    }
    println!(
        "{count} programs from seed {first}: {ok} ok, {differs} differ, {hang} hang, {reject} reject, {reference} no reference"
    );
    println!(
        "{} programs used (An)+ or -(An): {:?}",
        INDEXED_PROGRAMS.load(std::sync::atomic::Ordering::Relaxed),
        INDEXED_FORMS.lock().unwrap()
    );
}

/// A looping program is killed at the limit and leaves nothing behind. The
/// driver ran host programs and qemu under `timeout`, which did not kill them:
/// loops piled up at 97% CPU.
#[cfg(test)]
mod tests {
    use super::*;

    /// A wrong `$GCC_M68K` ran 300 programs against the host's gcc alone and
    /// reported `300 ok`; no root, or one without a driver, is an error unless
    /// the host-only run is asked for.
    #[test]
    fn a_missing_m68k_gcc_is_an_error_unless_host_only_is_asked_for() {
        use std::ffi::OsStr;
        assert!(reference(None, false).is_err());
        assert!(reference(Some(OsStr::new("/no/such/root")), false).is_err());
        assert_eq!(reference(Some(OsStr::new("/no/such/root")), true), Ok(Reference::HostOnly));
        assert_eq!(reference(None, true), Ok(Reference::HostOnly));
    }

    use super::*;

    /// The environment pads the stack, so a program that reads its stack
    /// address shows it: the second run of `check` relies on it. A read of
    /// an unwritten slot gave a plausible answer that moved with the stack
    /// (seed 86), and the driver took it for right.
    #[test]
    fn an_environment_pad_moves_the_stack() {
        let dir = std::env::temp_dir().join(format!("llrm-m68k-pad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let source = "unsigned char k(void) { volatile int x = 0; return (unsigned)&x >> 4; }\n";
        if gcc_m68k(&dir, source).is_none() {
            return; // no m68k gcc or qemu here
        }
        let exe = dir.join("m68k-gcc");
        let usual = qemu(&exe, "10", "m68020", 0).unwrap();
        let padded = qemu(&exe, "10", "m68020", 4001).unwrap();
        assert_ne!(usual, padded);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_program_that_never_exits_is_killed() {
        let dir = std::env::temp_dir().join(format!("llrm-m68k-guard-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("spin.c"), "int main(void) { for (;;) ; }\n").unwrap();
        assert!(
            Command::new("gcc").current_dir(&dir).args(["-O0", "-o", "spin", "spin.c"]).status().unwrap().success()
        );
        let bin = dir.join("spin").canonicalize().unwrap();
        let start = std::time::Instant::now();
        assert!(matches!(guarded(&bin, &[], 1, &[]), Some(Guarded::TimedOut)));
        assert!(start.elapsed().as_secs() < 10, "took {:?}", start.elapsed());
        let alive = std::fs::read_dir("/proc")
            .unwrap()
            .flatten()
            .filter(|entry| std::fs::read_link(entry.path().join("exe")).is_ok_and(|exe| exe == bin))
            .count();
        assert_eq!(alive, 0, "{alive} copies of {} still running", bin.display());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
