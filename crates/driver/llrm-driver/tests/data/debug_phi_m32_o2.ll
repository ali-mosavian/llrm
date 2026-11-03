; ModuleID = 'phi.c'
source_filename = "phi.c"
target datalayout = "@m32-datalayout@"

; Function Attrs: nofree norecurse nosync nounwind memory(none) uwtable
define dso_local i32 @f(i32 noundef %0, i32 noundef %1) local_unnamed_addr #0 !dbg !13 {
    #dbg_value(i32 %0, !18, !DIExpression(), !23)
    #dbg_value(i32 %1, !19, !DIExpression(), !23)
    #dbg_value(i32 0, !20, !DIExpression(), !23)
    #dbg_value(i32 0, !21, !DIExpression(), !24)
  %3 = icmp sgt i32 %0, 0, !dbg !25
  br i1 %3, label %6, label %4, !dbg !27

4:                                                ; preds = %6, %2
  %5 = phi i32 [ 0, %2 ], [ %13, %6 ], !dbg !23
  ret i32 %5, !dbg !28

6:                                                ; preds = %2, %6
  %7 = phi i32 [ %14, %6 ], [ 0, %2 ]
  %8 = phi i32 [ %13, %6 ], [ 0, %2 ]
    #dbg_value(i32 %7, !21, !DIExpression(), !24)
    #dbg_value(i32 %8, !20, !DIExpression(), !23)
  %9 = and i32 %7, 1, !dbg !29
  %10 = icmp eq i32 %9, 0, !dbg !32
  %11 = sub nsw i32 0, %7, !dbg !33
  %12 = select i1 %10, i32 %1, i32 %11, !dbg !33
  %13 = add i32 %12, %8, !dbg !33
    #dbg_value(i32 %13, !20, !DIExpression(), !23)
  %14 = add nuw nsw i32 %7, 1, !dbg !34
    #dbg_value(i32 %14, !21, !DIExpression(), !24)
  %15 = icmp eq i32 %14, %0, !dbg !35
  br i1 %15, label %4, label %6, !dbg !36, !llvm.loop !37
}

attributes #0 = { nofree norecurse nosync nounwind memory(none) uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="pentium4" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" }

!llvm.dbg.cu = !{!0}
!llvm.module.flags = !{!2, !3, !4, !5, !6}
!llvm.ident = !{!7}
!llvm.errno.tbaa = !{!8}

!0 = distinct !DICompileUnit(language: DW_LANG_C11, file: !1, producer: "clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)", isOptimized: true, runtimeVersion: 0, emissionKind: FullDebug, splitDebugInlining: false, nameTableKind: None)
!1 = !DIFile(filename: "phi.c", directory: "/src", checksumkind: CSK_MD5, checksum: "e2290d0992703b321234c26964347455")
!2 = !{i32 1, !"NumRegisterParameters", i32 0}
!3 = !{i32 7, !"Dwarf Version", i32 5}
!4 = !{i32 2, !"Debug Info Version", i32 3}
!5 = !{i32 7, !"uwtable", i32 2}
!6 = !{i32 7, !"debug-info-assignment-tracking", i1 true}
!7 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!8 = !{!9, !10, i64 0}
!9 = !{!"__libc_errno", !10, i64 0}
!10 = !{!"int", !11, i64 0}
!11 = !{!"omnipotent char", !12, i64 0}
!12 = !{!"Simple C/C++ TBAA"}
!13 = distinct !DISubprogram(name: "f", scope: !1, file: !1, line: 1, type: !14, scopeLine: 2, flags: DIFlagPrototyped | DIFlagAllCallsDescribed, spFlags: DISPFlagDefinition | DISPFlagOptimized, unit: !0, retainedNodes: !17, keyInstructions: true)
!14 = !DISubroutineType(types: !15)
!15 = !{!16, !16, !16}
!16 = !DIBasicType(name: "int", size: 32, encoding: DW_ATE_signed)
!17 = !{!18, !19, !20, !21}
!18 = !DILocalVariable(name: "n", arg: 1, scope: !13, file: !1, line: 1, type: !16)
!19 = !DILocalVariable(name: "k", arg: 2, scope: !13, file: !1, line: 1, type: !16)
!20 = !DILocalVariable(name: "s", scope: !13, file: !1, line: 3, type: !16)
!21 = !DILocalVariable(name: "i", scope: !22, file: !1, line: 4, type: !16)
!22 = distinct !DILexicalBlock(scope: !13, file: !1, line: 4, column: 5)
!23 = !DILocation(line: 0, scope: !13)
!24 = !DILocation(line: 0, scope: !22)
!25 = !DILocation(line: 4, column: 23, scope: !26, atomGroup: 12, atomRank: 1)
!26 = distinct !DILexicalBlock(scope: !22, file: !1, line: 4, column: 5)
!27 = !DILocation(line: 4, column: 5, scope: !22, atomGroup: 13, atomRank: 1)
!28 = !DILocation(line: 10, column: 5, scope: !13, atomGroup: 11, atomRank: 1)
!29 = !DILocation(line: 5, column: 15, scope: !30)
!30 = distinct !DILexicalBlock(scope: !31, file: !1, line: 5, column: 13)
!31 = distinct !DILexicalBlock(scope: !26, file: !1, line: 4, column: 33)
!32 = !DILocation(line: 5, column: 15, scope: !30, atomGroup: 5, atomRank: 2)
!33 = !DILocation(line: 5, column: 15, scope: !30, atomGroup: 5, atomRank: 1)
!34 = !DILocation(line: 4, column: 29, scope: !26, atomGroup: 8, atomRank: 2)
!35 = !DILocation(line: 4, column: 23, scope: !26, atomGroup: 3, atomRank: 1)
!36 = !DILocation(line: 4, column: 5, scope: !22, atomGroup: 4, atomRank: 1)
!37 = distinct !{!37, !38, !39, !40, !41}
!38 = !DILocation(line: 4, column: 5, scope: !22)
!39 = !DILocation(line: 9, column: 5, scope: !22)
!40 = !{!"llvm.loop.mustprogress"}
!41 = !{!"llvm.loop.unroll.disable"}
