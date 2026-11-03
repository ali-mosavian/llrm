source_filename = "kernels.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
target triple = "msp430"

define dso_local i16 @hash16(ptr noundef %0) #0 {
  %2 = alloca ptr, align 2
  %3 = alloca i16, align 2
  store ptr %0, ptr %2, align 2, !tbaa !2
  call void @llvm.lifetime.start.p0(i64 2, ptr %3) #2
  store i16 5381, ptr %3, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %8, %1
  %5 = load ptr, ptr %2, align 2, !tbaa !2
  %6 = load i8, ptr %5, align 1, !tbaa !9
  %7 = icmp ne i8 %6, 0
  br i1 %7, label %8, label %19

8:                                                ; preds = %4
  %9 = load i16, ptr %3, align 2, !tbaa !7
  %10 = shl i16 %9, 5
  %11 = load i16, ptr %3, align 2, !tbaa !7
  %12 = add i16 %10, %11
  %13 = load ptr, ptr %2, align 2, !tbaa !2
  %14 = load i8, ptr %13, align 1, !tbaa !9
  %15 = zext i8 %14 to i16
  %16 = add i16 %12, %15
  store i16 %16, ptr %3, align 2, !tbaa !7
  %17 = load ptr, ptr %2, align 2, !tbaa !2
  %18 = getelementptr inbounds nuw i8, ptr %17, i32 1
  store ptr %18, ptr %2, align 2, !tbaa !2
  br label %4, !llvm.loop !10

19:                                               ; preds = %4
  %20 = load i16, ptr %3, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(i64 2, ptr %3) #2
  ret i16 %20
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) #1

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) #1

attributes #0 = { nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nounwind }

!llvm.module.flags = !{!0}
!llvm.ident = !{!1}

!0 = !{i32 1, !"wchar_size", i32 2}
!1 = !{!"Ubuntu clang version 20.1.8 (0ubuntu4)"}
!2 = !{!3, !3, i64 0}
!3 = !{!"p1 omnipotent char", !4, i64 0}
!4 = !{!"any pointer", !5, i64 0}
!5 = !{!"omnipotent char", !6, i64 0}
!6 = !{!"Simple C/C++ TBAA"}
!7 = !{!8, !8, i64 0}
!8 = !{!"int", !5, i64 0}
!9 = !{!5, !5, i64 0}
!10 = distinct !{!10, !11, !12}
!11 = !{!"llvm.loop.mustprogress"}
!12 = !{!"llvm.loop.unroll.disable"}
