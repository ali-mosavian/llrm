target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
; ModuleID = 's1.c'
source_filename = "s1.c"
target triple = "msp430"

@cells = dso_local global [100 x i16] zeroinitializer, align 2

; Function Attrs: nounwind
define dso_local i32 @total(i16 noundef signext %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca i32, align 2
  %4 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #2
  store i32 0, ptr %3, align 2, !tbaa !9
  call void @llvm.lifetime.start.p0(ptr %4) #2
  store i16 0, ptr %4, align 2, !tbaa !7
  br label %5

5:                                                ; preds = %21, %1
  %6 = load i16, ptr %4, align 2, !tbaa !7
  %7 = load i16, ptr %2, align 2, !tbaa !7
  %8 = icmp slt i16 %6, %7
  br i1 %8, label %10, label %9

9:                                                ; preds = %5
  call void @llvm.lifetime.end.p0(ptr %4) #2
  br label %24

10:                                               ; preds = %5
  %11 = load i16, ptr %4, align 2, !tbaa !7
  %12 = getelementptr inbounds [100 x i16], ptr @cells, i16 0, i16 %11
  %13 = load i16, ptr %12, align 2, !tbaa !7
  %14 = sext i16 %13 to i32
  %15 = load i16, ptr %4, align 2, !tbaa !7
  %16 = add nsw i16 %15, 1
  %17 = sext i16 %16 to i32
  %18 = mul nsw i32 %14, %17
  %19 = load i32, ptr %3, align 2, !tbaa !9
  %20 = add nsw i32 %19, %18
  store i32 %20, ptr %3, align 2, !tbaa !9
  br label %21

21:                                               ; preds = %10
  %22 = load i16, ptr %4, align 2, !tbaa !7
  %23 = add nsw i16 %22, 1
  store i16 %23, ptr %4, align 2, !tbaa !7
  br label %5, !llvm.loop !11

24:                                               ; preds = %9
  %25 = load i32, ptr %3, align 2, !tbaa !9
  call void @llvm.lifetime.end.p0(ptr %3) #2
  ret i32 %25
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nounwind
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #2
  store i16 0, ptr %2, align 2, !tbaa !7
  br label %3

3:                                                ; preds = %12, %0
  %4 = load i16, ptr %2, align 2, !tbaa !7
  %5 = icmp slt i16 %4, 100
  br i1 %5, label %7, label %6

6:                                                ; preds = %3
  call void @llvm.lifetime.end.p0(ptr %2) #2
  br label %15

7:                                                ; preds = %3
  %8 = load i16, ptr %2, align 2, !tbaa !7
  %9 = add nsw i16 1000, %8
  %10 = load i16, ptr %2, align 2, !tbaa !7
  %11 = getelementptr inbounds [100 x i16], ptr @cells, i16 0, i16 %10
  store i16 %9, ptr %11, align 2, !tbaa !7
  br label %12

12:                                               ; preds = %7
  %13 = load i16, ptr %2, align 2, !tbaa !7
  %14 = add nsw i16 %13, 1
  store i16 %14, ptr %2, align 2, !tbaa !7
  br label %3, !llvm.loop !13

15:                                               ; preds = %6
  %16 = call i32 @total(i16 noundef signext 100)
  %17 = trunc i32 %16 to i16
  ret i16 %17
}

attributes #0 = { nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nounwind }

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
!11 = distinct !{!11, !12}
!12 = !{!"llvm.loop.mustprogress"}
!13 = distinct !{!13, !12}
