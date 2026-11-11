target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8"
target triple = "msp430"

@table = dso_local global [2 x ptr] [ptr @report, ptr @report], align 2

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #0

; Function Attrs: nounwind optsize
define dso_local i16 @main() #1 {
  %1 = alloca i16, align 2
  %2 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #3
  store i16 0, ptr %2, align 2, !tbaa !7
  br label %3

3:                                                ; preds = %13, %0
  %4 = load i16, ptr %2, align 2, !tbaa !7
  %5 = icmp slt i16 %4, 2
  br i1 %5, label %6, label %16

6:                                                ; preds = %3
  %7 = load i16, ptr %2, align 2, !tbaa !7
  %8 = getelementptr inbounds [2 x ptr], ptr @table, i16 0, i16 %7
  %9 = load ptr, ptr %8, align 2, !tbaa !8
  %10 = load i16, ptr %2, align 2, !tbaa !7
  %11 = add nsw i16 7, %10
  %12 = sext i16 %11 to i32
  call void %9(i32 noundef %12) #4
  br label %13

13:                                               ; preds = %6
  %14 = load i16, ptr %2, align 2, !tbaa !7
  %15 = add nsw i16 %14, 1
  store i16 %15, ptr %2, align 2, !tbaa !7
  br label %3, !llvm.loop !10

16:                                               ; preds = %3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #2

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #2

attributes #0 = { optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nounwind optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #2 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
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
!7 = !{!4, !4, i64 0}
!8 = !{!9, !9, i64 0}
!9 = !{!"any pointer", !5, i64 0}
!10 = distinct !{!10, !11}
!11 = !{!"llvm.loop.mustprogress"}
