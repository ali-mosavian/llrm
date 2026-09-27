target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

define internal i16 @sum(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) willreturn {
b1:
  %1 = load i16, ptr addrspace(1) %0
  %2 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %3 = load ptr addrspace(1), ptr addrspace(1) %2
  br label %b2

b2:
  %4 = phi i16 [ 0, %b1 ], [ %10, %b3 ]
  %5 = phi i16 [ 0, %b1 ], [ %11, %b3 ]
  %6 = icmp ult i16 %5, %1
  br i1 %6, label %b3, label %b5

b3:
  %7 = shl i16 %5, 1
  %8 = getelementptr i8, ptr addrspace(1) %3, i16 %7
  %9 = load i16, ptr addrspace(1) %8
  %10 = add i16 %4, %9
  %11 = add i16 %5, 1
  br label %b2

b5:
  ret i16 %4
}

define internal i16 @main() addrspace(1) willreturn {
b1:
  %0 = alloca [8 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 8, i1 false)
  %1 = getelementptr inbounds i16, ptr %0, i16 0
  store i16 1, ptr %1, !tbaa !2
  %2 = getelementptr inbounds i16, ptr %0, i16 1
  store i16 2, ptr %2, !tbaa !2
  %3 = getelementptr inbounds i16, ptr %0, i16 2
  store i16 3, ptr %3, !tbaa !2
  %4 = getelementptr inbounds i16, ptr %0, i16 3
  store i16 4, ptr %4, !tbaa !2
  %5 = addrspacecast ptr %0 to ptr addrspace(1)
  br label %6

6:
  %7 = phi i16 [ 0, %b1 ], [ %14, %10 ]
  %8 = phi i16 [ 0, %b1 ], [ %15, %10 ]
  %9 = icmp ult i16 %8, 4
  br i1 %9, label %10, label %16

10:
  %11 = shl i16 %8, 1
  %12 = getelementptr i8, ptr addrspace(1) %5, i16 %11
  %13 = load i16, ptr addrspace(1) %12
  %14 = add i16 %7, %13
  %15 = add i16 %8, 1
  br label %6

16:
  ret i16 %7
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
