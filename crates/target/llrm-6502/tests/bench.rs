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
use llrm_6502::select::{function_items, function_text};

/// The 6502 description's datalayout: n8 and 16-bit pointers.
fn datalayout() -> String {
    llrm_6502::LAYOUT
        .lines()
        .find_map(|l| l.strip_prefix("datalayout = "))
        .expect("a datalayout")
        .trim_matches('"')
        .to_owned()
}
const LZ: &str = "48,97,98,99,3,0,17,100,2,0,255,5,65,66,67,68,69,70,71,72,73,74,75,76,77,78,79,80,81,82,83,84,7,0,255,26,63,120,121,122,40,0,0,242,255,0,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,1,0,48,101,110,100";
const BUF: [u8; 16] = [3, 1, 4, 1, 5, 9, 2, 6, 5, 3, 5, 8, 9, 7, 9, 3];
/// The loop's `dec cnt` (6) and taken `bne` (3).
const LOOP_CYCLES: i64 = 9;

/// An argument: a byte, a word, or the address of a driver symbol plus an offset.
#[derive(Clone, Copy)]
enum Arg {
    B(u16),
    W(u16),
    /// `buf`, `str` or `tab` and the offset into it.
    P(&'static str, u16),
}

use Arg::{B, P, W};

struct Kernel {
    name: &'static str,
    args: &'static [Arg],
    /// The result is 16 bits: A is the low byte, X the high.
    wide_result: bool,
    expect: u16,
}

const fn k(
    name: &'static str,
    args: &'static [Arg],
    wide_result: bool,
    expect: u16,
) -> Kernel {
    Kernel { name, args, wide_result, expect }
}

/// Every expected value is worked out by hand from the arguments and `BUF`.
const KERNELS: &[Kernel] = &[
    k("zero", &[], false, 0),
    k("add8", &[B(200), B(100)], false, 44),
    k("add16", &[W(300), W(5000)], true, 5300),
    k("max8", &[B(3), B(9)], false, 9),
    k("max16", &[W(1000), W(300)], true, 1000),
    k("sum_arr", &[B(10)], false, 39),
    k("find", &[B(9)], false, 5),
    k("fill", &[B(8)], false, 7),
    k("popcount", &[B(0xB5)], false, 5),
    k("mul8", &[B(7), B(6)], false, 42),
    k("fib", &[B(12)], true, 144),
    k("gcd", &[B(48), B(18)], false, 6),
    k("sum4", &[], false, 9),
];

/// A second set, written after the first was tuned against: library-style
/// routines on pointers, strings and words, mixed argument widths. `str` is
/// "hello world", `tab` is 2 5 8 11 14 17 20 23; the expected values come from
/// a Python model of each routine.
const KERNELS2: &[Kernel] = &[
    k("slen", &[P("str", 0)], true, 11),
    k("scmp", &[P("str", 0), P("str", 6)], true, 0xFFF1),
    k("mcopy", &[P("buf", 8), P("buf", 0), B(8)], false, 6),
    k("crc16", &[P("buf", 0), B(12)], true, 0xA166),
    k("mul16", &[W(1234), W(56)], true, 3568),
    k("digits", &[W(40000)], false, 5),
    k("reverse", &[P("buf", 0), B(8)], false, 6),
    k("bsearch8", &[P("tab", 0), B(8), B(17)], false, 5),
    k("sum16", &[P("buf", 0), B(16)], true, 80),
    k("schr", &[P("str", 0), B(111)], false, 4),
];

/// The C of `lz4.s` on `lz`, decompressing to `lzout` (the answer is the length written; `lz4c` also
/// sums the bytes written, so a wrong byte shows), and loops that keep several variables live in
/// registers across the iteration; expected values from a Python model.
const KERNELS3: &[Kernel] = &[
    k("lz4", &[P("lz", 0), P("lzout", 0), W(634)], true, 634),
    k("lz4c", &[P("lz", 0), P("lzout", 0), W(634)], true, 52785),
    k("xs8", &[B(0xA5), B(5)], false, 107),
    k("minmax", &[P("buf", 0), B(16)], false, 8),
    k("fletcher", &[P("buf", 0), B(16)], true, 15952),
    k("hash16", &[P("str", 0)], true, 51393),
    k("dotx", &[P("buf", 0), P("tab", 0), B(8)], false, 101),
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
    // ours: bytes take the first free `rc` from `rc2`, words and pointers the
    // first free pair (`backfill`)
    let mut used = [false; 6];
    for (i, &arg) in kernel.args.iter().enumerate() {
        let wide = !matches!(arg, B(_));
        let (lo, hi) = match arg {
            B(v) | W(v) => (format!("#{}", v & 0xFF), format!("#{}", v >> 8)),
            P(symbol, off) => (format!("#<({symbol}+{off})"), format!("#>({symbol}+{off})")),
        };
        match abi {
            Abi::Cc65 => {
                let load = if wide { format!("  lda {lo}\n  ldx {hi}\n") } else { format!("  lda {lo}\n") };
                s.push_str(&load);
                if i < last {
                    s.push_str(if wide { "  jsr pushax\n" } else { "  jsr pusha\n" });
                }
            }
            Abi::Ours => {
                let at = if wide {
                    (0..3).map(|p| 2 * p).find(|&p| !used[p] && !used[p + 1]).expect("a free pair")
                } else {
                    (0..6).find(|&p| !used[p]).expect("a free byte")
                };
                used[at] = true;
                if wide {
                    used[at + 1] = true;
                    s.push_str(&format!("  lda {lo}\n  sta rc{}\n  lda {hi}\n  sta rc{}\n", 2 + at, 3 + at));
                } else {
                    s.push_str(&format!("  lda {lo}\n  sta rc{}\n", 2 + at));
                }
            }
        }
    }
    s.push_str(&format!("  jsr {symbol}\n  dec cnt\n  bne loop\n"));
    if high {
        s.push_str("  txa\n");
    }
    s.push_str("  rts\n.segment \"DATA\"\nbuf:\n_buf:\n");
    s.push_str(&format!("  .byte {}\n", BUF.map(|b| b.to_string()).join(",")));
    s.push_str("str:\n_str:\n  .byte \"hello world\",0\ntab:\n_tab:\n  .byte 2,5,8,11,14,17,20,23\n");
    // An LZ4 block built from plaintext rules (not by any decoder): "abc" + a 4-byte match at distance
    // 3, "d" + 5 at distance 2, 20 literals (length extension) + a 300-byte match at distance 7 (extension
    // 255 and more, overlapping), "xyz" + a match of exactly 19 (extension 0), 270 literals + 6 at
    // distance 1, and the last literals "end": 634 bytes whose Fletcher sum is 52785.
    s.push_str(&format!("lz:\n_lz:\n  .byte {LZ}\n.export lz, _lz\n"));
    s.push_str(".export str, _str, tab, _tab, lzout, _lzout\n");
    s.push_str(".segment \"BSS\"\ncnt: .res 1\nlzout:\n_lzout: .res 640\n");
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

/// A tag ending in `m` builds for the 65C02 (`sim65c02`), any other for the NMOS 6502.
fn cmos(tag: &str) -> bool {
    tag.split('-').next().is_some_and(|t| t.ends_with('m'))
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
    let target = if cmos(tag) { "sim65c02" } else { "sim6502" };
    tool(dir, "cl65", &["-t", target, "-o", &bin, &drv, &function_file])?;
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
    /// The linked program's bytes, library routines the code calls included.
    bytes: i64,
    /// The function's own code: the object's CODE segment, no library. Equal to `bytes` when it calls none.
    body: i64,
    cycles: i64,
    bss: usize,
}

/// The CODE segment of ca65 text `file` in `dir`, assembled alone (`od65 --dump-segsize`).
fn code_bytes(
    dir: &Path,
    file: &str,
) -> Result<i64, String> {
    let object = dir.join(format!("{file}.body.o"));
    let built = Command::new("ca65")
        .args(["-t", "sim65c02", "-o"])
        .arg(&object)
        .arg(dir.join(file))
        .output()
        .map_err(|e| e.to_string())?;
    if !built.status.success() {
        return Err(format!("ca65 {file}: {}", String::from_utf8_lossy(&built.stderr)));
    }
    let dump = Command::new("od65").arg("--dump-segsize").arg(&object).output().map_err(|e| e.to_string())?;
    String::from_utf8_lossy(&dump.stdout)
        .lines()
        .find_map(|l| l.trim().strip_prefix("CODE:").and_then(|n| n.trim().parse::<i64>().ok()))
        .ok_or_else(|| format!("no CODE size for {file}"))
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
    println!("check {tag:<9} {:<9} A = {code:<3} want {:<3} pass", kernel.name, kernel.expect & 0xFF);
    if kernel.wide_result {
        let high = link(dir, kernel, abi, kernel.name, 1, true, &format!("{tag}-hi"))?;
        let (code, _) = run_sim(&high)?;
        if code != i32::from(kernel.expect >> 8) {
            return Err(format!("wrong result: X = {code}, want {}", kernel.expect >> 8));
        }
        println!("check {tag:<9} {:<9} X = {code:<3} want {:<3} pass", kernel.name, kernel.expect >> 8);
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
    Ok(Row { bytes, body: bytes, cycles, bss })
}

fn cc65_row(
    kernel: &Kernel,
    flags: &[&str],
    tag: &str,
) -> Result<Row, String> {
    let dir = work(kernel.name);
    let mut args = vec!["-t", if cmos(tag) { "sim65c02" } else { "sim6502" }];
    args.extend_from_slice(flags);
    let define = format!("-DK_{}", kernel.name);
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/bench/kernels.c");
    let source = source.to_str().unwrap();
    let output = format!("f-{tag}.s");
    args.extend_from_slice(&[&define, "-o", &output, source]);
    tool(&dir, "cc65", &args)?;
    let mut row = measure(&dir, kernel, Abi::Cc65, tag, 0)?;
    row.body = code_bytes(&dir, &output)?;
    Ok(row)
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

/// Stands in for the 6502 override-table entry for loop unrolling: bytes over speed on an 8-bit
/// target, so no full or partial unroll (the table moves these to `opt`'s pipeline entry unchanged).
const UNROLL: [&str; 2] = ["-unroll-threshold=0", "-unroll-full-max-count=0"];

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
        &[clang_flag, UNROLL[0], UNROLL[1], "-vectorize-loops=false", "-vectorize-slp=false", "-S", &raw, "-o", &ll],
    )?;
    let text = sanitize(&std::fs::read_to_string(dir.join(&ll)).unwrap());
    let mut module =
        llrm_mir::parse::module(&text).map_err(|e| format!("parse: {e:?}").chars().take(100).collect::<String>())?;
    llrm_6502::prepare::split_phi_edges(&mut module);
    let name = kernel.name;
    let cpu = if cmos(tag) { Cpu::W65c02 } else { Cpu::Mos6502 };
    let compiled = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let selection = select_function(&module, name, cpu)?;
        { let mut selection = selection; selection.zero_page = Platform::parse(SIM65).unwrap().zero_page; function_text(&module, name, &selection) }
    }));
    let path = {
        let selection = select_function(&module, name, cpu).map_err(|e| format!("{e:?}"))?;
        let (items, _) = function_items(&module, name, &selection)?;
        if llrm_6502::alloc::build(name, &items).is_ok() { "allocator" } else { "cell" }
    };
    println!("path  {tag:<9} {name:<9} {path}");
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
    let row = measure(&dir, kernel, Abi::Ours, tag, bss)?;
    // the table's bytes are the linked binary's, to the byte
    let priced = llrm_6502::timing::estimate(cpu, &format!("{function}{runtime}"))?;
    assert_eq!(i64::from(priced.bytes), row.bytes, "{name} {tag}: timing.rs against the linked size");
    let alone = llrm_6502::timing::estimate(cpu, &function)?;
    Ok(Row { body: i64::from(alone.bytes), ..row })
}

/// cc65's hand-written `libsrc/common/lz4.s` (a162565e) as the best case for the same C: wrapped to
/// return the length written, and linked with the cc65 library routines it calls (counted).
fn hand_row(kernel: &Kernel) -> Result<Row, String> {
    let dir = work(kernel.name);
    let hand = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/bench/lz4.s")).unwrap();
    let wrapper = ".export _lz4\n_lz4:\n  jsr _decompress_lz4\n  lda regsave\n  sec\n  sbc #<lzout\n  tay\n  lda regsave+1\n  sbc #>lzout\n  tax\n  tya\n  rts\n.import lzout\n";
    std::fs::write(dir.join("f-h.s"), format!("{wrapper}{hand}")).unwrap();
    let mut row = measure(&dir, kernel, Abi::Cc65, "h", 0)?;
    row.body = code_bytes(&dir, "f-h.s")?;
    Ok(row)
}

fn table3() {
    let mut out = String::from("| kernel | cc65 -O | cc65 lz4.s (hand) | ours | ours 65C02 |\n|---|---|---|---|---|\n");
    let cell = |row: &Result<Row, String>| match row {
        Ok(r) if r.body != r.bytes => format!("{} B linked, {} B body, {} cy", r.bytes, r.body, r.cycles),
        Ok(r) => format!("{} B, {} cy", r.bytes, r.cycles),
        Err(why) if why.starts_with("no hand") => "-".to_owned(),
        Err(why) => format!("n/a ({why})"),
    };
    for kernel in KERNELS3 {
        let hand = if kernel.name == "lz4" { hand_row(kernel) } else { Err("no hand-written version".into()) };
        let rows = [cc65_row(kernel, &["-O"], "c1"), hand, ours_row(kernel, "-O2", "o2"), ours_row(kernel, "-O2", "o2m")];
        for row in rows.iter().filter(|row| !matches!(row, Err(why) if why.starts_with("no hand"))) {
            if let Err(why) = row {
                eprintln!("{}: {why}", kernel.name);
            }
        }
        out.push_str(&format!("| {} | {} |\n", kernel.name, rows.iter().map(cell).collect::<Vec<_>>().join(" | ")));
    }
    println!("{out}");
    std::fs::write(PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("bench3.md"), &out).unwrap();
}

fn table(
    kernels: &[Kernel],
    file: &str,
) {
    let mut out =
        String::from(
        "| kernel | cc65 | cc65 -O | ours (opt -O2 IR) | ours (opt -Os IR) | cc65 -O 65C02 | ours 65C02 |\n|---|---|---|---|---|---|---|\n",
    );
    let cell = |row: &Result<Row, String>| match row {
        Ok(r) if r.body != r.bytes => format!("{} B linked, {} B body, {} cy", r.bytes, r.body, r.cycles),
        Ok(r) => format!("{} B, {} cy", r.bytes, r.cycles),
        Err(why) => format!("n/a ({why})"),
    };
    let only = std::env::var("BENCH_ONLY").ok();
    for kernel in kernels.iter().filter(|k| only.as_deref().is_none_or(|o| o == k.name)) {
        let rows = [
            cc65_row(kernel, &[], "c0"),
            cc65_row(kernel, &["-O"], "c1"),
            ours_row(kernel, "-O2", "o2"),
            ours_row(kernel, "-Os", "os"),
            cc65_row(kernel, &["-O"], "c1m"),
            ours_row(kernel, "-O2", "o2m"),
        ];
        for (tag, row) in ["c0", "c1", "o2", "os", "c1m", "o2m"].iter().zip(&rows) {
            if let Err(why) = row {
                eprintln!("{} {tag}: {why}", kernel.name);
            }
        }
        out.push_str(&format!("| {} | {} |\n", kernel.name, rows.iter().map(cell).collect::<Vec<_>>().join(" | ")));
    }
    println!("{out}");
    std::fs::write(PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(file), &out).unwrap();
    let _ = common::build; // keep the shared module linked
}

#[test]
#[ignore = "needs cc65, clang and sim65"]
fn the_table() {
    table(KERNELS, "bench.md");
}

#[test]
#[ignore = "needs cc65, clang and sim65"]
fn the_second_table() {
    table(KERNELS2, "bench2.md");
}

#[test]
#[ignore = "needs cc65, clang and sim65"]
fn the_third_table() {
    table3();
}
