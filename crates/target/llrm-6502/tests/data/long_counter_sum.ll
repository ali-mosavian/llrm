target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8-S8"

@sum = internal unnamed_addr global i32 0, align 2
@state = internal unnamed_addr global i32 5, align 2

define internal void @run() noinline nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" memory(readwrite, inaccessiblemem: none) willreturn nofree nosync norecurse {
  %promoted = load i32, ptr @sum, align 2, !tbaa !7
  br label %1

1:
  %2 = phi i32 [ %6, %1 ], [ 1500, %0 ]
  %3 = phi i32 [ %promoted, %0 ], [ %5, %1 ]
  %4 = tail call i32 @next()
  %5 = add i32 %3, %4
  %6 = add nsw i32 %2, -1
  %exitcond = icmp eq i32 %6, 0
  br i1 %exitcond, label %7, label %1, !llvm.loop !9

7:
  store i32 %5, ptr @sum, align 2, !tbaa !7
  ret void
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

define internal i32 @next() noinline nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" memory(readwrite, argmem: none, inaccessiblemem: none) willreturn nofree nosync norecurse {
  %1 = load i32, ptr @state, align 2, !tbaa !7
  %2 = mul i32 %1, 3
  %3 = add i32 %2, 1
  store i32 %3, ptr @state, align 2, !tbaa !7
  ret i32 %3
}

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

define i16 @main() noinline nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" {
  tail call void @run()
  %1 = load i32, ptr @sum, align 2, !tbaa !7
  %2 = and i32 %1, 2147483647
  tail call void @report(i32 noundef %2)
  ret i16 0
}

declare void @report(i32 noundef) "no-trapping-math"="true" "stack-protector-buffer-size"="8"

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
!8 = !{!"long", !5, i64 0}
!9 = distinct !{!9, !10}
!10 = !{!"llvm.loop.mustprogress"}
