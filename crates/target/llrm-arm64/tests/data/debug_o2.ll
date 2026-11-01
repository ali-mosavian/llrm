; ModuleID = 'sieve.c'
source_filename = "sieve.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

@bench_sieve_via = dso_local global ptr @bench_sieve, align 8, !dbg !0

; Function Attrs: nounwind uwtable
define dso_local i64 @bench_sieve(i16 noundef %0) #0 !dbg !21 {
  %2 = alloca i16, align 2
  %3 = alloca [1024 x i8], align 1
  %4 = alloca i16, align 2
  %5 = alloca i16, align 2
  %6 = alloca i16, align 2
  %7 = alloca i64, align 8
  store i16 %0, ptr %2, align 2, !tbaa !33
    #dbg_declare(ptr %2, !23, !DIExpression(), !37)
  call void @llvm.lifetime.start.p0(i64 1024, ptr %3) #3, !dbg !38
    #dbg_declare(ptr %3, !24, !DIExpression(), !39)
  call void @llvm.lifetime.start.p0(i64 2, ptr %4) #3, !dbg !40
    #dbg_declare(ptr %4, !29, !DIExpression(), !41)
  call void @llvm.lifetime.start.p0(i64 2, ptr %5) #3, !dbg !40
    #dbg_declare(ptr %5, !30, !DIExpression(), !42)
  call void @llvm.lifetime.start.p0(i64 2, ptr %6) #3, !dbg !40
    #dbg_declare(ptr %6, !31, !DIExpression(), !43)
  store i16 0, ptr %6, align 2, !dbg !43, !tbaa !33
  call void @llvm.lifetime.start.p0(i64 8, ptr %7) #3, !dbg !44
    #dbg_declare(ptr %7, !32, !DIExpression(), !45)
  store i64 0, ptr %7, align 8, !dbg !45, !tbaa !46
  store i16 0, ptr %4, align 2, !dbg !48, !tbaa !33
  br label %8, !dbg !50

8:                                                ; preds = %18, %1
  %9 = load i16, ptr %4, align 2, !dbg !51, !tbaa !33
  %10 = zext i16 %9 to i32, !dbg !51
  %11 = load i16, ptr %2, align 2, !dbg !53, !tbaa !33
  %12 = zext i16 %11 to i32, !dbg !53
  %13 = icmp slt i32 %10, %12, !dbg !54
  br i1 %13, label %14, label %21, !dbg !55

14:                                               ; preds = %8
  %15 = load i16, ptr %4, align 2, !dbg !56, !tbaa !33
  %16 = zext i16 %15 to i64, !dbg !57
  %17 = getelementptr inbounds nuw [1024 x i8], ptr %3, i64 0, i64 %16, !dbg !57
  store i8 0, ptr %17, align 1, !dbg !58, !tbaa !59
  br label %18, !dbg !57

18:                                               ; preds = %14
  %19 = load i16, ptr %4, align 2, !dbg !60, !tbaa !33
  %20 = add i16 %19, 1, !dbg !60
  store i16 %20, ptr %4, align 2, !dbg !60, !tbaa !33
  br label %8, !dbg !61, !llvm.loop !62

21:                                               ; preds = %8
  store i16 2, ptr %4, align 2, !dbg !65, !tbaa !33
  br label %22, !dbg !67

22:                                               ; preds = %71, %21
  %23 = load i16, ptr %4, align 2, !dbg !68, !tbaa !33
  %24 = zext i16 %23 to i32, !dbg !68
  %25 = load i16, ptr %2, align 2, !dbg !70, !tbaa !33
  %26 = zext i16 %25 to i32, !dbg !70
  %27 = icmp slt i32 %24, %26, !dbg !71
  br i1 %27, label %28, label %74, !dbg !72

28:                                               ; preds = %22
  %29 = load i16, ptr %4, align 2, !dbg !73, !tbaa !33
  %30 = zext i16 %29 to i64, !dbg !76
  %31 = getelementptr inbounds nuw [1024 x i8], ptr %3, i64 0, i64 %30, !dbg !76
  %32 = load i8, ptr %31, align 1, !dbg !76, !tbaa !59
  %33 = icmp ne i8 %32, 0, !dbg !76
  br i1 %33, label %34, label %35, !dbg !76

34:                                               ; preds = %28
  br label %71, !dbg !77

35:                                               ; preds = %28
  %36 = load i16, ptr %6, align 2, !dbg !78, !tbaa !33
  %37 = add i16 %36, 1, !dbg !78
  store i16 %37, ptr %6, align 2, !dbg !78, !tbaa !33
  %38 = load i16, ptr %4, align 2, !dbg !79, !tbaa !33
  %39 = zext i16 %38 to i64, !dbg !79
  %40 = load i64, ptr %7, align 8, !dbg !80, !tbaa !46
  %41 = add i64 %40, %39, !dbg !80
  store i64 %41, ptr %7, align 8, !dbg !80, !tbaa !46
  %42 = load i16, ptr %4, align 2, !dbg !81, !tbaa !33
  %43 = zext i16 %42 to i32, !dbg !81
  %44 = icmp sle i32 %43, 31, !dbg !83
  br i1 %44, label %45, label %70, !dbg !83

45:                                               ; preds = %35
  %46 = load i16, ptr %4, align 2, !dbg !84, !tbaa !33
  %47 = zext i16 %46 to i32, !dbg !84
  %48 = load i16, ptr %4, align 2, !dbg !86, !tbaa !33
  %49 = zext i16 %48 to i32, !dbg !86
  %50 = mul nsw i32 %47, %49, !dbg !87
  %51 = trunc i32 %50 to i16, !dbg !88
  store i16 %51, ptr %5, align 2, !dbg !89, !tbaa !33
  br label %52, !dbg !90

52:                                               ; preds = %62, %45
  %53 = load i16, ptr %5, align 2, !dbg !91, !tbaa !33
  %54 = zext i16 %53 to i32, !dbg !91
  %55 = load i16, ptr %2, align 2, !dbg !93, !tbaa !33
  %56 = zext i16 %55 to i32, !dbg !93
  %57 = icmp slt i32 %54, %56, !dbg !94
  br i1 %57, label %58, label %69, !dbg !95

58:                                               ; preds = %52
  %59 = load i16, ptr %5, align 2, !dbg !96, !tbaa !33
  %60 = zext i16 %59 to i64, !dbg !97
  %61 = getelementptr inbounds nuw [1024 x i8], ptr %3, i64 0, i64 %60, !dbg !97
  store i8 1, ptr %61, align 1, !dbg !98, !tbaa !59
  br label %62, !dbg !97

62:                                               ; preds = %58
  %63 = load i16, ptr %5, align 2, !dbg !99, !tbaa !33
  %64 = zext i16 %63 to i32, !dbg !99
  %65 = load i16, ptr %4, align 2, !dbg !100, !tbaa !33
  %66 = zext i16 %65 to i32, !dbg !100
  %67 = add nsw i32 %64, %66, !dbg !101
  %68 = trunc i32 %67 to i16, !dbg !102
  store i16 %68, ptr %5, align 2, !dbg !103, !tbaa !33
  br label %52, !dbg !104, !llvm.loop !105

69:                                               ; preds = %52
  br label %70, !dbg !106

70:                                               ; preds = %69, %35
  br label %71, !dbg !107

71:                                               ; preds = %70, %34
  %72 = load i16, ptr %4, align 2, !dbg !108, !tbaa !33
  %73 = add i16 %72, 1, !dbg !108
  store i16 %73, ptr %4, align 2, !dbg !108, !tbaa !33
  br label %22, !dbg !109, !llvm.loop !110

74:                                               ; preds = %22
  %75 = load i16, ptr %6, align 2, !dbg !112, !tbaa !33
  %76 = zext i16 %75 to i64, !dbg !113
  %77 = shl i64 %76, 16, !dbg !114
  %78 = load i64, ptr %7, align 8, !dbg !115, !tbaa !46
  %79 = xor i64 %77, %78, !dbg !116
  call void @llvm.lifetime.end.p0(i64 8, ptr %7) #3, !dbg !117
  call void @llvm.lifetime.end.p0(i64 2, ptr %6) #3, !dbg !117
  call void @llvm.lifetime.end.p0(i64 2, ptr %5) #3, !dbg !117
  call void @llvm.lifetime.end.p0(i64 2, ptr %4) #3, !dbg !117
  call void @llvm.lifetime.end.p0(i64 1024, ptr %3) #3, !dbg !117
  ret i64 %79, !dbg !118
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) #1

; Function Attrs: nounwind uwtable
define dso_local i32 @main() #0 !dbg !119 {
  %1 = alloca i32, align 4
  store i32 0, ptr %1, align 4
  %2 = load volatile ptr, ptr @bench_sieve_via, align 8, !dbg !123, !tbaa !124
  %3 = call i64 %2(i16 noundef 1024), !dbg !126
  call void @report(i64 noundef %3), !dbg !127
  ret i32 0, !dbg !128
}

declare !dbg !129 void @report(i64 noundef) #2

attributes #0 = { nounwind uwtable "frame-pointer"="non-leaf" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+v8a,-fmv" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { "frame-pointer"="non-leaf" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+v8a,-fmv" }
attributes #3 = { nounwind }

!llvm.dbg.cu = !{!2}
!llvm.module.flags = !{!13, !14, !15, !16, !17, !18, !19}
!llvm.ident = !{!20}

!0 = !DIGlobalVariableExpression(var: !1, expr: !DIExpression())
!1 = distinct !DIGlobalVariable(name: "bench_sieve_via", scope: !2, file: !3, line: 23, type: !9, isLocal: false, isDefinition: true)
!2 = distinct !DICompileUnit(language: DW_LANG_C11, file: !3, producer: "Ubuntu clang version 20.1.8 (0ubuntu4)", isOptimized: true, runtimeVersion: 0, emissionKind: FullDebug, retainedTypes: !4, globals: !8, splitDebugInlining: false, nameTableKind: None)
!3 = !DIFile(filename: "sieve.c", directory: "", checksumkind: CSK_MD5, checksum: "b019a935f2a20667c16bf3a4fe86a937")
!4 = !{!5, !6, !7}
!5 = !DIBasicType(name: "unsigned short", size: 16, encoding: DW_ATE_unsigned)
!6 = !DIBasicType(name: "unsigned long", size: 64, encoding: DW_ATE_unsigned)
!7 = !DIBasicType(name: "long", size: 64, encoding: DW_ATE_signed)
!8 = !{!0}
!9 = !DIDerivedType(tag: DW_TAG_volatile_type, baseType: !10)
!10 = !DIDerivedType(tag: DW_TAG_pointer_type, baseType: !11, size: 64)
!11 = !DISubroutineType(types: !12)
!12 = !{!6, !5}
!13 = !{i32 7, !"Dwarf Version", i32 5}
!14 = !{i32 2, !"Debug Info Version", i32 3}
!15 = !{i32 1, !"wchar_size", i32 4}
!16 = !{i32 8, !"PIC Level", i32 2}
!17 = !{i32 7, !"PIE Level", i32 2}
!18 = !{i32 7, !"uwtable", i32 2}
!19 = !{i32 7, !"frame-pointer", i32 1}
!20 = !{!"Ubuntu clang version 20.1.8 (0ubuntu4)"}
!21 = distinct !DISubprogram(name: "bench_sieve", scope: !3, file: !3, line: 5, type: !11, scopeLine: 6, flags: DIFlagPrototyped | DIFlagAllCallsDescribed, spFlags: DISPFlagDefinition | DISPFlagOptimized, unit: !2, retainedNodes: !22)
!22 = !{!23, !24, !29, !30, !31, !32}
!23 = !DILocalVariable(name: "limit", arg: 1, scope: !21, file: !3, line: 5, type: !5)
!24 = !DILocalVariable(name: "composite", scope: !21, file: !3, line: 7, type: !25)
!25 = !DICompositeType(tag: DW_TAG_array_type, baseType: !26, size: 8192, elements: !27)
!26 = !DIBasicType(name: "unsigned char", size: 8, encoding: DW_ATE_unsigned_char)
!27 = !{!28}
!28 = !DISubrange(count: 1024)
!29 = !DILocalVariable(name: "i", scope: !21, file: !3, line: 8, type: !5)
!30 = !DILocalVariable(name: "multiple", scope: !21, file: !3, line: 8, type: !5)
!31 = !DILocalVariable(name: "count", scope: !21, file: !3, line: 8, type: !5)
!32 = !DILocalVariable(name: "sum", scope: !21, file: !3, line: 9, type: !6)
!33 = !{!34, !34, i64 0}
!34 = !{!"short", !35, i64 0}
!35 = !{!"omnipotent char", !36, i64 0}
!36 = !{!"Simple C/C++ TBAA"}
!37 = !DILocation(line: 5, column: 42, scope: !21)
!38 = !DILocation(line: 7, column: 5, scope: !21)
!39 = !DILocation(line: 7, column: 19, scope: !21)
!40 = !DILocation(line: 8, column: 5, scope: !21)
!41 = !DILocation(line: 8, column: 20, scope: !21)
!42 = !DILocation(line: 8, column: 23, scope: !21)
!43 = !DILocation(line: 8, column: 33, scope: !21)
!44 = !DILocation(line: 9, column: 5, scope: !21)
!45 = !DILocation(line: 9, column: 19, scope: !21)
!46 = !{!47, !47, i64 0}
!47 = !{!"long", !35, i64 0}
!48 = !DILocation(line: 11, column: 12, scope: !49)
!49 = distinct !DILexicalBlock(scope: !21, file: !3, line: 11, column: 5)
!50 = !DILocation(line: 11, column: 10, scope: !49)
!51 = !DILocation(line: 11, column: 17, scope: !52)
!52 = distinct !DILexicalBlock(scope: !49, file: !3, line: 11, column: 5)
!53 = !DILocation(line: 11, column: 21, scope: !52)
!54 = !DILocation(line: 11, column: 19, scope: !52)
!55 = !DILocation(line: 11, column: 5, scope: !49)
!56 = !DILocation(line: 11, column: 43, scope: !52)
!57 = !DILocation(line: 11, column: 33, scope: !52)
!58 = !DILocation(line: 11, column: 46, scope: !52)
!59 = !{!35, !35, i64 0}
!60 = !DILocation(line: 11, column: 28, scope: !52)
!61 = !DILocation(line: 11, column: 5, scope: !52)
!62 = distinct !{!62, !55, !63, !64}
!63 = !DILocation(line: 11, column: 48, scope: !49)
!64 = !{!"llvm.loop.mustprogress"}
!65 = !DILocation(line: 12, column: 12, scope: !66)
!66 = distinct !DILexicalBlock(scope: !21, file: !3, line: 12, column: 5)
!67 = !DILocation(line: 12, column: 10, scope: !66)
!68 = !DILocation(line: 12, column: 17, scope: !69)
!69 = distinct !DILexicalBlock(scope: !66, file: !3, line: 12, column: 5)
!70 = !DILocation(line: 12, column: 21, scope: !69)
!71 = !DILocation(line: 12, column: 19, scope: !69)
!72 = !DILocation(line: 12, column: 5, scope: !66)
!73 = !DILocation(line: 13, column: 23, scope: !74)
!74 = distinct !DILexicalBlock(scope: !75, file: !3, line: 13, column: 13)
!75 = distinct !DILexicalBlock(scope: !69, file: !3, line: 12, column: 33)
!76 = !DILocation(line: 13, column: 13, scope: !74)
!77 = !DILocation(line: 13, column: 27, scope: !74)
!78 = !DILocation(line: 14, column: 9, scope: !75)
!79 = !DILocation(line: 15, column: 16, scope: !75)
!80 = !DILocation(line: 15, column: 13, scope: !75)
!81 = !DILocation(line: 16, column: 13, scope: !82)
!82 = distinct !DILexicalBlock(scope: !75, file: !3, line: 16, column: 13)
!83 = !DILocation(line: 16, column: 15, scope: !82)
!84 = !DILocation(line: 17, column: 46, scope: !85)
!85 = distinct !DILexicalBlock(scope: !82, file: !3, line: 17, column: 13)
!86 = !DILocation(line: 17, column: 50, scope: !85)
!87 = !DILocation(line: 17, column: 48, scope: !85)
!88 = !DILocation(line: 17, column: 29, scope: !85)
!89 = !DILocation(line: 17, column: 27, scope: !85)
!90 = !DILocation(line: 17, column: 18, scope: !85)
!91 = !DILocation(line: 17, column: 54, scope: !92)
!92 = distinct !DILexicalBlock(scope: !85, file: !3, line: 17, column: 13)
!93 = !DILocation(line: 17, column: 65, scope: !92)
!94 = !DILocation(line: 17, column: 63, scope: !92)
!95 = !DILocation(line: 17, column: 13, scope: !85)
!96 = !DILocation(line: 18, column: 27, scope: !92)
!97 = !DILocation(line: 18, column: 17, scope: !92)
!98 = !DILocation(line: 18, column: 37, scope: !92)
!99 = !DILocation(line: 17, column: 100, scope: !92)
!100 = !DILocation(line: 17, column: 111, scope: !92)
!101 = !DILocation(line: 17, column: 109, scope: !92)
!102 = !DILocation(line: 17, column: 83, scope: !92)
!103 = !DILocation(line: 17, column: 81, scope: !92)
!104 = !DILocation(line: 17, column: 13, scope: !92)
!105 = distinct !{!105, !95, !106, !64}
!106 = !DILocation(line: 18, column: 39, scope: !85)
!107 = !DILocation(line: 19, column: 5, scope: !75)
!108 = !DILocation(line: 12, column: 28, scope: !69)
!109 = !DILocation(line: 12, column: 5, scope: !69)
!110 = distinct !{!110, !72, !111, !64}
!111 = !DILocation(line: 19, column: 5, scope: !66)
!112 = !DILocation(line: 20, column: 28, scope: !21)
!113 = !DILocation(line: 20, column: 13, scope: !21)
!114 = !DILocation(line: 20, column: 34, scope: !21)
!115 = !DILocation(line: 20, column: 43, scope: !21)
!116 = !DILocation(line: 20, column: 41, scope: !21)
!117 = !DILocation(line: 21, column: 1, scope: !21)
!118 = !DILocation(line: 20, column: 5, scope: !21)
!119 = distinct !DISubprogram(name: "main", scope: !3, file: !3, line: 24, type: !120, scopeLine: 25, flags: DIFlagPrototyped | DIFlagAllCallsDescribed, spFlags: DISPFlagDefinition | DISPFlagOptimized, unit: !2)
!120 = !DISubroutineType(types: !121)
!121 = !{!122}
!122 = !DIBasicType(name: "int", size: 32, encoding: DW_ATE_signed)
!123 = !DILocation(line: 26, column: 20, scope: !119)
!124 = !{!125, !125, i64 0}
!125 = !{!"any pointer", !35, i64 0}
!126 = !DILocation(line: 26, column: 18, scope: !119)
!127 = !DILocation(line: 26, column: 5, scope: !119)
!128 = !DILocation(line: 27, column: 5, scope: !119)
!129 = !DISubprogram(name: "report", scope: !3, file: !3, line: 3, type: !130, flags: DIFlagPrototyped, spFlags: DISPFlagOptimized)
!130 = !DISubroutineType(types: !131)
!131 = !{null, !7}
