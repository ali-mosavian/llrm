target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [9 x i8] c"\08\00\02\00\02\00ok\00"
@$str2 = internal constant [10 x i8] c"\08\00\03\00\03\00bad\00"

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

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca [12 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 12, i1 false)
  %1 = getelementptr inbounds i16, ptr %0, i16 0
  store i16 1, ptr %1, !tbaa !2
  %2 = getelementptr inbounds i16, ptr %0, i16 1
  store i16 2, ptr %2, !tbaa !2
  %3 = getelementptr inbounds i16, ptr %0, i16 2
  store i16 3, ptr %3, !tbaa !2
  %4 = getelementptr inbounds i16, ptr %0, i16 3
  store i16 4, ptr %4, !tbaa !2
  %5 = getelementptr inbounds i16, ptr %0, i16 4
  store i16 5, ptr %5, !tbaa !2
  %6 = getelementptr inbounds i16, ptr %0, i16 5
  store i16 6, ptr %6, !tbaa !2
  %7 = addrspacecast ptr %0 to ptr addrspace(1)
  br label %8

8:
  %9 = phi i16 [ 0, %b1 ], [ %16, %12 ]
  %10 = phi i16 [ 0, %b1 ], [ %17, %12 ]
  %11 = icmp ult i16 %10, 6
  br i1 %11, label %12, label %18

12:
  %13 = shl i16 %10, 1
  %14 = getelementptr i8, ptr addrspace(1) %7, i16 %13
  %15 = load i16, ptr addrspace(1) %14
  %16 = add i16 %9, %15
  %17 = add i16 %10, 1
  br label %8

18:
  %19 = icmp eq i16 %9, 21
  br i1 %19, label %b2, label %b3

b2:
  %20 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %20)
  call addrspace(1) void @N$PN()
  br label %b4

b3:
  %21 = getelementptr i8, ptr @$str2, i16 6
  call addrspace(1) void @N$PS(ptr %21)
  call addrspace(1) void @N$PN()
  br label %b4

b4:
  ret i16 %9
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PN() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
