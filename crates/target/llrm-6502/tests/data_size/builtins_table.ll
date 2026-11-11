target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8"
target triple = "msp430"

; Function Attrs: nounwind optsize willreturn memory(none)
define dso_local i16 @abs(i16 noundef %0) #0 {
  %2 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !7
  %3 = load i16, ptr %2, align 2, !tbaa !7
  %4 = icmp slt i16 %3, 0
  br i1 %4, label %5, label %8

5:                                                ; preds = %1
  %6 = load i16, ptr %2, align 2, !tbaa !7
  %7 = sub nsw i16 0, %6
  br label %10

8:                                                ; preds = %1
  %9 = load i16, ptr %2, align 2, !tbaa !7
  br label %10

10:                                               ; preds = %8, %5
  %11 = phi i16 [ %7, %5 ], [ %9, %8 ]
  ret i16 %11
}

; Function Attrs: nounwind optsize
define dso_local i16 @strlen(ptr noundef %0) #1 {
  %2 = alloca ptr, align 2
  %3 = alloca i16, align 2
  store ptr %0, ptr %2, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %3) #6
  store i16 0, ptr %3, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %10, %1
  %5 = load ptr, ptr %2, align 2, !tbaa !8
  %6 = load i16, ptr %3, align 2, !tbaa !7
  %7 = getelementptr inbounds nuw i8, ptr %5, i16 %6
  %8 = load i8, ptr %7, align 1, !tbaa !11
  %9 = icmp ne i8 %8, 0
  br i1 %9, label %10, label %13

10:                                               ; preds = %4
  %11 = load i16, ptr %3, align 2, !tbaa !7
  %12 = add i16 %11, 1
  store i16 %12, ptr %3, align 2, !tbaa !7
  br label %4, !llvm.loop !12

13:                                               ; preds = %4
  %14 = load i16, ptr %3, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %3) #6
  ret i16 %14
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #2

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #2

; Function Attrs: nounwind optsize
define dso_local i16 @main() #1 {
  %1 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  call void @report(i32 noundef 7) #7
  call void @report(i32 noundef 5) #7
  %2 = call i16 @classify(i16 noundef 5) #7
  %3 = sext i16 %2 to i32
  call void @report(i32 noundef %3) #7
  %4 = call i16 @classify(i16 noundef 40) #7
  %5 = sext i16 %4 to i32
  call void @report(i32 noundef %5) #7
  ret i16 0
}

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #3

; Function Attrs: nounwind optsize
define internal i16 @classify(i16 noundef %0) #1 {
  %2 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !7
  %3 = load i16, ptr %2, align 2, !tbaa !7
  %4 = icmp sgt i16 %3, 100
  br i1 %4, label %5, label %6

5:                                                ; preds = %1
  unreachable

6:                                                ; preds = %1
  call void @llvm.prefetch.p0(ptr %2, i32 0, i32 3, i32 1)
  %7 = load i16, ptr %2, align 2, !tbaa !7
  %8 = call i1 @llvm.is.constant.i16(i16 %7)
  %9 = zext i1 %8 to i16
  %10 = icmp eq i16 %9, 0
  br i1 %10, label %16, label %11

11:                                               ; preds = %6
  %12 = load i16, ptr %2, align 2, !tbaa !7
  %13 = call i1 @llvm.is.constant.i16(i16 %12)
  %14 = zext i1 %13 to i16
  %15 = icmp eq i16 %14, 1
  br label %16

16:                                               ; preds = %11, %6
  %17 = phi i1 [ true, %6 ], [ %15, %11 ]
  %18 = zext i1 %17 to i16
  %19 = load i16, ptr %2, align 2, !tbaa !7
  %20 = add nsw i16 %18, %19
  %21 = sub nsw i16 %20, 1
  ret i16 %21
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite, inaccessiblemem: readwrite)
declare void @llvm.prefetch.p0(ptr readonly captures(none), i32 immarg range(i32 0, 2), i32 immarg range(i32 0, 4), i32 immarg range(i32 0, 2)) #4

; Function Attrs: convergent nocallback nofree nosync nounwind willreturn memory(none)
declare i1 @llvm.is.constant.i16(i16) #5

attributes #0 = { nounwind optsize willreturn memory(none) "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nounwind optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #2 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #3 = { optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #4 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite, inaccessiblemem: readwrite) }
attributes #5 = { convergent nocallback nofree nosync nounwind willreturn memory(none) }
attributes #6 = { nounwind }
attributes #7 = { optsize }

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
!7 = !{!4, !4, i64 0}
!8 = !{!9, !9, i64 0}
!9 = !{!"p1 omnipotent char", !10, i64 0}
!10 = !{!"any pointer", !5, i64 0}
!11 = !{!5, !5, i64 0}
!12 = distinct !{!12, !13}
!13 = !{!"llvm.loop.mustprogress"}
