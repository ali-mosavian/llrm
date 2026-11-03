source_filename = "kernels.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
target triple = "msp430"

define dso_local i16 @slen(ptr noundef %0) #0 {
  %2 = alloca ptr, align 2
  %3 = alloca ptr, align 2
  store ptr %0, ptr %2, align 2, !tbaa !2
  call void @llvm.lifetime.start.p0(i64 2, ptr %3) #2
  %4 = load ptr, ptr %2, align 2, !tbaa !2
  store ptr %4, ptr %3, align 2, !tbaa !2
  br label %5

5:                                                ; preds = %9, %1
  %6 = load ptr, ptr %3, align 2, !tbaa !2
  %7 = load i8, ptr %6, align 1, !tbaa !7
  %8 = icmp ne i8 %7, 0
  br i1 %8, label %9, label %12

9:                                                ; preds = %5
  %10 = load ptr, ptr %3, align 2, !tbaa !2
  %11 = getelementptr inbounds nuw i8, ptr %10, i32 1
  store ptr %11, ptr %3, align 2, !tbaa !2
  br label %5, !llvm.loop !8

12:                                               ; preds = %5
  %13 = load ptr, ptr %3, align 2, !tbaa !2
  %14 = load ptr, ptr %2, align 2, !tbaa !2
  %15 = ptrtoint ptr %13 to i16
  %16 = ptrtoint ptr %14 to i16
  %17 = sub i16 %15, %16
  call void @llvm.lifetime.end.p0(i64 2, ptr %3) #2
  ret i16 %17
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
!7 = !{!5, !5, i64 0}
!8 = distinct !{!8, !9, !10}
!9 = !{!"llvm.loop.mustprogress"}
!10 = !{!"llvm.loop.unroll.disable"}
