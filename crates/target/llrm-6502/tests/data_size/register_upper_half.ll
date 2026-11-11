target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8"
target triple = "msp430"

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i32, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #3
  store i32 0, ptr %2, align 2, !tbaa !7
  %3 = call i32 @descend(i32 noundef 2, ptr noundef %2) #4
  call void @report(i32 noundef %3) #4
  %4 = load i32, ptr %2, align 2, !tbaa !7
  call void @report(i32 noundef %4) #4
  %5 = call i32 @descend(i32 noundef 10, ptr noundef %2) #4
  call void @report(i32 noundef %5) #4
  %6 = load i32, ptr %2, align 2, !tbaa !7
  call void @report(i32 noundef %6) #4
  call void @llvm.lifetime.end.p0(ptr %2) #3
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #2

; Function Attrs: nounwind optsize
define internal i32 @descend(i32 noundef %0, ptr noundef %1) #0 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = alloca ptr, align 2
  %6 = alloca i32, align 2
  %7 = alloca i32, align 2
  store i32 %0, ptr %4, align 2, !tbaa !7
  store ptr %1, ptr %5, align 2, !tbaa !9
  call void @llvm.lifetime.start.p0(ptr %6) #3
  %8 = load i32, ptr %4, align 2, !tbaa !7
  %9 = mul nsw i32 %8, 2
  store i32 %9, ptr %6, align 2, !tbaa !7
  %10 = load i32, ptr %4, align 2, !tbaa !7
  %11 = icmp eq i32 %10, 0
  br i1 %11, label %12, label %15

12:                                               ; preds = %2
  %13 = load ptr, ptr %5, align 2, !tbaa !9
  %14 = load i32, ptr %13, align 2, !tbaa !7
  store i32 %14, ptr %3, align 2
  store i32 1, ptr %7, align 2
  br label %28

15:                                               ; preds = %2
  %16 = load i32, ptr %6, align 2, !tbaa !7
  %17 = load ptr, ptr %5, align 2, !tbaa !9
  %18 = load i32, ptr %17, align 2, !tbaa !7
  %19 = add nsw i32 %18, %16
  store i32 %19, ptr %17, align 2, !tbaa !7
  %20 = load i32, ptr %4, align 2, !tbaa !7
  %21 = load i32, ptr %4, align 2, !tbaa !7
  %22 = sub nsw i32 %21, 1
  %23 = load ptr, ptr %5, align 2, !tbaa !9
  %24 = call i32 @descend(i32 noundef %22, ptr noundef %23) #4
  %25 = sub nsw i32 %20, %24
  %26 = load i32, ptr %6, align 2, !tbaa !7
  %27 = add nsw i32 %25, %26
  store i32 %27, ptr %3, align 2
  store i32 1, ptr %7, align 2
  br label %28

28:                                               ; preds = %15, %12
  call void @llvm.lifetime.end.p0(ptr %6) #3
  %29 = load i32, ptr %3, align 2
  ret i32 %29
}

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
!8 = !{!"long", !5, i64 0}
!9 = !{!10, !10, i64 0}
!10 = !{!"p1 long", !11, i64 0}
!11 = !{!"any pointer", !5, i64 0}
