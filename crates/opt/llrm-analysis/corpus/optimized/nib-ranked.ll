target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [8 x i8] c"\08\00\01\00\01\00 \00"

define internal i16 @trace(ptr addrspace(1) noalias readonly dereferenceable(10) %0) addrspace(1) {
b1:
  %1 = load i16, ptr addrspace(1) %0
  %2 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %3 = load i16, ptr addrspace(1) %2
  %4 = getelementptr i8, ptr addrspace(1) %0, i16 6
  %5 = load ptr addrspace(1), ptr addrspace(1) %4
  br label %b2

b2:
  %6 = phi i16 [ 0, %b1 ], [ %15, %b8 ]
  %7 = phi i16 [ 0, %b1 ], [ %16, %b8 ]
  %8 = icmp ult i16 %7, %1
  br i1 %8, label %b3, label %b5

b3:
  %9 = icmp ult i16 %7, %3
  br i1 %9, label %b8, label %b9

b5:
  ret i16 %6

b8:
  %10 = mul i16 %7, %3
  %11 = add i16 %10, %7
  %12 = shl i16 %11, 1
  %13 = getelementptr i8, ptr addrspace(1) %5, i16 %12
  %14 = load i16, ptr addrspace(1) %13
  %15 = add i16 %6, %14
  %16 = add i16 %7, 1
  br label %b2

b9:
  call addrspace(1) void @N$EBND()
  unreachable
}

define internal void @bump(ptr addrspace(1) noalias readonly dereferenceable(10) %0) addrspace(1) {
b1:
  %1 = load i16, ptr addrspace(1) %0
  %2 = icmp ugt i16 %1, 1
  br i1 %2, label %b2, label %b3

b2:
  %3 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %4 = load i16, ptr addrspace(1) %3
  %5 = icmp ugt i16 %4, 2
  br i1 %5, label %b4, label %b5

b3:
  call addrspace(1) void @N$EBND()
  unreachable

b4:
  %6 = add i16 %4, 2
  %7 = getelementptr i8, ptr addrspace(1) %0, i16 6
  %8 = load ptr addrspace(1), ptr addrspace(1) %7
  %9 = shl i16 %6, 1
  %10 = getelementptr i8, ptr addrspace(1) %8, i16 %9
  %11 = load i16, ptr addrspace(1) %10
  %12 = add i16 %11, 100
  store i16 %12, ptr addrspace(1) %10
  ret void

b5:
  call addrspace(1) void @N$EBND()
  unreachable
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca [64 x i8]
  %1 = alloca [24 x i8]
  %2 = alloca [18 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 64, i1 false)
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 24, i1 false)
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 18, i1 false)
  %3 = getelementptr inbounds i16, ptr %2, i16 0
  store i16 1, ptr %3, !tbaa !2
  %4 = getelementptr inbounds i16, ptr %2, i16 1
  store i16 2, ptr %4, !tbaa !2
  %5 = getelementptr inbounds i16, ptr %2, i16 2
  store i16 3, ptr %5, !tbaa !2
  %6 = getelementptr inbounds i16, ptr %2, i16 3
  store i16 4, ptr %6, !tbaa !2
  %7 = getelementptr inbounds i16, ptr %2, i16 4
  store i16 5, ptr %7, !tbaa !2
  %8 = getelementptr inbounds i16, ptr %2, i16 5
  store i16 6, ptr %8, !tbaa !2
  %9 = getelementptr inbounds i16, ptr %2, i16 6
  store i16 7, ptr %9, !tbaa !2
  %10 = getelementptr inbounds i16, ptr %2, i16 7
  store i16 8, ptr %10, !tbaa !2
  %11 = getelementptr inbounds i16, ptr %2, i16 8
  store i16 9, ptr %11, !tbaa !2
  br label %b2

b2:
  %12 = phi i16 [ 0, %b1 ], [ %21, %b9 ]
  %13 = icmp slt i16 %12, 2
  br i1 %13, label %b3, label %b5

b3:
  %14 = mul i16 %12, 3
  br label %b6

b5:
  %15 = getelementptr inbounds i8, ptr %1, i16 23
  store i8 1, ptr %15, !tbaa !2
  br label %b14

b6:
  %16 = phi i16 [ 0, %b3 ], [ %20, %b8 ]
  %17 = icmp slt i16 %16, 3
  br i1 %17, label %b7, label %b9

b7:
  %18 = add i16 %14, %16
  %19 = shl i16 %18, 2
  br label %b10

b8:
  %20 = add i16 %16, 1
  br label %b6

b9:
  %21 = add i16 %12, 1
  br label %b2

b10:
  %22 = phi i16 [ 0, %b7 ], [ %26, %b11 ]
  %23 = icmp slt i16 %22, 4
  br i1 %23, label %b11, label %b8

b11:
  %24 = add i16 %19, %22
  %25 = getelementptr inbounds i8, ptr %1, i16 %24
  store i8 7, ptr %25, !tbaa !2
  %26 = add i16 %22, 1
  br label %b10

b14:
  %27 = phi i16 [ 0, %b5 ], [ %31, %b16 ]
  %28 = icmp slt i16 %27, 2
  br i1 %28, label %b15, label %b17

b15:
  %29 = shl i16 %27, 1
  %30 = shl i16 %29, 1
  br label %b18

b16:
  %31 = add i16 %27, 1
  br label %b14

b17:
  %32 = getelementptr inbounds i32, ptr %0, i16 10
  store i32 42, ptr %32, !tbaa !2
  %33 = addrspacecast ptr %2 to ptr addrspace(1)
  %34 = getelementptr i8, ptr addrspace(1) %33, i16 10
  %35 = load i16, ptr addrspace(1) %34
  %36 = add i16 %35, 100
  store i16 %36, ptr addrspace(1) %34
  br label %b30

b18:
  %37 = phi i16 [ %30, %b15 ], [ %48, %b25 ]
  %38 = phi i16 [ 0, %b15 ], [ %47, %b25 ]
  %39 = icmp slt i16 %38, 2
  br i1 %39, label %b19, label %b16

b19:
  %40 = shl i16 %37, 1
  br label %b22

b22:
  %41 = phi i16 [ %40, %b19 ], [ %46, %b24 ]
  %42 = phi i16 [ 0, %b19 ], [ %45, %b24 ]
  %43 = icmp slt i16 %42, 2
  br i1 %43, label %b23, label %b25

b23:
  %44 = shl i16 %41, 2
  br label %b26

b24:
  %45 = add i16 %42, 1
  %46 = add i16 %41, 2
  br label %b22

b25:
  %47 = add i16 %38, 1
  %48 = add i16 %37, 2
  br label %b18

b26:
  %49 = phi i16 [ %44, %b23 ], [ %54, %b27 ]
  %50 = phi i16 [ 0, %b23 ], [ %53, %b27 ]
  %51 = icmp slt i16 %50, 2
  br i1 %51, label %b27, label %b24

b27:
  %52 = getelementptr inbounds i8, ptr %0, i16 %49
  store i32 0, ptr %52, !tbaa !2
  %53 = add i16 %50, 1
  %54 = add i16 %49, 4
  br label %b26

b30:
  %55 = phi i16 [ 0, %b17 ], [ %76, %b32 ]
  %56 = phi i16 [ 0, %b17 ], [ %59, %b32 ]
  %57 = icmp slt i16 %56, 2
  br i1 %57, label %b31, label %b33

b31:
  %58 = mul i16 %56, 3
  br label %b34

b32:
  %59 = add i16 %56, 1
  br label %b30

b33:
  br label %60

60:
  %61 = phi i16 [ 0, %b33 ], [ %70, %64 ]
  %62 = phi i16 [ 0, %b33 ], [ %71, %64 ]
  %63 = icmp ult i16 %62, 3
  br i1 %63, label %64, label %72

64:
  %65 = mul i16 %62, 3
  %66 = add i16 %65, %62
  %67 = shl i16 %66, 1
  %68 = getelementptr i8, ptr addrspace(1) %33, i16 %67
  %69 = load i16, ptr addrspace(1) %68
  %70 = add i16 %61, %69
  %71 = add i16 %62, 1
  br label %60

72:
  call addrspace(1) void @N$PI2(i16 %61)
  %73 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %73)
  %74 = load i16, ptr %8, !tbaa !2
  call addrspace(1) void @N$PI2(i16 %74)
  call addrspace(1) void @N$PS(ptr %73)
  call addrspace(1) void @N$PI2(i16 %55)
  call addrspace(1) void @N$PS(ptr %73)
  %75 = load i32, ptr %32, !tbaa !2
  call addrspace(1) void @N$PI4(i32 %75)
  call addrspace(1) void @N$PS(ptr %73)
  call addrspace(1) void @N$PU2(i16 9)
  call addrspace(1) void @N$PS(ptr %73)
  call addrspace(1) void @N$PU2(i16 3)
  call addrspace(1) void @N$PS(ptr %73)
  call addrspace(1) void @N$PU2(i16 4)
  call addrspace(1) void @N$PN()
  ret i16 0

b34:
  %76 = phi i16 [ %55, %b31 ], [ %81, %b41 ]
  %77 = phi i16 [ 0, %b31 ], [ %84, %b41 ]
  %78 = icmp slt i16 %77, 3
  br i1 %78, label %b35, label %b32

b35:
  %79 = add i16 %58, %77
  %80 = shl i16 %79, 2
  br label %b38

b38:
  %81 = phi i16 [ %76, %b35 ], [ %89, %b44 ]
  %82 = phi i16 [ 0, %b35 ], [ %90, %b44 ]
  %83 = icmp slt i16 %82, 4
  br i1 %83, label %b44, label %b41

b41:
  %84 = add i16 %77, 1
  br label %b34

b44:
  %85 = add i16 %80, %82
  %86 = getelementptr inbounds i8, ptr %1, i16 %85
  %87 = load i8, ptr %86, !tbaa !2
  %88 = zext i8 %87 to i16
  %89 = add i16 %81, %88
  %90 = add i16 %82, 1
  br label %b38
}

declare void @N$EBND() addrspace(1)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$PI2(i16) addrspace(1)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PI4(i32) addrspace(1)

declare void @N$PU2(i16) addrspace(1)

declare void @N$PN() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
