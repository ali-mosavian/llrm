target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8"
target triple = "msp430"

@sum = internal global i32 0, align 2
@state = internal global i32 5, align 2

; Function Attrs: nounwind optsize
define dso_local void @run() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #3
  store i32 0, ptr %1, align 2, !tbaa !7
  br label %3

3:                                                ; preds = %11, %0
  %4 = load i32, ptr %1, align 2, !tbaa !7
  %5 = icmp slt i32 %4, 1500
  br i1 %5, label %6, label %14

6:                                                ; preds = %3
  call void @llvm.lifetime.start.p0(ptr %2) #3
  %7 = call i32 @next() #4
  store i32 %7, ptr %2, align 2, !tbaa !7
  %8 = load i32, ptr %2, align 2, !tbaa !7
  %9 = load i32, ptr @sum, align 2, !tbaa !7
  %10 = add i32 %9, %8
  store i32 %10, ptr @sum, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %2) #3
  br label %11

11:                                               ; preds = %6
  %12 = load i32, ptr %1, align 2, !tbaa !7
  %13 = add nsw i32 %12, 1
  store i32 %13, ptr %1, align 2, !tbaa !7
  br label %3, !llvm.loop !9

14:                                               ; preds = %3
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret void
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nounwind optsize
define internal i32 @next() #0 {
  %1 = load i32, ptr @state, align 2, !tbaa !7
  %2 = mul i32 %1, 3
  %3 = add i32 %2, 1
  store i32 %3, ptr @state, align 2, !tbaa !7
  %4 = load i32, ptr @state, align 2, !tbaa !7
  ret i32 %4
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  call void @run() #4
  %2 = load i32, ptr @sum, align 2, !tbaa !7
  %3 = and i32 %2, 2147483647
  call void @report(i32 noundef %3) #4
  ret i16 0
}

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #2

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
!9 = distinct !{!9, !10}
!10 = !{!"llvm.loop.mustprogress"}
