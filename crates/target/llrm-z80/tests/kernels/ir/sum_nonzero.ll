target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
; ModuleID = 't/sum_nonzero.c'
source_filename = "t/sum_nonzero.c"
target triple = "msp430"

@a = dso_local global [64 x i16] zeroinitializer, align 2
@b = dso_local global [64 x i32] zeroinitializer, align 2

; Function Attrs: nounwind
define dso_local void @sum() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i16, align 2
  %3 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #3
  call void @llvm.lifetime.start.p0(ptr %2) #3
  store i16 0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  store i32 0, ptr %3, align 2, !tbaa !9
  store i16 0, ptr %1, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %22, %0
  %5 = load i16, ptr %1, align 2, !tbaa !7
  %6 = icmp slt i16 %5, 64
  br i1 %6, label %7, label %25

7:                                                ; preds = %4
  %8 = load i16, ptr %1, align 2, !tbaa !7
  %9 = getelementptr inbounds [64 x i16], ptr @a, i16 0, i16 %8
  %10 = load i16, ptr %9, align 2, !tbaa !7
  %11 = sext i16 %10 to i32
  %12 = load i32, ptr %3, align 2, !tbaa !9
  %13 = add nsw i32 %12, %11
  store i32 %13, ptr %3, align 2, !tbaa !9
  %14 = load i16, ptr %1, align 2, !tbaa !7
  %15 = getelementptr inbounds [64 x i32], ptr @b, i16 0, i16 %14
  %16 = load i32, ptr %15, align 2, !tbaa !9
  %17 = icmp ne i32 %16, 0
  br i1 %17, label %18, label %21

18:                                               ; preds = %7
  %19 = load i16, ptr %2, align 2, !tbaa !7
  %20 = add nsw i16 %19, 1
  store i16 %20, ptr %2, align 2, !tbaa !7
  br label %21

21:                                               ; preds = %18, %7
  br label %22

22:                                               ; preds = %21
  %23 = load i16, ptr %1, align 2, !tbaa !7
  %24 = add nsw i16 %23, 1
  store i16 %24, ptr %1, align 2, !tbaa !7
  br label %4, !llvm.loop !11

25:                                               ; preds = %4
  %26 = load i32, ptr %3, align 2, !tbaa !9
  call void @report(i32 noundef %26)
  %27 = load i16, ptr %2, align 2, !tbaa !7
  %28 = sext i16 %27 to i32
  call void @report(i32 noundef %28)
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret void
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

declare dso_local void @report(i32 noundef) #2

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

attributes #0 = { nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #3 = { nounwind }

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
!8 = !{!"short", !5, i64 0}
!9 = !{!10, !10, i64 0}
!10 = !{!"long", !5, i64 0}
!11 = distinct !{!11, !12, !13}
!12 = !{!"llvm.loop.mustprogress"}
!13 = !{!"llvm.loop.unroll.disable"}
