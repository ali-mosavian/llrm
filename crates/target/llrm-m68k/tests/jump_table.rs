//! A dense switch lowered to a jump table kept its table `jmp` and every case
//! block: branch folding took the table jump for a plain jump to its first
//! block and dropped the dispatch and the other nine cases.

use llrm_m68k::compile::{Item, Options, items_with, text_with};
use llrm_m68k::dagisel::hooks::Cpu;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

fn switch() -> String {
    let cases: String = (0..10).map(|n| format!("    i32 {n}, label %c{n}\n")).collect();
    let blocks: String = (0..10).map(|n| format!("c{n}:\n  ret i32 {}\n", n * 7 + 3)).collect();
    format!(
        "target datalayout = \"{DATALAYOUT}\"
define i32 @f(i32 %x) {{
entry:
  switch i32 %x, label %d [
{cases}  ]
{blocks}d:
  ret i32 -1
}}"
    )
}

#[test]
fn a_dense_switch_keeps_its_table_jump_and_every_case() {
    let module = llrm_mir::parse::module(&switch()).expect("parses");
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let out = text_with(&module, "f", cpu, Options::default()).expect("compiles");
        assert!(out.contains("jmp\t(.LJTI"), "{cpu:?}: no table jump:\n{out}");
        for n in 0..10 {
            let want = format!("#{}", n * 7 + 3);
            assert!(out.contains(&want), "{cpu:?}: case {n} ({want}) dropped:\n{out}");
        }
    }
}

/// The dispatch jumped through a table that was never emitted: the text had a
/// `lea` of an undefined label and no entries.
#[test]
fn the_table_has_one_address_per_case_in_text_and_in_the_object() {
    let module = llrm_mir::parse::module(&switch()).expect("parses");
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let text = text_with(&module, "f", cpu, Options::default()).expect("compiles");
        assert_eq!(text.matches("\t.word\t.LBB").count(), 10, "{cpu:?}:\n{text}");
        let items = items_with(&module, "f", cpu, Options::default()).expect("compiles");
        let at = items.iter().position(|item| matches!(item, Item::WordTable(_))).expect("a table");
        let Item::Label(table) = &items[at - 1] else { panic!("the table has no label") };
        assert!(text.contains(&format!("(%pc,%d0), %d0")) || text.contains(table), "{cpu:?}: names {table}:\n{text}");
        let Item::WordTable(cases) = &items[at] else { unreachable!() };
        let placed = llrm_m68k::object::assemble(&items, 0x1000).expect("assembles");
        let address = |name: &str| placed.labels.iter().find(|(label, _)| label == name).expect("a label").1;
        let start = (address(table) - 0x1000) as usize;
        for (n, case) in cases.iter().enumerate() {
            let word = &placed.code[start + 2 * n..start + 2 * n + 2];
            let offset = i32::from(i16::from_be_bytes(word.try_into().unwrap()));
            assert_eq!(address(table) as i32 + offset, address(case) as i32, "{cpu:?} entry {n}");
        }
    }
}

/// The dispatch (the doubled index, the PC-indexed `move.w`, the PC-indexed
/// `jmp`) must be in the timing tables, else every switch cell reads
/// UNMEASURED.
#[test]
fn every_instruction_of_the_dispatch_is_priced() {
    let module = llrm_mir::parse::module(&switch()).expect("parses");
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let text = text_with(&module, "f", cpu, Options::default()).expect("compiles");
        let dispatch: Vec<&str> = text
            .lines()
            .map(str::trim)
            .filter(|line| line.starts_with("move.w\t(.LJTI") || line.starts_with("jmp\t(.LJTI"))
            .collect();
        assert_eq!(dispatch.len(), 2, "{cpu:?}:\n{text}");
        for line in dispatch {
            let line = line.replace('\t', " ");
            llrm_m68k::timing::cost(cpu, &line, 2, false).unwrap_or_else(|why| panic!("{cpu:?} {line}: {why}"));
        }
    }
}

/// A four-case switch over a range of 28 (density 14%) became a 28-entry
/// table: +90..140 B and no faster than four compares on the jpeg_* cells. The
/// limit is the pin's default (10%), not m68k's own price of a table slot
/// against a compare.
#[test]
fn a_sparse_switch_stays_a_compare_chain_at_every_level() {
    let module = llrm_mir::parse::module(&format!(
        "target datalayout = \"{DATALAYOUT}\"
define i32 @f(i32 %x) {{
entry:
  switch i32 %x, label %d [
    i32 -64, label %a
    i32 -62, label %b
    i32 -60, label %c
    i32 -37, label %e
  ]
a:
  ret i32 11
b:
  ret i32 22
c:
  ret i32 33
e:
  ret i32 44
d:
  ret i32 -1
}}"
    ))
    .expect("parses");
    for cpu in [Cpu::M68000, Cpu::M68020] {
        for (level, options) in [("O2", Options::default()), ("Os", Options { size: true, ..Options::default() })] {
            let out = text_with(&module, "f", cpu, options).expect("compiles");
            assert!(!out.contains("jmp\t(.LJTI") && !out.contains(".word"), "{cpu:?} {level}: a table:\n{out}");
        }
    }
}

/// The word table's offsets are from its own start, which is where the `jmp`
/// ends: the `jmp (d8,%pc,Xn.w)` reads `d8` = 2 and the table starts 4 bytes
/// after its opcode. A table made 4 bytes an entry, or off by a word, jumped
/// into the middle of an instruction.
#[test]
fn the_table_starts_where_the_jump_through_it_ends() {
    let module = llrm_mir::parse::module(&switch()).expect("parses");
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let items = items_with(&module, "f", cpu, Options::default()).expect("compiles");
        let at = items.iter().position(|item| matches!(item, Item::WordTable(_))).expect("a word table");
        let Item::Label(table) = &items[at - 1] else { panic!("the table has no label") };
        let placed = llrm_m68k::object::assemble(&items, 0x1000).expect("assembles");
        let start = placed.labels.iter().find(|(label, _)| label == table).expect("a label").1 - 0x1000;
        let jump = &placed.code[start as usize - 4..start as usize];
        assert_eq!(jump, [0x4E, 0xFB, jump[2], 0x02], "{cpu:?}: jmp (d8,%pc,Xn.w) with d8 = 2");
        assert_eq!(jump[2] & 0x08, 0, "{cpu:?}: a word index (bit 11 of the extension word)");
    }
}

/// A function of thousands of instructions could pass 32 KiB, where a signed
/// 16-bit offset ends: it keeps the pin's 32-bit entries.
#[test]
fn a_function_too_big_for_word_offsets_keeps_long_entries() {
    let cases: String = (0..10).map(|n| format!("    i32 {n}, label %c{n}\n")).collect();
    let calls: String = (0..4200).map(|n| format!("  call void @g(i32 {n})\n")).collect();
    let blocks: String = (0..10).map(|n| format!("c{n}:\n  call void @h{n}()\n  ret void\n")).collect();
    let declares: String = (0..10).map(|n| format!("declare void @h{n}()\n")).collect();
    let module = llrm_mir::parse::module(&format!(
        "target datalayout = \"{DATALAYOUT}\"
declare void @g(i32)
{declares}define void @f(i32 %x) {{
entry:
{calls}  switch i32 %x, label %d [
{cases}  ]
{blocks}d:
  ret void
}}"
    ))
    .expect("parses");
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let items = items_with(&module, "f", cpu, Options::default()).expect("compiles");
        assert!(items.iter().any(|item| matches!(item, Item::Table(_))), "{cpu:?}: no long table");
        assert!(!items.iter().any(|item| matches!(item, Item::WordTable(_))), "{cpu:?}: a word table");
    }
}

/// `Options::word_jump_tables` off is the pin's table: 32-bit entries, `jmp
/// (%a0)`.
#[test]
fn the_flag_off_keeps_the_pins_long_entries() {
    let module = llrm_mir::parse::module(&switch()).expect("parses");
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let options = Options { word_jump_tables: false, ..Options::default() };
        let out = text_with(&module, "f", cpu, options).expect("compiles");
        assert!(out.contains("jmp\t(%a0)") && out.contains("\t.long\t.LBB"), "{cpu:?}:\n{out}");
    }
}
