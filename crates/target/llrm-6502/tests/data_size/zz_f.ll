target triple = "msp430"

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca double, align 2
  %3 = alloca i64, align 2
  %4 = alloca double, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #3
  store double f0x43F0000000000000, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  store i64 -1, ptr %3, align 2, !tbaa !9
  call void @llvm.lifetime.start.p0(ptr %4) #3
  %5 = load i64, ptr %3, align 2, !tbaa !9
  %6 = uitofp i64 %5 to double
  store double %6, ptr %4, align 2, !tbaa !7
  %7 = load double, ptr %2, align 2, !tbaa !7
  %8 = fcmp olt double %7, f0x43EFFFFFFFFFFFE7
  %9 = zext i1 %8 to i16
  %10 = sext i16 %9 to i32
  call void @report(i32 noundef %10) #4
  %11 = load double, ptr %2, align 2, !tbaa !7
  %12 = fcmp ogt double %11, f0x43F000000000000C
  %13 = zext i1 %12 to i16
  %14 = sext i16 %13 to i32
  call void @report(i32 noundef %14) #4
  call void @report(i32 noundef 0) #4
  %15 = load double, ptr %4, align 2, !tbaa !7
  %16 = load double, ptr %2, align 2, !tbaa !7
  %17 = fcmp oeq double %15, %16
  %18 = zext i1 %17 to i16
  %19 = sext i16 %18 to i32
  call void @report(i32 noundef %19) #4
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #2

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

attributes #0 = { nounwind optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #3 = { nounwind }
attributes #4 = { optsize }

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
!8 = !{!"double", !5, i64 0}
!9 = !{!10, !10, i64 0}
!10 = !{!"long long", !5, i64 0}
