target datalayout = "e-m:e-p:32:32-i128:128-f64:32:64-f80:32-n8:16:32-S128"

%struct.P = type { i32, i32 }

@p = global %struct.P zeroinitializer, align 4, !dbg !0
@q = global ptr null, align 4, !dbg !11

!llvm.dbg.cu = !{!2}

!0 = !DIGlobalVariableExpression(var: !1, expr: !DIExpression())
!1 = distinct !DIGlobalVariable(name: "p", scope: !2, file: !3, line: 1, type: !5, isLocal: false, isDefinition: true)
!2 = distinct !DICompileUnit(language: DW_LANG_C11, file: !3, producer: "clang", isOptimized: false, runtimeVersion: 0, emissionKind: FullDebug, globals: !10)
!3 = !DIFile(filename: "s.c", directory: "")
!5 = distinct !DICompositeType(tag: DW_TAG_structure_type, name: "P", file: !3, line: 1, size: 64, elements: !6)
!6 = !{!7, !8}
!7 = !DIDerivedType(tag: DW_TAG_member, name: "x", scope: !5, file: !3, line: 1, baseType: !9, size: 32)
!8 = !DIDerivedType(tag: DW_TAG_member, name: "y", scope: !5, file: !3, line: 1, baseType: !9, size: 32, offset: 32)
!9 = !DIBasicType(name: "int", size: 32, encoding: DW_ATE_signed)
!10 = !{!0, !11}
!11 = !DIGlobalVariableExpression(var: !12, expr: !DIExpression())
!12 = distinct !DIGlobalVariable(name: "q", scope: !2, file: !3, line: 2, type: !13, isLocal: false, isDefinition: true)
!13 = !DIDerivedType(tag: DW_TAG_pointer_type, baseType: !5, size: 32)
