target datalayout = "e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32"

@_values = internal unnamed_addr global [64 x i32] zeroinitializer, align 4

define cc 1002 i32 @bench_recchop_(i32 inreg noundef %0) nounwind "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87" memory(readwrite, argmem: read, inaccessiblemem: none) nofree nosync {
  br label %2

2:
  %3 = phi i32 [ 0, %1 ], [ %8, %2 ]
  %4 = mul nuw nsw i32 %3, 37
  %5 = add nsw i32 %4, %0
  %6 = and i32 %5, 127
  %7 = getelementptr inbounds nuw [4 x i8], ptr @_values, i32 %3
  store i32 %6, ptr %7, align 4, !tbaa !8
  %8 = add nuw nsw i32 %3, 1
  %exitcond = icmp eq i32 %8, 64
  br i1 %exitcond, label %9, label %2

9:
  %10 = tail call cc 1002 i32 @chop_(i32 inreg noundef 0, i32 inreg noundef 64)
  ret i32 %10
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

define internal cc 1002 i32 @chop_(i32 inreg noundef range(i32 0, 64) %0, i32 inreg noundef range(i32 0, 65) %1) nounwind "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87" nearcode memory(read, argmem: none, inaccessiblemem: none) nofree nosync {
  %3 = add nsw i32 %1, -2
  %4 = icmp sgt i32 %0, %3
  br i1 %4, label %25, label %5

5:
  %6 = phi i32 [ %9, %19 ], [ %0, %2 ]
  %7 = phi i32 [ %23, %19 ], [ 0, %2 ]
  %8 = add nuw nsw i32 %6, %1
  %9 = lshr i32 %8, 1
  %10 = icmp samesign ult i32 %6, %1
  br i1 %10, label %11, label %19

11:
  %12 = phi i32 [ %18, %11 ], [ %6, %5 ]
  %13 = phi i32 [ %17, %11 ], [ 0, %5 ]
  %14 = getelementptr inbounds nuw [4 x i8], ptr @_values, i32 %12
  %15 = load i32, ptr %14, align 4, !tbaa !8
  %16 = and i32 %15, %9
  %17 = add nuw nsw i32 %16, %13
  %18 = add nuw nsw i32 %12, 1
  %exitcond = icmp eq i32 %18, %1
  br i1 %exitcond, label %19, label %11

19:
  %20 = phi i32 [ 0, %5 ], [ %17, %11 ]
  %21 = tail call cc 1002 i32 @chop_(i32 inreg noundef %6, i32 inreg noundef %9)
  %22 = add i32 %20, %7
  %23 = add i32 %22, %21
  %24 = icmp sgt i32 %9, %3
  br i1 %24, label %25, label %5

25:
  %accumulator.tr.lcssa = phi i32 [ 0, %2 ], [ %23, %19 ]
  ret i32 %accumulator.tr.lcssa
}

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

define cc 1002 i32 @main_() nounwind "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87" {
  br label %1

1:
  %2 = phi i32 [ 0, %0 ], [ %7, %1 ]
  %3 = mul nuw nsw i32 %2, 37
  %4 = add nuw nsw i32 %3, 5
  %5 = and i32 %4, 127
  %6 = getelementptr inbounds nuw [4 x i8], ptr @_values, i32 %2
  store i32 %5, ptr %6, align 4, !tbaa !8
  %7 = add nuw nsw i32 %2, 1
  %exitcond = icmp eq i32 %7, 64
  br i1 %exitcond, label %8, label %1

8:
  %9 = tail call cc 1002 i32 @chop_(i32 inreg noundef 0, i32 inreg noundef 64)
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
