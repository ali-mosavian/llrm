target datalayout = "e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32"

@buf = internal unnamed_addr global [256 x i8] zeroinitializer, align 1
@switch.table.0 = private unnamed_addr constant [14 x i8] [i8 120, i8 124, i8 poison, i8 poison, i8 poison, i8 poison, i8 poison, i8 poison, i8 poison, i8 -128, i8 poison, i8 poison, i8 poison, i8 116]

define cc 1003 i32 @main() nounwind "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87" {
  br label %1

1:
  %2 = phi i32 [ 0, %0 ], [ %18, %16 ]
  %3 = phi i32 [ 0, %0 ], [ %5, %16 ]
  %4 = mul nsw i32 %2, 31
  %5 = add nuw nsw i32 %3, 1
  %6 = add i32 %3, -7
  %7 = icmp ult i32 %6, 14
  %8 = trunc i32 %6 to i16
  %9 = lshr i16 8707, %8
  %10 = trunc i16 %9 to i1
  %or.cond = select i1 %7, i1 %10, i1 false
  br i1 %or.cond, label %switch.lookup, label %16

switch.lookup:
  %11 = getelementptr inbounds nuw i8, ptr @switch.table.0, i32 %6
  %switch.load = load i8, ptr %11
  %switch.ext = zext i8 %switch.load to i32
  %12 = getelementptr inbounds nuw i8, ptr @buf, i32 %switch.ext
  %13 = load i8, ptr %12, align 1, !tbaa !12
  %14 = trunc nuw nsw i32 %5 to i8
  %15 = add i8 %13, %14
  store i8 %15, ptr %12, align 1, !tbaa !12
  br label %16

16:
  %17 = phi i32 [ 1, %switch.lookup ], [ 0, %1 ]
  %18 = add nuw nsw i32 %17, %4
  %exitcond = icmp eq i32 %5, 24
  br i1 %exitcond, label %19, label %1, !llvm.loop !10

19:
  br label %20

20:
  %21 = phi i32 [ %27, %20 ], [ %18, %19 ]
  %22 = phi i32 [ %28, %20 ], [ 0, %19 ]
  %23 = mul nsw i32 %21, 7
  %24 = getelementptr inbounds nuw i8, ptr @buf, i32 %22
  %25 = load i8, ptr %24, align 1, !tbaa !12
  %26 = sext i8 %25 to i32
  %27 = add nsw i32 %23, %26
  %28 = add nuw nsw i32 %22, 1
  %exitcond1 = icmp eq i32 %28, 256
  br i1 %exitcond1, label %29, label %20, !llvm.loop !13

29:
  tail call cc 1003 void @report(i32 noundef %27)
  ret i32 0
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare cc 1003 void @report(i32 noundef) "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87"

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

!llvm.module.flags = !{!0}
!llvm.ident = !{!1}
!llvm.errno.tbaa = !{!2}

!0 = !{i32 1, !"NumRegisterParameters", i32 0}
!1 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!2 = !{!3, !4, i64 0}
!3 = !{!"__libc_errno", !4, i64 0}
!4 = !{!"int", !5, i64 0}
!5 = !{!"omnipotent char", !6, i64 0}
!6 = !{!"Simple C/C++ TBAA"}
!7 = !{!8, !8, i64 0}
!8 = !{!"long", !5, i64 0}
!9 = !{!4, !4, i64 0}
!10 = distinct !{!10, !11}
!11 = !{!"llvm.loop.mustprogress"}
!12 = !{!5, !5, i64 0}
!13 = distinct !{!13, !11}
