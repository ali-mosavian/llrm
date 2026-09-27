target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [15 x i8] c"\08\00\08\00\08\00matmul: \00"

define internal i32 @matmul(i32 %0) addrspace(1) {
b1:
  %1 = alloca [256 x i8]
  %2 = alloca [256 x i8]
  %3 = alloca [256 x i8]
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 256, i1 false)
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 256, i1 false)
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 256, i1 false)
  br label %b2

b2:
  %4 = phi i16 [ 0, %b1 ], [ %7, %b3 ]
  %5 = icmp slt i16 %4, 64
  br i1 %5, label %b3, label %b5

b3:
  %6 = getelementptr inbounds i32, ptr %3, i16 %4
  store i32 0, ptr %6, !tbaa !2
  %7 = add i16 %4, 1
  br label %b2

b5:
  br label %b6

b6:
  %8 = phi i16 [ 0, %b5 ], [ %11, %b7 ]
  %9 = icmp slt i16 %8, 64
  br i1 %9, label %b7, label %b9

b7:
  %10 = getelementptr inbounds i32, ptr %2, i16 %8
  store i32 0, ptr %10, !tbaa !2
  %11 = add i16 %8, 1
  br label %b6

b9:
  br label %b10

b10:
  %12 = phi i16 [ 0, %b9 ], [ %15, %b11 ]
  %13 = icmp slt i16 %12, 64
  br i1 %13, label %b11, label %b13

b11:
  %14 = getelementptr inbounds i32, ptr %1, i16 %12
  store i32 0, ptr %14, !tbaa !2
  %15 = add i16 %12, 1
  br label %b10

b13:
  br label %b14

b14:
  %16 = phi i16 [ 0, %b13 ], [ %21, %b16 ]
  %17 = icmp slt i16 %16, 8
  br i1 %17, label %b15, label %b17

b15:
  %18 = shl i16 %16, 3
  %19 = mul i16 %16, 3
  %20 = add i16 %19, 1
  br label %b18

b16:
  %21 = add i16 %16, 1
  br label %b14

b17:
  br label %b31

b18:
  %22 = phi i16 [ %20, %b15 ], [ %28, %b20 ]
  %23 = phi i16 [ 0, %b15 ], [ %27, %b20 ]
  %24 = icmp slt i16 %23, 8
  br i1 %24, label %b19, label %b16

b19:
  %25 = add i16 %18, %23
  %26 = icmp ult i16 %25, 64
  br i1 %26, label %b22, label %b23

b20:
  %27 = add i16 %23, 1
  %28 = add i16 %22, 1
  br label %b18

b22:
  %29 = sext i16 %22 to i32
  %30 = add i32 %29, 1
  %31 = shl i32 %30, 8
  %32 = sext i32 %31 to i64
  %33 = shl i64 %32, 8
  %34 = sdiv i64 %33, 1024
  %35 = trunc i64 %34 to i32
  %36 = getelementptr inbounds i32, ptr %3, i16 %25
  store i32 %35, ptr %36, !tbaa !2
  %37 = icmp eq i16 %16, %23
  br i1 %37, label %b24, label %b29

b23:
  call addrspace(1) void @N$EBND()
  unreachable

b24:
  %38 = getelementptr inbounds i32, ptr %2, i16 %25
  store i32 512, ptr %38, !tbaa !2
  br label %b20

b29:
  %39 = add i16 %16, %23
  %40 = srem i16 %39, 3
  %41 = sext i16 %40 to i32
  %42 = shl i32 %41, 8
  %43 = sext i32 %42 to i64
  %44 = shl i64 %43, 8
  %45 = sdiv i64 %44, 512
  %46 = trunc i64 %45 to i32
  %47 = getelementptr inbounds i32, ptr %2, i16 %25
  store i32 %46, ptr %47, !tbaa !2
  br label %b20

b31:
  %48 = phi i16 [ 0, %b17 ], [ %51, %b33 ]
  %49 = icmp slt i16 %48, 8
  br i1 %49, label %b32, label %b34

b32:
  %50 = shl i16 %48, 3
  br label %b35

b33:
  %51 = add i16 %48, 1
  br label %b31

b34:
  br label %b49

b35:
  %52 = phi i16 [ 0, %b32 ], [ %76, %b47 ]
  %53 = icmp slt i16 %52, 8
  br i1 %53, label %b36, label %b33

b36:
  br label %b39

b39:
  %54 = phi i32 [ 0, %b36 ], [ %73, %b45 ]
  %55 = phi i16 [ 0, %b36 ], [ %74, %b45 ]
  %56 = icmp slt i16 %55, 8
  br i1 %56, label %b40, label %b42

b40:
  %57 = add i16 %50, %55
  %58 = icmp ult i16 %57, 64
  br i1 %58, label %b43, label %b44

b42:
  %59 = add i16 %50, %52
  %60 = icmp ult i16 %59, 64
  br i1 %60, label %b47, label %b48

b43:
  %61 = getelementptr inbounds i32, ptr %3, i16 %57
  %62 = load i32, ptr %61, !tbaa !2
  %63 = shl i16 %55, 3
  %64 = add i16 %63, %52
  %65 = icmp ult i16 %64, 64
  br i1 %65, label %b45, label %b46

b44:
  call addrspace(1) void @N$EBND()
  unreachable

b45:
  %66 = getelementptr inbounds i32, ptr %2, i16 %64
  %67 = load i32, ptr %66, !tbaa !2
  %68 = sext i32 %62 to i64
  %69 = sext i32 %67 to i64
  %70 = mul i64 %68, %69
  %71 = ashr i64 %70, 8
  %72 = trunc i64 %71 to i32
  %73 = add i32 %54, %72
  %74 = add i16 %55, 1
  br label %b39

b46:
  call addrspace(1) void @N$EBND()
  unreachable

b47:
  %75 = getelementptr inbounds i32, ptr %1, i16 %59
  store i32 %54, ptr %75, !tbaa !2
  %76 = add i16 %52, 1
  br label %b35

b48:
  call addrspace(1) void @N$EBND()
  unreachable

b49:
  %77 = phi i32 [ 0, %b34 ], [ %82, %b51 ]
  %78 = phi i16 [ 0, %b34 ], [ %81, %b51 ]
  %79 = icmp slt i16 %78, 8
  br i1 %79, label %b50, label %b52

b50:
  %80 = shl i16 %78, 3
  br label %b53

b51:
  %81 = add i16 %78, 1
  br label %b49

b52:
  ret i32 %77

b53:
  %82 = phi i32 [ %77, %b50 ], [ %97, %b57 ]
  %83 = phi i16 [ 0, %b50 ], [ %98, %b57 ]
  %84 = icmp slt i16 %83, 8
  br i1 %84, label %b54, label %b51

b54:
  %85 = add i16 %80, %83
  %86 = icmp ult i16 %85, 64
  br i1 %86, label %b57, label %b58

b57:
  %87 = getelementptr inbounds i32, ptr %1, i16 %85
  %88 = load i32, ptr %87, !tbaa !2
  %89 = add i16 %85, 1
  %90 = sext i16 %89 to i32
  %91 = shl i32 %90, 8
  %92 = sext i32 %88 to i64
  %93 = sext i32 %91 to i64
  %94 = mul i64 %92, %93
  %95 = ashr i64 %94, 8
  %96 = trunc i64 %95 to i32
  %97 = add i32 %82, %96
  %98 = add i16 %83, 1
  br label %b53

b58:
  call addrspace(1) void @N$EBND()
  unreachable
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = call addrspace(1) i32 @matmul(i32 1)
  %1 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %1)
  call addrspace(1) void @N$PQ4(i32 %0, i8 8)
  call addrspace(1) void @N$PN()
  ret i16 0
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$EBND() addrspace(1)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PQ4(i32, i8) addrspace(1)

declare void @N$PN() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
