; ModuleID = 'sw.c'
source_filename = "sw.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

; Function Attrs: nounwind uwtable
define dso_local void @f(i32 noundef %0) local_unnamed_addr #0 !dbg !11 {
    #dbg_value(i32 %0, !17, !DIExpression(), !18)
  switch i32 %0, label %10 [
    i32 0, label %2
    i32 1, label %3
    i32 2, label %4
    i32 3, label %5
    i32 4, label %6
    i32 5, label %7
    i32 6, label %8
    i32 7, label %9
  ], !dbg !19

2:                                                ; preds = %1
  tail call void @a(i32 noundef 3) #2, !dbg !20
  br label %10, !dbg !22

3:                                                ; preds = %1
  tail call void @b() #2, !dbg !23
  br label %10, !dbg !24

4:                                                ; preds = %1
  tail call void @a(i32 noundef 17) #2, !dbg !25
  br label %10, !dbg !26

5:                                                ; preds = %1
  tail call void @b() #2, !dbg !27
  tail call void @a(i32 noundef 1) #2, !dbg !28
  br label %10, !dbg !29

6:                                                ; preds = %1
  tail call void @a(i32 noundef 31) #2, !dbg !30
  br label %10, !dbg !31

7:                                                ; preds = %1
  tail call void @b() #2, !dbg !32
  br label %10, !dbg !33

8:                                                ; preds = %1
  tail call void @a(i32 noundef 45) #2, !dbg !34
  br label %10, !dbg !35

9:                                                ; preds = %1
  tail call void @a(i32 noundef 52) #2, !dbg !36
  tail call void @b() #2, !dbg !37
  br label %10, !dbg !38

10:                                               ; preds = %1, %9, %8, %7, %6, %5, %4, %3, %2
  ret void, !dbg !39
}

declare !dbg !40 void @a(i32 noundef) local_unnamed_addr #1

declare !dbg !41 void @b() local_unnamed_addr #1

attributes #0 = { nounwind uwtable "frame-pointer"="non-leaf" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #1 = { "frame-pointer"="non-leaf" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #2 = { nounwind }

!llvm.dbg.cu = !{!0}
!llvm.module.flags = !{!2, !3, !4, !5, !6, !7, !8, !9}
!llvm.ident = !{!10}

!0 = distinct !DICompileUnit(language: DW_LANG_C11, file: !1, producer: "Ubuntu clang version 20.1.8 (0ubuntu4)", isOptimized: true, runtimeVersion: 0, emissionKind: FullDebug, splitDebugInlining: false, nameTableKind: None)
!1 = !DIFile(filename: "sw.c", directory: "/src", checksumkind: CSK_MD5, checksum: "f5be3ad62ad43f5b4665dfb14302df80")
!2 = !{i32 7, !"Dwarf Version", i32 5}
!3 = !{i32 2, !"Debug Info Version", i32 3}
!4 = !{i32 1, !"wchar_size", i32 4}
!5 = !{i32 8, !"PIC Level", i32 2}
!6 = !{i32 7, !"PIE Level", i32 2}
!7 = !{i32 7, !"uwtable", i32 2}
!8 = !{i32 7, !"frame-pointer", i32 1}
!9 = !{i32 7, !"debug-info-assignment-tracking", i1 true}
!10 = !{!"Ubuntu clang version 20.1.8 (0ubuntu4)"}
!11 = distinct !DISubprogram(name: "f", scope: !12, file: !12, line: 2, type: !13, scopeLine: 2, flags: DIFlagPrototyped | DIFlagAllCallsDescribed, spFlags: DISPFlagDefinition | DISPFlagOptimized, unit: !0, retainedNodes: !16)
!12 = !DIFile(filename: "sw.c", directory: "/src", checksumkind: CSK_MD5, checksum: "f5be3ad62ad43f5b4665dfb14302df80")
!13 = !DISubroutineType(types: !14)
!14 = !{null, !15}
!15 = !DIBasicType(name: "int", size: 32, encoding: DW_ATE_signed)
!16 = !{!17}
!17 = !DILocalVariable(name: "x", arg: 1, scope: !11, file: !12, line: 2, type: !15)
!18 = !DILocation(line: 0, scope: !11)
!19 = !DILocation(line: 3, column: 3, scope: !11)
!20 = !DILocation(line: 4, column: 11, scope: !21)
!21 = distinct !DILexicalBlock(scope: !11, file: !12, line: 3, column: 14)
!22 = !DILocation(line: 4, column: 17, scope: !21)
!23 = !DILocation(line: 5, column: 11, scope: !21)
!24 = !DILocation(line: 5, column: 16, scope: !21)
!25 = !DILocation(line: 6, column: 11, scope: !21)
!26 = !DILocation(line: 6, column: 18, scope: !21)
!27 = !DILocation(line: 7, column: 11, scope: !21)
!28 = !DILocation(line: 7, column: 16, scope: !21)
!29 = !DILocation(line: 7, column: 22, scope: !21)
!30 = !DILocation(line: 8, column: 11, scope: !21)
!31 = !DILocation(line: 8, column: 18, scope: !21)
!32 = !DILocation(line: 9, column: 11, scope: !21)
!33 = !DILocation(line: 9, column: 16, scope: !21)
!34 = !DILocation(line: 10, column: 11, scope: !21)
!35 = !DILocation(line: 10, column: 18, scope: !21)
!36 = !DILocation(line: 11, column: 11, scope: !21)
!37 = !DILocation(line: 11, column: 18, scope: !21)
!38 = !DILocation(line: 11, column: 23, scope: !21)
!39 = !DILocation(line: 13, column: 1, scope: !11)
!40 = !DISubprogram(name: "a", scope: !12, file: !12, line: 1, type: !13, flags: DIFlagPrototyped, spFlags: DISPFlagOptimized)
!41 = !DISubprogram(name: "b", scope: !12, file: !12, line: 1, type: !42, flags: DIFlagPrototyped, spFlags: DISPFlagOptimized)
!42 = !DISubroutineType(types: !43)
!43 = !{null}
