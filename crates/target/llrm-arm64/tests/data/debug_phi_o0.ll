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
    #dbg_declare(ptr %3, !21, !DIExpression(), !22)
  store i32 %1, ptr %4, align 4
    #dbg_declare(ptr %4, !23, !DIExpression(), !24)
    #dbg_declare(ptr %5, !25, !DIExpression(), !26)
  store i32 0, ptr %5, align 4, !dbg !26
    #dbg_declare(ptr %6, !27, !DIExpression(), !29)
  store i32 0, ptr %6, align 4, !dbg !29
  br label %7, !dbg !30

7:                                                ; preds = %24, %2
  %8 = load i32, ptr %6, align 4, !dbg !31
  %9 = load i32, ptr %3, align 4, !dbg !33
  %10 = icmp slt i32 %8, %9, !dbg !34
  br i1 %10, label %11, label %27, !dbg !35

11:                                               ; preds = %7
  %12 = load i32, ptr %6, align 4, !dbg !36
  %13 = and i32 %12, 1, !dbg !39
  %14 = icmp ne i32 %13, 0, !dbg !39
  br i1 %14, label %15, label %19, !dbg !39

15:                                               ; preds = %11
  %16 = load i32, ptr %4, align 4, !dbg !40
  %17 = load i32, ptr %5, align 4, !dbg !41
  %18 = add nsw i32 %17, %16, !dbg !41
  store i32 %18, ptr %5, align 4, !dbg !41
  br label %23, !dbg !42

19:                                               ; preds = %11
  %20 = load i32, ptr %6, align 4, !dbg !43
  %21 = load i32, ptr %5, align 4, !dbg !44
  %22 = sub nsw i32 %21, %20, !dbg !44
  store i32 %22, ptr %5, align 4, !dbg !44
  br label %23

23:                                               ; preds = %19, %15
  br label %24, !dbg !45

24:                                               ; preds = %23
  %25 = load i32, ptr %6, align 4, !dbg !46
  %26 = add nsw i32 %25, 1, !dbg !46
  store i32 %26, ptr %6, align 4, !dbg !46
  br label %7, !dbg !47, !llvm.loop !48

27:                                               ; preds = %7
  %28 = load i32, ptr %5, align 4, !dbg !51
  ret i32 %28, !dbg !52
}

attributes #0 = { noinline nounwind optnone uwtable "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }

!llvm.dbg.cu = !{!0}
!llvm.module.flags = !{!2, !3, !4, !5, !6, !7, !8, !9, !10, !11, !12, !13}
!llvm.ident = !{!14}

!0 = distinct !DICompileUnit(language: DW_LANG_C11, file: !1, producer: "clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)", isOptimized: false, runtimeVersion: 0, emissionKind: FullDebug, splitDebugInlining: false, nameTableKind: None)
!1 = !DIFile(filename: "phi.c", directory: "/src", checksumkind: CSK_MD5, checksum: "29ebc02ebdf09a72665abbcd92166292")
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
!15 = distinct !DISubprogram(name: "f", scope: !16, file: !16, line: 1, type: !17, scopeLine: 1, flags: DIFlagPrototyped, spFlags: DISPFlagDefinition, unit: !0, retainedNodes: !20)
!16 = !DIFile(filename: "phi.c", directory: "", checksumkind: CSK_MD5, checksum: "29ebc02ebdf09a72665abbcd92166292")
!17 = !DISubroutineType(types: !18)
!18 = !{!19, !19, !19}
!19 = !DIBasicType(name: "int", size: 32, encoding: DW_ATE_signed)
!20 = !{}
!21 = !DILocalVariable(name: "n", arg: 1, scope: !15, file: !16, line: 1, type: !19)
!22 = !DILocation(line: 1, column: 11, scope: !15)
!23 = !DILocalVariable(name: "a", arg: 2, scope: !15, file: !16, line: 1, type: !19)
!24 = !DILocation(line: 1, column: 18, scope: !15)
!25 = !DILocalVariable(name: "s", scope: !15, file: !16, line: 2, type: !19)
!26 = !DILocation(line: 2, column: 7, scope: !15)
!27 = !DILocalVariable(name: "i", scope: !28, file: !16, line: 3, type: !19)
!28 = distinct !DILexicalBlock(scope: !15, file: !16, line: 3, column: 3)
!29 = !DILocation(line: 3, column: 12, scope: !28)
!30 = !DILocation(line: 3, column: 8, scope: !28)
!31 = !DILocation(line: 3, column: 19, scope: !32)
!32 = distinct !DILexicalBlock(scope: !28, file: !16, line: 3, column: 3)
!33 = !DILocation(line: 3, column: 23, scope: !32)
!34 = !DILocation(line: 3, column: 21, scope: !32)
!35 = !DILocation(line: 3, column: 3, scope: !28)
!36 = !DILocation(line: 4, column: 9, scope: !37)
!37 = distinct !DILexicalBlock(scope: !38, file: !16, line: 4, column: 9)
!38 = distinct !DILexicalBlock(scope: !32, file: !16, line: 3, column: 31)
!39 = !DILocation(line: 4, column: 11, scope: !37)
!40 = !DILocation(line: 5, column: 12, scope: !37)
!41 = !DILocation(line: 5, column: 9, scope: !37)
!42 = !DILocation(line: 5, column: 7, scope: !37)
!43 = !DILocation(line: 7, column: 12, scope: !37)
!44 = !DILocation(line: 7, column: 9, scope: !37)
!45 = !DILocation(line: 8, column: 3, scope: !38)
!46 = !DILocation(line: 3, column: 27, scope: !32)
!47 = !DILocation(line: 3, column: 3, scope: !32)
!48 = distinct !{!48, !35, !49, !50}
!49 = !DILocation(line: 8, column: 3, scope: !28)
!50 = !{!"llvm.loop.mustprogress"}
!51 = !DILocation(line: 9, column: 10, scope: !15)
!52 = !DILocation(line: 9, column: 3, scope: !15)
