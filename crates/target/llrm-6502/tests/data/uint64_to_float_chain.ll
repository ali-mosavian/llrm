
@classes = internal global i32 0, align 2

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  call void @classify(i64 noundef 0) #3
  call void @classify(i64 noundef 12345) #3
  call void @classify(i64 noundef 9007199254740993) #3
  call void @classify(i64 noundef -9223372036854775808) #3
  call void @classify(i64 noundef -6101065172474983726) #3
  call void @classify(i64 noundef -1) #3
  %2 = load i32, ptr @classes, align 2, !tbaa !7
  call void @report(i32 noundef %2) #3
  %3 = call double @to_double(i64 noundef -6101065172474983726) #3
  %4 = fdiv double %3, f0x4200000000000000
  %5 = fptosi double %4 to i32
  call void @report(i32 noundef %5) #3
  %6 = call double @to_double(i64 noundef -9223372036854775808) #3
  %7 = fdiv double %6, f0x4200000000000000
  %8 = fptosi double %7 to i32
  call void @report(i32 noundef %8) #3
  %9 = call float @to_float(i64 noundef -9223372036854775808) #3
  %10 = fdiv float %9, f0x4F800000
  %11 = fdiv float %10, f0x4E800000
  %12 = fptosi float %11 to i32
  call void @report(i32 noundef %12) #3
  ret i16 0
}

; Function Attrs: nounwind optsize
define internal void @classify(i64 noundef %0) #0 {
  %2 = alloca i64, align 2
  %3 = alloca double, align 2
  %4 = alloca float, align 2
  store i64 %0, ptr %2, align 2, !tbaa !9
  call void @llvm.lifetime.start.p0(ptr %3) #4
  %5 = load i64, ptr %2, align 2, !tbaa !9
  %6 = call double @to_double(i64 noundef %5) #3
  store double %6, ptr %3, align 2, !tbaa !11
  call void @llvm.lifetime.start.p0(ptr %4) #4
  %7 = load i64, ptr %2, align 2, !tbaa !9
  %8 = call float @to_float(i64 noundef %7) #3
  store float %8, ptr %4, align 2, !tbaa !13
  %9 = load i32, ptr @classes, align 2, !tbaa !7
  %10 = mul nsw i32 %9, 3
  %11 = load double, ptr %3, align 2, !tbaa !11
  %12 = fcmp oge double %11, f0x43E0000000000000
  %13 = zext i1 %12 to i16
  %14 = sext i16 %13 to i32
  %15 = add nsw i32 %10, %14
  %16 = load double, ptr %3, align 2, !tbaa !11
  %17 = fcmp oge double %16, 1.844600e+19
  %18 = zext i1 %17 to i16
  %19 = sext i16 %18 to i32
  %20 = add nsw i32 %15, %19
  %21 = load float, ptr %4, align 2, !tbaa !13
  %22 = fcmp oge float %21, f0x5F000000
  %23 = zext i1 %22 to i16
  %24 = sext i16 %23 to i32
  %25 = add nsw i32 %20, %24
  store i32 %25, ptr @classes, align 2, !tbaa !7
  %26 = load i32, ptr @classes, align 2, !tbaa !7
  %27 = mul nsw i32 %26, 3
  %28 = load double, ptr %3, align 2, !tbaa !11
  %29 = fcmp oge double %28, f0x4340000000000000
  %30 = zext i1 %29 to i16
  %31 = sext i16 %30 to i32
  %32 = add nsw i32 %27, %31
  %33 = load double, ptr %3, align 2, !tbaa !11
  %34 = fcmp oeq double %33, 1.234500e+04
  %35 = zext i1 %34 to i16
  %36 = sext i16 %35 to i32
  %37 = add nsw i32 %32, %36
  store i32 %37, ptr @classes, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %4) #4
  call void @llvm.lifetime.end.p0(ptr %3) #4
  ret void
}

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #1

; Function Attrs: nounwind optsize
define internal double @to_double(i64 noundef %0) #0 {
  %2 = alloca i64, align 2
  store i64 %0, ptr %2, align 2, !tbaa !9
  %3 = load i64, ptr %2, align 2, !tbaa !9
  %4 = uitofp i64 %3 to double
  ret double %4
}

; Function Attrs: nounwind optsize
define internal float @to_float(i64 noundef %0) #0 {
  %2 = alloca i64, align 2
  store i64 %0, ptr %2, align 2, !tbaa !9
  %3 = load i64, ptr %2, align 2, !tbaa !9
  %4 = uitofp i64 %3 to float
  ret float %4
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #2

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #2

attributes #0 = { nounwind optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #2 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #3 = { optsize }
attributes #4 = { nounwind }

!llvm.module.flags = !{!0}
!llvm.ident = !{!1}
!llvm.errno.tbaa = !{!2}

!0 = !{i32 1, !"wchar_size", i32 2}
!1 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!2 = !{!3, !4, i64 0}
!3 = !{!"__libc_errno", !4, i64 0}
!4 = !{!"int", !5, i64 0}
!5 = !{!"omnipotent char", !6, i64 0}
!6 = !{!"Simple C/C++ TBAA"}
!7 = !{!8, !8, i64 0}
!8 = !{!"long", !5, i64 0}
!9 = !{!10, !10, i64 0}
!10 = !{!"long long", !5, i64 0}
!11 = !{!12, !12, i64 0}
!12 = !{!"double", !5, i64 0}
!13 = !{!14, !14, i64 0}
!14 = !{!"float", !5, i64 0}
