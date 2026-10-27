; ModuleID = 'dbg.c'
source_filename = "dbg.c"
target datalayout = "e-m:e-p:32:32-p270:32:32-p271:32:32-p272:64:64-i128:128-f64:32:64-f80:32-n8:16:32-S128"

%struct.P = type { i32, i32 }

; Function Attrs: noinline nounwind optnone uwtable
define i32 @dist(ptr noundef %0, i32 noundef %1) #0 !dbg !10 {
  %3 = alloca ptr, align 4
  %4 = alloca i32, align 4
  %5 = alloca i32, align 4
  %6 = alloca i32, align 4
  store ptr %0, ptr %3, align 4
    #dbg_declare(ptr %3, !20, !DIExpression(), !21)
  store i32 %1, ptr %4, align 4
    #dbg_declare(ptr %4, !22, !DIExpression(), !23)
    #dbg_declare(ptr %5, !24, !DIExpression(), !25)
  store i32 0, ptr %5, align 4, !dbg !25
    #dbg_declare(ptr %6, !26, !DIExpression(), !28)
  store i32 0, ptr %6, align 4, !dbg !28
  br label %7, !dbg !29

7:                                                ; preds = %22, %2
  %8 = load i32, ptr %6, align 4, !dbg !30
  %9 = load i32, ptr %4, align 4, !dbg !32
  %10 = icmp slt i32 %8, %9, !dbg !33
  br i1 %10, label %11, label %25, !dbg !34

11:                                               ; preds = %7
  %12 = load ptr, ptr %3, align 4, !dbg !35
  %13 = getelementptr inbounds nuw %struct.P, ptr %12, i32 0, i32 0, !dbg !36
  %14 = load i32, ptr %13, align 4, !dbg !36
  %15 = call i32 @sq(i32 noundef %14), !dbg !37
  %16 = load ptr, ptr %3, align 4, !dbg !38
  %17 = getelementptr inbounds nuw %struct.P, ptr %16, i32 0, i32 1, !dbg !39
  %18 = load i32, ptr %17, align 4, !dbg !39
  %19 = add nsw i32 %15, %18, !dbg !40
  %20 = load i32, ptr %5, align 4, !dbg !41
  %21 = add nsw i32 %20, %19, !dbg !41
  store i32 %21, ptr %5, align 4, !dbg !41
  br label %22, !dbg !42

22:                                               ; preds = %11
  %23 = load i32, ptr %6, align 4, !dbg !43
  %24 = add nsw i32 %23, 1, !dbg !43
  store i32 %24, ptr %6, align 4, !dbg !43
  br label %7, !dbg !44, !llvm.loop !45

25:                                               ; preds = %7
  %26 = load i32, ptr %5, align 4, !dbg !48
  ret i32 %26, !dbg !49
}

; Function Attrs: noinline nounwind optnone uwtable
define internal i32 @sq(i32 noundef %0) #0 !dbg !50 {
  %2 = alloca i32, align 4
  store i32 %0, ptr %2, align 4
    #dbg_declare(ptr %2, !53, !DIExpression(), !54)
  %3 = load i32, ptr %2, align 4, !dbg !55
  %4 = load i32, ptr %2, align 4, !dbg !56
  %5 = mul nsw i32 %3, %4, !dbg !57
  ret i32 %5, !dbg !58
}

attributes #0 = { noinline nounwind optnone uwtable "frame-pointer"="all" "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="pentium4" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" }

!llvm.dbg.cu = !{!0}
!llvm.module.flags = !{!2, !3, !4, !5, !6, !7, !8}
!llvm.ident = !{!9}

!0 = distinct !DICompileUnit(language: DW_LANG_C11, file: !1, producer: "clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)", isOptimized: false, runtimeVersion: 0, emissionKind: FullDebug, splitDebugInlining: false, nameTableKind: None)
!1 = !DIFile(filename: "dbg.c", directory: "/work", checksumkind: CSK_MD5, checksum: "edf9135163a14d7b5141ba381daf995b")
!2 = !{i32 1, !"NumRegisterParameters", i32 0}
!3 = !{i32 7, !"Dwarf Version", i32 5}
!4 = !{i32 2, !"Debug Info Version", i32 3}
!5 = !{i32 8, !"PIC Level", i32 2}
!6 = !{i32 7, !"PIE Level", i32 2}
!7 = !{i32 7, !"uwtable", i32 2}
!8 = !{i32 7, !"frame-pointer", i32 2}
!9 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!10 = distinct !DISubprogram(name: "dist", scope: !1, file: !1, line: 3, type: !11, scopeLine: 4, flags: DIFlagPrototyped, spFlags: DISPFlagDefinition, unit: !0, retainedNodes: !19)
!11 = !DISubroutineType(types: !12)
!12 = !{!13, !14, !13}
!13 = !DIBasicType(name: "int", size: 32, encoding: DW_ATE_signed)
!14 = !DIDerivedType(tag: DW_TAG_pointer_type, baseType: !15, size: 32)
!15 = distinct !DICompositeType(tag: DW_TAG_structure_type, name: "P", file: !1, line: 1, size: 64, elements: !16)
!16 = !{!17, !18}
!17 = !DIDerivedType(tag: DW_TAG_member, name: "x", scope: !15, file: !1, line: 1, baseType: !13, size: 32)
!18 = !DIDerivedType(tag: DW_TAG_member, name: "y", scope: !15, file: !1, line: 1, baseType: !13, size: 32, offset: 32)
!19 = !{}
!20 = !DILocalVariable(name: "p", arg: 1, scope: !10, file: !1, line: 3, type: !14)
!21 = !DILocation(line: 3, column: 20, scope: !10)
!22 = !DILocalVariable(name: "k", arg: 2, scope: !10, file: !1, line: 3, type: !13)
!23 = !DILocation(line: 3, column: 27, scope: !10)
!24 = !DILocalVariable(name: "s", scope: !10, file: !1, line: 5, type: !13)
!25 = !DILocation(line: 5, column: 9, scope: !10)
!26 = !DILocalVariable(name: "i", scope: !27, file: !1, line: 6, type: !13)
!27 = distinct !DILexicalBlock(scope: !10, file: !1, line: 6, column: 5)
!28 = !DILocation(line: 6, column: 14, scope: !27)
!29 = !DILocation(line: 6, column: 10, scope: !27)
!30 = !DILocation(line: 6, column: 21, scope: !31)
!31 = distinct !DILexicalBlock(scope: !27, file: !1, line: 6, column: 5)
!32 = !DILocation(line: 6, column: 25, scope: !31)
!33 = !DILocation(line: 6, column: 23, scope: !31)
!34 = !DILocation(line: 6, column: 5, scope: !27)
!35 = !DILocation(line: 7, column: 17, scope: !31)
!36 = !DILocation(line: 7, column: 20, scope: !31)
!37 = !DILocation(line: 7, column: 14, scope: !31)
!38 = !DILocation(line: 7, column: 25, scope: !31)
!39 = !DILocation(line: 7, column: 28, scope: !31)
!40 = !DILocation(line: 7, column: 23, scope: !31)
!41 = !DILocation(line: 7, column: 11, scope: !31)
!42 = !DILocation(line: 7, column: 9, scope: !31)
!43 = !DILocation(line: 6, column: 29, scope: !31)
!44 = !DILocation(line: 6, column: 5, scope: !31)
!45 = distinct !{!45, !34, !46, !47}
!46 = !DILocation(line: 7, column: 28, scope: !27)
!47 = !{!"llvm.loop.mustprogress"}
!48 = !DILocation(line: 8, column: 12, scope: !10)
!49 = !DILocation(line: 8, column: 5, scope: !10)
!50 = distinct !DISubprogram(name: "sq", scope: !1, file: !1, line: 2, type: !51, scopeLine: 2, flags: DIFlagPrototyped, spFlags: DISPFlagLocalToUnit | DISPFlagDefinition, unit: !0, retainedNodes: !19)
!51 = !DISubroutineType(types: !52)
!52 = !{!13, !13}
!53 = !DILocalVariable(name: "a", arg: 1, scope: !50, file: !1, line: 2, type: !13)
!54 = !DILocation(line: 2, column: 19, scope: !50)
!55 = !DILocation(line: 2, column: 31, scope: !50)
!56 = !DILocation(line: 2, column: 35, scope: !50)
!57 = !DILocation(line: 2, column: 33, scope: !50)
!58 = !DILocation(line: 2, column: 24, scope: !50)
