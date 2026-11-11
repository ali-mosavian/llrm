target datalayout = "e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32"

define cc 1002 i32 @bench_queens_(i32 inreg noundef %0) nounwind "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87" nofree nosync memory(none) {
  %2 = alloca [12 x i32], align 4
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  %3 = call cc 1002 i32 @place_(ptr inreg noundef %2, i32 inreg noundef 0, i32 inreg noundef %0)
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  ret i32 %3
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

define internal cc 1002 i32 @place_(ptr inreg noundef nonnull nocapture nofree %0, i32 inreg noundef %1, i32 inreg noundef %2) nounwind "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87" nearcode nofree nosync memory(argmem: readwrite) {
  %4 = icmp eq i32 %1, %2
  br i1 %4, label %32, label %5

5:
  %6 = icmp sgt i32 %2, 0
  br i1 %6, label %7, label %32

7:
  %8 = icmp sgt i32 %1, 0
  %9 = getelementptr inbounds [4 x i8], ptr %0, i32 %1
  %10 = add nsw i32 %1, 1
  br label %11

11:
  %12 = phi i32 [ 0, %7 ], [ %30, %29 ]
  %13 = phi i32 [ 0, %7 ], [ %31, %29 ]
  br i1 %8, label %16, label %26

14:
  %15 = add nuw nsw i32 %17, 1
  %exitcond = icmp eq i32 %15, %1
  br i1 %exitcond, label %26, label %16

16:
  %17 = phi i32 [ %15, %14 ], [ 0, %11 ]
  %18 = getelementptr inbounds nuw [4 x i8], ptr %0, i32 %17
  %19 = load i32, ptr %18, align 4, !tbaa !8
  %20 = sub nsw i32 %19, %13
  %21 = icmp eq i32 %19, %13
  %22 = sub nuw nsw i32 %1, %17
  %23 = icmp eq i32 %20, %22
  %or.cond = select i1 %21, i1 true, i1 %23
  %24 = sub nsw i32 %17, %1
  %25 = icmp eq i32 %20, %24
  %or.cond1 = select i1 %or.cond, i1 true, i1 %25
  br i1 %or.cond1, label %29, label %14

26:
  store i32 %13, ptr %9, align 4, !tbaa !8
  %27 = tail call cc 1002 i32 @place_(ptr inreg noundef %0, i32 inreg noundef %10, i32 inreg noundef %2)
  %28 = add nsw i32 %27, %12
  br label %29

29:
  %30 = phi i32 [ %28, %26 ], [ %12, %16 ]
  %31 = add nuw nsw i32 %13, 1
  %exitcond1 = icmp eq i32 %31, %2
  br i1 %exitcond1, label %32, label %11

32:
  %33 = phi i32 [ 1, %3 ], [ 0, %5 ], [ %30, %29 ]
  ret i32 %33
}

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

define cc 1002 i32 @main_() nounwind "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87" {
  %1 = alloca [12 x i32], align 4
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  %2 = call cc 1002 i32 @place_(ptr inreg noundef %1, i32 inreg noundef 0, i32 inreg noundef 7)
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  tail call cc 1002 void @report_(i32 inreg noundef %2)
  ret i32 0
}

declare cc 1002 void @report_(i32 inreg noundef) "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87"

define internal cc 1002 range(i32 0, 2) i32 @safe_(ptr inreg noundef nonnull nocapture nofree readonly %0, i32 inreg noundef %1, i32 inreg noundef %2) nounwind "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87" nearcode nofree nosync norecurse memory(argmem: read) {
  unreachable
}

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
!10 = !{!"p1 int", !11, i64 0}
!11 = !{!"any pointer", !6, i64 0}
