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

/// The first words binutils decodes for `cpu`, by mnemonic, with the
/// extension word `ext` after each (so a form that needs one decodes).
fn oracle(
    cpu: Cpu,
    ext: u16,
) -> Vec<Option<String>> {
    let root = std::env::var("GCC_M68K").ok().filter(|root| !root.is_empty()).or_else(|| {
        let skip = std::env::var("M68K_ISA_SKIP_ORACLE").is_ok_and(|flag| flag == "1");
        assert!(skip, "GCC_M68K is not set: the binutils oracle cannot run (M68K_ISA_SKIP_ORACLE=1 skips it)");
        None
    }).unwrap_or_default();
    if root.is_empty() {
        return Vec::new();
    }
    const RECORD: usize = 16;
    let mut image = Vec::with_capacity(0x10000 * RECORD);
    for word in 0..=0xffffu16 {
        let mut record = [0x4e, 0x71].repeat(RECORD / 2);
        record[..2].copy_from_slice(&word.to_be_bytes());
        record[2..4].copy_from_slice(&ext.to_be_bytes());
        image.extend(record);
    }
    let path = std::env::temp_dir().join(format!("isa-oracle-{}-{ext:04x}-{}.bin", cpu.name(), std::process::id()));
    std::fs::write(&path, &image).unwrap();
    let out = Command::new(format!("{root}/usr/bin/m68k-linux-gnu-objdump"))
        .env("LD_LIBRARY_PATH", format!("{root}/usr/lib/x86_64-linux-gnu"))
        .args(["-D", "-b", "binary", "-EB", "--no-show-raw-insn", "-m"])
        .arg(format!("m68k:{}", cpu.name()))
        .arg(&path)
        .output()
        .expect("objdump runs");
    std::fs::remove_file(&path).ok();
    let mut by_record = vec![None; 0x10000];
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let Some((address, text)) = line.trim_start().split_once(":\t") else { continue };
        let Ok(address) = usize::from_str_radix(address, 16) else { continue };
        let text = text.trim();
        let junk = text.starts_with(".") || text.contains("<unknown>");
        if address % RECORD == 0 && !junk {
            by_record[address / RECORD] = text.split_whitespace().next().map(str::to_owned);
        }
    }
    by_record
}

/// A table that disagrees with the disassembler on which words are
/// instructions is wrong about the CPU, whatever else it gets right.
#[test]
fn table_decodes_what_binutils_decodes() {
    for cpu in Cpu::ALL {
        let (plain, full) = (oracle(cpu, 0x0000), oracle(cpu, 0x0100));
        if plain.is_empty() {
            return;
        }
        let mut wrong = Vec::new();
        for word in (0..=0xffffu16).filter(|word| word >> 12 != 0xf) {
            let seen: BTreeSet<_> = [&plain, &full].iter().filter_map(|run| run[word as usize].as_deref()).collect();
            let ours = mnemonics(word, cpu);
            if seen.iter().map(|name| name.to_string()).collect::<BTreeSet<_>>() != ours.iter().map(|name| name.to_string()).collect() {
                wrong.push(format!("{word:04x}: binutils {seen:?}, table {ours:?}"));
            }
        }
        assert!(wrong.is_empty(), "{} words differ on the {}: {:#?}", wrong.len(), cpu.name(), &wrong[..wrong.len().min(10)]);
    }
}

/// `(mask, value)` of a printed pattern: `0`/`1` fixed, anything else free.
fn pattern(text: &str) -> (u16, u16) {
    text.chars().fold((0, 0), |(mask, value), bit| {
        (mask << 1 | u16::from(bit == '0' || bit == '1'), value << 1 | u16::from(bit == '1'))
    })
}

fn manual_rows() -> Vec<(String, (u16, u16))> {
    [include_str!("../src/isa/prm/int_a.tsv"), include_str!("../src/isa/prm/int_b.tsv"), include_str!("../src/isa/prm/sup.tsv")]
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
        .filter(|(_, (mask, value))| value >> 12 != 0xf && mask >> 12 == 0xf)
        .filter(|(_, (mask, value))| {
            !(0..=0xffffu16).any(|word| {
                word & mask == *value && Cpu::ALL.iter().any(|&cpu| decode(word, cpu).next().is_some())
            })
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
