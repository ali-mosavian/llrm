; ModuleID = '/tmp/bf.c'
source_filename = "/tmp/bf.c"
target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32"

%struct.s = type <{ i8, i32, [3 x i8] }>

@main.i = internal global %struct.s zeroinitializer, align 8

@__main_void = hidden alias i32 (), ptr @main

; Function Attrs: nounwind
define hidden void @f(ptr dead_on_unwind noalias writable sret(%struct.s) align 8 %0, ptr noundef byval(%struct.s) align 8 %1) #0 {
  %3 = getelementptr inbounds nuw %struct.s, ptr %1, i32 0, i32 1
  store i32 -839970252, ptr %3, align 1
  call void @llvm.memcpy.p0.p0.i32(ptr align 8 %0, ptr align 8 %1, i32 8, i1 false), !tbaa.struct !6
  ret void
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.memcpy.p0.p0.i32(ptr noalias writeonly captures(none), ptr noalias readonly captures(none), i32, i1 immarg) #1

; Function Attrs: nounwind
define hidden i32 @main() #0 {
  %1 = alloca %struct.s, align 8
  store i8 12, ptr @main.i, align 8
  call void @llvm.lifetime.start.p0(ptr %1) #3
  call void @f(ptr dead_on_unwind writable sret(%struct.s) align 8 %1, ptr noundef byval(%struct.s) align 8 @main.i) #4
  call void @llvm.memcpy.p0.p0.i32(ptr align 8 @main.i, ptr align 8 %1, i32 8, i1 false), !tbaa.struct !6
  call void @llvm.lifetime.end.p0(ptr %1) #3
  %2 = load i8, ptr @main.i, align 8
  %3 = zext i8 %2 to i64
  %4 = trunc i64 %3 to i32
  call void @report(i32 noundef %4) #4
  ret i32 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

declare void @report(i32 noundef) #2

attributes #0 = { nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+bulk-memory,+bulk-memory-opt,+call-indirect-overlong,+multivalue,+mutable-globals,+nontrapping-fptoint,+reference-types,+sign-ext" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+bulk-memory,+bulk-memory-opt,+call-indirect-overlong,+multivalue,+mutable-globals,+nontrapping-fptoint,+reference-types,+sign-ext" }
attributes #3 = { nounwind }
attributes #4 = { nobuiltin "no-builtins" }

!llvm.ident = !{!0}
!llvm.errno.tbaa = !{!1}

!0 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!1 = !{!2, !3, i64 0}
!2 = !{!"__libc_errno", !3, i64 0}
!3 = !{!"int", !4, i64 0}
!4 = !{!"omnipotent char", !5, i64 0}
!5 = !{!"Simple C/C++ TBAA"}
!6 = !{i64 0, i64 1, !7, i64 1, i64 4, !7}
!7 = !{!4, !4, i64 0}
