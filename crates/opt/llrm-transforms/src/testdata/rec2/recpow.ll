target datalayout = "e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32"

define cc 1002 i32 @bench_recpow_(i32 inreg noundef %0) nounwind "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87" memory(none) nofree nosync {
  %2 = icmp slt i32 %0, 1
  br i1 %2, label %14, label %3

3:
  %4 = phi i32 [ %12, %3 ], [ 1, %1 ]
  %5 = phi i32 [ %11, %3 ], [ 0, %1 ]
  %6 = urem i32 %4, 97
  %7 = add nuw nsw i32 %6, 2
  %8 = add nuw nsw i32 %4, 40
  %9 = tail call cc 1002 i32 @power_(i32 inreg noundef %7, i32 inreg noundef %8)
  %.lhs.trunc = trunc nsw i32 %9 to i16
  %10 = srem i16 %.lhs.trunc, 1000
  %.sext = sext i16 %10 to i32
  %11 = add nsw i32 %5, %.sext
  %12 = add nuw i32 %4, 1
  %13 = icmp eq i32 %4, %0
  br i1 %13, label %14, label %3

14:
  %15 = phi i32 [ 0, %1 ], [ %11, %3 ]
  ret i32 %15
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

define internal cc 1002 range(i32 -30010, 30011) i32 @power_(i32 inreg noundef range(i32 -30010, 30011) %0, i32 inreg noundef %1) nounwind "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87" nearcode memory(none) nofree nosync {
  %3 = icmp eq i32 %1, 0
  br i1 %3, label %common.ret, label %4

4:
  %5 = phi i32 [ %17, %14 ], [ %1, %2 ]
  %6 = phi i32 [ %16, %14 ], [ %0, %2 ]
  %7 = and i32 %5, 1
  %8 = icmp eq i32 %7, 0
  br i1 %8, label %14, label %9

common.ret:
  %common.op = phi i32 [ %13, %9 ], [ 1, %2 ]
  ret i32 %common.op

9:
  %10 = add nsw i32 %5, -1
  %11 = tail call cc 1002 i32 @power_(i32 inreg noundef %6, i32 inreg noundef %10)
  %12 = mul nsw i32 %11, %6
  %13 = srem i32 %12, 30011
  br label %common.ret

14:
  %15 = mul nsw i32 %6, %6
  %16 = urem i32 %15, 30011
  %17 = ashr exact i32 %5, 1
  br label %4
}

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

define cc 1002 i32 @main_() nounwind "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87" {
  br label %1

1:
  %2 = phi i32 [ %10, %1 ], [ 1, %0 ]
  %3 = phi i32 [ %9, %1 ], [ 0, %0 ]
  %4 = urem i32 %2, 97
  %5 = add nuw nsw i32 %4, 2
  %6 = add nuw nsw i32 %2, 40
  %7 = tail call cc 1002 i32 @power_(i32 inreg noundef %5, i32 inreg noundef %6)
  %.lhs.trunc = trunc nsw i32 %7 to i16
  %8 = srem i16 %.lhs.trunc, 1000
  %.sext = sext i16 %8 to i32
  %9 = add nsw i32 %3, %.sext
  %10 = add nuw nsw i32 %2, 1
  %11 = icmp eq i32 %2, 400
  br i1 %11, label %12, label %1

12:
  tail call cc 1002 void @report_(i32 inreg noundef %9)
  ret i32 0
}

declare cc 1002 void @report_(i32 inreg noundef) "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87"

!llvm.module.flags = !{!0, !1}
!llvm.ident = !{!2}
!llvm.errno.tbaa = !{!3}

!0 = !{i32 1, !"NumRegisterParameters", i32 0}
!1 = !{i32 1, !"wchar_size", i32 2}
!2 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!3 = !{!4, !5, i64 0}
!4 = !{!"__libc_errno", !5, i64 0}
!5 = !{!"int", !6, i64 0}
!6 = !{!"omnipotent char", !7, i64 0}
!7 = !{!"Simple C/C++ TBAA"}
!8 = !{!5, !5, i64 0}
!9 = !{!10, !10, i64 0}
!10 = !{!"long", !6, i64 0}
