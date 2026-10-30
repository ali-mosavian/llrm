; ModuleID = 'al.c'
source_filename = "al.c"
target datalayout = "e-m:e-p:32:32-p270:32:32-p271:32:32-p272:64:64-i128:128-f64:32:64-f80:32-n8:16:32-S128"

; Function Attrs: nounwind uwtable
define noundef i32 @f(i32 noundef %a, i32 noundef %b, i32 noundef %c) local_unnamed_addr #0 !dbg !11 {
entry:
    #dbg_value(i32 %a, !16, !DIExpression(), !23)
    #dbg_value(i32 %b, !17, !DIExpression(), !23)
    #dbg_value(i32 %c, !18, !DIExpression(), !23)
  %mul = mul nsw i32 %a, 3, !dbg !24
    #dbg_value(i32 %mul, !19, !DIExpression(), !23)
  %xor = xor i32 %c, %b, !dbg !25
    #dbg_value(i32 %xor, !20, !DIExpression(), !23)
    #dbg_value(!DIArgList(i32 %mul, i32 %xor), !21, !DIExpression(DW_OP_LLVM_arg, 0, DW_OP_LLVM_arg, 1, DW_OP_plus, DW_OP_stack_value), !23)
  tail call void @use(i32 noundef %mul) #2, !dbg !26
  tail call void @use(i32 noundef %xor) #2, !dbg !27
    #dbg_value(!DIArgList(i32 %xor), !22, !DIExpression(DW_OP_LLVM_arg, 0, DW_OP_constu, 3, DW_OP_shl, DW_OP_consts, 5, DW_OP_minus, DW_OP_stack_value), !23)
  tail call void @use(i32 noundef 1) #2, !dbg !28
  %add = shl i32 %a, 1, !dbg !29
  %sub = add i32 %add, %c, !dbg !30
  %add1 = add i32 %sub, %xor, !dbg !31
  ret i32 %add1, !dbg !32
}

declare !dbg !33 void @use(i32 noundef) local_unnamed_addr #1

attributes #0 = { nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i686" "target-features"="+cmov,+cx8,+x87" "tune-cpu"="generic" }
attributes #1 = { "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i686" "target-features"="+cmov,+cx8,+x87" "tune-cpu"="generic" }
attributes #2 = { nounwind }

!llvm.dbg.cu = !{!0}
!llvm.module.flags = !{!2, !3, !4, !5, !6, !7, !8, !9}
!llvm.ident = !{!10}

!0 = distinct !DICompileUnit(language: DW_LANG_C11, file: !1, producer: "Ubuntu clang version 20.1.8 (0ubuntu4)", isOptimized: true, runtimeVersion: 0, emissionKind: FullDebug, splitDebugInlining: false, nameTableKind: None)
!1 = !DIFile(filename: "al.c", directory: "/tmp/claude-1000/dx", checksumkind: CSK_MD5, checksum: "7636b6fcee8f2265a170007025967690")
!2 = !{i32 1, !"NumRegisterParameters", i32 0}
!3 = !{i32 7, !"Dwarf Version", i32 5}
!4 = !{i32 2, !"Debug Info Version", i32 3}
!5 = !{i32 1, !"wchar_size", i32 4}
!6 = !{i32 8, !"PIC Level", i32 2}
!7 = !{i32 7, !"PIE Level", i32 2}
!8 = !{i32 7, !"uwtable", i32 2}
!9 = !{i32 7, !"debug-info-assignment-tracking", i1 true}
!10 = !{!"Ubuntu clang version 20.1.8 (0ubuntu4)"}
!11 = distinct !DISubprogram(name: "f", scope: !1, file: !1, line: 2, type: !12, scopeLine: 2, flags: DIFlagPrototyped | DIFlagAllCallsDescribed, spFlags: DISPFlagDefinition | DISPFlagOptimized, unit: !0, retainedNodes: !15)
!12 = !DISubroutineType(types: !13)
!13 = !{!14, !14, !14, !14}
!14 = !DIBasicType(name: "int", size: 32, encoding: DW_ATE_signed)
!15 = !{!16, !17, !18, !19, !20, !21, !22}
!16 = !DILocalVariable(name: "a", arg: 1, scope: !11, file: !1, line: 2, type: !14)
!17 = !DILocalVariable(name: "b", arg: 2, scope: !11, file: !1, line: 2, type: !14)
!18 = !DILocalVariable(name: "c", arg: 3, scope: !11, file: !1, line: 2, type: !14)
!19 = !DILocalVariable(name: "s", scope: !11, file: !1, line: 3, type: !14)
!20 = !DILocalVariable(name: "t", scope: !11, file: !1, line: 4, type: !14)
!21 = !DILocalVariable(name: "d", scope: !11, file: !1, line: 5, type: !14)
!22 = !DILocalVariable(name: "e", scope: !11, file: !1, line: 8, type: !14)
!23 = !DILocation(line: 0, scope: !11)
!24 = !DILocation(line: 3, column: 13, scope: !11)
!25 = !DILocation(line: 4, column: 13, scope: !11)
!26 = !DILocation(line: 6, column: 3, scope: !11)
!27 = !DILocation(line: 7, column: 3, scope: !11)
!28 = !DILocation(line: 9, column: 3, scope: !11)
!29 = !DILocation(line: 5, column: 13, scope: !11)
!30 = !DILocation(line: 8, column: 13, scope: !11)
!31 = !DILocation(line: 10, column: 12, scope: !11)
!32 = !DILocation(line: 10, column: 3, scope: !11)
!33 = !DISubprogram(name: "use", scope: !1, file: !1, line: 1, type: !34, flags: DIFlagPrototyped, spFlags: DISPFlagOptimized)
!34 = !DISubroutineType(types: !35)
!35 = !{null, !14}
