; ModuleID = 'w.c'
source_filename = "w.c"
target datalayout = "@m32-datalayout@"

; Function Attrs: noinline nounwind optnone uwtable
define dso_local i32 @w(i32 noundef %0) #0 !dbg !8 {
  %2 = alloca i32, align 4
  %3 = alloca i32, align 4
  store i32 %0, ptr %2, align 4
    #dbg_declare(ptr %2, !13, !DIExpression(), !14)
    #dbg_declare(ptr %3, !15, !DIExpression(), !16)
  store i32 0, ptr %3, align 4, !dbg !16
  br label %4, !dbg !17

4:                                                ; preds = %8, %1
  %5 = load i32, ptr %3, align 4, !dbg !18
  %6 = load i32, ptr %2, align 4, !dbg !19
  %7 = icmp slt i32 %5, %6, !dbg !20
  br i1 %7, label %8, label %11, !dbg !17

8:                                                ; preds = %4
  %9 = load i32, ptr %3, align 4, !dbg !21
  %10 = add nsw i32 %9, 1, !dbg !21
  store i32 %10, ptr %3, align 4, !dbg !21
  br label %4, !dbg !17, !llvm.loop !22

11:                                               ; preds = %4
  %12 = load i32, ptr %3, align 4, !dbg !24
  ret i32 %12, !dbg !25
}

attributes #0 = { noinline nounwind optnone uwtable "frame-pointer"="all" "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="pentium4" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" }

!llvm.dbg.cu = !{!0}
!llvm.module.flags = !{!2, !3, !4, !5, !6}
!llvm.ident = !{!7}

!0 = distinct !DICompileUnit(language: DW_LANG_C11, file: !1, producer: "clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)", isOptimized: false, runtimeVersion: 0, emissionKind: FullDebug, splitDebugInlining: false, nameTableKind: None)
!1 = !DIFile(filename: "w.c", directory: "/src", checksumkind: CSK_MD5, checksum: "ac45dcd551c3ba8f2cc31fb45f8e5e85")
!2 = !{i32 1, !"NumRegisterParameters", i32 0}
!3 = !{i32 7, !"Dwarf Version", i32 5}
!4 = !{i32 2, !"Debug Info Version", i32 3}
!5 = !{i32 7, !"uwtable", i32 2}
!6 = !{i32 7, !"frame-pointer", i32 2}
!7 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!8 = distinct !DISubprogram(name: "w", scope: !1, file: !1, line: 1, type: !9, scopeLine: 2, flags: DIFlagPrototyped, spFlags: DISPFlagDefinition, unit: !0, retainedNodes: !12)
!9 = !DISubroutineType(types: !10)
!10 = !{!11, !11}
!11 = !DIBasicType(name: "int", size: 32, encoding: DW_ATE_signed)
!12 = !{}
!13 = !DILocalVariable(name: "n", arg: 1, scope: !8, file: !1, line: 1, type: !11)
!14 = !DILocation(line: 1, column: 11, scope: !8)
!15 = !DILocalVariable(name: "i", scope: !8, file: !1, line: 3, type: !11)
!16 = !DILocation(line: 3, column: 9, scope: !8)
!17 = !DILocation(line: 3, column: 16, scope: !8)
!18 = !DILocation(line: 3, column: 23, scope: !8)
!19 = !DILocation(line: 3, column: 27, scope: !8)
!20 = !DILocation(line: 3, column: 25, scope: !8)
!21 = !DILocation(line: 4, column: 10, scope: !8)
!22 = distinct !{!22, !17, !21, !23}
!23 = !{!"llvm.loop.mustprogress"}
!24 = !DILocation(line: 5, column: 12, scope: !8)
!25 = !DILocation(line: 5, column: 5, scope: !8)
