//! `difftest FIRST COUNT [JOBS]` (or `DIFF_SEEDS=first:count`, `DIFF_JOBS`, `DIFF_FILE`): random C programs (`llrm-cgen`, 32-bit `int`) compiled by the pin's
//! clang to IR, by this compiler to a static m68k executable run under `qemu-m68k`, against the same
//! program built by m68k gcc -O2 for the 68020 under qemu (the toolchain has no libgcc, so no 68000 divide)
//! and by the host's gcc at -O0, whose 64-bit `long` can differ. The 8-bit
//! fold of the 16-bit checksum must agree.
//!
//! Each seed ends `ok`, `differs`, `hang` (ours ran past the time limit), `reject` (clang, the
//! compiler or the assembler said no: the reason is printed) or `reference` (the host could not build
//! it). A program that does not agree is kept in `$DIFF_KEEP` (default `$TMPDIR/llrm-m68k-difftest`).
//!
//! `$LLRM_PIN_CLANG` is the pin's clang; `$GCC_M68K` the unpacked cross toolchain.

use std::path::{Path, PathBuf};
use std::process::Command;

use llrm_cgen::{DataModel, Options as Gen, program};
use llrm_m68k::compile::{Item, Options, items_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::kernels::{DATALAYOUT, source_in};
use llrm_m68k::mc::{Inst, Operand};
use llrm_m68k::object::{executable, write_executable};
use llrm_m68k::reg;

/// The data the generated programs declare: `buf` (16 bytes), `tab` (8) and the volatile `zero`.
const BUF: [u8; 16] = [3, 1, 4, 1, 5, 9, 2, 6, 5, 3, 5, 8, 9, 7, 9, 3];
const TAB: [u8; 8] = [2, 5, 8, 11, 14, 17, 20, 23];

/// What the harness adds to a program: the function the image calls, folding `test` to a byte.
const FOLD: &str = "\nunsigned char k(void) { unsigned short r = test(); return (unsigned char)(r ^ (r >> 8)); }\n";
const HOST_MAIN: &str = "#include <stdio.h>\nconst unsigned char buf[16] = {3,1,4,1,5,9,2,6,5,3,5,8,9,7,9,3};\nconst unsigned char tab[8] = {2,5,8,11,14,17,20,23};\nconst volatile unsigned char zero = 0;\nunsigned short test(void);\nint main(void) { unsigned r = test(); printf(\"%u\\n\", (r ^ (r >> 8)) & 255); return 0; }\n";

#[derive(Debug)]
enum Outcome {
    Ok,
    Differs { ours: i32, reference: i32, host: i32 },
    Hang,
    Reject(String),
    Reference(String),
}

/// `$DIFF_OPTIONS`: `raw` (no mid-end), `size`, `pinned` (no overlay) or `os-raw`; which layer a failure is in.
fn options() -> Options {
    match std::env::var("DIFF_OPTIONS").as_deref() {
        Ok("raw") => Options { mid_end: false, ..Options::default() },
        Ok("size") => Options { size: true, ..Options::default() },
        Ok("pinned") => Options::PINNED,
        _ => Options::default(),
    }
}

fn clang() -> PathBuf {
    std::env::var_os("LLRM_PIN_CLANG").map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(std::env::var("HOME").unwrap()).join("scratch/llvm-build/bin/clang")
    })
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

/// `_start`, the compiled functions, padding to a long and the data with its labels.
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

fn qemu(
    exe: &Path,
    limit: &str,
    cpu: &str,
) -> Result<i32, bool> {
    let status = Command::new("timeout").args([limit, "qemu-m68k", "-cpu", cpu]).arg(exe).status().map_err(|_| false)?;
    match status.code() {
        Some(124) | Some(137) => Err(true),
        Some(code) => Ok(code),
        None => Err(false),
    }
}

fn gcc_m68k(
    dir: &Path,
    source: &str,
) -> Option<i32> {
    let root = PathBuf::from(
        std::env::var("GCC_M68K").unwrap_or(format!("{}/scratch/gcc-m68k/root", std::env::var("HOME").ok()?)),
    );
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
    qemu(&exe, "10", "m68020").ok()
}

fn check(
    seed: u64,
    work: &Path,
) -> Outcome {
    let dir = work.join(seed.to_string());
    std::fs::create_dir_all(&dir).unwrap();
    // `$DIFF_FILE` runs a hand-written program (with a `test`), the way `llrm-cgen/reduce.py` cuts one down
    let source = match std::env::var("DIFF_FILE") {
        Ok(path) => std::fs::read_to_string(path).unwrap(),
        Err(_) => program(seed, &Gen { opaque: true, ..Gen::new(DataModel::INT32) }),
    };
    let full = format!("{source}{FOLD}");
    std::fs::write(dir.join("p.c"), &full).unwrap();

    // a reduced program must stay defined: no missing return, no read of a local before it is set
    if std::env::var_os("DIFF_STRICT").is_some() {
        let strict = run(
            Command::new(clang()).current_dir(&dir).args([
                "--target=m68k-linux-gnu",
                "-fsyntax-only",
                "-Werror=return-type",
                "-Werror=uninitialized",
                "-Werror=sometimes-uninitialized",
                "-Werror=conditional-uninitialized",
                "p.c",
            ]),
            "not a defined program",
        );
        if let Err(why) = strict {
            return Outcome::Reject(why);
        }
    }

    // the reference: the host's gcc at -O0
    std::fs::write(dir.join("main.c"), HOST_MAIN).unwrap();
    let built = run(
        Command::new("gcc").current_dir(&dir).args(["-O0", "-w", "-o", "host", "main.c", "p.c"]),
        "host gcc",
    );
    if let Err(why) = built {
        return Outcome::Reference(why);
    }
    let host: i32 = match run(&mut Command::new("timeout").arg("10").arg(dir.join("host")), "host run") {
        Ok(text) => text.trim().parse().unwrap_or(-1),
        Err(why) => return Outcome::Reference(why),
    };
    // m68k gcc has the 32-bit `long` of the target; the host's is 64 bits, so it is the second opinion
    let gcc_m68k = gcc_m68k(&dir, &full);
    let reference = gcc_m68k.unwrap_or(host);

    // ours
    let ir = match run(
        Command::new(clang()).current_dir(&dir).args([
            "--target=m68k-linux-gnu",
            "-mcpu=68000",
            "-O1",
            "-fno-unroll-loops",
            "-S",
            "-emit-llvm",
            "p.c",
            "-o",
            "p.ll",
        ]),
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
        match items_with(&module, &name, Cpu::M68000, options()) {
            Ok(code) => items.extend(code),
            Err(why) => return Outcome::Reject(format!("{name}: {why}")),
        }
    }
    let bytes = match executable(&image(items)) {
        Ok(bytes) => bytes,
        Err(why) => return Outcome::Reject(format!("assemble: {why}")),
    };
    let exe = dir.join("ours");
    write_executable(&exe, &bytes).unwrap();
    match qemu(&exe, "10", "m68000") {
        Ok(code) if code == reference => Outcome::Ok,
        Ok(code) => Outcome::Differs { ours: code, reference, host },
        Err(true) => Outcome::Hang,
        Err(false) => Outcome::Reject("qemu did not run".to_owned()),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    // `$DIFF_SEEDS=first:count` and `$DIFF_JOBS` are what the reducer sets
    let (first, count) = std::env::var("DIFF_SEEDS")
        .ok()
        .and_then(|spec| spec.split_once(':').and_then(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?))))
        .unwrap_or_else(|| {
            (args.get(1).and_then(|a| a.parse().ok()).unwrap_or(0), args.get(2).and_then(|a| a.parse().ok()).unwrap_or(100))
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
                    if matches!(outcome, Outcome::Ok) {
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
            Outcome::Differs { ours, reference, host } => {
                differs += 1;
                println!("seed {seed}: MISMATCH ours {ours}, m68k gcc -O2 {reference}, host gcc -O0 {host}");
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
    println!("{count} programs from seed {first}: {ok} ok, {differs} differ, {hang} hang, {reject} reject, {reference} no reference");
}
