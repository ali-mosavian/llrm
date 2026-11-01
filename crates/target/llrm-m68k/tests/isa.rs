//! The opcode table against its two outside readers: binutils' disassembler
//! and the Motorola reference manual's printed bit patterns.

use std::collections::BTreeSet;
use std::process::Command;

use llrm_m68k::isa::{Cpu, decode, forms};

fn mnemonics(
    word: u16,
    cpu: Cpu,
) -> BTreeSet<&'static str> {
    decode(word, cpu).map(|form| form.mnemonic).collect()
}

/// `movec` is the 68010's, `rtd` too; `callm` and `rtm` are the 68020's only.
#[test]
fn cpus_differ_where_the_family_grew() {
    assert!(mnemonics(0x4e7a, Cpu::M68000).is_empty(), "movec on a 68000");
    assert!(mnemonics(0x4e7a, Cpu::M68010).contains("movec"));
    assert!(mnemonics(0x4e74, Cpu::M68010).contains("rtd"));
    assert!(mnemonics(0x06d0, Cpu::M68020).contains("callm"));
    assert!(mnemonics(0x06d0, Cpu::M68030).is_empty(), "callm outlived the 68020");
    assert!(mnemonics(0x06c8, Cpu::M68060).contains("rtm") || mnemonics(0x06c8, Cpu::M68020).contains("rtm"));
}

/// The manual prints BFFFO with BFEXTU's first word (page 4-43, e9c0); the
/// disassembler decodes it at edc0. A table copied from the page would put
/// `bfffo` on top of `bfextu`.
#[test]
fn bfffo_is_edc0_not_bfextus_word() {
    assert_eq!(mnemonics(0xe9c0, Cpu::M68020), BTreeSet::from(["bfextu"]));
    assert_eq!(mnemonics(0xedc0, Cpu::M68020), BTreeSet::from(["bfffo"]));
}

/// binutils' first mnemonic for each `(first word, extension word)` pair on
/// `cpu`, `None` where it decodes none.
fn oracle(
    cpu: Cpu,
    pairs: &[(u16, u16)],
) -> Option<Vec<Option<String>>> {
    let root = std::env::var("GCC_M68K").ok().filter(|root| !root.is_empty());
    let Some(root) = root else {
        let skip = std::env::var("M68K_ISA_SKIP_ORACLE").is_ok_and(|flag| flag == "1");
        assert!(skip, "GCC_M68K is not set: the binutils oracle cannot run (M68K_ISA_SKIP_ORACLE=1 skips it)");
        return None;
    };
    const RECORD: usize = 16;
    let mut image = Vec::with_capacity(pairs.len() * RECORD);
    for &(first, ext) in pairs {
        let mut record = [0x4e, 0x71].repeat(RECORD / 2);
        record[..2].copy_from_slice(&first.to_be_bytes());
        record[2..4].copy_from_slice(&ext.to_be_bytes());
        image.extend(record);
    }
    let path = std::env::temp_dir().join(format!("isa-oracle-{}-{}.bin", cpu.name(), std::process::id()));
    std::fs::write(&path, &image).unwrap();
    let out = Command::new(format!("{root}/usr/bin/m68k-linux-gnu-objdump"))
        .env("LD_LIBRARY_PATH", format!("{root}/usr/lib/x86_64-linux-gnu"))
        .args(["-D", "-b", "binary", "-EB", "--no-show-raw-insn", "-m"])
        .arg(format!("m68k:{}", cpu.name()))
        .arg(&path)
        .output()
        .expect("objdump runs");
    std::fs::remove_file(&path).ok();
    let mut by_record = vec![None; pairs.len()];
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let Some((address, text)) = line.trim_start().split_once(":\t") else { continue };
        let Ok(address) = usize::from_str_radix(address, 16) else { continue };
        let text = text.trim();
        let junk = text.starts_with('.') || text.contains("<unknown>");
        if address % RECORD == 0 && !junk {
            by_record[address / RECORD] = text.split_whitespace().next().map(str::to_owned);
        }
    }
    Some(by_record)
}

/// The extension words each first word of lines 0-E is tried with: the
/// operand forms that depend on one (`chk2` and `muls.l` on bit 11, 64-bit
/// `mul`/`div` on bit 10, `movec` on bit 15, full-format indexes on bit 8)
/// need one that sets it.
const EXTENSION_WORDS: [u16; 6] = [0x0000, 0x0100, 0x0400, 0x0800, 0x0c00, 0x8000];

/// A table that disagrees with the disassembler on which words are
/// instructions is wrong about the CPU, whatever else it gets right.
#[test]
fn table_decodes_what_binutils_decodes() {
    let words: Vec<u16> = (0..=0xffffu16).filter(|word| word >> 12 != 0xf).collect();
    for cpu in Cpu::ALL {
        let pairs = |ext: u16| words.iter().map(|&word| (word, ext)).collect::<Vec<_>>();
        let mut runs = Vec::new();
        for ext in EXTENSION_WORDS {
            let Some(run) = oracle(cpu, &pairs(ext)) else { return };
            runs.push(run);
        }
        let mut wrong = Vec::new();
        for (index, &word) in words.iter().enumerate() {
            let seen: BTreeSet<String> = runs.iter().filter_map(|run| run[index].clone()).collect();
            let ours: BTreeSet<String> = mnemonics(word, cpu).iter().map(|name| name.to_string()).collect();
            if seen != ours {
                wrong.push(format!("{word:04x}: binutils {seen:?}, table {ours:?}"));
            }
        }
        assert!(
            wrong.is_empty(),
            "{} words differ on the {}: {:#?}",
            wrong.len(),
            cpu.name(),
            &wrong[..wrong.len().min(10)]
        );
    }
}

/// Whether the table has a form of `cpu` for `(first, ext)` whose name
/// binutils' mnemonic begins with (binutils appends the FPU format letter).
fn table_has(
    cpu: Cpu,
    first: u16,
    ext: u16,
    seen: &str,
) -> bool {
    decode(first, cpu).any(|form| form.matches_ext(ext) && seen.starts_with(form.mnemonic))
}

/// The FPU and the MMU under cpid 0 and 1: every command word, each CPU. The
/// 68000 and 68010 are left out: binutils decodes these there (limit 1).
#[test]
fn table_decodes_the_coprocessor_commands_binutils_does() {
    const MODES: [u16; 12] = [0o00, 0o10, 0o20, 0o30, 0o40, 0o50, 0o60, 0o70, 0o71, 0o72, 0o73, 0o74];
    let mut pairs = Vec::new();
    for cpid in 0..2u16 {
        for kind in 0..6u16 {
            for mode in MODES {
                let first = 0xf000 | cpid << 9 | kind << 6 | mode;
                let exts: Vec<u16> = match (cpid, kind) {
                    (1, 0) => (0..8u16)
                        .flat_map(|class| {
                            (0..8u16).flat_map(move |src| {
                                (0..128u16).flat_map(move |op| {
                                    [0u16, 5].map(move |dst| class << 13 | src << 10 | dst << 7 | op)
                                })
                            })
                        })
                        .collect(),
                    (0, 0) => (0..=0xffff).collect(),
                    (_, 1) => (0..64).collect(),
                    _ => vec![0],
                };
                pairs.extend(exts.into_iter().map(|ext| (first, ext)));
            }
        }
    }
    for cpu in [Cpu::M68020, Cpu::M68030, Cpu::M68040, Cpu::M68060] {
        let Some(seen) = oracle(cpu, &pairs) else { return };
        let wrong: Vec<String> = pairs
            .iter()
            .zip(&seen)
            .filter(|((first, ext), seen)| match seen {
                Some(name) => !table_has(cpu, *first, *ext, name),
                None => decode(*first, cpu).any(|form| !form.ext.is_empty() && form.matches_ext(*ext)),
            })
            .map(|((first, ext), seen)| format!("{first:04x} {ext:04x}: binutils {seen:?}"))
            .collect();
        assert!(
            wrong.is_empty(),
            "{} commands differ on the {}: {:#?}",
            wrong.len(),
            cpu.name(),
            &wrong[..wrong.len().min(10)]
        );
    }
}

/// `(mask, value)` of a printed pattern: `0`/`1` fixed, anything else free.
fn pattern(text: &str) -> (u16, u16) {
    text.chars().fold((0, 0), |(mask, value), bit| {
        (mask << 1 | u16::from(bit == '0' || bit == '1'), value << 1 | u16::from(bit == '1'))
    })
}

fn manual_rows() -> Vec<(String, (u16, u16))> {
    [
        include_str!("../src/isa/prm/int_a.tsv"),
        include_str!("../src/isa/prm/int_b.tsv"),
        include_str!("../src/isa/prm/sup.tsv"),
    ]
    .iter()
    .flat_map(|file| file.lines())
    .filter_map(|line| {
        let column: Vec<_> = line.split('\t').collect();
        (column.len() > 4 && column[4].len() == 16).then(|| (column[0].to_owned(), pattern(column[4])))
    })
    .collect()
}

/// Every word the table decodes is inside some pattern the manual prints.
#[test]
fn every_decoded_word_is_in_a_pattern_of_the_manual() {
    let rows = manual_rows();
    let lost: Vec<_> = (0..=0xffffu16)
        .filter(|word| word >> 12 != 0xf)
        .filter(|&word| Cpu::ALL.iter().any(|&cpu| decode(word, cpu).next().is_some()))
        .filter(|&word| !rows.iter().any(|(_, (mask, value))| word & mask == *value))
        .map(|word| format!("{word:04x} {:?}", mnemonics(word, Cpu::M68060)))
        .collect();
    assert!(lost.is_empty(), "{} words outside the manual: {lost:?}", lost.len());
}

/// Every line 0-E pattern of the manual has a word the table decodes (a
/// transcription that reads no word is a page error or a form missing from the
/// table). Line F is outside the table (limit 1 in `isa.rs`).
#[test]
fn every_manual_pattern_reaches_a_decoded_word() {
    let dead: Vec<_> = manual_rows()
        .into_iter()
        .filter(|(_, (mask, value))| mask >> 12 == 0xf && !value_is_fpu_or_mmu(*value))
        .filter(|(_, (mask, value))| {
            !(0..=0xffffu16)
                .any(|word| word & mask == *value && Cpu::ALL.iter().any(|&cpu| decode(word, cpu).next().is_some()))
        })
        .map(|(name, _)| name)
        .collect();
    assert!(dead.is_empty(), "manual patterns with no decoded word: {dead:?}");
}

#[test]
fn table_has_rows_for_every_cpu() {
    for cpu in Cpu::ALL {
        assert!(forms().iter().filter(|form| form.on(cpu)).count() > 1000, "{}", cpu.name());
    }
}

/// A first word of coprocessor 0 or 1 (the MMU and the FPU) with its type
/// bits: the table has those for one cpid only, so a manual pattern that
/// leaves the cpid free may match none of them.
fn value_is_fpu_or_mmu(value: u16) -> bool {
    value >> 12 == 0xf && value >> 9 & 7 <= 1
}

/// The FPU and MMU exist from the 68020 (coprocessor interface) on.
#[test]
fn no_coprocessor_before_the_68020() {
    for cpu in [Cpu::M68000, Cpu::M68010] {
        assert!(decode(0xf200, cpu).next().is_none(), "an FPU form on the {}", cpu.name());
        assert!(decode(0xf000, cpu).next().is_none(), "an MMU form on the {}", cpu.name());
    }
    assert!(decode(0xf200, Cpu::M68020).next().is_some());
}

#[test]
fn rounded_fpu_forms_start_on_the_68040() {
    let on = |cpu| forms().iter().any(|form| form.mnemonic == "fdadd" && form.on(cpu));
    assert!(!on(Cpu::M68030) && on(Cpu::M68040) && on(Cpu::M68060));
}

/// Table 9-10 of the 68040 manual: FSIN decodes and traps; FADD runs.
#[test]
fn the_68040_traps_the_transcendentals() {
    let form = |name: &str| forms().iter().find(|form| form.mnemonic == name && form.on(Cpu::M68040)).unwrap();
    assert!(form("fsin").emulated_on(Cpu::M68040));
    assert!(!form("fadd").emulated_on(Cpu::M68040));
    assert!(!form("fsin").emulated_on(Cpu::M68030));
}

#[test]
fn cache_move16_and_lpstop_belong_to_the_late_cpus() {
    let only = |word: u16, name: &str| {
        Cpu::ALL
            .into_iter()
            .filter(|&cpu| decode(word, cpu).any(|form| form.mnemonic.starts_with(name)))
            .collect::<Vec<_>>()
    };
    assert_eq!(only(0xf600, "move16"), [Cpu::M68040, Cpu::M68060], "move16");
    assert_eq!(only(0xf418, "cinv"), [Cpu::M68040, Cpu::M68060], "cinva");
    assert_eq!(only(0xf800, "lpstop"), [Cpu::M68060], "lpstop");
}

/// Tables 6-11, C-3 and D-1 of the 68060 manual (`isa/prm/m68060_support.tsv`):
/// the 68060 runs `fadd` and `fdbcc` differently from the 68040, and never
/// runs `movep`.
#[test]
fn the_68060_traps_more_than_the_68040() {
    let form = |name: &str, cpu: Cpu| forms().iter().find(|form| form.mnemonic == name && form.on(cpu)).unwrap();
    assert!(
        !form("fdbeq", Cpu::M68040).emulated_on(Cpu::M68040) && form("fdbeq", Cpu::M68060).emulated_on(Cpu::M68060)
    );
    assert!(
        form("movepw", Cpu::M68060).emulated_on(Cpu::M68060) && !form("movepw", Cpu::M68030).emulated_on(Cpu::M68030)
    );
    assert!(!form("fadd", Cpu::M68060).emulated_on(Cpu::M68060));
}
