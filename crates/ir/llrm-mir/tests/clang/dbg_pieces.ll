; ModuleID = 't3.c'
source_filename = "t3.c"
target datalayout = "e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32"

; Function Attrs: nounwind
define i64 @f(i64 noundef %0, i32 noundef %1) #0 !dbg !13 {
    #dbg_value(i64 %0, !19, !DIExpression(), !24)
    #dbg_value(i32 %1, !20, !DIExpression(), !24)
    #dbg_value(i64 %0, !21, !DIExpression(), !24)
    #dbg_value(i32 0, !22, !DIExpression(), !25)
  %3 = icmp sgt i32 %1, 0, !dbg !26
  br i1 %3, label %6, label %4, !dbg !28

4:                                                ; preds = %6, %2
  %5 = phi i64 [ %0, %2 ], [ %12, %6 ], !dbg !29
  ret i64 %5, !dbg !30

6:                                                ; preds = %2, %6
  %7 = phi i32 [ %13, %6 ], [ 0, %2 ]
  %8 = phi i64 [ %12, %6 ], [ %0, %2 ]
    #dbg_value(i32 %7, !22, !DIExpression(), !25)
    #dbg_value(i64 %8, !21, !DIExpression(), !24)
  %9 = tail call i64 @g(i64 noundef %8) #2, !dbg !31
  %10 = zext nneg i32 %7 to i64, !dbg !32
  %11 = xor i64 %9, %10, !dbg !33
  %12 = add nsw i64 %11, %8, !dbg !34
    #dbg_value(i64 %12, !21, !DIExpression(), !24)
  %13 = add nuw nsw i32 %7, 1, !dbg !35
    #dbg_value(i32 %13, !22, !DIExpression(), !25)
  %14 = icmp eq i32 %13, %1, !dbg !36
  br i1 %14, label %4, label %6, !dbg !37, !llvm.loop !38
}

declare !dbg !42 i64 @g(i64 noundef) #1

attributes #0 = { nounwind "frame-pointer"="all" "min-legal-vector-width"="0" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="pentium4" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" }
attributes #1 = { "frame-pointer"="all" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="pentium4" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" }
attributes #2 = { nobuiltin nounwind "no-builtins" }

!llvm.dbg.cu = !{!0}
!llvm.module.flags = !{!2, !3, !4, !5, !6}
!llvm.ident = !{!7}
!llvm.errno.tbaa = !{!8}

!0 = distinct !DICompileUnit(language: DW_LANG_C11, file: !1, producer: "clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)", isOptimized: true, runtimeVersion: 0, emissionKind: FullDebug, splitDebugInlining: false, nameTableKind: None)
!1 = !DIFile(filename: "t3.c", directory: "/tmp", checksumkind: CSK_MD5, checksum: "23040b45bda09cb36bb643d6686c5910")
!2 = !{i32 1, !"NumRegisterParameters", i32 0}
!3 = !{i32 7, !"Dwarf Version", i32 5}
!4 = !{i32 2, !"Debug Info Version", i32 3}
!5 = !{i32 7, !"frame-pointer", i32 2}
!6 = !{i32 7, !"debug-info-assignment-tracking", i1 true}
!7 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!8 = !{!9, !10, i64 0}
!9 = !{!"__libc_errno", !10, i64 0}
!10 = !{!"int", !11, i64 0}
!11 = !{!"omnipotent char", !12, i64 0}
!12 = !{!"Simple C/C++ TBAA"}
!13 = distinct !DISubprogram(name: "f", scope: !1, file: !1, line: 2, type: !14, scopeLine: 2, flags: DIFlagPrototyped | DIFlagAllCallsDescribed, spFlags: DISPFlagDefinition | DISPFlagOptimized, unit: !0, retainedNodes: !18, keyInstructions: true)
!14 = !DISubroutineType(types: !15)
!15 = !{!16, !16, !17}
!16 = !DIBasicType(name: "long long", size: 64, encoding: DW_ATE_signed)
!17 = !DIBasicType(name: "int", size: 32, encoding: DW_ATE_signed)
!18 = !{!19, !20, !21, !22}
!19 = !DILocalVariable(name: "a", arg: 1, scope: !13, file: !1, line: 2, type: !16)
!20 = !DILocalVariable(name: "n", arg: 2, scope: !13, file: !1, line: 2, type: !17)
!21 = !DILocalVariable(name: "s", scope: !13, file: !1, line: 2, type: !16)
!22 = !DILocalVariable(name: "i", scope: !23, file: !1, line: 2, type: !17)
!23 = distinct !DILexicalBlock(scope: !13, file: !1, line: 2, column: 52)
!24 = !DILocation(line: 0, scope: !13)
!25 = !DILocation(line: 0, scope: !23)
!26 = !DILocation(line: 2, column: 70, scope: !27, atomGroup: 10, atomRank: 1)
!27 = distinct !DILexicalBlock(scope: !23, file: !1, line: 2, column: 52)
!28 = !DILocation(line: 2, column: 52, scope: !23, atomGroup: 11, atomRank: 1)
!29 = !DILocation(line: 2, scope: !13, atomGroup: 1, atomRank: 1)
!30 = !DILocation(line: 2, column: 95, scope: !13, atomGroup: 9, atomRank: 1)
!31 = !DILocation(line: 2, column: 85, scope: !27)
!32 = !DILocation(line: 2, column: 92, scope: !27)
!33 = !DILocation(line: 2, column: 90, scope: !27)
!34 = !DILocation(line: 2, column: 82, scope: !27, atomGroup: 5, atomRank: 2)
!35 = !DILocation(line: 2, column: 76, scope: !27, atomGroup: 6, atomRank: 2)
!36 = !DILocation(line: 2, column: 70, scope: !27, atomGroup: 3, atomRank: 1)
!37 = !DILocation(line: 2, column: 52, scope: !23, atomGroup: 4, atomRank: 1)
!38 = distinct !{!38, !39, !40, !41}
!39 = !DILocation(line: 2, column: 52, scope: !23)
!40 = !DILocation(line: 2, column: 92, scope: !23)
!41 = !{!"llvm.loop.mustprogress"}
!42 = !DISubprogram(name: "g", scope: !1, file: !1, line: 1, type: !43, flags: DIFlagPrototyped, spFlags: DISPFlagOptimized)
!43 = !DISubroutineType(types: !44)
!44 = !{!16, !16}
