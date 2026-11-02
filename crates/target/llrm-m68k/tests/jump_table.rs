//! A dense switch lowered to a jump table kept its `jmp (%a0)` and every case
//! block: branch folding took the table jump for a plain jump to its first
//! block and dropped the dispatch and the other nine cases.

use llrm_m68k::compile::{Options, text_with};
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
