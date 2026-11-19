target triple = "msp430"

@owner = dso_local global [4 x i8] c"\01\FE\03\FC", align 1

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #3
  store i16 0, ptr %2, align 2, !tbaa !7
  br label %3

3:                                                ; preds = %8, %0
  %4 = load i16, ptr %2, align 2, !tbaa !7
  %5 = icmp slt i16 %4, 4
  br i1 %5, label %6, label %11

6:                                                ; preds = %3
  %7 = load i16, ptr %2, align 2, !tbaa !7
  call void @release(i16 noundef signext %7) #4
  br label %8

8:                                                ; preds = %6
  %9 = load i16, ptr %2, align 2, !tbaa !7
  %10 = add nsw i16 %9, 1
  store i16 %10, ptr %2, align 2, !tbaa !7
  br label %3, !llvm.loop !9

11:                                               ; preds = %3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nounwind optsize
define internal void @release(i16 noundef signext %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  %4 = load i16, ptr %2, align 2, !tbaa !7
  %5 = getelementptr inbounds [4 x i8], ptr @owner, i16 0, i16 %4
  %6 = load i8, ptr %5, align 1, !tbaa !11
  %7 = sext i8 %6 to i16
  %8 = sub nsw i16 %7, 1
  store i16 %8, ptr %3, align 2, !tbaa !7
  %9 = load i16, ptr %3, align 2, !tbaa !7
  %10 = sext i16 %9 to i32
  call void @report(i32 noundef %10) #4
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret void
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

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
!8 = !{!"short", !5, i64 0}
!9 = distinct !{!9, !10}
!10 = !{!"llvm.loop.mustprogress"}
!11 = !{!5, !5, i64 0}
