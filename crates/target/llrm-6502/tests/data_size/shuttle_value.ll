target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8"
target triple = "msp430"

; Function Attrs: nounwind optsize
define dso_local void @g(i64 noundef %0, i16 noundef %1, i16 noundef %2, ptr noundef %3) #0 {
  %5 = alloca i64, align 2
  %6 = alloca i16, align 2
  %7 = alloca i16, align 2
  %8 = alloca ptr, align 2
  %9 = alloca i32, align 2
  store i64 %0, ptr %5, align 2, !tbaa !7
  store i16 %1, ptr %6, align 2, !tbaa !9
  store i16 %2, ptr %7, align 2, !tbaa !9
  store ptr %3, ptr %8, align 2, !tbaa !10
  call void @llvm.lifetime.start.p0(ptr %9) #3
  %10 = load i64, ptr %5, align 2, !tbaa !7
  %11 = load i16, ptr %6, align 2, !tbaa !9
  %12 = zext i16 %11 to i64
  %13 = lshr i64 %10, %12
  %14 = and i64 %13, 4294967295
  %15 = load i16, ptr %7, align 2, !tbaa !9
  %16 = and i16 %15, 31
  %17 = zext i16 %16 to i64
  %18 = shl i64 %14, %17
  %19 = trunc i64 %18 to i32
  store i32 %19, ptr %9, align 2, !tbaa !13
  %20 = load i32, ptr %9, align 2, !tbaa !13
  call void @report(i32 noundef %20) #4
  %21 = load i32, ptr %9, align 2, !tbaa !13
  %22 = zext i32 %21 to i64
  %23 = and i64 %22, 4294967295
  %24 = load i16, ptr %7, align 2, !tbaa !9
  %25 = zext i16 %24 to i64
  %26 = shl i64 %23, %25
  %27 = load ptr, ptr %8, align 2, !tbaa !10
  %28 = load i64, ptr %27, align 2, !tbaa !7
  %29 = or i64 %28, %26
  store i64 %29, ptr %27, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %9) #3
  ret void
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #2

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i64, align 2
  %3 = alloca i64, align 2
  %4 = alloca i64, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #3
  store i64 0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  store i64 0, ptr %3, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %4) #3
  store i64 0, ptr %4, align 2, !tbaa !7
  call void @g(i64 noundef -2401053092593056409, i16 noundef 0, i16 noundef 0, ptr noundef %2) #4
  call void @g(i64 noundef 1311768467463790320, i16 noundef 8, i16 noundef 3, ptr noundef %3) #4
  call void @g(i64 noundef -1, i16 noundef 33, i16 noundef 37, ptr noundef %4) #4
  %5 = load i64, ptr %2, align 2, !tbaa !7
  %6 = trunc i64 %5 to i32
  call void @report(i32 noundef %6) #4
  %7 = load i64, ptr %3, align 2, !tbaa !7
  %8 = lshr i64 %7, 32
  %9 = trunc i64 %8 to i32
  call void @report(i32 noundef %9) #4
  %10 = load i64, ptr %3, align 2, !tbaa !7
  %11 = trunc i64 %10 to i32
  call void @report(i32 noundef %11) #4
  %12 = load i64, ptr %4, align 2, !tbaa !7
  %13 = lshr i64 %12, 32
  %14 = trunc i64 %13 to i32
  call void @report(i32 noundef %14) #4
  %15 = load i64, ptr %4, align 2, !tbaa !7
  %16 = trunc i64 %15 to i32
  call void @report(i32 noundef %16) #4
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  ret i16 0
}

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
!8 = !{!"long long", !5, i64 0}
!9 = !{!4, !4, i64 0}
!10 = !{!11, !11, i64 0}
!11 = !{!"p1 long long", !12, i64 0}
!12 = !{!"any pointer", !5, i64 0}
!13 = !{!14, !14, i64 0}
!14 = !{!"long", !5, i64 0}
