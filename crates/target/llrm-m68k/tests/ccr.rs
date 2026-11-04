//! `isa/ccr.tsv` against the machine: each row's claims about the codes, run
//! under `qemu-m68k -cpu m68000` over operand values at the edges, from two
//! starting codes (all clear, all set). The table is read from the manual; this
//! is the second reader.

use std::path::PathBuf;
use std::process::Command;

mod support;

const VALUES: [u32; 13] =
    [0, 1, 2, 0x7f, 0x80, 0xff, 0x7fff, 0x8000, 0xffff, 0x7fff_ffff, 0x8000_0000, 0xffff_ffff, 0x1234_5678];
const COUNTS: [u32; 10] = [0, 1, 3, 7, 8, 31, 32, 33, 63, 64];

fn tool(name: &str) -> Option<Command> {
    let root = std::env::var("GCC_M68K").ok().filter(|root| !root.is_empty());
    let Some(root) = root else {
        assert!(std::env::var("M68K_ISA_SKIP_ORACLE").is_ok_and(|flag| flag == "1"), "GCC_M68K is not set");
        return None;
    };
    let mut command = Command::new(format!("{root}/usr/bin/m68k-linux-gnu-{name}"));
    command.env("LD_LIBRARY_PATH", format!("{root}/usr/lib/x86_64-linux-gnu"));
    Some(command)
}

/// One case's two codes bytes, after the instruction and after the instruction
/// and a `tst` of its result.
struct Case {
    start: u8,
    after: u8,
    tested: u8,
}

/// The instruction `mnemonic` at `size`, over `%d4` (and `%d5` where it has a
/// source), as an assembler line; `None` for a size it has not.
fn line(
    mnemonic: &str,
    size: char,
) -> Option<String> {
    Some(match mnemonic {
        "swap" if size == 'l' => "swap %d4".to_owned(),
        "swap" => return None,
        "ext" if size == 'b' => return None,
        "ext" => format!("ext.{size} %d4"),
        "neg" | "negx" | "not" => format!("{mnemonic}.{size} %d4"),
        "muls" | "mulu" if size == 'w' => format!("{mnemonic}.w %d5,%d4"),
        "muls" | "mulu" => return None,
        // the pin's `XOR*di`: the source is an immediate
        "eori" => format!("eori.{size} #IMM,%d4"),
        _ => format!("{mnemonic}.{size} %d5,%d4"),
    })
}

fn run(
    mnemonic: &str,
    size: char,
    instruction: &str,
) -> Option<Vec<(u32, u32, Case)>> {
    let shift = matches!(mnemonic, "asr" | "lsl" | "lsr" | "rol" | "ror");
    let mask = match size {
        'b' => 0xff,
        'w' => 0xffff,
        _ => u32::MAX,
    };
    let sources: &[u32] = if shift { &COUNTS } else { &VALUES };
    let mut cases = Vec::new();
    let mut text = String::from(".globl _start\n_start:\n\tlea buf,%a3\n\tmove.l %a3,%a2\n");
    for &a in &VALUES {
        for &b in sources {
            for start in [0u8, 0x1f] {
                cases.push((a, b, start));
                for with_test in [false, true] {
                    text += &format!(
                        "\tmove.l #{a},%d4\n\tmove.l #{b},%d5\n\tmove.w #{start},%ccr\n\t{}\n",
                        instruction.replace("IMM", &(b & mask).to_string())
                    );
                    if with_test {
                        // a word multiply makes a long
                        let result = if mnemonic.starts_with("mul") { 'l' } else { size };
                        text += &format!("\ttst.{result} %d4\n");
                    }
                    text += "\tmove.w %sr,%d6\n\tmove.b %d6,(%a2)+\n";
                }
            }
        }
    }
    text +=
        &format!("\tmove.l #4,%d0\n\tmove.l #1,%d1\n\tmove.l %a3,%d2\n\tmove.l #{},%d3\n\ttrap #0\n", cases.len() * 2);
    text += "\tmove.l #1,%d0\n\tmove.l #0,%d1\n\ttrap #0\n.bss\nbuf: .space 8192\n";
    let source = support::unique_path("ccr-s");
    let object = support::unique_path("ccr-o");
    let exe = support::unique_path("ccr-x");
    std::fs::write(&source, text).unwrap();
    assert!(tool("as")?.args(["-m68000", "-o"]).arg(&object).arg(&source).status().unwrap().success(), "{instruction}");
    assert!(tool("ld")?.arg("-o").arg(&exe).arg(&object).status().unwrap().success());
    let qemu: PathBuf = support::qemu()?;
    let out = Command::new(&qemu).args(["-cpu", "m68000"]).arg(&exe).output().unwrap();
    for path in [source, object, exe] {
        std::fs::remove_file(path).ok();
    }
    assert_eq!(out.stdout.len(), cases.len() * 2, "{instruction}: {:?}", String::from_utf8_lossy(&out.stderr));
    Some(
        cases
            .iter()
            .enumerate()
            .map(|(at, &(a, b, start))| {
                (a, b, Case { start, after: out.stdout[2 * at] & 0x1f, tested: out.stdout[2 * at + 1] & 0x1f })
            })
            .collect(),
    )
}

/// What the table says each flag does: `*` according to the result, `-`
/// unaffected, `0` cleared, `~` cleared unless the result is zero.
#[test]
fn every_row_of_the_table_holds_on_the_machine() {
    let mut wrong = Vec::new();
    let mut runs = 0;
    for row in include_str!("../src/isa/ccr.tsv").lines().filter(|line| !line.starts_with('#')) {
        let cells: Vec<&str> = row.split('\t').collect();
        let symbols: Vec<char> = cells[1..6].iter().map(|cell| cell.chars().next().unwrap()).collect();
        for size in ['b', 'w', 'l'] {
            let Some(instruction) = line(cells[0], size) else { continue };
            let Some(cases) = run(cells[0], size, &instruction) else { return };
            runs += 1;
            for (a, b, case) in cases {
                // bit 4 is X, then N, Z, V, C
                let bit = |codes: u8, flag: usize| (codes >> (4 - flag)) & 1;
                for (flag, symbol) in symbols.iter().enumerate() {
                    let (got, start, tst) = (bit(case.after, flag), bit(case.start, flag), bit(case.tested, flag));
                    let nonzero = bit(case.tested, 2) == 0;
                    let holds = match symbol {
                        '*' if flag == 1 || flag == 2 => got == tst,
                        '-' => got == start,
                        '0' => got == 0,
                        '~' => got == if nonzero { 0 } else { start },
                        _ => true,
                    };
                    if !holds {
                        wrong.push(format!(
                            "{} {instruction}: flag {} {symbol} at {a:#x},{b:#x} from {:#x}",
                            row.split('\t').next().unwrap(),
                            "XNZVC".chars().nth(flag).unwrap(),
                            case.start
                        ));
                    }
                }
                // what the compare elimination relies on
                let same = llrm_m68k::ccr::like_test(cells[0]).unwrap();
                for (flag, name) in [(1, "N"), (2, "Z"), (3, "V"), (4, "C")] {
                    if same & llrm_m68k::ccr::flag_bit(name) != 0 && bit(case.after, flag) != bit(case.tested, flag) {
                        wrong.push(format!(
                            "{instruction}: {name} differs from tst at {a:#x},{b:#x} from {:#x}",
                            case.start
                        ));
                    }
                }
            }
        }
    }
    wrong.truncate(20);
    assert!(wrong.is_empty(), "{wrong:#?}");
    assert!(runs >= 40, "only {runs} programs ran");
}

/// `conditions` sat under `[flags]` in `dialect.toml`, so the dialect did not
/// parse ("flag `conditions` is no bit") and the first use of it panicked, in
/// every compile once the compare elimination read it.
#[test]
fn the_dialect_names_the_sixteen_conditions_and_the_flags_each_tests() {
    assert_eq!(llrm_m68k::ccr::tested_by(6), llrm_m68k::ccr::tested_by_name("ne"));
    assert_eq!(llrm_m68k::ccr::tested_by(6), Some(llrm_m68k::ccr::flag_bit("Z")));
    assert_eq!(llrm_m68k::ccr::tested_by(15), Some(["N", "V", "Z"].iter().map(|f| llrm_m68k::ccr::flag_bit(f)).sum()));
    assert_eq!(llrm_m68k::ccr::tested_by(16), None);
}
