//! cc65 against llrm-6502 on the same C kernels. `cargo test -p llrm-6502
//! --test bench -- --ignored --nocapture` prints the table (and writes
//! `bench.md` beside the test binaries' tmp dir).
//!
//! Method, the same for every compiler: a driver calls the function 1 and then
//! 101 times in a loop; cycles per call = (T101 - T1) / 100 - 9 (the loop's
//! `dec cnt; bne`, hand-counted), so crt0 and the program's start-up cancel and
//! a call includes its argument set-up and return. Bytes = the linked file with
//! the function minus the file with `fn: rts` (the one byte is added back), so
//! a library or runtime routine the function calls is counted and nothing else
//! is. The result is checked against a hand-derived value first.
//!
//! Needs cc65, clang (msp430 front end), LLVM 20 `opt`, cl65 and sim65.

mod common;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use llrm_6502::RUNTIME;
use llrm_6502::dagisel::function::select_function;
use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::emit::equates;
use llrm_6502::platform::{Platform, SIM65};
use llrm_6502::select::function_text;

/// The 6502 description's datalayout: n8 and 16-bit pointers.
fn datalayout() -> String {
    llrm_6502::LAYOUT
        .lines()
        .find_map(|l| l.strip_prefix("datalayout = "))
        .expect("a datalayout")
        .trim_matches('"')
        .to_owned()
}
const BUF: [u8; 16] = [3, 1, 4, 1, 5, 9, 2, 6, 5, 3, 5, 8, 9, 7, 9, 3];
/// The loop's `dec cnt` (6) and taken `bne` (3).
const LOOP_CYCLES: i64 = 9;

#[derive(Clone, Copy)]
enum Width {
    Byte,
    Word,
}

struct Kernel {
    name: &'static str,
    args: &'static [u16],
    width: Width,
    /// The result is 16 bits: A is the low byte, X the high.
    wide_result: bool,
    expect: u16,
}

const fn k(
    name: &'static str,
    args: &'static [u16],
    width: Width,
    wide_result: bool,
    expect: u16,
) -> Kernel {
    Kernel { name, args, width, wide_result, expect }
}

/// Every expected value is worked out by hand from the arguments and `BUF`.
const KERNELS: &[Kernel] = &[
    k("zero", &[], Width::Byte, false, 0),
    k("add8", &[200, 100], Width::Byte, false, 44),
    k("add16", &[300, 5000], Width::Word, true, 5300),
    k("max8", &[3, 9], Width::Byte, false, 9),
    k("max16", &[1000, 300], Width::Word, true, 1000),
    k("sum_arr", &[10], Width::Byte, false, 39),
    k("find", &[9], Width::Byte, false, 5),
    k("fill", &[8], Width::Byte, false, 7),
    k("popcount", &[0xB5], Width::Byte, false, 5),
    k("mul8", &[7, 6], Width::Byte, false, 42),
    k("fib", &[12], Width::Byte, true, 144),
    k("gcd", &[48, 18], Width::Byte, false, 6),
    k("sum4", &[], Width::Byte, false, 9),
];

fn work(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("bench").join(name);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn tool(
    dir: &Path,
    program: &str,
    args: &[&str],
) -> Result<String, String> {
    let out = Command::new(program).args(args).current_dir(dir).output().map_err(|e| format!("{program}: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(format!("{program}: {}", String::from_utf8_lossy(&out.stderr).lines().next().unwrap_or("")))
    }
}

/// What one build reports.
struct Built {
    /// The linked binary with the function, and with a one-byte stub.
    with: PathBuf,
    stub: PathBuf,
    /// RAM cells the function uses (ours: BSS cells; cc65: none, it uses the C
    /// stack).
    bss: usize,
}

/// Which compiler's convention the driver uses.
#[derive(Clone, Copy, PartialEq)]
enum Abi {
    Cc65,
    Ours,
}

fn driver(
    kernel: &Kernel,
    abi: Abi,
    calls: u32,
    high: bool,
) -> String {
    let symbol = if abi == Abi::Cc65 { format!("_{}", kernel.name) } else { kernel.name.to_owned() };
    let mut s = String::from(".export _main\n.export buf, _buf\n");
    if abi == Abi::Cc65 {
        s.push_str(".import pusha, pushax\n");
    }
    s.push_str(".import ");
    s.push_str(&symbol);
    s.push('\n');
    if abi == Abi::Ours {
        s.push_str(&equates(&Platform::parse(SIM65).unwrap()));
    }
    s.push_str("_main:\n");
    if abi == Abi::Ours {
        s.push_str("  lda #0\n  sta rc0\n  lda #$80\n  sta rc1\n");
    }
    s.push_str(&format!("  lda #{calls}\n  sta cnt\nloop:\n"));
    let last = kernel.args.len().wrapping_sub(1);
    for (i, &value) in kernel.args.iter().enumerate() {
        let (lo, hi) = (value & 0xFF, value >> 8);
        match (abi, kernel.width) {
            (Abi::Cc65, Width::Byte) if i < last => s.push_str(&format!("  lda #{lo}\n  jsr pusha\n")),
            (Abi::Cc65, Width::Byte) => s.push_str(&format!("  lda #{lo}\n")),
            (Abi::Cc65, Width::Word) if i < last => s.push_str(&format!("  lda #{lo}\n  ldx #{hi}\n  jsr pushax\n")),
            (Abi::Cc65, Width::Word) => s.push_str(&format!("  lda #{lo}\n  ldx #{hi}\n")),
            (Abi::Ours, Width::Byte) => s.push_str(&format!("  lda #{lo}\n  sta rc{}\n", 2 + i)),
            (Abi::Ours, Width::Word) => {
                s.push_str(&format!("  lda #{lo}\n  sta rc{}\n  lda #{hi}\n  sta rc{}\n", 2 + 2 * i, 3 + 2 * i));
            }
        }
    }
    s.push_str(&format!("  jsr {symbol}\n  dec cnt\n  bne loop\n"));
    if high {
        s.push_str("  txa\n");
    }
    s.push_str("  rts\n.segment \"DATA\"\nbuf:\n_buf:\n");
    s.push_str(&format!("  .byte {}\n", BUF.map(|b| b.to_string()).join(",")));
    s.push_str(".segment \"BSS\"\ncnt: .res 1\n");
    s
}

/// The runtime routines `text` reaches, as ca65 text. Routines are the
/// top-level labels of `mos_rt.s`; one is reached when another's text names it.
fn reachable_runtime(text: &str) -> String {
    let mut blocks: Vec<(String, String)> = Vec::new();
    let mut bss = String::new();
    let mut in_bss = false;
    let mut pending = String::new();
    for line in RUNTIME.lines() {
        if line.starts_with(".segment \"BSS\"") {
            in_bss = true;
        }
        if in_bss {
            bss.push_str(line);
            bss.push('\n');
            if line.starts_with(".segment \"CODE\"") {
                in_bss = false;
            }
            continue;
        }
        let label = line
            .split(':')
            .next()
            .filter(
                |l| line.contains(':')
                    && l.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
                    && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
            );
        if let Some(label) = label {
            blocks.push((label.to_owned(), std::mem::take(&mut pending)));
        }
        if let Some((_, body)) = blocks.last_mut().filter(|_| !line.starts_with(".export") || label.is_some()) {
            if label.is_some() || !line.starts_with(".export") {
                body.push_str(line);
                body.push('\n');
            }
        }
        if line.starts_with(".export") {
            pending.push_str(line);
            pending.push('\n');
        }
    }
    let by_name: HashMap<&str, usize> = blocks.iter().enumerate().map(|(i, (n, _))| (n.as_str(), i)).collect();
    let mut wanted = vec![false; blocks.len()];
    let mut queue: Vec<usize> = Vec::new();
    let tokens = |s: &str| -> Vec<usize> {
        s.split(|c: char| !(c.is_alphanumeric() || c == '_')).filter_map(|t| by_name.get(t).copied()).collect()
    };
    for i in tokens(text) {
        queue.push(i);
    }
    while let Some(i) = queue.pop() {
        if !std::mem::replace(&mut wanted[i], true) {
            queue.extend(tokens(&blocks[i].1).into_iter().filter(|&j| j != i));
        }
    }
    let mut out = String::new();
    let mut any = false;
    for (i, (_, body)) in blocks.iter().enumerate().filter(|(i, _)| wanted[*i]) {
        let _ = i;
        out.push_str(body);
        any = true;
    }
    if any && out.contains("__sa") {
        out.push_str(&bss);
    }
    out
}

fn link(
    dir: &Path,
    kernel: &Kernel,
    abi: Abi,
    function: &str,
    calls: u32,
    high: bool,
    tag: &str,
) -> Result<PathBuf, String> {
    let drv = format!("d-{tag}.s");
    std::fs::write(dir.join(&drv), driver(kernel, abi, calls, high)).unwrap();
    let bin = format!("p-{tag}.bin");
    let function_file = format!("f-{}.s", tag.split('-').next().unwrap());
    let _ = function;
    tool(dir, "cl65", &["-t", "sim6502", "-o", &bin, &drv, &function_file])?;
    Ok(dir.join(bin))
}

fn run_sim(bin: &Path) -> Result<(i32, i64), String> {
    let out = Command::new("sim65").args(["-c", "-x", "200000000"]).arg(bin).output().map_err(|e| e.to_string())?;
    let cycles = String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|l| l.trim().strip_suffix(" cycles").and_then(|n| n.trim().parse::<i64>().ok()))
        .ok_or("no cycle count")?;
    Ok((out.status.code().unwrap_or(-1), cycles))
}

struct Row {
    bytes: i64,
    cycles: i64,
    bss: usize,
}

/// Builds and measures one kernel with the function text in `f-<tag>.s` (and
/// the stub in `s-<tag>.s`).
fn measure(
    dir: &Path,
    kernel: &Kernel,
    abi: Abi,
    tag: &str,
    bss: usize,
) -> Result<Row, String> {
    // correctness: one call, A (and X) against the hand-derived value
    let low = link(dir, kernel, abi, kernel.name, 1, false, &format!("{tag}-lo"))?;
    let (code, _) = run_sim(&low)?;
    if code != i32::from(kernel.expect & 0xFF) {
        return Err(format!("wrong result: A = {code}, want {}", kernel.expect & 0xFF));
    }
    if kernel.wide_result {
        let high = link(dir, kernel, abi, kernel.name, 1, true, &format!("{tag}-hi"))?;
        let (code, _) = run_sim(&high)?;
        if code != i32::from(kernel.expect >> 8) {
            return Err(format!("wrong result: X = {code}, want {}", kernel.expect >> 8));
        }
    }
    let many = link(dir, kernel, abi, kernel.name, 101, false, &format!("{tag}-n"))?;
    let (_, t1) = run_sim(&low)?;
    let (_, t101) = run_sim(&many)?;
    let cycles = (t101 - t1) / 100 - LOOP_CYCLES;
    // bytes against the stub
    let stub_name = if abi == Abi::Cc65 { format!("_{}", kernel.name) } else { kernel.name.to_owned() };
    std::fs::write(dir.join(format!("f-{tag}stub.s")), format!(".export {stub_name}\n{stub_name}:\n  rts\n")).unwrap();
    let stub = link(dir, kernel, abi, kernel.name, 1, false, &format!("{tag}stub-lo"))?;
    let bytes = std::fs::metadata(&low).unwrap().len() as i64 - std::fs::metadata(&stub).unwrap().len() as i64 + 1;
    Ok(Row { bytes, cycles, bss })
}

fn cc65_row(
    kernel: &Kernel,
    flags: &[&str],
    tag: &str,
) -> Result<Row, String> {
    let dir = work(kernel.name);
    let mut args = vec!["-t", "sim6502"];
    args.extend_from_slice(flags);
    let define = format!("-DK_{}", kernel.name);
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/bench/kernels.c");
    let source = source.to_str().unwrap();
    let output = format!("f-{tag}.s");
    args.extend_from_slice(&[&define, "-o", &output, source]);
    tool(&dir, "cc65", &args)?;
    measure(&dir, kernel, Abi::Cc65, tag, 0)
}

/// clang's IR in MIR's subset: our datalayout, no attributes, linkage or
/// metadata decorations.
fn sanitize(ir: &str) -> String {
    const DROP: [&str; 12] = [
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
    ];
    let mut out = Vec::new();
    for line in ir.lines() {
        if line.starts_with("target triple")
            || line.starts_with("attributes ")
            || line.starts_with('!')
            || line.starts_with("source_filename")
        {
            continue;
        }
        if line.starts_with("target datalayout") {
            out.push(format!("target datalayout = \"{}\"", datalayout()));
            continue;
        }
        let mut line = line.to_owned();
        for word in DROP {
            line = line.replace(word, "");
        }
        // trailing `, !tbaa !2`, `, !llvm.loop !5` and ` #0` on define
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

fn ours_row(
    kernel: &Kernel,
    clang_flag: &str,
    tag: &str,
) -> Result<Row, String> {
    let dir = work(kernel.name);
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/bench/kernels.c");
    let define = format!("-DK_{}", kernel.name);
    // clang is only the C front end (msp430: int and pointers are 16 bits, as
    // for cc65); the optimizer then runs on our datalayout, so only i8 is a
    // legal integer and nothing widens to i16. `opt` stands in for the
    // middle end, with the vectorizers off: there is no vector MIR.
    let raw = format!("raw-{tag}.ll");
    let ll = format!("k-{tag}.ll");
    tool(
        &dir,
        "clang",
        &[
            "--target=msp430",
            "-funsigned-char",
            "-fpack-struct=1",
            "-O2",
            "-Xclang",
            "-disable-llvm-passes",
            "-S",
            "-emit-llvm",
            "-fno-unwind-tables",
            &define,
            "-o",
            &raw,
            source.to_str().unwrap(),
        ],
    )?;
    let retargeted: String = std::fs::read_to_string(dir.join(&raw))
        .unwrap()
        .lines()
        .filter(|l| !l.starts_with("target triple"))
        .map(|l| {
            if l.starts_with("target datalayout") {
                format!("target datalayout = \"{}\"", datalayout())
            } else {
                l.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(dir.join(&raw), retargeted).unwrap();
    tool(
        &dir,
        "/usr/lib/llvm-20/bin/opt",
        &[clang_flag, "-vectorize-loops=false", "-vectorize-slp=false", "-S", &raw, "-o", &ll],
    )?;
    let text = sanitize(&std::fs::read_to_string(dir.join(&ll)).unwrap());
    let module =
        llrm_mir::parse::module(&text).map_err(|e| format!("parse: {e:?}").chars().take(100).collect::<String>())?;
    let name = kernel.name;
    let compiled = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let selection = select_function(&module, name, Cpu::Mos6502)?;
        function_text(&module, name, &selection)
    }));
    let function = match compiled {
        Ok(Ok(text)) => text,
        Ok(Err(why)) => return Err(format!("select: {why}").chars().take(100).collect::<String>()),
        Err(_) => return Err("select: panic".into()),
    };
    let bss = function
        .lines()
        .filter(|l| l.contains(": .res "))
        .map(|l| l.rsplit(' ').next().unwrap().parse::<usize>().unwrap())
        .sum();
    let platform = Platform::parse(SIM65).unwrap();
    let runtime = reachable_runtime(&function);
    let file = format!("{}.import buf\n{function}{runtime}", equates(&platform));
    std::fs::write(dir.join(format!("f-{tag}.s")), file).unwrap();
    measure(&dir, kernel, Abi::Ours, tag, bss)
}

#[test]
#[ignore = "needs cc65, clang and sim65"]
fn the_table() {
    let mut out =
        String::from("| kernel | cc65 | cc65 -O | ours (opt -O2 IR) | ours (opt -Os IR) |\n|---|---|---|---|---|\n");
    let cell = |row: &Result<Row, String>| match row {
        Ok(r) => format!("{} B, {} cy", r.bytes, r.cycles),
        Err(why) => format!("n/a ({why})"),
    };
    for kernel in KERNELS {
        let rows = [
            cc65_row(kernel, &[], "c0"),
            cc65_row(kernel, &["-O"], "c1"),
            ours_row(kernel, "-O2", "o2"),
            ours_row(kernel, "-Os", "os"),
        ];
        out.push_str(&format!("| {} | {} |\n", kernel.name, rows.iter().map(cell).collect::<Vec<_>>().join(" | ")));
    }
    println!("{out}");
    std::fs::write(PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("bench.md"), &out).unwrap();
    let _ = common::build; // keep the shared module linked
}
