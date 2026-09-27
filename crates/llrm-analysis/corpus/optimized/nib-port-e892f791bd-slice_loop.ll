target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

define internal i16 @main() addrspace(1) willreturn {
b1:
  %0 = alloca [10 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 10, i1 false)
  %1 = getelementptr inbounds i16, ptr %0, i16 0
  store i16 1, ptr %1, !tbaa !2
  %2 = getelementptr inbounds i16, ptr %0, i16 1
  store i16 2, ptr %2, !tbaa !2
  %3 = getelementptr inbounds i16, ptr %0, i16 2
  store i16 4, ptr %3, !tbaa !2
  %4 = getelementptr inbounds i16, ptr %0, i16 3
  store i16 8, ptr %4, !tbaa !2
  %5 = getelementptr inbounds i16, ptr %0, i16 4
  store i16 16, ptr %5, !tbaa !2
  %6 = addrspacecast ptr %0 to ptr addrspace(1)
  %7 = getelementptr i8, ptr addrspace(1) %6, i16 2
  br label %b2

b2:
  %8 = phi i16 [ 0, %b1 ], [ %14, %b3 ]
  %9 = phi i16 [ 0, %b1 ], [ %15, %b3 ]
  %10 = icmp ult i16 %9, 3
  br i1 %10, label %b3, label %b5

b3:
  %11 = shl i16 %9, 1
  %12 = getelementptr i8, ptr addrspace(1) %7, i16 %11
  %13 = load i16, ptr addrspace(1) %12
  %14 = add i16 %8, %13
  %15 = add i16 %9, 1
  br label %b2

b5:
  ret i16 %8
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
