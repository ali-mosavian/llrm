target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

define internal void @nudge(ptr addrspace(1) %0) addrspace(1) memory(argmem: readwrite) willreturn {
b1:
  %1 = load i16, ptr addrspace(1) %0
  %2 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %3 = load i16, ptr addrspace(1) %2
  %4 = add i16 %1, %3
  store i16 %4, ptr addrspace(1) %0
  ret void
}

define internal void @update(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) willreturn {
b1:
  %1 = load i16, ptr addrspace(1) %0
  %2 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %3 = load ptr addrspace(1), ptr addrspace(1) %2
  br label %b2

b2:
  %4 = phi i16 [ 0, %b1 ], [ %12, %b3 ]
  %5 = icmp ult i16 %4, %1
  br i1 %5, label %b3, label %b5

b3:
  %6 = shl i16 %4, 2
  %7 = getelementptr i8, ptr addrspace(1) %3, i16 %6
  %8 = load i16, ptr addrspace(1) %7
  %9 = getelementptr i8, ptr addrspace(1) %7, i16 2
  %10 = load i16, ptr addrspace(1) %9
  %11 = add i16 %8, %10
  store i16 %11, ptr addrspace(1) %7
  %12 = add i16 %4, 1
  br label %b2

b5:
  ret void
}

define internal i16 @calculate() addrspace(1) willreturn {
b1:
  %0 = alloca [8 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 8, i1 false)
  %1 = getelementptr inbounds [4 x i8], ptr %0, i16 0
  store i16 1, ptr %1, !tbaa !2
  %2 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 2, ptr %2, !tbaa !2
  %3 = getelementptr inbounds [4 x i8], ptr %0, i16 1
  store i16 10, ptr %3, !tbaa !2
  %4 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 20, ptr %4, !tbaa !2
  %5 = addrspacecast ptr %0 to ptr addrspace(1)
  br label %6

6:
  %7 = phi i16 [ 0, %b1 ], [ %16, %9 ]
  %8 = icmp ult i16 %7, 2
  br i1 %8, label %9, label %17

9:
  %10 = shl i16 %7, 2
  %11 = getelementptr i8, ptr addrspace(1) %5, i16 %10
  %12 = load i16, ptr addrspace(1) %11
  %13 = getelementptr i8, ptr addrspace(1) %11, i16 2
  %14 = load i16, ptr addrspace(1) %13
  %15 = add i16 %12, %14
  store i16 %15, ptr addrspace(1) %11
  %16 = add i16 %7, 1
  br label %6

17:
  %18 = load i16, ptr %1, !tbaa !2
  %19 = load i16, ptr %3, !tbaa !2
  %20 = add i16 %18, %19
  ret i16 %20
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
