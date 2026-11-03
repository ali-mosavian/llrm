//! `-g` on the 6502 (`g-for-all-targets-card.md`, 6502 step 1): the selected
//! instructions carry the source place of the IR instruction they came from.
//! They carried none, so no line table could be made of them.

use llrm_6502::dagisel::function::select_function;
use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::select::{Item, function_items_with_places};

const DATALAYOUT: &str = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8";

const LOCATED: &str = "define i8 @f(i8 %a) !dbg !5 {
  %b = add i8 %a, 1, !dbg !7
  %c = add i8 %b, 2, !dbg !8
  ret i8 %c, !dbg !8
}
!llvm.dbg.cu = !{!0}
!llvm.module.flags = !{!2}
!0 = distinct !DICompileUnit(language: DW_LANG_C11, file: !1, producer: \"t\", isOptimized: true, runtimeVersion: 0, emissionKind: FullDebug)
!1 = !DIFile(filename: \"f.c\", directory: \"/d\")
!2 = !{i32 2, !\"Debug Info Version\", i32 3}
!5 = distinct !DISubprogram(name: \"f\", scope: !1, file: !1, line: 1, type: !6, scopeLine: 2, spFlags: DISPFlagDefinition, unit: !0)
!6 = !DISubroutineType(types: !{null})
!7 = !DILocation(line: 3, column: 5, scope: !5)
!8 = !DILocation(line: 4, column: 3, scope: !5)
";

#[test]
fn an_instruction_selected_from_a_located_node_carries_its_place() {
    let module = llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{LOCATED}")).expect("parses");
    let selection = select_function(&module, "f", Cpu::Mos6502).expect("selects");
    let (items, _, places) = function_items_with_places(&module, "f", &selection).expect("items");
    let lines: Vec<u32> = places.iter().map(|place| place.line).collect();
    // The two adds fold into one, which keeps the later place.
    assert!(lines.contains(&4), "{lines:?}");
    let numbered = items.iter().filter(|item| matches!(item, Item::Inst(inst) if inst.origin != 0)).count();
    assert!(numbered >= 1, "no instruction carries a place of {items:?}");
}
