; ModuleID = 'phi.c'
source_filename = "phi.c"
target datalayout = "@m32-datalayout@"

; Function Attrs: noinline nounwind optnone uwtable
define dso_local i32 @f(i32 noundef %0, i32 noundef %1) #0 !dbg !8 {
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  %5 = alloca i32, align 4
  %6 = alloca i32, align 4
  store i32 %0, ptr %3, align 4
    #dbg_declare(ptr %3, !13, !DIExpression(), !14)
  store i32 %1, ptr %4, align 4
    #dbg_declare(ptr %4, !15, !DIExpression(), !16)
    #dbg_declare(ptr %5, !17, !DIExpression(), !18)
  store i32 0, ptr %5, align 4, !dbg !18
    #dbg_declare(ptr %6, !19, !DIExpression(), !21)
  store i32 0, ptr %6, align 4, !dbg !21
  br label %7, !dbg !22

7:                                                ; preds = %24, %2
  %8 = load i32, ptr %6, align 4, !dbg !23
  %9 = load i32, ptr %3, align 4, !dbg !25
  %10 = icmp slt i32 %8, %9, !dbg !26
  br i1 %10, label %11, label %27, !dbg !27

11:                                               ; preds = %7
  %12 = load i32, ptr %6, align 4, !dbg !28
  %13 = and i32 %12, 1, !dbg !31
  %14 = icmp ne i32 %13, 0, !dbg !31
  br i1 %14, label %15, label %19, !dbg !31

15:                                               ; preds = %11
  %16 = load i32, ptr %6, align 4, !dbg !32
  %17 = load i32, ptr %5, align 4, !dbg !33
  %18 = sub nsw i32 %17, %16, !dbg !33
  store i32 %18, ptr %5, align 4, !dbg !33
  br label %23, !dbg !34

19:                                               ; preds = %11
  %20 = load i32, ptr %4, align 4, !dbg !35
  %21 = load i32, ptr %5, align 4, !dbg !36
  %22 = add nsw i32 %21, %20, !dbg !36
  store i32 %22, ptr %5, align 4, !dbg !36
  br label %23

23:                                               ; preds = %19, %15
  br label %24, !dbg !37

24:                                               ; preds = %23
  %25 = load i32, ptr %6, align 4, !dbg !38
  %26 = add nsw i32 %25, 1, !dbg !38
  store i32 %26, ptr %6, align 4, !dbg !38
  br label %7, !dbg !39, !llvm.loop !40

27:                                               ; preds = %7
  %28 = load i32, ptr %5, align 4, !dbg !43
  ret i32 %28, !dbg !44
}

attributes #0 = { noinline nounwind optnone uwtable "frame-pointer"="all" "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="pentium4" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" }

!llvm.dbg.cu = !{!0}
!llvm.module.flags = !{!2, !3, !4, !5, !6}
!llvm.ident = !{!7}

!0 = distinct !DICompileUnit(language: DW_LANG_C11, file: !1, producer: "clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)", isOptimized: false, runtimeVersion: 0, emissionKind: FullDebug, splitDebugInlining: false, nameTableKind: None)
!1 = !DIFile(filename: "phi.c", directory: "/src", checksumkind: CSK_MD5, checksum: "e2290d0992703b321234c26964347455")
!2 = !{i32 1, !"NumRegisterParameters", i32 0}
!3 = !{i32 7, !"Dwarf Version", i32 5}
!4 = !{i32 2, !"Debug Info Version", i32 3}
!5 = !{i32 7, !"uwtable", i32 2}
!6 = !{i32 7, !"frame-pointer", i32 2}
!7 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!8 = distinct !DISubprogram(name: "f", scope: !1, file: !1, line: 1, type: !9, scopeLine: 2, flags: DIFlagPrototyped, spFlags: DISPFlagDefinition, unit: !0, retainedNodes: !12)
!9 = !DISubroutineType(types: !10)
!10 = !{!11, !11, !11}
!11 = !DIBasicType(name: "int", size: 32, encoding: DW_ATE_signed)
!12 = !{}
!13 = !DILocalVariable(name: "n", arg: 1, scope: !8, file: !1, line: 1, type: !11)
!14 = !DILocation(line: 1, column: 11, scope: !8)
!15 = !DILocalVariable(name: "k", arg: 2, scope: !8, file: !1, line: 1, type: !11)
!16 = !DILocation(line: 1, column: 18, scope: !8)
!17 = !DILocalVariable(name: "s", scope: !8, file: !1, line: 3, type: !11)
!18 = !DILocation(line: 3, column: 9, scope: !8)
!19 = !DILocalVariable(name: "i", scope: !20, file: !1, line: 4, type: !11)
!20 = distinct !DILexicalBlock(scope: !8, file: !1, line: 4, column: 5)
!21 = !DILocation(line: 4, column: 14, scope: !20)
!22 = !DILocation(line: 4, column: 10, scope: !20)
!23 = !DILocation(line: 4, column: 21, scope: !24)
!24 = distinct !DILexicalBlock(scope: !20, file: !1, line: 4, column: 5)
!25 = !DILocation(line: 4, column: 25, scope: !24)
!26 = !DILocation(line: 4, column: 23, scope: !24)
!27 = !DILocation(line: 4, column: 5, scope: !20)
!28 = !DILocation(line: 5, column: 13, scope: !29)
!29 = distinct !DILexicalBlock(scope: !30, file: !1, line: 5, column: 13)
!30 = distinct !DILexicalBlock(scope: !24, file: !1, line: 4, column: 33)
!31 = !DILocation(line: 5, column: 15, scope: !29)
!32 = !DILocation(line: 6, column: 18, scope: !29)
!33 = !DILocation(line: 6, column: 15, scope: !29)
!34 = !DILocation(line: 6, column: 13, scope: !29)
!35 = !DILocation(line: 8, column: 18, scope: !29)
!36 = !DILocation(line: 8, column: 15, scope: !29)
!37 = !DILocation(line: 9, column: 5, scope: !30)
!38 = !DILocation(line: 4, column: 29, scope: !24)
!39 = !DILocation(line: 4, column: 5, scope: !24)
!40 = distinct !{!40, !27, !41, !42}
!41 = !DILocation(line: 9, column: 5, scope: !20)
!42 = !{!"llvm.loop.mustprogress"}
!43 = !DILocation(line: 10, column: 12, scope: !8)
!44 = !DILocation(line: 10, column: 5, scope: !8)
