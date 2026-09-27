target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [15 x i8] c"\08\00\08\00\08\00matmul: \00"
@$str2 = internal constant [19 x i8] c"\08\00\0C\00\0C\00matmul: bad \00"

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
  %16 = phi i32 [ 0, %b13 ], [ %21, %b16 ]
  %17 = icmp slt i32 %16, 8
  br i1 %17, label %b15, label %b17

b15:
  %18 = shl i32 %16, 3
  %19 = mul i32 %16, 3
  %20 = add i32 %19, 2
  br label %b18

b16:
  %21 = add i32 %16, 1
  br label %b14

b17:
  br label %b31

b18:
  %22 = phi i32 [ %20, %b15 ], [ %28, %b20 ]
  %23 = phi i32 [ 0, %b15 ], [ %27, %b20 ]
  %24 = icmp slt i32 %23, 8
  br i1 %24, label %b19, label %b16

b19:
  %25 = add i32 %18, %23
  %26 = icmp ult i32 %25, 64
  br i1 %26, label %b22, label %b23

b20:
  %27 = add i32 %23, 1
  %28 = add i32 %22, 1
  br label %b18

b22:
  %29 = trunc i32 %25 to i16
  %30 = getelementptr inbounds i32, ptr %3, i16 %29
  store i32 %22, ptr %30, !tbaa !2
  %31 = icmp eq i32 %16, %23
  br i1 %31, label %b24, label %b29

b23:
  call addrspace(1) void @N$EBND()
  unreachable

b24:
  %32 = getelementptr inbounds i32, ptr %2, i16 %29
  store i32 2, ptr %32, !tbaa !2
  br label %b20

b29:
  %33 = add i32 %16, %23
  %34 = srem i32 %33, 3
  %35 = getelementptr inbounds i32, ptr %2, i16 %29
  store i32 %34, ptr %35, !tbaa !2
  br label %b20

b31:
  %36 = phi i32 [ 0, %b17 ], [ %39, %b33 ]
  %37 = icmp slt i32 %36, 8
  br i1 %37, label %b32, label %b34

b32:
  %38 = shl i32 %36, 3
  br label %b35

b33:
  %39 = add i32 %36, 1
  br label %b31

b34:
  br label %b49

b35:
  %40 = phi i32 [ 0, %b32 ], [ %63, %b47 ]
  %41 = icmp slt i32 %40, 8
  br i1 %41, label %b36, label %b33

b36:
  br label %b39

b39:
  %42 = phi i32 [ 0, %b36 ], [ %59, %b45 ]
  %43 = phi i32 [ 0, %b36 ], [ %60, %b45 ]
  %44 = icmp slt i32 %43, 8
  br i1 %44, label %b40, label %b42

b40:
  %45 = add i32 %38, %43
  %46 = icmp ult i32 %45, 64
  br i1 %46, label %b43, label %b44

b42:
  %47 = add i32 %38, %40
  %48 = icmp ult i32 %47, 64
  br i1 %48, label %b47, label %b48

b43:
  %49 = trunc i32 %45 to i16
  %50 = getelementptr inbounds i32, ptr %3, i16 %49
  %51 = load i32, ptr %50, !tbaa !2
  %52 = shl i32 %43, 3
  %53 = add i32 %52, %40
  %54 = icmp ult i32 %53, 64
  br i1 %54, label %b45, label %b46

b44:
  call addrspace(1) void @N$EBND()
  unreachable

b45:
  %55 = trunc i32 %53 to i16
  %56 = getelementptr inbounds i32, ptr %2, i16 %55
  %57 = load i32, ptr %56, !tbaa !2
  %58 = mul i32 %51, %57
  %59 = add i32 %42, %58
  %60 = add i32 %43, 1
  br label %b39

b46:
  call addrspace(1) void @N$EBND()
  unreachable

b47:
  %61 = trunc i32 %47 to i16
  %62 = getelementptr inbounds i32, ptr %1, i16 %61
  store i32 %42, ptr %62, !tbaa !2
  %63 = add i32 %40, 1
  br label %b35

b48:
  call addrspace(1) void @N$EBND()
  unreachable

b49:
  %64 = phi i32 [ 0, %b34 ], [ %69, %b51 ]
  %65 = phi i32 [ 0, %b34 ], [ %68, %b51 ]
  %66 = icmp slt i32 %65, 8
  br i1 %66, label %b50, label %b52

b50:
  %67 = shl i32 %65, 3
  br label %b53

b51:
  %68 = add i32 %65, 1
  br label %b49

b52:
  ret i32 %64

b53:
  %69 = phi i32 [ %64, %b50 ], [ %79, %b57 ]
  %70 = phi i32 [ 0, %b50 ], [ %80, %b57 ]
  %71 = icmp slt i32 %70, 8
  br i1 %71, label %b54, label %b51

b54:
  %72 = add i32 %67, %70
  %73 = icmp ult i32 %72, 64
  br i1 %73, label %b57, label %b58

b57:
  %74 = trunc i32 %72 to i16
  %75 = getelementptr inbounds i32, ptr %1, i16 %74
  %76 = load i32, ptr %75, !tbaa !2
  %77 = add i32 %72, 1
  %78 = mul i32 %76, %77
  %79 = add i32 %69, %78
  %80 = add i32 %70, 1
  br label %b53

b58:
  call addrspace(1) void @N$EBND()
  unreachable
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = call addrspace(1) i32 @matmul(i32 1)
  %1 = icmp eq i32 %0, 372432
  br i1 %1, label %b2, label %b3

b2:
  %2 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %2)
  call addrspace(1) void @N$PI4(i32 %0)
  call addrspace(1) void @N$PN()
  ret i16 0

b3:
  %3 = getelementptr i8, ptr @$str2, i16 6
  call addrspace(1) void @N$PS(ptr %3)
  call addrspace(1) void @N$PI4(i32 %0)
  call addrspace(1) void @N$PN()
  ret i16 1
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$EBND() addrspace(1)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PI4(i32) addrspace(1)

declare void @N$PN() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
