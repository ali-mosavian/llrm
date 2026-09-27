target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

define internal i16 @value() addrspace(1) memory(none) willreturn {
b1:
  %0 = alloca [6 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 6, i1 false)
  br label %b5

b5:
  %1 = phi i16 [ 0, %b1 ], [ %4, %b6 ]
  %2 = icmp slt i16 %1, 3
  br i1 %2, label %b6, label %b8

b6:
  %3 = getelementptr inbounds i16, ptr %0, i16 %1
  store i16 5, ptr %3, !tbaa !2
  %4 = add i16 %1, 1
  br label %b5

b8:
  %5 = getelementptr inbounds i16, ptr %0, i16 0
  %6 = load i16, ptr %5, !tbaa !2
  %7 = getelementptr inbounds i16, ptr %0, i16 2
  %8 = load i16, ptr %7, !tbaa !2
  %9 = add i16 %6, %8
  ret i16 %9
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
