target triple = "msp430"

@.str = private unnamed_addr constant [13 x i8] c"strlen idiom\00", align 1
@.str.1 = private unnamed_addr constant [1 x i8] zeroinitializer, align 1

; Function Attrs: nounwind optsize
define dso_local i16 @strlen(ptr noundef %0) #0 {
  %2 = alloca ptr, align 2
  %3 = alloca i16, align 2
  store ptr %0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  store i16 0, ptr %3, align 2, !tbaa !10
  br label %4

4:                                                ; preds = %10, %1
  %5 = load ptr, ptr %2, align 2, !tbaa !7
  %6 = load i16, ptr %3, align 2, !tbaa !10
  %7 = getelementptr inbounds nuw i8, ptr %5, i16 %6
  %8 = load i8, ptr %7, align 1, !tbaa !11
  %9 = icmp ne i8 %8, 0
  br i1 %9, label %10, label %13

10:                                               ; preds = %4
  %11 = load i16, ptr %3, align 2, !tbaa !10
  %12 = add i16 %11, 1
  store i16 %12, ptr %3, align 2, !tbaa !10
  br label %4, !llvm.loop !12

13:                                               ; preds = %4
  %14 = load i16, ptr %3, align 2, !tbaa !10
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret i16 %14
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca ptr, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #3
  store ptr @.str, ptr %2, align 2, !tbaa !7
  %3 = load ptr, ptr %2, align 2, !tbaa !7
  %4 = call i16 @count(ptr noundef %3) #4
  %5 = sext i16 %4 to i32
  call void @report(i32 noundef %5) #4
  %6 = load ptr, ptr %2, align 2, !tbaa !7
  %7 = call ptr @end_of(ptr noundef %6) #4
  %8 = load ptr, ptr %2, align 2, !tbaa !7
  %9 = ptrtoaddr ptr %7 to i16
  %10 = ptrtoaddr ptr %8 to i16
  %11 = sub i16 %9, %10
  %12 = sext i16 %11 to i32
  call void @report(i32 noundef %12) #4
  %13 = call i16 @count(ptr noundef @.str.1) #4
  %14 = sext i16 %13 to i32
  call void @report(i32 noundef %14) #4
  %15 = load ptr, ptr %2, align 2, !tbaa !7
  %16 = call i16 @strlen(ptr noundef %15) #5
  %17 = zext i16 %16 to i32
  call void @report(i32 noundef %17) #4
  call void @llvm.lifetime.end.p0(ptr %2) #3
  ret i16 0
}

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #2

; Function Attrs: nounwind optsize
define internal i16 @count(ptr noundef %0) #0 {
  %2 = alloca ptr, align 2
  %3 = alloca i16, align 2
  store ptr %0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  store i16 0, ptr %3, align 2, !tbaa !10
  br label %4

4:                                                ; preds = %10, %1
  %5 = load ptr, ptr %2, align 2, !tbaa !7
  %6 = load i16, ptr %3, align 2, !tbaa !10
  %7 = getelementptr inbounds i8, ptr %5, i16 %6
  %8 = load i8, ptr %7, align 1, !tbaa !11
  %9 = icmp ne i8 %8, 0
  br i1 %9, label %10, label %13

10:                                               ; preds = %4
  %11 = load i16, ptr %3, align 2, !tbaa !10
  %12 = add nsw i16 %11, 1
  store i16 %12, ptr %3, align 2, !tbaa !10
  br label %4, !llvm.loop !14

13:                                               ; preds = %4
  %14 = load i16, ptr %3, align 2, !tbaa !10
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret i16 %14
}

; Function Attrs: nounwind optsize
define internal ptr @end_of(ptr noundef %0) #0 {
  %2 = alloca ptr, align 2
  store ptr %0, ptr %2, align 2, !tbaa !7
  br label %3

3:                                                ; preds = %7, %1
  %4 = load ptr, ptr %2, align 2, !tbaa !7
  %5 = load i8, ptr %4, align 1, !tbaa !11
  %6 = icmp ne i8 %5, 0
  br i1 %6, label %7, label %10

7:                                                ; preds = %3
  %8 = load ptr, ptr %2, align 2, !tbaa !7
  %9 = getelementptr inbounds nuw i8, ptr %8, i32 1
  store ptr %9, ptr %2, align 2, !tbaa !7
  br label %3, !llvm.loop !15

10:                                               ; preds = %3
  %11 = load ptr, ptr %2, align 2, !tbaa !7
  ret ptr %11
}

attributes #0 = { nounwind optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #3 = { nounwind }
attributes #4 = { optsize }
attributes #5 = { nounwind optsize }

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
!8 = !{!"p1 omnipotent char", !9, i64 0}
!9 = !{!"any pointer", !5, i64 0}
!10 = !{!4, !4, i64 0}
!11 = !{!5, !5, i64 0}
!12 = distinct !{!12, !13}
!13 = !{!"llvm.loop.mustprogress"}
!14 = distinct !{!14, !13}
!15 = distinct !{!15, !13}
