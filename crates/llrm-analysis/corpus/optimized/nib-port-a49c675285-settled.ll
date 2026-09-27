target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

define internal i32 @value(i16 %0) addrspace(1) {
b1:
  %1 = alloca [256 x i8]
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 256, i1 false)
  br label %b2

b2:
  %2 = phi i16 [ 0, %b1 ], [ %13, %b9 ]
  %3 = icmp slt i16 %2, 8
  br i1 %3, label %b3, label %b5

b3:
  %4 = shl i16 %2, 3
  %5 = shl i16 %4, 2
  br label %b6

b5:
  %6 = icmp ult i16 %0, 8
  br i1 %6, label %b10, label %b11

b6:
  %7 = phi i16 [ %5, %b3 ], [ %12, %b7 ]
  %8 = phi i16 [ 0, %b3 ], [ %11, %b7 ]
  %9 = icmp slt i16 %8, 8
  br i1 %9, label %b7, label %b9

b7:
  %10 = getelementptr inbounds i8, ptr %1, i16 %7
  store i32 0, ptr %10, !tbaa !2
  %11 = add i16 %8, 1
  %12 = add i16 %7, 4
  br label %b6

b9:
  %13 = add i16 %2, 1
  br label %b2

b10:
  %14 = shl i16 %0, 3
  %15 = add i16 %14, 1
  %16 = getelementptr inbounds i32, ptr %1, i16 %15
  store i32 5, ptr %16, !tbaa !2
  %17 = add i16 %14, 2
  %18 = getelementptr inbounds i32, ptr %1, i16 %17
  %19 = load i32, ptr %18, !tbaa !2
  ret i32 %19

b11:
  call addrspace(1) void @N$EBND()
  unreachable
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$EBND() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
