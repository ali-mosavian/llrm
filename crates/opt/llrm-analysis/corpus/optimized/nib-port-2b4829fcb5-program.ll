target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

define internal i32 @value() addrspace(1) memory(none) willreturn {
b1:
  %0 = alloca [20 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 20, i1 false)
  br label %b2

b2:
  %1 = phi i16 [ 0, %b1 ], [ %4, %b3 ]
  %2 = icmp slt i16 %1, 5
  br i1 %2, label %b3, label %b5

b3:
  %3 = getelementptr inbounds i32, ptr %0, i16 %1
  store i32 7, ptr %3, !tbaa !2
  %4 = add i16 %1, 1
  br label %b2

b5:
  %5 = getelementptr inbounds i32, ptr %0, i16 2
  store i32 1, ptr %5, !tbaa !2
  br label %b6

b6:
  %6 = phi i32 [ 0, %b5 ], [ %11, %b7 ]
  %7 = phi i16 [ 0, %b5 ], [ %12, %b7 ]
  %8 = icmp ult i16 %7, 5
  br i1 %8, label %b7, label %b9

b7:
  %9 = getelementptr inbounds i32, ptr %0, i16 %7
  %10 = load i32, ptr %9, !tbaa !2
  %11 = add i32 %6, %10
  %12 = add i16 %7, 1
  br label %b6

b9:
  ret i32 %6
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
