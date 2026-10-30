; ModuleID = 'as.c'
source_filename = "as.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"

; Function Attrs: nounwind uwtable
define i32 @f(i32 noundef %n) local_unnamed_addr #0 !dbg !14 {
entry:
  %a = alloca [3 x i32], align 4, !DIAssignID !24
    #dbg_assign(i1 poison, !20, !DIExpression(), !24, ptr %a, !DIExpression(), !25)
    #dbg_value(i32 %n, !19, !DIExpression(), !25)
  store i32 %n, ptr %a, align 4, !dbg !26, !tbaa !27, !DIAssignID !28
    #dbg_assign(i32 %n, !20, !DIExpression(DW_OP_LLVM_fragment, 0, 32), !28, ptr %a, !DIExpression(), !25)
  %call = call i32 @g(ptr noundef nonnull %a) #2, !dbg !29
  %0 = load i32, ptr %a, align 4, !dbg !30, !tbaa !27
  %add = add nsw i32 %0, %call, !dbg !31
  ret i32 %add, !dbg !32
}

declare !dbg !33 i32 @g(ptr noundef) local_unnamed_addr #1

attributes #0 = { nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" }
attributes #1 = { "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" }
attributes #2 = { nounwind }

!llvm.dbg.cu = !{!0}
!llvm.module.flags = !{!2, !3, !4, !5, !6, !7}
!llvm.ident = !{!8}
!llvm.errno.tbaa = !{!9}

!0 = distinct !DICompileUnit(language: DW_LANG_C11, file: !1, producer: "clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)", isOptimized: true, runtimeVersion: 0, emissionKind: FullDebug, splitDebugInlining: false, nameTableKind: None)
!1 = !DIFile(filename: "as.c", directory: "/tmp/claude-1000/iv10", checksumkind: CSK_MD5, checksum: "c332a8ffa497f9494383a4c67fd936b8")
!2 = !{i32 7, !"Dwarf Version", i32 5}
!3 = !{i32 2, !"Debug Info Version", i32 3}
!4 = !{i32 8, !"PIC Level", i32 2}
!5 = !{i32 7, !"PIE Level", i32 2}
!6 = !{i32 7, !"uwtable", i32 2}
!7 = !{i32 7, !"debug-info-assignment-tracking", i1 true}
!8 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!9 = !{!10, !11, i64 0}
!10 = !{!"__libc_errno", !11, i64 0}
!11 = !{!"int", !12, i64 0}
!12 = !{!"omnipotent char", !13, i64 0}
!13 = !{!"Simple C/C++ TBAA"}
!14 = distinct !DISubprogram(name: "f", scope: !1, file: !1, line: 2, type: !15, scopeLine: 2, flags: DIFlagPrototyped | DIFlagAllCallsDescribed, spFlags: DISPFlagDefinition | DISPFlagOptimized, unit: !0, retainedNodes: !18, keyInstructions: true)
!15 = !DISubroutineType(types: !16)
!16 = !{!17, !17}
!17 = !DIBasicType(name: "int", size: 32, encoding: DW_ATE_signed)
!18 = !{!19, !20}
!19 = !DILocalVariable(name: "n", arg: 1, scope: !14, file: !1, line: 2, type: !17)
!20 = !DILocalVariable(name: "a", scope: !14, file: !1, line: 3, type: !21)
!21 = !DICompositeType(tag: DW_TAG_array_type, baseType: !17, size: 96, elements: !22)
!22 = !{!23}
!23 = !DISubrange(count: 3)
!24 = distinct !DIAssignID()
!25 = !DILocation(line: 0, scope: !14)
!26 = !DILocation(line: 4, column: 8, scope: !14, atomGroup: 1, atomRank: 1)
!27 = !{!11, !11, i64 0}
!28 = distinct !DIAssignID()
!29 = !DILocation(line: 5, column: 10, scope: !14)
!30 = !DILocation(line: 5, column: 17, scope: !14)
!31 = !DILocation(line: 5, column: 15, scope: !14, atomGroup: 2, atomRank: 2)
!32 = !DILocation(line: 5, column: 3, scope: !14, atomGroup: 2, atomRank: 1)
!33 = !DISubprogram(name: "g", scope: !1, file: !1, line: 1, type: !34, flags: DIFlagPrototyped, spFlags: DISPFlagOptimized)
!34 = !DISubroutineType(types: !35)
!35 = !{!17, !36}
!36 = !DIDerivedType(tag: DW_TAG_pointer_type, baseType: !17, size: 64)
