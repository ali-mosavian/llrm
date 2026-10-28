; ModuleID = 'pu2.c'
source_filename = "pu2.c"
target datalayout = "e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32"
target triple = "i486-unknown-linux-gnu"

; Function Attrs: nounwind uwtable
define dso_local i32 @g(ptr noundef %0) local_unnamed_addr #0 !dbg !15 {
    #dbg_value(ptr %0, !26, !DIExpression(), !29)
  %2 = getelementptr inbounds nuw i8, ptr %0, i32 8, !dbg !30
    #dbg_value(ptr %2, !27, !DIExpression(DW_OP_plus_uconst, 8, DW_OP_stack_value), !29)
  %3 = tail call i32 @use(ptr noundef nonnull %2) #2, !dbg !31
  %4 = load i32, ptr %2, align 4, !dbg !32, !tbaa !33
  %5 = add nsw i32 %4, %3, !dbg !34
  ret i32 %5, !dbg !35
}

declare !dbg !36 i32 @use(ptr noundef) local_unnamed_addr #1

attributes #0 = { nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="pentium4" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" }
attributes #1 = { "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="pentium4" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" }
attributes #2 = { nounwind }

!llvm.dbg.cu = !{!0}
!llvm.module.flags = !{!2, !3, !4, !5, !6, !7, !8}
!llvm.ident = !{!9}
!llvm.errno.tbaa = !{!10}

!0 = distinct !DICompileUnit(language: DW_LANG_C11, file: !1, producer: "clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)", isOptimized: true, runtimeVersion: 0, emissionKind: FullDebug, splitDebugInlining: false, nameTableKind: None)
!1 = !DIFile(filename: "pu2.c", directory: "/tmp/claude-1000", checksumkind: CSK_MD5, checksum: "43df466f46a9c7166fee3131a1841996")
!2 = !{i32 1, !"NumRegisterParameters", i32 0}
!3 = !{i32 7, !"Dwarf Version", i32 5}
!4 = !{i32 2, !"Debug Info Version", i32 3}
!5 = !{i32 8, !"PIC Level", i32 2}
!6 = !{i32 7, !"PIE Level", i32 2}
!7 = !{i32 7, !"uwtable", i32 2}
!8 = !{i32 7, !"debug-info-assignment-tracking", i1 true}
!9 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!10 = !{!11, !12, i64 0}
!11 = !{!"__libc_errno", !12, i64 0}
!12 = !{!"int", !13, i64 0}
!13 = !{!"omnipotent char", !14, i64 0}
!14 = !{!"Simple C/C++ TBAA"}
!15 = distinct !DISubprogram(name: "g", scope: !1, file: !1, line: 3, type: !16, scopeLine: 3, flags: DIFlagPrototyped | DIFlagAllCallsDescribed, spFlags: DISPFlagDefinition | DISPFlagOptimized, unit: !0, retainedNodes: !25, keyInstructions: true)
!16 = !DISubroutineType(types: !17)
!17 = !{!18, !19}
!18 = !DIBasicType(name: "int", size: 32, encoding: DW_ATE_signed)
!19 = !DIDerivedType(tag: DW_TAG_pointer_type, baseType: !20, size: 32)
!20 = distinct !DICompositeType(tag: DW_TAG_structure_type, name: "S", file: !1, line: 1, size: 96, elements: !21)
!21 = !{!22, !23, !24}
!22 = !DIDerivedType(tag: DW_TAG_member, name: "a", scope: !20, file: !1, line: 1, baseType: !18, size: 32)
!23 = !DIDerivedType(tag: DW_TAG_member, name: "b", scope: !20, file: !1, line: 1, baseType: !18, size: 32, offset: 32)
!24 = !DIDerivedType(tag: DW_TAG_member, name: "c", scope: !20, file: !1, line: 1, baseType: !18, size: 32, offset: 64)
!25 = !{!26, !27}
!26 = !DILocalVariable(name: "s", arg: 1, scope: !15, file: !1, line: 3, type: !19)
!27 = !DILocalVariable(name: "q", scope: !15, file: !1, line: 3, type: !28)
!28 = !DIDerivedType(tag: DW_TAG_pointer_type, baseType: !18, size: 32)
!29 = !DILocation(line: 0, scope: !15)
!30 = !DILocation(line: 3, column: 35, scope: !15, atomGroup: 1, atomRank: 2)
!31 = !DILocation(line: 3, column: 45, scope: !15)
!32 = !DILocation(line: 3, column: 54, scope: !15)
!33 = !{!12, !12, i64 0}
!34 = !DILocation(line: 3, column: 52, scope: !15, atomGroup: 3, atomRank: 2)
!35 = !DILocation(line: 3, column: 38, scope: !15, atomGroup: 3, atomRank: 1)
!36 = !DISubprogram(name: "use", scope: !1, file: !1, line: 2, type: !37, flags: DIFlagPrototyped, spFlags: DISPFlagOptimized)
!37 = !DISubroutineType(types: !38)
!38 = !{!18, !28}
