target datalayout = "e-m:e-p:32:32-p270:32:32-p271:32:32-p272:64:64-i128:128-f64:32:64-f80:32-n8:16:32-S128"

@bench_recgcd_via = global ptr @bench_recgcd, align 4, !dbg !0

; Function Attrs: nofree norecurse nosync nounwind memory(none) uwtable
define i32 @bench_recgcd(i32 noundef %0) #0 !dbg !25 {
    #dbg_value(i32 %0, !27, !DIExpression(), !30)
    #dbg_value(i32 0, !28, !DIExpression(), !30)
    #dbg_value(i32 1, !29, !DIExpression(), !30)
  %2 = icmp slt i32 %0, 1, !dbg !31
  br i1 %2, label %27, label %3, !dbg !34

3:                                                ; preds = %1, %22
  %4 = phi i32 [ %25, %22 ], [ 1, %1 ]
  %5 = phi i32 [ %24, %22 ], [ 0, %1 ]
    #dbg_value(i32 %4, !29, !DIExpression(), !30)
    #dbg_value(i32 %5, !28, !DIExpression(), !30)
  %6 = mul nuw nsw i32 %4, 7, !dbg !35
  %7 = add nuw nsw i32 %6, 3, !dbg !36
  %8 = mul nuw nsw i32 %4, 5, !dbg !37
  %9 = add nuw nsw i32 %8, 2, !dbg !38
    #dbg_value(i32 %7, !39, !DIExpression(), !45)
    #dbg_value(i32 %9, !44, !DIExpression(), !45)
  br label %10, !dbg !47

10:                                               ; preds = %3, %10
  %11 = phi i32 [ %13, %10 ], [ %9, %3 ]
  %12 = phi i32 [ %11, %10 ], [ %7, %3 ]
    #dbg_value(i32 %11, !44, !DIExpression(), !45)
    #dbg_value(i32 %12, !39, !DIExpression(), !45)
  %13 = srem i32 %12, %11, !dbg !49
    #dbg_value(i32 %11, !39, !DIExpression(), !45)
    #dbg_value(i32 %13, !44, !DIExpression(), !45)
  %14 = icmp eq i32 %13, 0, !dbg !50
  br i1 %14, label %15, label %10, !dbg !51

15:                                               ; preds = %10
  %16 = add nuw nsw i32 %4, 90, !dbg !52
    #dbg_value(i32 %16, !39, !DIExpression(), !53)
    #dbg_value(i32 360, !44, !DIExpression(), !53)
  br label %17, !dbg !55

17:                                               ; preds = %17, %15
  %18 = phi i32 [ %20, %17 ], [ 360, %15 ]
  %19 = phi i32 [ %18, %17 ], [ %16, %15 ]
    #dbg_value(i32 %18, !44, !DIExpression(), !53)
    #dbg_value(i32 %19, !39, !DIExpression(), !53)
  %20 = srem i32 %19, %18, !dbg !56
    #dbg_value(i32 %18, !39, !DIExpression(), !53)
    #dbg_value(i32 %20, !44, !DIExpression(), !53)
  %21 = icmp eq i32 %20, 0, !dbg !57
  br i1 %21, label %22, label %17, !dbg !58

22:                                               ; preds = %17
  %23 = add i32 %11, %5, !dbg !59
  %24 = add i32 %23, %18, !dbg !60
    #dbg_value(!DIArgList(i32 %5, i32 %11, i32 %18), !28, !DIExpression(DW_OP_LLVM_arg, 0, DW_OP_LLVM_arg, 1, DW_OP_LLVM_arg, 2, DW_OP_plus, DW_OP_plus, DW_OP_stack_value), !30)
  %25 = add nuw i32 %4, 1, !dbg !61
    #dbg_value(i32 %25, !29, !DIExpression(), !30)
    #dbg_value(i32 %24, !28, !DIExpression(), !30)
  %26 = icmp eq i32 %4, %0, !dbg !62
  br i1 %26, label %27, label %3, !dbg !63, !llvm.loop !64

27:                                               ; preds = %22, %1
  %28 = phi i32 [ 0, %1 ], [ %24, %22 ], !dbg !30
  ret i32 %28, !dbg !68
}

; Function Attrs: nounwind uwtable
define noundef i32 @main() local_unnamed_addr #1 !dbg !69 {
  %1 = load volatile ptr, ptr @bench_recgcd_via, align 4, !dbg !72, !tbaa !73
  %2 = tail call i32 %1(i32 noundef 300) #3, !dbg !75
  tail call void @report(i32 noundef %2) #3, !dbg !76
  ret i32 0, !dbg !77
}

declare !dbg !78 void @report(i32 noundef) local_unnamed_addr #2

attributes #0 = { nofree norecurse nosync nounwind memory(none) uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="pentium4" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" }
attributes #1 = { nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="pentium4" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" }
attributes #2 = { "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="pentium4" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" }
attributes #3 = { nounwind }

!llvm.dbg.cu = !{!2}
!llvm.module.flags = !{!12, !13, !14, !15, !16, !17, !18}
!llvm.ident = !{!19}
!llvm.errno.tbaa = !{!20}

!0 = !DIGlobalVariableExpression(var: !1, expr: !DIExpression())
!1 = distinct !DIGlobalVariable(name: "bench_recgcd_via", scope: !2, file: !5, line: 19, type: !6, isLocal: false, isDefinition: true)
!2 = distinct !DICompileUnit(language: DW_LANG_C11, file: !3, producer: "clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)", isOptimized: true, runtimeVersion: 0, emissionKind: FullDebug, globals: !4, splitDebugInlining: false, nameTableKind: None)
!3 = !DIFile(filename: "/home/alim/scratch/rows-aac/main/b/recgcd.c", directory: "/tmp/claude-1000/dx", checksumkind: CSK_MD5, checksum: "e747c7d259981bbdc53e573ee2e05f21")
!4 = !{!0}
!5 = !DIFile(filename: "/home/alim/scratch/rows-aac/main/b/recgcd.c", directory: "", checksumkind: CSK_MD5, checksum: "e747c7d259981bbdc53e573ee2e05f21")
!6 = !DIDerivedType(tag: DW_TAG_volatile_type, baseType: !7)
!7 = !DIDerivedType(tag: DW_TAG_pointer_type, baseType: !8, size: 32)
!8 = !DISubroutineType(types: !9)
!9 = !{!10, !11}
!10 = !DIBasicType(name: "long", size: 32, encoding: DW_ATE_signed)
!11 = !DIBasicType(name: "int", size: 32, encoding: DW_ATE_signed)
!12 = !{i32 1, !"NumRegisterParameters", i32 0}
!13 = !{i32 7, !"Dwarf Version", i32 5}
!14 = !{i32 2, !"Debug Info Version", i32 3}
!15 = !{i32 8, !"PIC Level", i32 2}
!16 = !{i32 7, !"PIE Level", i32 2}
!17 = !{i32 7, !"uwtable", i32 2}
!18 = !{i32 7, !"debug-info-assignment-tracking", i1 true}
!19 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!20 = !{!21, !22, i64 0}
!21 = !{!"__libc_errno", !22, i64 0}
!22 = !{!"int", !23, i64 0}
!23 = !{!"omnipotent char", !24, i64 0}
!24 = !{!"Simple C/C++ TBAA"}
!25 = distinct !DISubprogram(name: "bench_recgcd", scope: !5, file: !5, line: 10, type: !8, scopeLine: 11, flags: DIFlagPrototyped | DIFlagAllCallsDescribed, spFlags: DISPFlagDefinition | DISPFlagOptimized, unit: !2, retainedNodes: !26, keyInstructions: true)
!26 = !{!27, !28, !29}
!27 = !DILocalVariable(name: "n", arg: 1, scope: !25, file: !5, line: 10, type: !11)
!28 = !DILocalVariable(name: "total", scope: !25, file: !5, line: 12, type: !10)
!29 = !DILocalVariable(name: "i", scope: !25, file: !5, line: 13, type: !11)
!30 = !DILocation(line: 0, scope: !25)
!31 = !DILocation(line: 15, column: 19, scope: !32, atomGroup: 11, atomRank: 1)
!32 = distinct !DILexicalBlock(scope: !33, file: !5, line: 15, column: 5)
!33 = distinct !DILexicalBlock(scope: !25, file: !5, line: 15, column: 5)
!34 = !DILocation(line: 15, column: 5, scope: !33, atomGroup: 12, atomRank: 1)
!35 = !DILocation(line: 15, column: 45, scope: !32)
!36 = !DILocation(line: 15, column: 49, scope: !32)
!37 = !DILocation(line: 15, column: 56, scope: !32)
!38 = !DILocation(line: 15, column: 60, scope: !32)
!39 = !DILocalVariable(name: "a", arg: 1, scope: !40, file: !5, line: 4, type: !11)
!40 = distinct !DISubprogram(name: "gcd", scope: !5, file: !5, line: 4, type: !41, scopeLine: 5, flags: DIFlagPrototyped | DIFlagAllCallsDescribed, spFlags: DISPFlagLocalToUnit | DISPFlagDefinition | DISPFlagOptimized, unit: !2, retainedNodes: !43, keyInstructions: true)
!41 = !DISubroutineType(types: !42)
!42 = !{!11, !11, !11}
!43 = !{!39, !44}
!44 = !DILocalVariable(name: "b", arg: 2, scope: !40, file: !5, line: 4, type: !11)
!45 = distinct !DILocation(line: 0, scope: !40, inlinedAt: !46)
!46 = distinct !DILocation(line: 15, column: 39, scope: !32)
!47 = distinct !DILocation(line: 6, column: 11, scope: !48, inlinedAt: !46, atomGroup: 10, atomRank: 1)
!48 = distinct !DILexicalBlock(scope: !40, file: !5, line: 6, column: 9)
!49 = distinct !DILocation(line: 7, column: 21, scope: !40, inlinedAt: !46)
!50 = distinct !DILocation(line: 6, column: 11, scope: !48, inlinedAt: !46, atomGroup: 1, atomRank: 2)
!51 = distinct !DILocation(line: 6, column: 11, scope: !48, inlinedAt: !46, atomGroup: 1, atomRank: 1)
!52 = !DILocation(line: 15, column: 73, scope: !32)
!53 = distinct !DILocation(line: 0, scope: !40, inlinedAt: !54)
!54 = distinct !DILocation(line: 15, column: 67, scope: !32)
!55 = distinct !DILocation(line: 6, column: 11, scope: !48, inlinedAt: !54, atomGroup: 10, atomRank: 1)
!56 = distinct !DILocation(line: 7, column: 21, scope: !40, inlinedAt: !54)
!57 = distinct !DILocation(line: 6, column: 11, scope: !48, inlinedAt: !54, atomGroup: 1, atomRank: 2)
!58 = distinct !DILocation(line: 6, column: 11, scope: !48, inlinedAt: !54, atomGroup: 1, atomRank: 1)
!59 = !DILocation(line: 15, column: 65, scope: !32)
!60 = !DILocation(line: 15, column: 36, scope: !32, atomGroup: 5, atomRank: 2)
!61 = !DILocation(line: 15, column: 25, scope: !32, atomGroup: 6, atomRank: 2)
!62 = !DILocation(line: 15, column: 19, scope: !32, atomGroup: 3, atomRank: 1)
!63 = !DILocation(line: 15, column: 5, scope: !33, atomGroup: 4, atomRank: 1)
!64 = distinct !{!64, !65, !66, !67}
!65 = !DILocation(line: 15, column: 5, scope: !33)
!66 = !DILocation(line: 15, column: 82, scope: !33)
!67 = !{!"llvm.loop.mustprogress"}
!68 = !DILocation(line: 16, column: 5, scope: !25, atomGroup: 9, atomRank: 1)
!69 = distinct !DISubprogram(name: "main", scope: !5, file: !5, line: 20, type: !70, scopeLine: 21, flags: DIFlagPrototyped | DIFlagAllCallsDescribed, spFlags: DISPFlagDefinition | DISPFlagOptimized, unit: !2, keyInstructions: true)
!70 = !DISubroutineType(types: !71)
!71 = !{!11}
!72 = !DILocation(line: 22, column: 14, scope: !69)
!73 = !{!74, !74, i64 0}
!74 = !{!"any pointer", !23, i64 0}
!75 = !DILocation(line: 22, column: 12, scope: !69)
!76 = !DILocation(line: 22, column: 5, scope: !69)
!77 = !DILocation(line: 23, column: 5, scope: !69, atomGroup: 1, atomRank: 1)
!78 = !DISubprogram(name: "report", scope: !5, file: !5, line: 2, type: !79, flags: DIFlagPrototyped, spFlags: DISPFlagOptimized)
!79 = !DISubroutineType(types: !80)
!80 = !{null, !10}
