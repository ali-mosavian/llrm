target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [12 x i8] c"\08\00\05\00\05\00metal\00"
@$str2 = internal constant [22 x i8] c"\08\00\0F\00\0F\00descriptors: ok\00"
@$str3 = internal constant [23 x i8] c"\08\00\10\00\10\00descriptors: bad\00"

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

define internal i8 @first(ptr %0) addrspace(1) {
b1:
  %1 = getelementptr i8, ptr %0, i16 -4
  %2 = load i16, ptr %1
  %3 = icmp ugt i16 %2, 0
  br i1 %3, label %b3, label %b5

b3:
  %4 = getelementptr i8, ptr %0, i16 0
  %5 = load i8, ptr %4
  call addrspace(1) void @N$BDRP(ptr %0)
  ret i8 %5

b5:
  call addrspace(1) void @N$BDRP(ptr %0)
  ret i8 0
}

define internal i16 @main() addrspace(1) {
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
  %6 = getelementptr i8, ptr @$str1, i16 6
  %7 = addrspacecast ptr %0 to ptr addrspace(1)
  %8 = getelementptr i8, ptr addrspace(1) %7, i16 2
  br label %9

9:
  %10 = phi i16 [ 0, %b1 ], [ %17, %13 ]
  %11 = phi i16 [ 0, %b1 ], [ %18, %13 ]
  %12 = icmp ult i16 %11, 3
  br i1 %12, label %13, label %19

13:
  %14 = shl i16 %11, 1
  %15 = getelementptr i8, ptr addrspace(1) %8, i16 %14
  %16 = load i16, ptr addrspace(1) %15
  %17 = add i16 %10, %16
  %18 = add i16 %11, 1
  br label %9

19:
  %20 = icmp eq i16 %10, 14
  br i1 %20, label %b8, label %b10

b8:
  %21 = getelementptr i8, ptr %6, i16 -4
  %22 = load i16, ptr %21
  %23 = icmp eq i16 %22, 5
  br i1 %23, label %b11, label %b13

b10:
  %24 = phi ptr [ %6, %19 ], [ %29, %b13 ]
  %25 = getelementptr i8, ptr @$str3, i16 6
  call addrspace(1) void @N$PS(ptr %25)
  call addrspace(1) void @N$PN()
  call addrspace(1) void @N$BDRP(ptr %24)
  ret i16 1

b11:
  %26 = getelementptr i8, ptr %6, i16 -2
  %27 = load i16, ptr %26
  %28 = icmp eq i16 %27, 5
  br i1 %28, label %b14, label %b16

b13:
  %29 = phi ptr [ %6, %b8 ], [ %38, %b16 ]
  br label %b10

b14:
  %30 = icmp ugt i16 %22, 0
  br i1 %30, label %31, label %34

31:
  %32 = getelementptr i8, ptr %6, i16 0
  %33 = load i8, ptr %32
  call addrspace(1) void @N$BDRP(ptr %6)
  br label %35

34:
  call addrspace(1) void @N$BDRP(ptr %6)
  br label %35

35:
  %36 = phi i8 [ %33, %31 ], [ 0, %34 ]
  %37 = icmp eq i8 %36, 109
  br i1 %37, label %b17, label %b16

b16:
  %38 = phi ptr [ %6, %b11 ], [ null, %35 ]
  br label %b13

b17:
  %39 = getelementptr i8, ptr @$str2, i16 6
  call addrspace(1) void @N$PS(ptr %39)
  call addrspace(1) void @N$PN()
  call addrspace(1) void @N$BDRP(ptr null)
  ret i16 0
}

declare void @N$BDRP(ptr) addrspace(1)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PN() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
