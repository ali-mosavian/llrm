; ModuleID = 'algebra.c'
source_filename = "algebra.c"
target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32"

@input_a1 = internal global i16 23, align 2
@demo_a = internal global i16 0, align 2
@input_b1 = internal global i16 7, align 2
@demo_b = internal global i16 0, align 2
@input_a2 = internal global i16 -11, align 2
@input_b2 = internal global i16 4, align 2

@__main_void = hidden alias i32 (), ptr @main

; Function Attrs: nounwind optsize
define hidden i32 @parity_algebra(ptr noundef %0, ptr noundef %1) #0 {
  %3 = alloca ptr, align 4
  %4 = alloca ptr, align 4
  %5 = alloca i32, align 4
  store ptr %0, ptr %3, align 4, !tbaa !6
  store ptr %1, ptr %4, align 4, !tbaa !6
  call void @llvm.lifetime.start.p0(ptr %5) #3
  %6 = load ptr, ptr %3, align 4, !tbaa !6
  %7 = load i16, ptr %6, align 2, !tbaa !9
  %8 = sext i16 %7 to i32
  %9 = mul nsw i32 %8, 9
  %10 = load ptr, ptr %4, align 4, !tbaa !6
  %11 = load i16, ptr %10, align 2, !tbaa !9
  %12 = sext i16 %11 to i32
  %13 = mul nsw i32 %12, 5
  %14 = add nsw i32 %9, %13
  store i32 %14, ptr %5, align 4, !tbaa !11
  %15 = load i32, ptr %5, align 4, !tbaa !11
  %16 = mul nsw i32 %15, 3
  %17 = load ptr, ptr %3, align 4, !tbaa !6
  %18 = load i16, ptr %17, align 2, !tbaa !9
  %19 = sext i16 %18 to i32
  %20 = sub nsw i32 %16, %19
  call void @llvm.lifetime.end.p0(ptr %5) #3
  ret i32 %20
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nounwind optsize
define hidden i32 @bench_algebra() #0 {
  %1 = alloca i32, align 4
  call void @llvm.lifetime.start.p0(ptr %1) #3
  %2 = load volatile i16, ptr @input_a1, align 2, !tbaa !9
  store i16 %2, ptr @demo_a, align 2, !tbaa !9
  %3 = load volatile i16, ptr @input_b1, align 2, !tbaa !9
  store i16 %3, ptr @demo_b, align 2, !tbaa !9
  %4 = call i32 @parity_algebra(ptr noundef @demo_a, ptr noundef @demo_b) #4
  store i32 %4, ptr %1, align 4, !tbaa !11
  %5 = load volatile i16, ptr @input_a2, align 2, !tbaa !9
  store i16 %5, ptr @demo_a, align 2, !tbaa !9
  %6 = load volatile i16, ptr @input_b2, align 2, !tbaa !9
  store i16 %6, ptr @demo_b, align 2, !tbaa !9
  %7 = load i32, ptr %1, align 4, !tbaa !11
  %8 = mul nsw i32 %7, 1000
  %9 = call i32 @parity_algebra(ptr noundef @demo_a, ptr noundef @demo_b) #4
  %10 = add nsw i32 %8, %9
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret i32 %10
}

; Function Attrs: nounwind optsize
define hidden i32 @main() #0 {
  %1 = call i32 @bench_algebra() #4
  call void @report(i32 noundef %1) #4
  ret i32 0
}

; Function Attrs: optsize
declare void @report(i32 noundef) #2

attributes #0 = { nounwind optsize "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+bulk-memory,+bulk-memory-opt,+call-indirect-overlong,+multivalue,+mutable-globals,+nontrapping-fptoint,+reference-types,+sign-ext" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { optsize "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+bulk-memory,+bulk-memory-opt,+call-indirect-overlong,+multivalue,+mutable-globals,+nontrapping-fptoint,+reference-types,+sign-ext" }
attributes #3 = { nounwind }
attributes #4 = { nobuiltin optsize "no-builtins" }

!llvm.ident = !{!0}
!llvm.errno.tbaa = !{!1}

!0 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!1 = !{!2, !3, i64 0}
!2 = !{!"__libc_errno", !3, i64 0}
!3 = !{!"int", !4, i64 0}
!4 = !{!"omnipotent char", !5, i64 0}
!5 = !{!"Simple C/C++ TBAA"}
!6 = !{!7, !7, i64 0}
!7 = !{!"p1 short", !8, i64 0}
!8 = !{!"any pointer", !4, i64 0}
!9 = !{!10, !10, i64 0}
!10 = !{!"short", !4, i64 0}
!11 = !{!12, !12, i64 0}
!12 = !{!"long", !4, i64 0}
