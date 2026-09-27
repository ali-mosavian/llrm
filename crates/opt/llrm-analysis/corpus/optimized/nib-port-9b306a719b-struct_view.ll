target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

define internal i32 @total(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) willreturn {
b1:
  %1 = load i16, ptr addrspace(1) %0
  %2 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %3 = load ptr addrspace(1), ptr addrspace(1) %2
  br label %b2

b2:
  %4 = phi i32 [ 0, %b1 ], [ %11, %b3 ]
  %5 = phi i16 [ 0, %b1 ], [ %12, %b3 ]
  %6 = icmp ult i16 %5, %1
  br i1 %6, label %b3, label %b5

b3:
  %7 = mul i16 %5, 10
  %8 = getelementptr i8, ptr addrspace(1) %3, i16 %7
  %9 = getelementptr i8, ptr addrspace(1) %8, i16 2
  %10 = load i32, ptr addrspace(1) %9
  %11 = add i32 %4, %10
  %12 = add i16 %5, 1
  br label %b2

b5:
  ret i32 %4
}

define internal i16 @main() addrspace(1) willreturn {
b1:
  %0 = alloca [20 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 20, i1 false)
  %1 = getelementptr inbounds [10 x i8], ptr %0, i16 0
  store i16 0, ptr %1, !tbaa !2
  %2 = getelementptr inbounds i8, ptr %1, i16 2
  store i32 1, ptr %2, !tbaa !2
  %3 = getelementptr inbounds i8, ptr %1, i16 6
  store i32 2, ptr %3, !tbaa !2
  %4 = getelementptr inbounds [10 x i8], ptr %0, i16 1
  store i16 0, ptr %4, !tbaa !2
  %5 = getelementptr inbounds i8, ptr %4, i16 2
  store i32 2, ptr %5, !tbaa !2
  %6 = getelementptr inbounds i8, ptr %4, i16 6
  store i32 3, ptr %6, !tbaa !2
  %7 = addrspacecast ptr %0 to ptr addrspace(1)
  br label %8

8:
  %9 = phi i32 [ 0, %b1 ], [ %17, %12 ]
  %10 = phi i16 [ 0, %b1 ], [ %18, %12 ]
  %11 = icmp ult i16 %10, 2
  br i1 %11, label %12, label %19

12:
  %13 = mul i16 %10, 10
  %14 = getelementptr i8, ptr addrspace(1) %7, i16 %13
  %15 = getelementptr i8, ptr addrspace(1) %14, i16 2
  %16 = load i32, ptr addrspace(1) %15
  %17 = add i32 %9, %16
  %18 = add i16 %10, 1
  br label %8

19:
  %20 = trunc i32 %9 to i16
  ret i16 %20
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
