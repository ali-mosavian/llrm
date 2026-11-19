; ModuleID = '/tmp/sc.c'
source_filename = "/tmp/sc.c"
target datalayout = "e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32"
target triple = "i486-unknown-linux-gnu"

; Function Attrs: mustprogress nofree norecurse nosync nounwind willreturn memory(none)
define dso_local i32 @f(i32 noundef %0) local_unnamed_addr #0 !dbg !12 {
    #dbg_value(i32 %0, !18, !DIExpression(), !22)
    #dbg_value(i32 0, !19, !DIExpression(), !22)
  %2 = add nsw i32 %0, 1, !dbg !23
    #dbg_value(i32 %2, !20, !DIExpression(), !24)
    #dbg_value(i32 %2, !25, !DIExpression(), !28)
  %3 = mul nsw i32 %2, %2, !dbg !30
    #dbg_value(i32 %3, !19, !DIExpression(), !22)
    #dbg_value(i32 %0, !25, !DIExpression(), !31)
  %4 = mul nsw i32 %0, %0, !dbg !33
  %5 = add nuw nsw i32 %3, %4, !dbg !34
  ret i32 %5, !dbg !35
}

attributes #0 = { mustprogress nofree norecurse nosync nounwind willreturn memory(none) "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="pentium4" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" }

!llvm.dbg.cu = !{!0}
!llvm.module.flags = !{!2, !3, !4, !5}
!llvm.ident = !{!6}
!llvm.errno.tbaa = !{!7}

!0 = distinct !DICompileUnit(language: DW_LANG_C11, file: !1, producer: "clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)", isOptimized: true, runtimeVersion: 0, emissionKind: FullDebug, splitDebugInlining: false, nameTableKind: None)
!1 = !DIFile(filename: "/tmp/sc.c", directory: "", checksumkind: CSK_MD5, checksum: "2c4aa88b4bafb45bfd789cefb81f4990")
!2 = !{i32 1, !"NumRegisterParameters", i32 0}
!3 = !{i32 7, !"Dwarf Version", i32 5}
!4 = !{i32 2, !"Debug Info Version", i32 3}
!5 = !{i32 7, !"debug-info-assignment-tracking", i1 true}
!6 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!7 = !{!8, !9, i64 0}
!8 = !{!"__libc_errno", !9, i64 0}
!9 = !{!"int", !10, i64 0}
!10 = !{!"omnipotent char", !11, i64 0}
!11 = !{!"Simple C/C++ TBAA"}
!12 = distinct !DISubprogram(name: "f", scope: !13, file: !13, line: 2, type: !14, scopeLine: 2, flags: DIFlagPrototyped | DIFlagAllCallsDescribed, spFlags: DISPFlagDefinition | DISPFlagOptimized, unit: !0, retainedNodes: !17, keyInstructions: true)
!13 = !DIFile(filename: "/tmp/sc.c", directory: "", checksumkind: CSK_MD5, checksum: "2c4aa88b4bafb45bfd789cefb81f4990")
!14 = !DISubroutineType(types: !15)
!15 = !{!16, !16}
!16 = !DIBasicType(name: "int", size: 32, encoding: DW_ATE_signed)
!17 = !{!18, !19, !20}
!18 = !DILocalVariable(name: "a", arg: 1, scope: !12, file: !13, line: 2, type: !16)
!19 = !DILocalVariable(name: "r", scope: !12, file: !13, line: 2, type: !16)
!20 = !DILocalVariable(name: "t", scope: !21, file: !13, line: 2, type: !16)
!21 = distinct !DILexicalBlock(scope: !12, file: !13, line: 2, column: 27)
!22 = !DILocation(line: 0, scope: !12)
!23 = !DILocation(line: 2, column: 39, scope: !21, atomGroup: 2, atomRank: 2)
!24 = !DILocation(line: 0, scope: !21)
!25 = !DILocalVariable(name: "x", arg: 1, scope: !26, file: !13, line: 1, type: !16)
!26 = distinct !DISubprogram(name: "sq", scope: !13, file: !13, line: 1, type: !14, scopeLine: 1, flags: DIFlagPrototyped | DIFlagAllCallsDescribed, spFlags: DISPFlagLocalToUnit | DISPFlagDefinition | DISPFlagOptimized, unit: !0, retainedNodes: !27, keyInstructions: true)
!27 = !{!25}
!28 = distinct !DILocation(line: 0, scope: !26, inlinedAt: !29)
!29 = distinct !DILocation(line: 2, column: 48, scope: !21)
!30 = distinct !DILocation(line: 1, column: 40, scope: !26, inlinedAt: !29, atomGroup: 1, atomRank: 2)
!31 = distinct !DILocation(line: 0, scope: !26, inlinedAt: !32)
!32 = distinct !DILocation(line: 2, column: 68, scope: !12)
!33 = distinct !DILocation(line: 1, column: 40, scope: !26, inlinedAt: !32, atomGroup: 1, atomRank: 2)
!34 = !DILocation(line: 2, column: 66, scope: !12, atomGroup: 5, atomRank: 2)
!35 = !DILocation(line: 2, column: 57, scope: !12, atomGroup: 5, atomRank: 1)
