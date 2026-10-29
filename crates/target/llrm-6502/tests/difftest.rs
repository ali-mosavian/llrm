//! Differential tests: random small C programs, compiled by cc65 -O and by clang -> opt -> ours, run on
//! sim65; the 8-bit fold of a 16-bit checksum must agree. `DIFF_SEEDS=<first>:<count>` (default 0:40),
//! `DIFF_JOBS` threads (default 4). A mismatch leaves the program in `target/tmp/difftest/<seed>/`.
//!
//! The programs are free of undefined behaviour: unsigned arithmetic only (narrow operands are widened
//! first), signed values come from conversions of unsigned ones, no signed division, shifts are by
//! constants or masked counts, array indexes are masked, loops are bounded, recursion is bounded.
//!
//! Needs cc65, clang (msp430 front end), LLVM 20 `opt`, cl65 and sim65.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use common::pipeline::{datalayout, reachable_runtime, sanitize, tool};
use llrm_6502::alloc::Tuning;
use llrm_6502::dagisel::function::select_function;
use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::emit::equates;
use llrm_6502::platform::{Platform, SIM65};
use llrm_6502::select::function_text;

/// The generator's switches, from `DIFF_OPAQUE`, `DIFF_NOSIGNED` and
/// `DIFF_LIMIT`; cc65's `int` is 16 bits.
fn options() -> llrm_cgen::Options {
    llrm_cgen::Options {
        opaque: std::env::var("DIFF_OPAQUE").is_ok(),
        signed: std::env::var("DIFF_NOSIGNED").is_err(),
        limit: std::env::var("DIFF_LIMIT").ok().and_then(|k| k.parse().ok()),
        ..llrm_cgen::Options::new(llrm_cgen::DataModel::INT16)
    }
}

fn program(seed: u64) -> String {
    llrm_cgen::program(seed, &options())
}

const DATA: &str = ".export buf, _buf, tab, _tab, zero, _zero\n.segment \"RODATA\"\nzero:\n_zero:\n  .byte 0\nbuf:\n_buf:\n  .byte 3,1,4,1,5,9,2,6,5,3,5,8,9,7,9,3\ntab:\n_tab:\n  .byte 2,5,8,11,14,17,20,23\n";

fn work(seed: u64) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("difftest").join(seed.to_string());
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The most cycles sim65 may run (`DIFF_CAP`).
fn cap() -> String {
    std::env::var("DIFF_CAP").unwrap_or_else(|_| "30000000".into())
}

/// sim65 on `bin`: the exit code, or `None` when the cycle cap is hit. sim65 then says "Maximum number of
/// cycles reached." and exits 126; that was matched as "Max number" and so read as the program's own
/// exit code 126 (differential seeds 26 and 51: cc65 -O "agreed" with ours on a hang of both).
fn run(bin: &Path) -> Result<Option<i32>, String> {
    let out = Command::new("sim65").args(["-x", &cap()]).arg(bin).output().map_err(|e| e.to_string())?;
    if String::from_utf8_lossy(&out.stderr).contains("Maximum number of cycles") {
        return Ok(None);
    }
    Ok(out.status.code())
}

fn cc65(dir: &Path, source: &str, optimize: bool) -> Result<Option<i32>, String> {
    std::fs::write(dir.join("c.c"), format!("{source}\nint main(void) {{ unsigned r = test(); return (r ^ (r >> 8)) & 255; }}\n")).unwrap();
    std::fs::write(dir.join("data.s"), DATA).unwrap();
    tool(dir, "cl65", &["-t", "sim6502", if optimize { "-O" } else { "-g0" }, "-o", "c.bin", "c.c", "data.s"])?;
    run(&dir.join("c.bin"))
}

fn ours(dir: &Path, source: &str, seed: u64, optimize: bool) -> Result<Option<i32>, String> {
    std::fs::write(dir.join("o.c"), source).unwrap();
    tool(
        dir,
        "clang",
        &[
            "--target=msp430", "-funsigned-char", "-fpack-struct=1", "-O2", "-Xclang", "-disable-llvm-passes", "-S",
            "-emit-llvm", "-fno-unwind-tables", "-o", "raw.ll", "o.c",
        ],
    )?;
    let raw: String = std::fs::read_to_string(dir.join("raw.ll"))
        .unwrap()
        .lines()
        .filter(|l| !l.starts_with("target triple"))
        .map(|l| if l.starts_with("target datalayout") { format!("target datalayout = \"{}\"", datalayout()) } else { l.to_owned() })
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(dir.join("raw.ll"), raw).unwrap();
    let inline = if seed % 2 == 0 { "-inline-threshold=0" } else { "-inline-threshold=225" };
    if !optimize {
        std::fs::copy(dir.join("raw.ll"), dir.join("k.ll")).unwrap();
    } else {
    tool(
        dir,
        "/usr/lib/llvm-20/bin/opt",
        &["-O2", inline, "-unroll-threshold=0", "-unroll-full-max-count=0", "-vectorize-loops=false", "-vectorize-slp=false", "-S", "raw.ll", "-o", "k.ll"],
    )?;
    }
    let mut text = sanitize(&std::fs::read_to_string(dir.join("k.ll")).unwrap());
    if std::env::var("DIFF_NOLIFETIME").is_ok() {
        text = text.lines().filter(|l| !l.contains("llvm.lifetime")).collect::<Vec<_>>().join("\n");
    }
    let mut module = llrm_mir::parse::module(&text).map_err(|e| format!("parse: {e:?}").chars().take(120).collect::<String>())?;
    llrm_6502::prepare::split_phi_edges(&mut module);
    let platform = Platform::parse(SIM65).unwrap();
    let mut asm = String::new();
    let names: Vec<String> = std::iter::once("test".to_owned()).chain((0..4).map(|n| format!("h{n}"))).chain(["bump".to_owned()]).collect();
    for name in names.iter().filter(|n| module.named(n).is_some_and(|id| module.global(id).function().is_some_and(|f| !f.is_declaration()))) {
        if std::env::var("DIFF_TRACE").is_ok() {
            eprintln!("trace: seed {seed}: compiling {name}");
        }
        let compiled = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut selection = select_function(&module, name, Cpu::Mos6502)?;
            selection.zero_page = platform.zero_page;
            if seed % 3 == 0 {
                selection.tuning = Tuning::SIZE;
            }
            function_text(&module, name, &selection)
        }));
        match compiled {
            Ok(Ok(code)) => asm.push_str(&code),
            Ok(Err(why)) => return Err(format!("ours: {name}: {}", why.chars().take(160).collect::<String>())),
            Err(_) => return Err(format!("ours: {name}: panic")),
        }
    }
    let runtime = reachable_runtime(&asm);
    std::fs::write(dir.join("f.s"), format!("{}.import buf\n.import tab\n.import zero\n{asm}{runtime}", equates(&platform))).unwrap();
    let driver = format!(
        ".export _main\n{}.import test\n_main:\n  lda #0\n  sta rc0\n  lda #$80\n  sta rc1\n  jsr test\n  stx fold\n  eor fold\n  ldx #0\n  rts\n.segment \"BSS\"\nfold: .res 1\n",
        equates(&platform)
    );
    std::fs::write(dir.join("d.s"), driver).unwrap();
    std::fs::write(dir.join("data.s"), DATA).unwrap();
    tool(dir, "cl65", &["-t", "sim6502", "-o", "o.bin", "d.s", "f.s", "data.s"])?;
    run(&dir.join("o.bin"))
}

/// The third opinion: the program compiled by the host gcc with the target's widths (`unsigned` is 16
/// bits, `unsigned long` 32), run, and folded the way `main` folds it. `None` when gcc fails.
fn native(dir: &Path, source: &str) -> Option<i32> {
    let text = source
        .lines()
        .filter(|line| !line.starts_with("extern const"))
        .collect::<Vec<_>>()
        .join("\n")
        .replace("(int)", "(int16_t)")
        .replace("unsigned long", "uint32_t")
        .replace("unsigned char", "uint8_t")
        .replace("signed char", "int8_t")
        .replace("unsigned", "uint16_t");
    let program = format!(
        "#include <stdint.h>\n#include <stdio.h>\nconst uint8_t buf[16]={{3,1,4,1,5,9,2,6,5,3,5,8,9,7,9,3}};\nconst uint8_t tab[8]={{2,5,8,11,14,17,20,23}};\nconst volatile uint8_t zero=0;\n{text}\nint main(void){{ uint16_t r = test(); printf(\"%d\\n\", (r ^ (r >> 8)) & 255); return 0; }}\n"
    );
    std::fs::write(dir.join("n.c"), program).ok()?;
    tool(dir, "gcc", &["-O0", "-fwrapv", "-fpack-struct=1", "-w", "-o", "n.bin", "n.c"]).ok()?;
    let out = Command::new(dir.join("n.bin")).output().ok()?;
    String::from_utf8_lossy(&out.stdout).trim().parse().ok()
}

/// Programs on which cc65 and ours agree and the host gcc does not.
static ORACLE_DIFFERS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// What sim65 counts for `bin` (`-c`), start-up and exit included.
fn cycles(bin: &Path) -> Option<i64> {
    let out = Command::new("sim65").args(["-c", "-x", &cap()]).arg(bin).output().ok()?;
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    text.lines().find_map(|line| line.strip_suffix(" cycles")?.trim().parse().ok())
}

/// The linked size and cycles of the two builds in `dir`: cc65 -O (`c.bin`) and ours (`o.bin`).
fn measure(dir: &Path) -> Option<[i64; 4]> {
    let bytes = |name: &str| std::fs::metadata(dir.join(name)).ok().map(|m| m.len() as i64);
    Some([bytes("c.bin")?, bytes("o.bin")?, cycles(&dir.join("c.bin"))?, cycles(&dir.join("o.bin"))?])
}

/// [`measure`] of a program that does nothing: what both builds cost before the program's own code.
fn baseline() -> [i64; 4] {
    static BASE: std::sync::OnceLock<[i64; 4]> = std::sync::OnceLock::new();
    *BASE.get_or_init(|| {
        let dir = work(u64::MAX - 1);
        let source = "extern const unsigned char buf[16];\nunsigned test(void) { return 0; }\n";
        cc65(&dir, source, true).expect("the empty program builds with cc65");
        ours(&dir, source, 0, true).expect("the empty program builds with ours");
        let base = measure(&dir).expect("the empty program runs");
        let _ = std::fs::remove_dir_all(&dir);
        base
    })
}

enum Outcome {
    /// The programs' [`measure`] when `DIFF_SIZES` is set.
    Agree(Option<[i64; 4]>),
    /// A program the harness could not compare: the reason.
    Skipped(String),
    /// cc65 -O, ours -O2, and the cross-checks: cc65 without -O, ours on the unoptimized IR and the
    /// host gcc.
    Mismatch(i32, i32, Option<i32>, Option<i32>, Option<i32>),
    /// cc65 -O disagrees with ours and the host gcc agrees with ours: a bug in cc65.
    /// cc65 -O, ours and the host gcc, and cc65 without -O
    Cc65(i32, i32, Option<i32>),
}

fn check(seed: u64) -> Outcome {
    let dir = work(seed);
    // `DIFF_FILE=<path>` runs a hand-written program (with a `test`) in place of the generated one
    let source = std::env::var("DIFF_FILE").map(|path| std::fs::read_to_string(path).unwrap()).unwrap_or_else(|_| program(seed));
    std::fs::write(dir.join("p.c"), &source).unwrap();
    // a reduced program must stay defined: no missing return, no read of a local before it is set
    if std::env::var("DIFF_STRICT").is_ok() {
        let strict = tool(
            &dir,
            "clang",
            &["--target=msp430", "-funsigned-char", "-fsyntax-only", "-Werror=return-type", "-Werror=uninitialized", "-Werror=sometimes-uninitialized", "-Werror=conditional-uninitialized", "p.c"],
        );
        if let Err(why) = strict {
            return Outcome::Skipped(format!("not a defined program: {why}"));
        }
    }
    let reference = match cc65(&dir, &source, true) {
        Ok(Some(code)) => code,
        Ok(None) => return Outcome::Skipped("cc65 run hit the cycle cap".into()),
        Err(why) => return Outcome::Skipped(format!("cc65 side: {why}")),
    };
    match ours(&dir, &source, seed, true) {
        Ok(Some(code)) if code == reference => {
            let sizes = std::env::var("DIFF_SIZES").ok().and_then(|_| measure(&dir));
            if std::env::var("DIFF_ORACLE").is_ok() {
                // cc65 and ours agree: the host model disagreeing is a hole in the model
                match native(&dir, &source) {
                    Some(host) if host != code => {
                        ORACLE_DIFFERS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        println!("seed {seed}: cc65 -O and ours {code}, host gcc {host}");
                    }
                    Some(_) => {}
                    None => println!("seed {seed}: host gcc could not build the program"),
                }
            }
            if std::env::var("DIFF_KEEP").is_err() {
                let _ = std::fs::remove_dir_all(&dir);
            }
            Outcome::Agree(sizes)
        }
        Ok(Some(code)) => {
            let plain = cc65(&dir, &source, false).ok().flatten();
            let unoptimized = ours(&dir, &source, seed, false).ok().flatten();
            let third = native(&dir, &source);
            if third == Some(code) {
                Outcome::Cc65(reference, code, plain)
            } else {
                Outcome::Mismatch(reference, code, plain, unoptimized, third)
            }
        }
        Ok(None) => Outcome::Skipped("ours hit the cycle cap".into()),
        Err(why) => Outcome::Skipped(format!("ours: {why}")),
    }
}

#[test]
#[ignore = "needs cc65, clang and LLVM 20; run with --ignored --nocapture"]
fn random_programs_agree_with_cc65() {
    let spec = std::env::var("DIFF_SEEDS").unwrap_or_else(|_| "0:40".into());
    let (first, count) = spec.split_once(':').map(|(a, b)| (a.parse::<u64>().unwrap(), b.parse::<u64>().unwrap())).unwrap();
    let jobs: usize = std::env::var("DIFF_JOBS").ok().and_then(|j| j.parse().ok()).unwrap_or(4);
    let seeds: Vec<u64> = (first..first + count).collect();
    let results = std::sync::Mutex::new(Vec::new());
    let next = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..jobs {
            scope.spawn(|| {
                loop {
                    let at = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    let Some(&seed) = seeds.get(at) else { break };
                    let outcome = check(seed);
                    results.lock().unwrap().push((seed, outcome));
                }
            });
        }
    });
    let mut results = results.into_inner().unwrap();
    results.sort_by_key(|(seed, _)| *seed);
    let (mut agree, mut skipped, mut bad, mut cc65_bugs, mut cc65_o_only) = (0, 0, 0, 0, 0);
    let mut measured: Vec<(u64, [i64; 4])> = Vec::new();
    for (seed, outcome) in &results {
        match outcome {
            Outcome::Agree(sizes) => {
                agree += 1;
                measured.extend(sizes.map(|m| (*seed, m)));
            }
            Outcome::Skipped(why) => {
                skipped += 1;
                println!("seed {seed}: skipped: {why}");
            }
            Outcome::Cc65(want, got, plain) => {
                cc65_bugs += 1;
                if *plain == Some(*got) {
                    cc65_o_only += 1;
                }
                println!("seed {seed}: cc65 bug: cc65 -O {want}, ours and the host gcc {got}; cc65 without -O {plain:?}");
            }
            Outcome::Mismatch(want, got, plain, unoptimized, third) => {
                bad += 1;
                println!("seed {seed}: MISMATCH cc65 -O {want}, ours -O2 {got}; cc65 plain {plain:?}, ours unoptimized {unoptimized:?}, host gcc {third:?}");
            }
        }
    }
    if !measured.is_empty() {
        // net of the empty program's start-up, ours against cc65 -O
        let base = baseline();
        let (mut smaller, mut same, mut larger, mut faster, mut slower, mut equal) = (0, 0, 0, 0, 0, 0);
        let mut totals = [0i64; 4];
        for (seed, m) in &measured {
            let net = [m[0] - base[0], m[1] - base[1], m[2] - base[2], m[3] - base[3]];
            totals.iter_mut().zip(net).for_each(|(t, n)| *t += n);
            match net[1].cmp(&net[0]) {
                std::cmp::Ordering::Less => smaller += 1,
                std::cmp::Ordering::Equal => same += 1,
                std::cmp::Ordering::Greater => larger += 1,
            }
            match net[3].cmp(&net[2]) {
                std::cmp::Ordering::Less => faster += 1,
                std::cmp::Ordering::Equal => equal += 1,
                std::cmp::Ordering::Greater => slower += 1,
            }
            println!("seed {seed}: net bytes cc65 -O {} ours {}; net cycles cc65 -O {} ours {}", net[0], net[1], net[2], net[3]);
        }
        println!(
            "{} agreeing programs, net of an empty one: bytes ours smaller {smaller} / equal {same} / larger {larger} (totals cc65 -O {}, ours {}); cycles ours fewer {faster} / equal {equal} / more {slower} (totals cc65 -O {}, ours {})",
            measured.len(), totals[0], totals[1], totals[2], totals[3]
        );
    }
    if std::env::var("DIFF_ORACLE").is_ok() {
        println!("host gcc differs from cc65 -O and ours on {} of {agree} agreeing programs", ORACLE_DIFFERS.load(std::sync::atomic::Ordering::Relaxed));
    }
    println!("{count} programs from seed {first}: {agree} agree, {bad} mismatch, {cc65_bugs} cc65 bugs ({cc65_o_only} of them only with -O), {skipped} skipped");
    assert_eq!(bad, 0, "miscompiles; programs kept in target/tmp/difftest/<seed>");
}

/// A macro the runtime defines after one routine and another uses was left out when only the other
/// was reached: `__umodsi3` reached `cmp_sub_32` and ca65 stopped at `':' expected` (differential
/// seeds 14, 24, 33 were rejected for it).
#[test]
fn a_reachable_routine_gets_the_macros_it_uses() {
    let runtime = reachable_runtime("  jsr __umodsi3\n");
    assert!(runtime.contains(".macro cmp_sub_32"), "{runtime}");
    assert!(runtime.contains("__umodsi3:"));
}

/// A program that runs into the cycle cap is `None`, not an exit code of 126.
#[test]
#[ignore = "needs cc65 and sim65"]
fn a_program_that_hits_the_cycle_cap_has_no_exit_code() {
    let dir = work(u64::MAX - 2);
    std::fs::write(dir.join("spin.c"), "int main(void) { for (;;) ; }\n").unwrap();
    tool(&dir, "cl65", &["-t", "sim6502", "-o", "spin.bin", "spin.c"]).expect("builds");
    // SAFETY: tests of this file read DIFF_CAP only here
    unsafe { std::env::set_var("DIFF_CAP", "100000") };
    assert_eq!(run(&dir.join("spin.bin")), Ok(None));
    let _ = std::fs::remove_dir_all(&dir);
}
