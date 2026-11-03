target triple = "wasm32-unknown-unknown"

declare void @g(i32)

define void @f(i32 %x, ptr %p) !dbg !5 {
  %y = add i32 %x, 1, !dbg !7
  store i32 %y, ptr %p, !dbg !8
  call void @g(i32 %y), !dbg !9
  ret void, !dbg !10
}

!llvm.dbg.cu = !{!0}
!llvm.module.flags = !{!3}
!0 = distinct !DICompileUnit(language: DW_LANG_C11, file: !1, producer: "t", isOptimized: false, runtimeVersion: 0, emissionKind: FullDebug)
!1 = !DIFile(filename: "d.c", directory: "/src")
!3 = !{i32 2, !"Debug Info Version", i32 3}
!5 = distinct !DISubprogram(name: "f", scope: !1, file: !1, line: 1, type: !6, scopeLine: 1, spFlags: DISPFlagDefinition, unit: !0)
!6 = !DISubroutineType(types: !{})
!7 = !DILocation(line: 11, scope: !5)
!8 = !DILocation(line: 12, scope: !5)
!9 = !DILocation(line: 13, scope: !5)
!10 = !DILocation(line: 14, scope: !5)
