; ModuleID = 'm.c'
source_filename = "m.c"
target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-unknown-elf"

; Function Attrs: mustprogress nofree norecurse nosync nounwind willreturn memory(none)
define dso_local range(i32 -2147483647, -2147483648) i32 @f(i32 noundef %0, i32 noundef %1) local_unnamed_addr #0 !dbg !11 {
    #dbg_value(i32 %0, !16, !DIExpression(), !19)
    #dbg_value(i32 %1, !17, !DIExpression(), !19)
  %3 = mul nsw i32 %1, %0, !dbg !20
    #dbg_value(i32 %3, !18, !DIExpression(), !19)
  %4 = add nsw i32 %3, 1, !dbg !21
  ret i32 %4, !dbg !22
}

attributes #0 = { mustprogress nofree norecurse nosync nounwind willreturn memory(none) "no-trapping-math"="true" "stack-protector-buffer-size"="8" }

!llvm.dbg.cu = !{!0}
!llvm.module.flags = !{!2, !3, !4}
!llvm.ident = !{!5}
!llvm.errno.tbaa = !{!6}

!0 = distinct !DICompileUnit(language: DW_LANG_C11, file: !1, producer: "clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)", isOptimized: true, runtimeVersion: 0, emissionKind: FullDebug, splitDebugInlining: false, nameTableKind: None)
!1 = !DIFile(filename: "m.c", directory: "/tmp/ldv", checksumkind: CSK_MD5, checksum: "53ff53e020c529f8137e4c00083e4c3c")
!2 = !{i32 7, !"Dwarf Version", i32 5}
!3 = !{i32 2, !"Debug Info Version", i32 3}
!4 = !{i32 7, !"debug-info-assignment-tracking", i1 true}
!5 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!6 = !{!7, !8, i64 0}
!7 = !{!"__libc_errno", !8, i64 0}
!8 = !{!"int", !9, i64 0}
!9 = !{!"omnipotent char", !10, i64 0}
!10 = !{!"Simple C/C++ TBAA"}
!11 = distinct !DISubprogram(name: "f", scope: !1, file: !1, line: 1, type: !12, scopeLine: 2, flags: DIFlagPrototyped | DIFlagAllCallsDescribed, spFlags: DISPFlagDefinition | DISPFlagOptimized, unit: !0, retainedNodes: !15, keyInstructions: true)
!12 = !DISubroutineType(types: !13)
!13 = !{!14, !14, !14}
!14 = !DIBasicType(name: "int", size: 32, encoding: DW_ATE_signed)
!15 = !{!16, !17, !18}
!16 = !DILocalVariable(name: "a", arg: 1, scope: !11, file: !1, line: 1, type: !14)
!17 = !DILocalVariable(name: "b", arg: 2, scope: !11, file: !1, line: 1, type: !14)
!18 = !DILocalVariable(name: "c", scope: !11, file: !1, line: 3, type: !14)
!19 = !DILocation(line: 0, scope: !11)
!20 = !DILocation(line: 3, column: 15, scope: !11, atomGroup: 1, atomRank: 2)
!21 = !DILocation(line: 4, column: 14, scope: !11, atomGroup: 3, atomRank: 2)
!22 = !DILocation(line: 4, column: 5, scope: !11, atomGroup: 3, atomRank: 1)
