target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

define internal i32 @value(i16 %0) addrspace(1) memory(none) willreturn {
b1:
  %1 = alloca [256 x i8]
  %2 = alloca [256 x i8]
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 256, i1 false)
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 256, i1 false)
  br label %b2

b2:
  %3 = phi i16 [ 0, %b1 ], [ %13, %b9 ]
  %4 = icmp slt i16 %3, 8
  br i1 %4, label %b3, label %b5

b3:
  %5 = shl i16 %3, 3
  %6 = shl i16 %5, 2
  br label %b6

b5:
  br label %b10

b6:
  %7 = phi i16 [ %6, %b3 ], [ %12, %b7 ]
  %8 = phi i16 [ 0, %b3 ], [ %11, %b7 ]
  %9 = icmp slt i16 %8, 8
  br i1 %9, label %b7, label %b9

b7:
  %10 = getelementptr inbounds i8, ptr %2, i16 %7
  store i32 0, ptr %10, !tbaa !2
  %11 = add i16 %8, 1
  %12 = add i16 %7, 4
  br label %b6

b9:
  %13 = add i16 %3, 1
  br label %b2

b10:
  %14 = phi i16 [ 0, %b5 ], [ %18, %b12 ]
  %15 = icmp slt i16 %14, 8
  br i1 %15, label %b11, label %b13

b11:
  %16 = shl i16 %14, 3
  %17 = shl i16 %16, 2
  br label %b14

b12:
  %18 = add i16 %14, 1
  br label %b10

b13:
  br label %b18

b14:
  %19 = phi i16 [ %17, %b11 ], [ %24, %b15 ]
  %20 = phi i16 [ 0, %b11 ], [ %23, %b15 ]
  %21 = icmp slt i16 %20, 8
  br i1 %21, label %b15, label %b12

b15:
  %22 = getelementptr inbounds i8, ptr %1, i16 %19
  store i32 0, ptr %22, !tbaa !2
  %23 = add i16 %20, 1
  %24 = add i16 %19, 4
  br label %b14

b18:
  %25 = phi i16 [ 0, %b13 ], [ %30, %b20 ]
  %26 = icmp slt i16 %25, 8
  br i1 %26, label %b19, label %b21

b19:
  %27 = mul i16 %25, 3
  %28 = shl i16 %25, 3
  %29 = add i16 %27, 1
  br label %b22

b20:
  %30 = add i16 %25, 1
  br label %b18

b21:
  %31 = getelementptr inbounds i32, ptr %2, i16 25
  %32 = load i32, ptr %31, !tbaa !2
  %33 = getelementptr inbounds i32, ptr %1, i16 26
  %34 = load i32, ptr %33, !tbaa !2
  %35 = add i32 %32, %34
  ret i32 %35

b22:
  %36 = phi i16 [ %29, %b19 ], [ %40, %b24 ]
  %37 = phi i16 [ 0, %b19 ], [ %39, %b24 ]
  %38 = icmp slt i16 %37, 8
  br i1 %38, label %b28, label %b20

b24:
  %39 = add i16 %37, 1
  %40 = add i16 %36, 1
  br label %b22

b28:
  %41 = sext i16 %36 to i32
  %42 = shl i32 %41, 8
  %43 = sext i32 %42 to i64
  %44 = shl i64 %43, 8
  %45 = sdiv i64 %44, 1024
  %46 = trunc i64 %45 to i32
  %47 = add i16 %28, %37
  %48 = getelementptr inbounds i32, ptr %2, i16 %47
  store i32 %46, ptr %48, !tbaa !2
  %49 = icmp eq i16 %25, %37
  br i1 %49, label %b35, label %b39

b35:
  %50 = getelementptr inbounds i32, ptr %1, i16 %47
  store i32 512, ptr %50, !tbaa !2
  br label %b24

b39:
  %51 = add i16 %25, %37
  %52 = srem i16 %51, 3
  %53 = sext i16 %52 to i32
  %54 = shl i32 %53, 8
  %55 = sext i32 %54 to i64
  %56 = shl i64 %55, 8
  %57 = sdiv i64 %56, 512
  %58 = trunc i64 %57 to i32
  %59 = getelementptr inbounds i32, ptr %1, i16 %47
  store i32 %58, ptr %59, !tbaa !2
  br label %b24
}

define internal i16 @main() addrspace(1) memory(none) willreturn {
b1:
  ret i16 0
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$EBND() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
