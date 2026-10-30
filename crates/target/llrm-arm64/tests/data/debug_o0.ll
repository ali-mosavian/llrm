; ModuleID = 'a64.c'
source_filename = "a64.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

; Function Attrs: noinline nounwind optnone uwtable
define dso_local i32 @f(i32 noundef %0, i32 noundef %1) #0 !dbg !10 {
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  %5 = alloca i32, align 4
  store i32 %0, ptr %3, align 4
    #dbg_declare(ptr %3, !15, !DIExpression(), !16)
  store i32 %1, ptr %4, align 4
    #dbg_declare(ptr %4, !17, !DIExpression(), !18)
    #dbg_declare(ptr %5, !19, !DIExpression(), !20)
  %6 = load i32, ptr %3, align 4, !dbg !21
  %7 = call i32 @g(i32 noundef %6), !dbg !22
  %8 = load i32, ptr %4, align 4, !dbg !23
  %9 = add nsw i32 %7, %8, !dbg !24
  store i32 %9, ptr %5, align 4, !dbg !20
  %10 = load i32, ptr %5, align 4, !dbg !25
  %11 = mul nsw i32 %10, 2, !dbg !26
  ret i32 %11, !dbg !27
}

declare i32 @g(i32 noundef) #1

attributes #0 = { noinline nounwind optnone uwtable "frame-pointer"="non-leaf" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+v8a,-fmv" }
attributes #1 = { "frame-pointer"="non-leaf" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+v8a,-fmv" }

!llvm.dbg.cu = !{!0}
!llvm.module.flags = !{!2, !3, !4, !5, !6, !7, !8}
!llvm.ident = !{!9}

!0 = distinct !DICompileUnit(language: DW_LANG_C11, file: !1, producer: "Ubuntu clang version 20.1.8 (0ubuntu4)", isOptimized: false, runtimeVersion: 0, emissionKind: FullDebug, splitDebugInlining: false, nameTableKind: None)
!1 = !DIFile(filename: "a64.c", directory: "/tmp/claude-1000/iv10", checksumkind: CSK_MD5, checksum: "450995d35c1ad2e2996796096c5d4639")
!2 = !{i32 7, !"Dwarf Version", i32 5}
!3 = !{i32 2, !"Debug Info Version", i32 3}
!4 = !{i32 1, !"wchar_size", i32 4}
!5 = !{i32 8, !"PIC Level", i32 2}
!6 = !{i32 7, !"PIE Level", i32 2}
!7 = !{i32 7, !"uwtable", i32 2}
!8 = !{i32 7, !"frame-pointer", i32 1}
!9 = !{!"Ubuntu clang version 20.1.8 (0ubuntu4)"}
!10 = distinct !DISubprogram(name: "f", scope: !1, file: !1, line: 2, type: !11, scopeLine: 2, flags: DIFlagPrototyped, spFlags: DISPFlagDefinition, unit: !0, retainedNodes: !14)
!11 = !DISubroutineType(types: !12)
!12 = !{!13, !13, !13}
!13 = !DIBasicType(name: "int", size: 32, encoding: DW_ATE_signed)
!14 = !{}
!15 = !DILocalVariable(name: "a", arg: 1, scope: !10, file: !1, line: 2, type: !13)
!16 = !DILocation(line: 2, column: 11, scope: !10)
!17 = !DILocalVariable(name: "b", arg: 2, scope: !10, file: !1, line: 2, type: !13)
!18 = !DILocation(line: 2, column: 18, scope: !10)
!19 = !DILocalVariable(name: "t", scope: !10, file: !1, line: 3, type: !13)
!20 = !DILocation(line: 3, column: 7, scope: !10)
!21 = !DILocation(line: 3, column: 13, scope: !10)
!22 = !DILocation(line: 3, column: 11, scope: !10)
!23 = !DILocation(line: 3, column: 18, scope: !10)
!24 = !DILocation(line: 3, column: 16, scope: !10)
!25 = !DILocation(line: 4, column: 10, scope: !10)
!26 = !DILocation(line: 4, column: 12, scope: !10)
!27 = !DILocation(line: 4, column: 3, scope: !10)
