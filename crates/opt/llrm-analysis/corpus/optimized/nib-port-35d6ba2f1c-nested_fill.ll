target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

define internal i32 @value(i16 %0) addrspace(1) memory(none) willreturn {
b1:
  %1 = alloca [256 x i8]
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 256, i1 false)
  br label %b2

b2:
  %2 = phi i16 [ 0, %b1 ], [ %15, %b9 ]
  %3 = icmp slt i16 %2, 8
  br i1 %3, label %b3, label %b5

b3:
  %4 = shl i16 %2, 3
  %5 = shl i16 %4, 2
  br label %b6

b5:
  %6 = getelementptr inbounds i32, ptr %1, i16 25
  store i32 5, ptr %6, !tbaa !2
  %7 = getelementptr inbounds i32, ptr %1, i16 26
  %8 = load i32, ptr %7, !tbaa !2
  ret i32 %8

b6:
  %9 = phi i16 [ %5, %b3 ], [ %14, %b7 ]
  %10 = phi i16 [ 0, %b3 ], [ %13, %b7 ]
  %11 = icmp slt i16 %10, 8
  br i1 %11, label %b7, label %b9

b7:
  %12 = getelementptr inbounds i8, ptr %1, i16 %9
  store i32 0, ptr %12, !tbaa !2
  %13 = add i16 %10, 1
  %14 = add i16 %9, 4
  br label %b6

b9:
  %15 = add i16 %2, 1
  br label %b2
}

define internal i16 @main() addrspace(1) memory(none) willreturn {
b1:
  %0 = alloca [256 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 256, i1 false)
  br label %1

1:
  %2 = phi i16 [ 0, %b1 ], [ %21, %20 ]
  %3 = icmp slt i16 %2, 8
  br i1 %3, label %4, label %7

4:
  %5 = shl i16 %2, 3
  %6 = shl i16 %5, 2
  br label %12

7:
  %8 = getelementptr inbounds i32, ptr %0, i16 25
  store i32 5, ptr %8
  %9 = getelementptr inbounds i32, ptr %0, i16 26
  %10 = load i32, ptr %9
  %11 = trunc i32 %10 to i16
  ret i16 %11

12:
  %13 = phi i16 [ %6, %4 ], [ %19, %16 ]
  %14 = phi i16 [ 0, %4 ], [ %18, %16 ]
  %15 = icmp slt i16 %14, 8
  br i1 %15, label %16, label %20

16:
  %17 = getelementptr inbounds i8, ptr %0, i16 %13
  store i32 0, ptr %17
  %18 = add i16 %14, 1
  %19 = add i16 %13, 4
  br label %12

20:
  %21 = add i16 %2, 1
  br label %1
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$EBND() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
