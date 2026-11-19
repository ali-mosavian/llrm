; ModuleID = 'sw.c'
source_filename = "sw.c"
target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-unknown-elf"

; Function Attrs: nounwind
define dso_local i32 @h(i32 noundef %0) local_unnamed_addr #0 !dbg !11 {
    #dbg_value(i32 %0, !16, !DIExpression(), !17)
  %2 = icmp eq i32 %0, 2, !dbg !18
  %3 = select i1 %2, i32 20, i32 30, !dbg !18
  %4 = icmp eq i32 %0, 1, !dbg !18
  %5 = select i1 %4, i32 10, i32 %3, !dbg !18
  %6 = tail call i32 @g(i32 noundef %5) #2, !dbg !19
  ret i32 %6, !dbg !21
}

declare !dbg !22 dso_local i32 @g(i32 noundef) local_unnamed_addr #1

attributes #0 = { nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #2 = { nounwind }

!llvm.dbg.cu = !{!0}
!llvm.module.flags = !{!2, !3, !4}
!llvm.ident = !{!5}
!llvm.errno.tbaa = !{!6}

!0 = distinct !DICompileUnit(language: DW_LANG_C11, file: !1, producer: "clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)", isOptimized: true, runtimeVersion: 0, emissionKind: FullDebug, splitDebugInlining: false, nameTableKind: None)
!1 = !DIFile(filename: "sw.c", directory: "/tmp/ldv", checksumkind: CSK_MD5, checksum: "d2f0cde525ba2b82992a00f67df0be19")
!2 = !{i32 7, !"Dwarf Version", i32 5}
!3 = !{i32 2, !"Debug Info Version", i32 3}
!4 = !{i32 7, !"debug-info-assignment-tracking", i1 true}
!5 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!6 = !{!7, !8, i64 0}
!7 = !{!"__libc_errno", !8, i64 0}
!8 = !{!"int", !9, i64 0}
!9 = !{!"omnipotent char", !10, i64 0}
!10 = !{!"Simple C/C++ TBAA"}
!11 = distinct !DISubprogram(name: "h", scope: !1, file: !1, line: 2, type: !12, scopeLine: 3, flags: DIFlagPrototyped | DIFlagAllCallsDescribed, spFlags: DISPFlagDefinition | DISPFlagOptimized, unit: !0, retainedNodes: !15, keyInstructions: true)
!12 = !DISubroutineType(types: !13)
!13 = !{!14, !14}
!14 = !DIBasicType(name: "int", size: 32, encoding: DW_ATE_signed)
!15 = !{!16}
!16 = !DILocalVariable(name: "x", arg: 1, scope: !11, file: !1, line: 2, type: !14)
!17 = !DILocation(line: 0, scope: !11)
!18 = !DILocation(line: 4, column: 5, scope: !11, atomGroup: 1, atomRank: 1)
!19 = !DILocation(line: 0, scope: !20)
!20 = distinct !DILexicalBlock(scope: !11, file: !1, line: 4, column: 16)
!21 = !DILocation(line: 9, column: 1, scope: !11, atomGroup: 5, atomRank: 1)
!22 = !DISubprogram(name: "g", scope: !1, file: !1, line: 1, type: !12, flags: DIFlagPrototyped, spFlags: DISPFlagOptimized)
