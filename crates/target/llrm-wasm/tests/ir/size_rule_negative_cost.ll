; ModuleID = 'tests/run/c/uint64_to_float.c'
source_filename = "tests/run/c/uint64_to_float.c"
target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"

@classes = internal global i32 0, align 4

@__main_void = hidden alias i32 (), ptr @main

; Function Attrs: nounwind
define hidden i32 @main() #0 {
  call void @classify(i64 noundef 0) #3
  call void @classify(i64 noundef 12345) #3
  call void @classify(i64 noundef 9007199254740993) #3
  call void @classify(i64 noundef -9223372036854775808) #3
  call void @classify(i64 noundef -6101065172474983726) #3
  call void @classify(i64 noundef -1) #3
  %1 = load i32, ptr @classes, align 4, !tbaa !6
  call void @report(i32 noundef %1) #3
  %2 = call double @to_double(i64 noundef -6101065172474983726) #3
  %3 = fdiv double %2, f0x4200000000000000
  %4 = fptosi double %3 to i32
  call void @report(i32 noundef %4) #3
  %5 = call double @to_double(i64 noundef -9223372036854775808) #3
  %6 = fdiv double %5, f0x4200000000000000
  %7 = fptosi double %6 to i32
  call void @report(i32 noundef %7) #3
  %8 = call float @to_float(i64 noundef -9223372036854775808) #3
  %9 = fdiv float %8, f0x4F800000
  %10 = fdiv float %9, f0x4E800000
  %11 = fptosi float %10 to i32
  call void @report(i32 noundef %11) #3
  ret i32 0
}

; Function Attrs: nounwind
define internal void @classify(i64 noundef %0) #0 {
  %2 = alloca i64, align 8
  %3 = alloca double, align 8
  %4 = alloca float, align 4
  store i64 %0, ptr %2, align 8, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %3) #4
  %5 = load i64, ptr %2, align 8, !tbaa !8
  %6 = call double @to_double(i64 noundef %5) #3
  store double %6, ptr %3, align 8, !tbaa !10
  call void @llvm.lifetime.start.p0(ptr %4) #4
  %7 = load i64, ptr %2, align 8, !tbaa !8
  %8 = call float @to_float(i64 noundef %7) #3
  store float %8, ptr %4, align 4, !tbaa !12
  %9 = load i32, ptr @classes, align 4, !tbaa !6
  %10 = mul nsw i32 %9, 3
  %11 = load double, ptr %3, align 8, !tbaa !10
  %12 = fcmp oge double %11, f0x43E0000000000000
  %13 = zext i1 %12 to i32
  %14 = add nsw i32 %10, %13
  %15 = load double, ptr %3, align 8, !tbaa !10
  %16 = fcmp oge double %15, 1.844600e+19
  %17 = zext i1 %16 to i32
  %18 = add nsw i32 %14, %17
  %19 = load float, ptr %4, align 4, !tbaa !12
  %20 = fcmp oge float %19, f0x5F000000
  %21 = zext i1 %20 to i32
  %22 = add nsw i32 %18, %21
  store i32 %22, ptr @classes, align 4, !tbaa !6
  %23 = load i32, ptr @classes, align 4, !tbaa !6
  %24 = mul nsw i32 %23, 3
  %25 = load double, ptr %3, align 8, !tbaa !10
  %26 = fcmp oge double %25, f0x4340000000000000
  %27 = zext i1 %26 to i32
  %28 = add nsw i32 %24, %27
  %29 = load double, ptr %3, align 8, !tbaa !10
  %30 = fcmp oeq double %29, 1.234500e+04
  %31 = zext i1 %30 to i32
  %32 = add nsw i32 %28, %31
  store i32 %32, ptr @classes, align 4, !tbaa !6
  call void @llvm.lifetime.end.p0(ptr %4) #4
  call void @llvm.lifetime.end.p0(ptr %3) #4
  ret void
}

declare void @report(i32 noundef) #1

; Function Attrs: nounwind
define internal double @to_double(i64 noundef %0) #0 {
  %2 = alloca i64, align 8
  store i64 %0, ptr %2, align 8, !tbaa !8
  %3 = load i64, ptr %2, align 8, !tbaa !8
  %4 = uitofp i64 %3 to double
  ret double %4
}

; Function Attrs: nounwind
define internal float @to_float(i64 noundef %0) #0 {
  %2 = alloca i64, align 8
  store i64 %0, ptr %2, align 8, !tbaa !8
  %3 = load i64, ptr %2, align 8, !tbaa !8
  %4 = uitofp i64 %3 to float
  ret float %4
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #2

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #2

attributes #0 = { nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+bulk-memory,+bulk-memory-opt,+call-indirect-overlong,+multivalue,+mutable-globals,+nontrapping-fptoint,+reference-types,+sign-ext" }
attributes #1 = { "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+bulk-memory,+bulk-memory-opt,+call-indirect-overlong,+multivalue,+mutable-globals,+nontrapping-fptoint,+reference-types,+sign-ext" }
attributes #2 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #3 = { nobuiltin "no-builtins" }
attributes #4 = { nounwind }

!llvm.ident = !{!0}
!llvm.errno.tbaa = !{!1}

!0 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!1 = !{!2, !3, i64 0}
!2 = !{!"__libc_errno", !3, i64 0}
!3 = !{!"int", !4, i64 0}
!4 = !{!"omnipotent char", !5, i64 0}
!5 = !{!"Simple C/C++ TBAA"}
!6 = !{!7, !7, i64 0}
!7 = !{!"long", !4, i64 0}
!8 = !{!9, !9, i64 0}
!9 = !{!"long long", !4, i64 0}
!10 = !{!11, !11, i64 0}
!11 = !{!"double", !4, i64 0}
!12 = !{!13, !13, i64 0}
!13 = !{!"float", !4, i64 0}
