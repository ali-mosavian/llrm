target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [15 x i8] c"\08\00\08\00\08\00matmul: \00"

define internal void @multiply(ptr addrspace(1) noalias readonly dereferenceable(10) %0, ptr addrspace(1) noalias readonly dereferenceable(10) %1, ptr addrspace(1) noalias readonly dereferenceable(10) %2) addrspace(1) {
b1:
  %3 = load i16, ptr addrspace(1) %0
  %4 = getelementptr i8, ptr addrspace(1) %1, i16 2
  %5 = load i16, ptr addrspace(1) %4
  %6 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %7 = load i16, ptr addrspace(1) %6
  %8 = getelementptr i8, ptr addrspace(1) %0, i16 6
  %9 = load ptr addrspace(1), ptr addrspace(1) %8
  %10 = load i16, ptr addrspace(1) %1
  %11 = getelementptr i8, ptr addrspace(1) %1, i16 6
  %12 = load ptr addrspace(1), ptr addrspace(1) %11
  %13 = load i16, ptr addrspace(1) %2
  %14 = getelementptr i8, ptr addrspace(1) %2, i16 2
  %15 = load i16, ptr addrspace(1) %14
  %16 = getelementptr i8, ptr addrspace(1) %2, i16 6
  %17 = load ptr addrspace(1), ptr addrspace(1) %16
  br label %b2

b2:
  %18 = phi i16 [ 0, %b1 ], [ %28, %b9 ]
  %19 = icmp ult i16 %18, %3
  br i1 %19, label %b3, label %b5

b3:
  %20 = mul i16 %18, %7
  %21 = icmp ult i16 %18, %13
  %22 = mul i16 %18, %15
  %23 = shl i16 %22, 2
  br label %b6

b5:
  ret void

b6:
  %24 = phi i16 [ %23, %b3 ], [ %52, %b24 ]
  %25 = phi i16 [ 0, %b3 ], [ %51, %b24 ]
  %26 = icmp ult i16 %25, %5
  br i1 %26, label %b7, label %b9

b7:
  %27 = shl i16 %20, 2
  br label %b10

b9:
  %28 = add i16 %18, 1
  br label %b2

b10:
  %29 = phi i16 [ %27, %b7 ], [ %48, %b18 ]
  %30 = phi i32 [ 0, %b7 ], [ %46, %b18 ]
  %31 = phi i16 [ 0, %b7 ], [ %47, %b18 ]
  %32 = icmp ult i16 %31, %7
  br i1 %32, label %b16, label %b13

b13:
  br i1 %21, label %b22, label %b23

b16:
  %33 = getelementptr i8, ptr addrspace(1) %9, i16 %29
  %34 = load i32, ptr addrspace(1) %33
  %35 = icmp ult i16 %31, %10
  br i1 %35, label %b18, label %b19

b18:
  %36 = mul i16 %31, %5
  %37 = add i16 %36, %25
  %38 = shl i16 %37, 2
  %39 = getelementptr i8, ptr addrspace(1) %12, i16 %38
  %40 = load i32, ptr addrspace(1) %39
  %41 = sext i32 %34 to i64
  %42 = sext i32 %40 to i64
  %43 = mul i64 %41, %42
  %44 = ashr i64 %43, 8
  %45 = trunc i64 %44 to i32
  %46 = add i32 %30, %45
  %47 = add i16 %31, 1
  %48 = add i16 %29, 4
  br label %b10

b19:
  call addrspace(1) void @N$EBND()
  unreachable

b22:
  %49 = icmp ult i16 %25, %15
  br i1 %49, label %b24, label %b25

b23:
  call addrspace(1) void @N$EBND()
  unreachable

b24:
  %50 = getelementptr i8, ptr addrspace(1) %17, i16 %24
  store i32 %30, ptr addrspace(1) %50
  %51 = add i16 %25, 1
  %52 = add i16 %24, 4
  br label %b6

b25:
  call addrspace(1) void @N$EBND()
  unreachable
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca [10 x i8]
  %1 = alloca [10 x i8]
  %2 = alloca [10 x i8]
  %3 = alloca [256 x i8]
  %4 = alloca [256 x i8]
  %5 = alloca [256 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 10, i1 false)
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 10, i1 false)
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 10, i1 false)
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 256, i1 false)
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 256, i1 false)
  call void @llvm.memset.p0.i16(ptr %5, i8 0, i16 256, i1 false)
  br label %b2

b2:
  %6 = phi i16 [ 0, %b1 ], [ %16, %b9 ]
  %7 = icmp slt i16 %6, 8
  br i1 %7, label %b3, label %b5

b3:
  %8 = shl i16 %6, 3
  %9 = shl i16 %8, 2
  br label %b6

b5:
  br label %b10

b6:
  %10 = phi i16 [ %9, %b3 ], [ %15, %b7 ]
  %11 = phi i16 [ 0, %b3 ], [ %14, %b7 ]
  %12 = icmp slt i16 %11, 8
  br i1 %12, label %b7, label %b9

b7:
  %13 = getelementptr inbounds i8, ptr %5, i16 %10
  store i32 0, ptr %13, !tbaa !2
  %14 = add i16 %11, 1
  %15 = add i16 %10, 4
  br label %b6

b9:
  %16 = add i16 %6, 1
  br label %b2

b10:
  %17 = phi i16 [ 0, %b5 ], [ %21, %b12 ]
  %18 = icmp slt i16 %17, 8
  br i1 %18, label %b11, label %b13

b11:
  %19 = shl i16 %17, 3
  %20 = shl i16 %19, 2
  br label %b14

b12:
  %21 = add i16 %17, 1
  br label %b10

b13:
  br label %b18

b14:
  %22 = phi i16 [ %20, %b11 ], [ %27, %b15 ]
  %23 = phi i16 [ 0, %b11 ], [ %26, %b15 ]
  %24 = icmp slt i16 %23, 8
  br i1 %24, label %b15, label %b12

b15:
  %25 = getelementptr inbounds i8, ptr %4, i16 %22
  store i32 0, ptr %25, !tbaa !2
  %26 = add i16 %23, 1
  %27 = add i16 %22, 4
  br label %b14

b18:
  %28 = phi i16 [ 0, %b13 ], [ %32, %b20 ]
  %29 = icmp slt i16 %28, 8
  br i1 %29, label %b19, label %b21

b19:
  %30 = shl i16 %28, 3
  %31 = shl i16 %30, 2
  br label %b22

b20:
  %32 = add i16 %28, 1
  br label %b18

b21:
  br label %b26

b22:
  %33 = phi i16 [ %31, %b19 ], [ %38, %b23 ]
  %34 = phi i16 [ 0, %b19 ], [ %37, %b23 ]
  %35 = icmp slt i16 %34, 8
  br i1 %35, label %b23, label %b20

b23:
  %36 = getelementptr inbounds i8, ptr %3, i16 %33
  store i32 0, ptr %36, !tbaa !2
  %37 = add i16 %34, 1
  %38 = add i16 %33, 4
  br label %b22

b26:
  %39 = phi i16 [ 0, %b21 ], [ %44, %b28 ]
  %40 = icmp slt i16 %39, 8
  br i1 %40, label %b27, label %b29

b27:
  %41 = mul i16 %39, 3
  %42 = shl i16 %39, 3
  %43 = add i16 %41, 2
  br label %b30

b28:
  %44 = add i16 %39, 1
  br label %b26

b29:
  %45 = addrspacecast ptr %5 to ptr addrspace(1)
  store i16 8, ptr %2, !tbaa !2
  %46 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 8, ptr %46, !tbaa !2
  %47 = getelementptr inbounds i8, ptr %2, i16 4
  store i16 64, ptr %47, !tbaa !2
  %48 = getelementptr inbounds i8, ptr %2, i16 6
  store ptr addrspace(1) %45, ptr %48, !tbaa !2
  %49 = addrspacecast ptr %2 to ptr addrspace(1)
  %50 = addrspacecast ptr %4 to ptr addrspace(1)
  store i16 8, ptr %1, !tbaa !2
  %51 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 8, ptr %51, !tbaa !2
  %52 = getelementptr inbounds i8, ptr %1, i16 4
  store i16 64, ptr %52, !tbaa !2
  %53 = getelementptr inbounds i8, ptr %1, i16 6
  store ptr addrspace(1) %50, ptr %53, !tbaa !2
  %54 = addrspacecast ptr %1 to ptr addrspace(1)
  %55 = addrspacecast ptr %3 to ptr addrspace(1)
  store i16 8, ptr %0, !tbaa !2
  %56 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 8, ptr %56, !tbaa !2
  %57 = getelementptr inbounds i8, ptr %0, i16 4
  store i16 64, ptr %57, !tbaa !2
  %58 = getelementptr inbounds i8, ptr %0, i16 6
  store ptr addrspace(1) %55, ptr %58, !tbaa !2
  %59 = addrspacecast ptr %0 to ptr addrspace(1)
  call addrspace(1) void @multiply(ptr addrspace(1) %49, ptr addrspace(1) %54, ptr addrspace(1) %59)
  br label %b49

b30:
  %60 = phi i16 [ %43, %b27 ], [ %64, %b32 ]
  %61 = phi i16 [ 0, %b27 ], [ %63, %b32 ]
  %62 = icmp slt i16 %61, 8
  br i1 %62, label %b36, label %b28

b32:
  %63 = add i16 %61, 1
  %64 = add i16 %60, 1
  br label %b30

b36:
  %65 = sext i16 %60 to i32
  %66 = shl i32 %65, 8
  %67 = sext i32 %66 to i64
  %68 = shl i64 %67, 8
  %69 = sdiv i64 %68, 1024
  %70 = trunc i64 %69 to i32
  %71 = add i16 %42, %61
  %72 = getelementptr inbounds i32, ptr %5, i16 %71
  store i32 %70, ptr %72, !tbaa !2
  %73 = icmp eq i16 %39, %61
  br i1 %73, label %b43, label %b47

b43:
  %74 = getelementptr inbounds i32, ptr %4, i16 %71
  store i32 512, ptr %74, !tbaa !2
  br label %b32

b47:
  %75 = add i16 %39, %61
  %76 = srem i16 %75, 3
  %77 = sext i16 %76 to i32
  %78 = shl i32 %77, 8
  %79 = sext i32 %78 to i64
  %80 = shl i64 %79, 8
  %81 = sdiv i64 %80, 512
  %82 = trunc i64 %81 to i32
  %83 = getelementptr inbounds i32, ptr %4, i16 %71
  store i32 %82, ptr %83, !tbaa !2
  br label %b32

b49:
  %84 = phi i32 [ 0, %b29 ], [ %89, %b56 ]
  %85 = phi i16 [ 0, %b29 ], [ %92, %b56 ]
  %86 = icmp slt i16 %85, 8
  br i1 %86, label %b50, label %b52

b50:
  %87 = shl i16 %85, 3
  br label %b53

b52:
  %88 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %88)
  call addrspace(1) void @N$PQ4(i32 %84, i8 8)
  call addrspace(1) void @N$PN()
  ret i16 0

b53:
  %89 = phi i32 [ %84, %b50 ], [ %104, %b59 ]
  %90 = phi i16 [ 0, %b50 ], [ %105, %b59 ]
  %91 = icmp slt i16 %90, 8
  br i1 %91, label %b59, label %b56

b56:
  %92 = add i16 %85, 1
  br label %b49

b59:
  %93 = add i16 %87, %90
  %94 = getelementptr inbounds i32, ptr %3, i16 %93
  %95 = load i32, ptr %94, !tbaa !2
  %96 = add i16 %93, 1
  %97 = sext i16 %96 to i32
  %98 = shl i32 %97, 8
  %99 = sext i32 %95 to i64
  %100 = sext i32 %98 to i64
  %101 = mul i64 %99, %100
  %102 = ashr i64 %101, 8
  %103 = trunc i64 %102 to i32
  %104 = add i32 %89, %103
  %105 = add i16 %90, 1
  br label %b53
}

declare void @N$EBND() addrspace(1)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PQ4(i32, i8) addrspace(1)

declare void @N$PN() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
