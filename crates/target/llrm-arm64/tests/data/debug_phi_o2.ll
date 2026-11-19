; ModuleID = 'phi.c'
source_filename = "phi.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

; Function Attrs: nofree norecurse nosync nounwind memory(none) uwtable
define dso_local i32 @f(i32 noundef %0, i32 noundef %1) local_unnamed_addr #0 !dbg !21 {
    #dbg_value(i32 %0, !27, !DIExpression(), !32)
    #dbg_value(i32 %1, !28, !DIExpression(), !32)
    #dbg_value(i32 0, !29, !DIExpression(), !32)
    #dbg_value(i32 0, !30, !DIExpression(), !33)
  %3 = icmp sgt i32 %0, 0, !dbg !34
  br i1 %3, label %6, label %4, !dbg !36

4:                                                ; preds = %6, %2
  %5 = phi i32 [ 0, %2 ], [ %13, %6 ], !dbg !32
  ret i32 %5, !dbg !37

6:                                                ; preds = %2, %6
  %7 = phi i32 [ %14, %6 ], [ 0, %2 ]
  %8 = phi i32 [ %13, %6 ], [ 0, %2 ]
    #dbg_value(i32 %7, !30, !DIExpression(), !33)
    #dbg_value(i32 %8, !29, !DIExpression(), !32)
  %9 = and i32 %7, 1, !dbg !38
  %10 = icmp eq i32 %9, 0, !dbg !41
  %11 = sub nsw i32 0, %7, !dbg !42
  %12 = select i1 %10, i32 %11, i32 %1, !dbg !42
  %13 = add i32 %12, %8, !dbg !42
    #dbg_value(i32 %13, !29, !DIExpression(), !32)
  %14 = add nuw nsw i32 %7, 1, !dbg !43
    #dbg_value(i32 %14, !30, !DIExpression(), !33)
  %15 = icmp eq i32 %14, %0, !dbg !44
  br i1 %15, label %4, label %6, !dbg !45, !llvm.loop !46
}

attributes #0 = { nofree norecurse nosync nounwind memory(none) uwtable "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }

!llvm.dbg.cu = !{!0}
!llvm.module.flags = !{!2, !3, !4, !5, !6, !7, !8, !9, !10, !11, !12, !13, !14}
!llvm.ident = !{!15}
!llvm.errno.tbaa = !{!16}

!0 = distinct !DICompileUnit(language: DW_LANG_C11, file: !1, producer: "clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)", isOptimized: true, runtimeVersion: 0, emissionKind: FullDebug, splitDebugInlining: false, nameTableKind: None)
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
!14 = !{i32 7, !"debug-info-assignment-tracking", i1 true}
!15 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!16 = !{!17, !18, i64 0}
!17 = !{!"__libc_errno", !18, i64 0}
!18 = !{!"int", !19, i64 0}
!19 = !{!"omnipotent char", !20, i64 0}
!20 = !{!"Simple C/C++ TBAA"}
!21 = distinct !DISubprogram(name: "f", scope: !22, file: !22, line: 1, type: !23, scopeLine: 1, flags: DIFlagPrototyped | DIFlagAllCallsDescribed, spFlags: DISPFlagDefinition | DISPFlagOptimized, unit: !0, retainedNodes: !26, keyInstructions: true)
!22 = !DIFile(filename: "phi.c", directory: "", checksumkind: CSK_MD5, checksum: "29ebc02ebdf09a72665abbcd92166292")
!23 = !DISubroutineType(types: !24)
!24 = !{!25, !25, !25}
!25 = !DIBasicType(name: "int", size: 32, encoding: DW_ATE_signed)
!26 = !{!27, !28, !29, !30}
!27 = !DILocalVariable(name: "n", arg: 1, scope: !21, file: !22, line: 1, type: !25)
!28 = !DILocalVariable(name: "a", arg: 2, scope: !21, file: !22, line: 1, type: !25)
!29 = !DILocalVariable(name: "s", scope: !21, file: !22, line: 2, type: !25)
!30 = !DILocalVariable(name: "i", scope: !31, file: !22, line: 3, type: !25)
!31 = distinct !DILexicalBlock(scope: !21, file: !22, line: 3, column: 3)
!32 = !DILocation(line: 0, scope: !21)
!33 = !DILocation(line: 0, scope: !31)
!34 = !DILocation(line: 3, column: 21, scope: !35, atomGroup: 12, atomRank: 1)
!35 = distinct !DILexicalBlock(scope: !31, file: !22, line: 3, column: 3)
!36 = !DILocation(line: 3, column: 3, scope: !31, atomGroup: 13, atomRank: 1)
!37 = !DILocation(line: 9, column: 3, scope: !21, atomGroup: 11, atomRank: 1)
!38 = !DILocation(line: 4, column: 11, scope: !39)
!39 = distinct !DILexicalBlock(scope: !40, file: !22, line: 4, column: 9)
!40 = distinct !DILexicalBlock(scope: !35, file: !22, line: 3, column: 31)
!41 = !DILocation(line: 4, column: 11, scope: !39, atomGroup: 5, atomRank: 2)
!42 = !DILocation(line: 4, column: 11, scope: !39, atomGroup: 5, atomRank: 1)
!43 = !DILocation(line: 3, column: 27, scope: !35, atomGroup: 8, atomRank: 2)
!44 = !DILocation(line: 3, column: 21, scope: !35, atomGroup: 3, atomRank: 1)
!45 = !DILocation(line: 3, column: 3, scope: !31, atomGroup: 4, atomRank: 1)
!46 = distinct !{!46, !47, !48, !49}
!47 = !DILocation(line: 3, column: 3, scope: !31)
!48 = !DILocation(line: 8, column: 3, scope: !31)
!49 = !{!"llvm.loop.mustprogress"}
