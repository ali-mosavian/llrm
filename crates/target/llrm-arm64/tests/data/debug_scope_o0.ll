; ModuleID = 'phi.c'
source_filename = "phi.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

; Function Attrs: noinline nounwind optnone uwtable
define dso_local i32 @f(i32 noundef %0, i32 noundef %1) #0 !dbg !15 {
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  %5 = alloca i32, align 4
  %6 = alloca i32, align 4
  store i32 %0, ptr %3, align 4
    #dbg_declare(ptr %3, !20, !DIExpression(), !21)
  store i32 %1, ptr %4, align 4
    #dbg_declare(ptr %4, !22, !DIExpression(), !23)
    #dbg_declare(ptr %5, !24, !DIExpression(), !25)
  store i32 0, ptr %5, align 4, !dbg !25
    #dbg_declare(ptr %6, !26, !DIExpression(), !28)
  store i32 0, ptr %6, align 4, !dbg !28
  br label %7, !dbg !29

7:                                                ; preds = %24, %2
  %8 = load i32, ptr %6, align 4, !dbg !30
  %9 = load i32, ptr %3, align 4, !dbg !32
  %10 = icmp slt i32 %8, %9, !dbg !33
  br i1 %10, label %11, label %27, !dbg !34

11:                                               ; preds = %7
  %12 = load i32, ptr %6, align 4, !dbg !35
  %13 = and i32 %12, 1, !dbg !38
  %14 = icmp ne i32 %13, 0, !dbg !38
  br i1 %14, label %15, label %19, !dbg !38

15:                                               ; preds = %11
  %16 = load i32, ptr %6, align 4, !dbg !39
  %17 = load i32, ptr %5, align 4, !dbg !40
  %18 = sub nsw i32 %17, %16, !dbg !40
  store i32 %18, ptr %5, align 4, !dbg !40
  br label %23, !dbg !41

19:                                               ; preds = %11
  %20 = load i32, ptr %4, align 4, !dbg !42
  %21 = load i32, ptr %5, align 4, !dbg !43
  %22 = add nsw i32 %21, %20, !dbg !43
  store i32 %22, ptr %5, align 4, !dbg !43
  br label %23

23:                                               ; preds = %19, %15
  br label %24, !dbg !44

24:                                               ; preds = %23
  %25 = load i32, ptr %6, align 4, !dbg !45
  %26 = add nsw i32 %25, 1, !dbg !45
  store i32 %26, ptr %6, align 4, !dbg !45
  br label %7, !dbg !46, !llvm.loop !47

27:                                               ; preds = %7
  %28 = load i32, ptr %5, align 4, !dbg !50
  ret i32 %28, !dbg !51
}

attributes #0 = { noinline nounwind optnone uwtable "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }

!llvm.dbg.cu = !{!0}
!llvm.module.flags = !{!2, !3, !4, !5, !6, !7, !8, !9, !10, !11, !12, !13}
!llvm.ident = !{!14}

!0 = distinct !DICompileUnit(language: DW_LANG_C11, file: !1, producer: "clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)", isOptimized: false, runtimeVersion: 0, emissionKind: FullDebug, splitDebugInlining: false, nameTableKind: None)
!1 = !DIFile(filename: "phi.c", directory: "/src", checksumkind: CSK_MD5, checksum: "e2290d0992703b321234c26964347455")
!2 = !{i32 7, !"Dwarf Version", i32 5}
!3 = !{i32 2, !"Debug Info Version", i32 3}
!4 = !{i32 1, !"ptrauth-elf-got", i32 0}
!5 = !{i32 1, !"ptrauth-init-fini", i32 0}
!6 = !{i32 1, !"ptrauth-init-fini-address-discrimination", i32 0}
!7 = !{i32 1, !"ptrauth-sign-personality", i32 0}
!8 = !{i32 1, !"aarch64-elf-pauthabi-platform", i32 268435458}
!9 = !{i32 1, !"aarch64-elf-pauthabi-version", i32 0}
!10 = !{i32 8, !"PIC Level", i32 2}
!11 = !{i32 7, !"PIE Level", i32 2}
!12 = !{i32 7, !"uwtable", i32 2}
!13 = !{i32 7, !"frame-pointer", i32 4}
!14 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!15 = distinct !DISubprogram(name: "f", scope: !1, file: !1, line: 1, type: !16, scopeLine: 2, flags: DIFlagPrototyped, spFlags: DISPFlagDefinition, unit: !0, retainedNodes: !19)
!16 = !DISubroutineType(types: !17)
!17 = !{!18, !18, !18}
!18 = !DIBasicType(name: "int", size: 32, encoding: DW_ATE_signed)
!19 = !{}
!20 = !DILocalVariable(name: "n", arg: 1, scope: !15, file: !1, line: 1, type: !18)
!21 = !DILocation(line: 1, column: 11, scope: !15)
!22 = !DILocalVariable(name: "k", arg: 2, scope: !15, file: !1, line: 1, type: !18)
!23 = !DILocation(line: 1, column: 18, scope: !15)
!24 = !DILocalVariable(name: "s", scope: !15, file: !1, line: 3, type: !18)
!25 = !DILocation(line: 3, column: 9, scope: !15)
!26 = !DILocalVariable(name: "i", scope: !27, file: !1, line: 4, type: !18)
!27 = distinct !DILexicalBlock(scope: !15, file: !1, line: 4, column: 5)
!28 = !DILocation(line: 4, column: 14, scope: !27)
!29 = !DILocation(line: 4, column: 10, scope: !27)
!30 = !DILocation(line: 4, column: 21, scope: !31)
!31 = distinct !DILexicalBlock(scope: !27, file: !1, line: 4, column: 5)
!32 = !DILocation(line: 4, column: 25, scope: !31)
!33 = !DILocation(line: 4, column: 23, scope: !31)
!34 = !DILocation(line: 4, column: 5, scope: !27)
!35 = !DILocation(line: 5, column: 13, scope: !36)
!36 = distinct !DILexicalBlock(scope: !37, file: !1, line: 5, column: 13)
!37 = distinct !DILexicalBlock(scope: !31, file: !1, line: 4, column: 33)
!38 = !DILocation(line: 5, column: 15, scope: !36)
!39 = !DILocation(line: 6, column: 18, scope: !36)
!40 = !DILocation(line: 6, column: 15, scope: !36)
!41 = !DILocation(line: 6, column: 13, scope: !36)
!42 = !DILocation(line: 8, column: 18, scope: !36)
!43 = !DILocation(line: 8, column: 15, scope: !36)
!44 = !DILocation(line: 9, column: 5, scope: !37)
!45 = !DILocation(line: 4, column: 29, scope: !31)
!46 = !DILocation(line: 4, column: 5, scope: !31)
!47 = distinct !{!47, !34, !48, !49}
!48 = !DILocation(line: 9, column: 5, scope: !27)
!49 = !{!"llvm.loop.mustprogress"}
!50 = !DILocation(line: 10, column: 12, scope: !15)
!51 = !DILocation(line: 10, column: 5, scope: !15)
