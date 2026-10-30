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

use std::path::{Path, PathBuf};
use std::process::Command;

use common::pipeline::{llrm_cc, reachable_runtime, tool};
use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::emit::equates;
use llrm_6502::platform::{Platform, SIM65};

const LZ: &str = "48,97,98,99,3,0,17,100,2,0,255,5,65,66,67,68,69,70,71,72,73,74,75,76,77,78,79,80,81,82,83,84,7,0,255,26,63,120,121,122,40,0,0,242,255,0,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,1,0,48,101,110,100";
const BUF: [u8; 16] = [3, 1, 4, 1, 5, 9, 2, 6, 5, 3, 5, 8, 9, 7, 9, 3];
/// The loop's `dec cnt` (6) and taken `bne` (3).
const LOOP_CYCLES: i64 = 9;

/// An argument: a byte, a word, or the address of a driver symbol plus an
/// offset.
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
    /// A routine of the hand-written corpus (`tests/bench/hand`), if it is one.
    hand: Option<Hand>,
}

/// What a `tests/bench/hand/<file>.spec` says about the hand routine against its C kernel `name`.
struct Hand {
    file: &'static str,
    /// The symbol of the hand routine as linked; `-` when the bench cannot drive it.
    symbol: &'static str,
    /// A pointer result is measured from this driver symbol.
    rel: Option<&'static str>,
    /// What the hand routine answers when it differs from the kernel's `expect` (a truth value).
    expect: Option<u16>,
    /// Other hand-corpus routines linked with it (the clone's, not the installed library's).
    deps: &'static [&'static str],
}

const fn k(
    name: &'static str,
    args: &'static [Arg],
    wide_result: bool,
    expect: u16,
) -> Kernel {
    Kernel { name, args, wide_result, expect, hand: None }
}

/// The symbol the driver calls for the row `tag`: the hand routine's own for the hand row of a
/// hand-corpus kernel, else the kernel (cc65 prefixes an underscore).
fn call_symbol(
    kernel: &Kernel,
    abi: Abi,
    tag: &str,
) -> String {
    match &kernel.hand {
        Some(hand) if tag.starts_with('h') => hand.symbol.to_owned(),
        _ if abi == Abi::Cc65 => format!("_{}", kernel.name),
        _ => kernel.name.to_owned(),
    }
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

/// The C of `lz4.s` on `lz`, decompressing to `lzout` (the answer is the length
/// written; `lz4c` also sums the bytes written, so a wrong byte shows), and
/// loops that keep several variables live in registers across the iteration;
/// expected values from a Python model.
const KERNELS3: &[Kernel] = &[
    k("lz4", &[P("lz", 0), P("lzout", 0), W(634)], true, 634),
    k("lz4c", &[P("lz", 0), P("lzout", 0), W(634)], true, 52785),
    k("xs8", &[B(0xA5), B(5)], false, 107),
    k("minmax", &[P("buf", 0), B(16)], false, 8),
    k("fletcher", &[P("buf", 0), B(16)], true, 15952),
    k("hash16", &[P("str", 0)], true, 51393),
    k("dotx", &[P("buf", 0), P("tab", 0), B(8)], false, 101),
];

/// cc65's optimizer flag for the "cc65 -O" rows: `-O`, or what `BENCH_CC65` names (`-Oirs`).
fn cc65_optimize() -> &'static str {
    Box::leak(std::env::var("BENCH_CC65").unwrap_or_else(|_| "-O".to_owned()).into_boxed_str())
}

/// The level of the "ours" -O2 rows, or `BENCH_OURS` (`-O0`: the control that the table can show cc65 winning).
fn ours_level(default: &'static str) -> &'static str {
    std::env::var("BENCH_OURS").map_or(default, |level| &*Box::leak(level.into_boxed_str()))
}

fn work(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("bench").join(name);
    std::fs::create_dir_all(&dir).unwrap();
    dir
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
    tag: &str,
) -> String {
    let symbol = call_symbol(kernel, abi, tag);
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
    if let Some(from) = kernel.hand.as_ref().and_then(|hand| hand.rel) {
        s.push_str(&format!("  sec\n  sbc #<{from}\n  pha\n  txa\n  sbc #>{from}\n  tax\n  pla\n"));
    }
    if high {
        s.push_str("  txa\n");
    }
    s.push_str("  rts\n.segment \"DATA\"\nbuf:\n_buf:\n");
    s.push_str(&format!("  .byte {}\n", BUF.map(|b| b.to_string()).join(",")));
    s.push_str("str:\n_str:\n  .byte \"hello world\",0\ntab:\n_tab:\n  .byte 2,5,8,11,14,17,20,23\n");
    // An LZ4 block built from plaintext rules (not by any decoder): "abc" + a
    // 4-byte match at distance 3, "d" + 5 at distance 2, 20 literals
    // (length extension) + a 300-byte match at distance 7 (extension
    // 255 and more, overlapping), "xyz" + a match of exactly 19 (extension 0),
    // 270 literals + 6 at distance 1, and the last literals "end": 634
    // bytes whose Fletcher sum is 52785.
    s.push_str(&format!("lz:\n_lz:\n  .byte {LZ}\n.export lz, _lz\n"));
    s.push_str(".export str, _str, tab, _tab, lzout, _lzout\n");
    // the hand corpus' strings and the output area (the same as `tests/bench/hand/data.c`)
    for (name, text) in [
        ("mix", "Hello, World 42!"),
        ("mix2", "hELLO, wORLD 42?"),
        ("num", "  -4829xyz"),
        ("pat", "o w"),
        ("set", "lo"),
    ] {
        s.push_str(&format!("{name}:\n_{name}:\n  .byte \"{text}\",0\n.export {name}, _{name}\n"));
    }
    s.push_str(".segment \"BSS\"\ncnt: .res 1\nlzout:\n_lzout: .res 640\ndst:\n_dst: .res 64\n.export dst, _dst\n");
    s
}

/// A tag ending in `m` builds for the 65C02 (`sim65c02`), any other for the
/// NMOS 6502.
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
    std::fs::write(dir.join(&drv), driver(kernel, abi, calls, high, tag)).unwrap();
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

#[derive(Debug)]
struct Row {
    /// The linked program's bytes, library routines the code calls included.
    bytes: i64,
    /// The function's own code: the object's CODE segment, no library. Equal to
    /// `bytes` when it calls none.
    body: i64,
    cycles: i64,
    bss: usize,
}

/// The CODE segment of ca65 text `file` in `dir`, assembled alone (`od65
/// --dump-segsize`).
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
    let expect = match &kernel.hand {
        Some(Hand { expect: Some(own), .. }) if tag.starts_with('h') => *own,
        _ => kernel.expect,
    };
    let low = link(dir, kernel, abi, kernel.name, 1, false, &format!("{tag}-lo"))?;
    let (code, _) = run_sim(&low)?;
    if code != i32::from(expect & 0xFF) {
        return Err(format!("wrong result: A = {code}, want {}", expect & 0xFF));
    }
    println!("check {tag:<9} {:<9} A = {code:<3} want {:<3} pass", kernel.name, expect & 0xFF);
    if kernel.wide_result {
        let high = link(dir, kernel, abi, kernel.name, 1, true, &format!("{tag}-hi"))?;
        let (code, _) = run_sim(&high)?;
        if code != i32::from(expect >> 8) {
            return Err(format!("wrong result: X = {code}, want {}", expect >> 8));
        }
        println!("check {tag:<9} {:<9} X = {code:<3} want {:<3} pass", kernel.name, expect >> 8);
    }
    let many = link(dir, kernel, abi, kernel.name, 101, false, &format!("{tag}-n"))?;
    let (_, t1) = run_sim(&low)?;
    let (_, t101) = run_sim(&many)?;
    let cycles = (t101 - t1) / 100 - LOOP_CYCLES;
    // bytes against the stub
    let stub_name = call_symbol(kernel, abi, tag);
    std::fs::write(dir.join(format!("f-{tag}stub.s")), format!(".export {stub_name}\n{stub_name}:\n  rts\n")).unwrap();
    let stub = link(dir, kernel, abi, kernel.name, 1, false, &format!("{tag}stub-lo"))?;
    let bytes = std::fs::metadata(&low).unwrap().len() as i64 - std::fs::metadata(&stub).unwrap().len() as i64 + 1;
    Ok(Row { bytes, body: bytes, cycles, bss })
}

/// The `-D` flag and the C file of a kernel: its `#ifdef` in `kernels.c`, or its own file of the hand corpus.
fn kernel_source(kernel: &Kernel) -> (String, PathBuf) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/bench");
    match &kernel.hand {
        Some(hand) => ("-DHAND".to_owned(), dir.join("hand").join(format!("{}.c", hand.file))),
        None => (format!("-DK_{}", kernel.name), dir.join("kernels.c")),
    }
}

fn cc65_row(
    kernel: &Kernel,
    flags: &[&str],
    tag: &str,
) -> Result<Row, String> {
    let dir = work(kernel.name);
    let mut args = vec!["-t", if cmos(tag) { "sim65c02" } else { "sim6502" }];
    args.extend_from_slice(flags);
    let (define, source) = kernel_source(kernel);
    let source = source.to_str().unwrap();
    let output = format!("f-{tag}.s");
    args.extend_from_slice(&[&define, "-o", &output, source]);
    tool(&dir, "cc65", &args)?;
    let mut row = measure(&dir, kernel, Abi::Cc65, tag, 0)?;
    row.body = code_bytes(&dir, &output)?;
    Ok(row)
}

fn ours_row(
    kernel: &Kernel,
    level: &str,
    tag: &str,
) -> Result<Row, String> {
    let dir = work(kernel.name);
    let (define, source) = kernel_source(kernel);
    // llrm-cc: clang's msp430 front end (int and pointers are 16 bits, as for cc65), llrm's mid-end with no
    // vectorizer (there is no vector MIR), and our backend, to ca65 text.
    let name = kernel.name;
    let asm = format!("k-{tag}.s");
    let target = if cmos(tag) { "65c02-sim65" } else { "6502-sim65" };
    tool(
        &dir,
        llrm_cc().to_str().unwrap(),
        &[&format!("--target={target}"), level, "-S", &define, "-o", &asm, source.to_str().unwrap()],
    )?;
    let cpu = if cmos(tag) { Cpu::W65c02 } else { Cpu::Mos6502 };
    let text = std::fs::read_to_string(dir.join(&asm)).unwrap();
    // the equates are the platform's, not the function's
    let function: String = text.lines().filter(|l| !l.starts_with("rc") || !l.contains(" = $")).map(|l| format!("{l}\n")).collect();
    let bss = function
        .lines()
        .filter(|l| l.contains(": .res "))
        .map(|l| l.rsplit(' ').next().unwrap().parse::<usize>().unwrap())
        .sum();
    let runtime = reachable_runtime(&function);
    let file = format!(".import buf\n{text}{runtime}");
    std::fs::write(dir.join(format!("f-{tag}.s")), file).unwrap();
    let row = measure(&dir, kernel, Abi::Ours, tag, bss)?;
    // the table's bytes are the linked binary's, to the byte
    let priced = llrm_6502::timing::estimate(cpu, &format!("{function}{runtime}"))?;
    assert_eq!(i64::from(priced.bytes), row.bytes, "{name} {tag}: timing.rs against the linked size");
    let alone = llrm_6502::timing::estimate(cpu, &function)?;
    Ok(Row { body: i64::from(alone.bytes), ..row })
}

/// cc65's hand-written `libsrc/common/lz4.s` (a162565e) as the best case for
/// the same C: wrapped to return the length written, and linked with the cc65
/// library routines it calls (counted).
fn hand_row(kernel: &Kernel) -> Result<Row, String> {
    let dir = work(kernel.name);
    let bench = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/bench");
    let (wrapper, hand) = match &kernel.hand {
        Some(hand) if hand.symbol == "-" => return Err("no hand: the bench cannot drive it".into()),
        // the cc65 routine, with the glue that turns a runtime convention into a call, if it has one
        Some(hand) => {
            let read = |name: String| std::fs::read_to_string(bench.join("hand").join(name));
            let deps: String = hand.deps.iter().map(|dep| read(format!("{dep}.s")).unwrap()).collect();
            (read(format!("{}.glue.s", hand.file)).unwrap_or_default(), read(format!("{}.s", hand.file)).unwrap_or_default() + &deps)
        }
        None => (
            ".export _lz4\n_lz4:\n  jsr _decompress_lz4\n  lda regsave\n  sec\n  sbc #<lzout\n  tay\n  lda regsave+1\n  sbc #>lzout\n  tax\n  tya\n  rts\n.import lzout\n".to_owned(),
            std::fs::read_to_string(bench.join("lz4.s")).unwrap(),
        ),
    };
    std::fs::write(dir.join("f-h.s"), without_own_imports(&format!("{wrapper}{hand}"))).unwrap();
    let mut row = measure(&dir, kernel, Abi::Cc65, "h", 0)?;
    row.body = code_bytes(&dir, "f-h.s")?;
    Ok(row)
}

/// The source with no `.import` of a symbol it exports: a glue file and the routine it calls, or a routine and the
/// routine it depends on, are joined into one unit, where ca65 refuses an import of what the unit defines.
fn without_own_imports(source: &str) -> String {
    let names = |line: &str, directive: &str| -> Option<Vec<String>> {
        let rest = line.trim().strip_prefix(directive).filter(|rest| rest.starts_with(char::is_whitespace))?;
        Some(rest.split(';').next()?.split(',').map(|name| name.trim().to_owned()).filter(|name| !name.is_empty()).collect())
    };
    let exported: Vec<String> =
        source.lines().flat_map(|line| names(line, ".export").or_else(|| names(line, ".exportzp")).unwrap_or_default()).collect();
    source
        .lines()
        .filter_map(|line| {
            let (directive, imported) = match (names(line, ".importzp"), names(line, ".import")) {
                (Some(imported), _) => (".importzp", imported),
                (None, Some(imported)) => (".import", imported),
                _ => return Some(line.to_owned()),
            };
            let kept: Vec<String> = imported.into_iter().filter(|name| !exported.contains(name)).collect();
            (!kept.is_empty()).then(|| format!("        {directive} {}", kept.join(", ")))
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

/// The hand-written corpus: every `tests/bench/hand/*.spec` (see that directory's README) as a kernel.
fn hand_kernels() -> Vec<Kernel> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/bench/hand");
    let mut specs: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "spec"))
        .collect();
    specs.sort();
    let leak = |text: &str| -> &'static str { Box::leak(text.trim().to_owned().into_boxed_str()) };
    let number = |text: &str| -> u16 {
        let text = text.trim();
        text.strip_prefix("0x").map_or_else(|| text.parse(), |hex| u16::from_str_radix(hex, 16)).unwrap()
    };
    specs
        .iter()
        .map(|path| {
            let text = std::fs::read_to_string(path).unwrap();
            let field = |key: &str| {
                text.lines().find_map(|line| line.strip_prefix(key)?.strip_prefix(':')).map(|v| v.split('#').next().unwrap().trim().to_owned())
            };
            let args: Vec<Arg> = field("args")
                .unwrap_or_default()
                .replace("),", ") ")
                .split_whitespace()
                .map(|arg| {
                    let (kind, rest) = arg.split_once('(').unwrap();
                    let rest = rest.trim_end_matches(')');
                    match kind {
                        "B" => B(number(rest)),
                        "W" => W(number(rest)),
                        "P" => {
                            let (symbol, offset) = rest.split_once(',').unwrap();
                            P(leak(symbol), number(offset))
                        }
                        other => panic!("{}: argument {other}", path.display()),
                    }
                })
                .collect();
            let rel = field("rel").filter(|r| r != "-");
            Kernel {
                name: leak(&field("kernel").unwrap()),
                args: Box::leak(args.into_boxed_slice()),
                wide_result: field("wide").is_some_and(|w| w == "1"),
                expect: number(&field("expect").unwrap()),
                hand: Some(Hand {
                    file: leak(path.file_stem().unwrap().to_str().unwrap()),
                    symbol: leak(&field("hand").unwrap()),
                    rel: rel.as_deref().map(leak),
                    expect: field("hand_expect").map(|v| number(&v)),
                    deps: Box::leak(
                        field("deps")
                            .unwrap_or_default()
                            .split_whitespace()
                            .map(leak)
                            .collect::<Vec<_>>()
                            .into_boxed_slice(),
                    ),
                }),
            }
        })
        .collect()
}

/// The hand corpus against cc65 -O, the hand code and ours, with the totals that say who wins.
fn table4() {
    let mut out = String::from("| kernel | cc65 -O | cc65 hand | ours | ours 65C02 |\n|---|---|---|---|---|\n");
    let cell = |row: &Result<Row, String>| match row {
        Ok(r) => format!("{} B, {} cy", r.bytes, r.cycles),
        Err(why) if why.starts_with("no hand") => "-".to_owned(),
        Err(why) => format!("n/a ({why})"),
    };
    let only = std::env::var("BENCH_ONLY").ok();
    // (kernel, [bytes, cycles] of cc65, hand, ours)
    let mut measured: Vec<(&str, [Option<(i64, i64)>; 3])> = Vec::new();
    let kernels = hand_kernels();
    for kernel in kernels.iter().filter(|k| only.as_deref().is_none_or(|o| o == k.name)) {
        let rows =
            [cc65_row(kernel, &[cc65_optimize()], "c1"), hand_row(kernel), ours_row(kernel, ours_level("-O2"), "o2"), ours_row(kernel, ours_level("-O2"), "o2m")];
        for row in rows.iter().filter(|row| matches!(row, Err(why) if !why.starts_with("no hand"))) {
            eprintln!("{}: {}", kernel.name, row.as_ref().unwrap_err());
        }
        let pair = |row: &Result<Row, String>| row.as_ref().ok().map(|r| (r.bytes, r.cycles));
        measured.push((kernel.name, [pair(&rows[0]), pair(&rows[1]), pair(&rows[2])]));
        out.push_str(&format!("| {} | {} |\n", kernel.name, rows.iter().map(cell).collect::<Vec<_>>().join(" | ")));
    }
    // ours against cc65 -O and against the hand code: smaller/equal/larger and cycles, geometric means
    for (label, other) in [("cc65 -O", 0usize), ("hand", 1)] {
        let pairs: Vec<((i64, i64), (i64, i64))> = measured
            .iter()
            .filter_map(|(_, row)| Some((row[other]?, row[2]?)))
            .collect();
        let count = |f: &dyn Fn(&((i64, i64), (i64, i64))) -> std::cmp::Ordering, which: std::cmp::Ordering| {
            pairs.iter().filter(|pair| f(pair) == which).count()
        };
        let bytes = |pair: &((i64, i64), (i64, i64))| pair.1.0.cmp(&pair.0.0);
        let cycles = |pair: &((i64, i64), (i64, i64))| pair.1.1.cmp(&pair.0.1);
        let geomean = |f: &dyn Fn(&((i64, i64), (i64, i64))) -> f64| {
            (pairs.iter().map(|pair| f(pair).ln()).sum::<f64>() / pairs.len().max(1) as f64).exp()
        };
        let line = format!(
            "ours against {label}, {} kernels: bytes smaller {} / equal {} / larger {} (geometric mean {:.3}x); cycles fewer {} / equal {} / more {} (geometric mean {:.3}x)",
            pairs.len(),
            count(&bytes, std::cmp::Ordering::Less),
            count(&bytes, std::cmp::Ordering::Equal),
            count(&bytes, std::cmp::Ordering::Greater),
            geomean(&|pair| pair.1.0 as f64 / pair.0.0 as f64),
            count(&cycles, std::cmp::Ordering::Less),
            count(&cycles, std::cmp::Ordering::Equal),
            count(&cycles, std::cmp::Ordering::Greater),
            geomean(&|pair| pair.1.1.max(1) as f64 / pair.0.1.max(1) as f64),
        );
        out.push_str(&format!("\n{line}\n"));
    }
    println!("{out}");
    std::fs::write(PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("bench4.md"), &out).unwrap();
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
        let rows =
            [cc65_row(kernel, &[cc65_optimize()], "c1"), hand, ours_row(kernel, ours_level("-O2"), "o2"), ours_row(kernel, ours_level("-O2"), "o2m")];
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
    let mut out = String::from(
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
            cc65_row(kernel, &[cc65_optimize()], "c1"),
            ours_row(kernel, ours_level("-O2"), "o2"),
            ours_row(kernel, "-Os", "os"),
            cc65_row(kernel, &[cc65_optimize()], "c1m"),
            ours_row(kernel, ours_level("-O2"), "o2m"),
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

/// The cc65 library's hand-written routines as C (`tests/bench/hand`).
#[test]
#[ignore = "needs cc65, clang and sim65"]
fn the_hand_corpus() {
    table4();
}
