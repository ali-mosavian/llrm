//! A dense switch lowered to a jump table kept its `jmp (%a0)` and every case
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
        assert!(out.contains("jmp\t(%a0)"), "{cpu:?}: no table jump:\n{out}");
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
        assert_eq!(text.matches("\t.long\t.LBB").count(), 10, "{cpu:?}:\n{text}");
        let items = items_with(&module, "f", cpu, Options::default()).expect("compiles");
        let at = items.iter().position(|item| matches!(item, Item::Table(_))).expect("a table");
        let Item::Label(table) = &items[at - 1] else { panic!("the table has no label") };
        assert!(
            text.contains(&format!("(%pc, {table})")) || text.contains(table),
            "{cpu:?}: lea names {table}:\n{text}"
        );
        let Item::Table(cases) = &items[at] else { unreachable!() };
        let placed = llrm_m68k::object::assemble(&items, 0x1000).expect("assembles");
        let address = |name: &str| placed.labels.iter().find(|(label, _)| label == name).expect("a label").1;
        let start = (address(table) - 0x1000) as usize;
        for (n, case) in cases.iter().enumerate() {
            let word = &placed.code[start + 4 * n..start + 4 * n + 4];
            assert_eq!(u32::from_be_bytes(word.try_into().unwrap()), address(case), "{cpu:?} entry {n}");
        }
    }
}
