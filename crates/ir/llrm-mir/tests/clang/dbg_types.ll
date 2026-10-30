; ModuleID = '/tmp/dbgt.c'
source_filename = "/tmp/dbgt.c"
target datalayout = "e-m:e-p:32:32-p270:32:32-p271:32:32-p272:64:64-i128:128-f64:32:64-f80:32-n8:16:32-S128"

@g = global i32 3, align 4, !dbg !0
@arr = internal global [4 x i32] zeroinitializer, align 4, !dbg !12

; Function Attrs: noinline nounwind optnone uwtable
define i32 @pick(i32 noundef %i) #0 !dbg !26 {
entry:
  %i.addr = alloca i32, align 4
  store i32 %i, ptr %i.addr, align 4
    #dbg_declare(ptr %i.addr, !30, !DIExpression(), !31)
  %0 = load i32, ptr %i.addr, align 4, !dbg !32
  %tobool = icmp ne i32 %0, 0, !dbg !32
  %1 = zext i1 %tobool to i64, !dbg !32
  %cond = select i1 %tobool, i32 5, i32 0, !dbg !32
  ret i32 %cond, !dbg !33
}

; Function Attrs: noinline nounwind optnone uwtable
define i32 @get(i32 noundef %i) #0 !dbg !34 {
entry:
  %i.addr = alloca i32, align 4
  store i32 %i, ptr %i.addr, align 4
    #dbg_declare(ptr %i.addr, !37, !DIExpression(), !38)
  %0 = load i32, ptr %i.addr, align 4, !dbg !39
  %arrayidx = getelementptr inbounds [4 x i32], ptr @arr, i32 0, i32 %0, !dbg !40
  %1 = load i32, ptr %arrayidx, align 4, !dbg !40
  %2 = load i32, ptr @g, align 4, !dbg !41
  %add = add nsw i32 %1, %2, !dbg !42
  ret i32 %add, !dbg !43
}

attributes #0 = { noinline nounwind optnone uwtable "frame-pointer"="all" "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="pentium4" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" }

!llvm.dbg.cu = !{!2}
!llvm.module.flags = !{!18, !19, !20, !21, !22, !23, !24}
!llvm.ident = !{!25}

!0 = !DIGlobalVariableExpression(var: !1, expr: !DIExpression())
!1 = distinct !DIGlobalVariable(name: "g", scope: !2, file: !6, line: 3, type: !15, isLocal: false, isDefinition: true)
!2 = distinct !DICompileUnit(language: DW_LANG_C11, file: !3, producer: "clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)", isOptimized: false, runtimeVersion: 0, emissionKind: FullDebug, enums: !4, globals: !11, splitDebugInlining: false, nameTableKind: None)
!3 = !DIFile(filename: "/tmp/dbgt.c", directory: "/work", checksumkind: CSK_MD5, checksum: "1cd46cd2c4a05022e8780f34dd9e3449")
!4 = !{!5}
!5 = !DICompositeType(tag: DW_TAG_enumeration_type, name: "E", file: !6, line: 1, baseType: !7, size: 32, elements: !8)
!6 = !DIFile(filename: "/tmp/dbgt.c", directory: "", checksumkind: CSK_MD5, checksum: "1cd46cd2c4a05022e8780f34dd9e3449")
!7 = !DIBasicType(name: "unsigned int", size: 32, encoding: DW_ATE_unsigned)
!8 = !{!9, !10}
!9 = !DIEnumerator(name: "A", value: 0)
!10 = !DIEnumerator(name: "B", value: 5)
!11 = !{!0, !12}
!12 = !DIGlobalVariableExpression(var: !13, expr: !DIExpression())
!13 = distinct !DIGlobalVariable(name: "arr", scope: !2, file: !6, line: 2, type: !14, isLocal: true, isDefinition: true)
!14 = !DICompositeType(tag: DW_TAG_array_type, baseType: !15, size: 128, elements: !16)
!15 = !DIBasicType(name: "int", size: 32, encoding: DW_ATE_signed)
!16 = !{!17}
!17 = !DISubrange(count: 4)
!18 = !{i32 1, !"NumRegisterParameters", i32 0}
!19 = !{i32 7, !"Dwarf Version", i32 5}
!20 = !{i32 2, !"Debug Info Version", i32 3}
!21 = !{i32 8, !"PIC Level", i32 2}
!22 = !{i32 7, !"PIE Level", i32 2}
!23 = !{i32 7, !"uwtable", i32 2}
!24 = !{i32 7, !"frame-pointer", i32 2}
!25 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!26 = distinct !DISubprogram(name: "pick", scope: !6, file: !6, line: 4, type: !27, scopeLine: 4, flags: DIFlagPrototyped, spFlags: DISPFlagDefinition, unit: !2, retainedNodes: !29)
!27 = !DISubroutineType(types: !28)
!28 = !{!5, !15}
!29 = !{}
!30 = !DILocalVariable(name: "i", arg: 1, scope: !26, file: !6, line: 4, type: !15)
!31 = !DILocation(line: 4, column: 17, scope: !26)
!32 = !DILocation(line: 4, column: 29, scope: !26)
!33 = !DILocation(line: 4, column: 22, scope: !26)
!34 = distinct !DISubprogram(name: "get", scope: !6, file: !6, line: 5, type: !35, scopeLine: 5, flags: DIFlagPrototyped, spFlags: DISPFlagDefinition, unit: !2, retainedNodes: !29)
!35 = !DISubroutineType(types: !36)
!36 = !{!15, !15}
!37 = !DILocalVariable(name: "i", arg: 1, scope: !34, file: !6, line: 5, type: !15)
!38 = !DILocation(line: 5, column: 13, scope: !34)
!39 = !DILocation(line: 5, column: 29, scope: !34)
!40 = !DILocation(line: 5, column: 25, scope: !34)
!41 = !DILocation(line: 5, column: 34, scope: !34)
!42 = !DILocation(line: 5, column: 32, scope: !34)
!43 = !DILocation(line: 5, column: 18, scope: !34)
