; ModuleID = 'sw.c'
source_filename = "phi.c"
target datalayout = "@m32-datalayout@"
target triple = "i686-unknown-linux-gnu"

; Function Attrs: nounwind uwtable
define dso_local void @f(i32 noundef %0) local_unnamed_addr #0 !dbg !11 {
    #dbg_value(i32 %0, !16, !DIExpression(), !17)
  switch i32 %0, label %10 [
    i32 0, label %2
    i32 1, label %3
    i32 2, label %4
    i32 3, label %5
    i32 4, label %6
    i32 5, label %7
    i32 6, label %8
    i32 7, label %9
  ], !dbg !18

2:                                                ; preds = %1
  tail call void @a(i32 noundef 3) #2, !dbg !19
  br label %10, !dbg !21

3:                                                ; preds = %1
  tail call void @b() #2, !dbg !22
  br label %10, !dbg !23

4:                                                ; preds = %1
  tail call void @a(i32 noundef 17) #2, !dbg !24
  br label %10, !dbg !25

5:                                                ; preds = %1
  tail call void @b() #2, !dbg !26
  tail call void @a(i32 noundef 1) #2, !dbg !27
  br label %10, !dbg !28

6:                                                ; preds = %1
  tail call void @a(i32 noundef 31) #2, !dbg !29
  br label %10, !dbg !30

7:                                                ; preds = %1
  tail call void @b() #2, !dbg !31
  br label %10, !dbg !32

8:                                                ; preds = %1
  tail call void @a(i32 noundef 45) #2, !dbg !33
  br label %10, !dbg !34

9:                                                ; preds = %1
  tail call void @a(i32 noundef 52) #2, !dbg !35
  tail call void @b() #2, !dbg !36
  br label %10, !dbg !37

10:                                               ; preds = %1, %9, %8, %7, %6, %5, %4, %3, %2
  ret void, !dbg !38
}

declare !dbg !39 void @a(i32 noundef) local_unnamed_addr #1

declare !dbg !40 void @b() local_unnamed_addr #1

attributes #0 = { nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i686" "target-features"="+cmov,+cx8,+x87" "tune-cpu"="generic" }
attributes #1 = { "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i686" "target-features"="+cmov,+cx8,+x87" "tune-cpu"="generic" }
attributes #2 = { nounwind }

!llvm.dbg.cu = !{!0}
!llvm.module.flags = !{!2, !3, !4, !5, !6, !7, !8, !9}
!llvm.ident = !{!10}

!0 = distinct !DICompileUnit(language: DW_LANG_C11, file: !1, producer: "Ubuntu clang version 20.1.8 (0ubuntu4)", isOptimized: true, runtimeVersion: 0, emissionKind: FullDebug, splitDebugInlining: false, nameTableKind: None)
!1 = !DIFile(filename: "phi.c", directory: "/src", checksumkind: CSK_MD5, checksum: "f5be3ad62ad43f5b4665dfb14302df80")
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
!13 = !{null, !14}
!14 = !DIBasicType(name: "int", size: 32, encoding: DW_ATE_signed)
!15 = !{!16}
!16 = !DILocalVariable(name: "x", arg: 1, scope: !11, file: !1, line: 2, type: !14)
!17 = !DILocation(line: 0, scope: !11)
!18 = !DILocation(line: 3, column: 3, scope: !11)
!19 = !DILocation(line: 4, column: 11, scope: !20)
!20 = distinct !DILexicalBlock(scope: !11, file: !1, line: 3, column: 14)
!21 = !DILocation(line: 4, column: 17, scope: !20)
!22 = !DILocation(line: 5, column: 11, scope: !20)
!23 = !DILocation(line: 5, column: 16, scope: !20)
!24 = !DILocation(line: 6, column: 11, scope: !20)
!25 = !DILocation(line: 6, column: 18, scope: !20)
!26 = !DILocation(line: 7, column: 11, scope: !20)
!27 = !DILocation(line: 7, column: 16, scope: !20)
!28 = !DILocation(line: 7, column: 22, scope: !20)
!29 = !DILocation(line: 8, column: 11, scope: !20)
!30 = !DILocation(line: 8, column: 18, scope: !20)
!31 = !DILocation(line: 9, column: 11, scope: !20)
!32 = !DILocation(line: 9, column: 16, scope: !20)
!33 = !DILocation(line: 10, column: 11, scope: !20)
!34 = !DILocation(line: 10, column: 18, scope: !20)
!35 = !DILocation(line: 11, column: 11, scope: !20)
!36 = !DILocation(line: 11, column: 18, scope: !20)
!37 = !DILocation(line: 11, column: 23, scope: !20)
!38 = !DILocation(line: 13, column: 1, scope: !11)
!39 = !DISubprogram(name: "a", scope: !1, file: !1, line: 1, type: !12, flags: DIFlagPrototyped, spFlags: DISPFlagOptimized)
!40 = !DISubprogram(name: "b", scope: !1, file: !1, line: 1, type: !41, flags: DIFlagPrototyped, spFlags: DISPFlagOptimized)
!41 = !DISubroutineType(types: !42)
!42 = !{null}
