target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

define internal i32 @value(i16 %0) addrspace(1) memory(none) willreturn {
b1:
  %1 = alloca [256 x i8]
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 256, i1 false)
  br label %b2

b2:
  %2 = phi i16 [ 0, %b1 ], [ %5, %b3 ]
  %3 = icmp slt i16 %2, 64
  br i1 %3, label %b3, label %b5

b3:
  %4 = getelementptr inbounds i32, ptr %1, i16 %2
  store i32 0, ptr %4, !tbaa !2
  %5 = add i16 %2, 1
  br label %b2

b5:
  %6 = getelementptr inbounds i32, ptr %1, i16 3
  store i32 5, ptr %6, !tbaa !2
  %7 = getelementptr inbounds i32, ptr %1, i16 4
  %8 = load i32, ptr %7, !tbaa !2
  %9 = add i32 %8, 5
  ret i32 %9
}

define internal i16 @main() addrspace(1) memory(none) willreturn {
b1:
  %0 = alloca [256 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 256, i1 false)
  br label %1

1:
  %2 = phi i16 [ 0, %b1 ], [ %6, %4 ]
  %3 = icmp slt i16 %2, 64
  br i1 %3, label %4, label %7

4:
  %5 = getelementptr inbounds i32, ptr %0, i16 %2
  store i32 0, ptr %5
  %6 = add i16 %2, 1
  br label %1

7:
  %8 = getelementptr inbounds i32, ptr %0, i16 3
  store i32 5, ptr %8
  %9 = getelementptr inbounds i32, ptr %0, i16 4
  %10 = load i32, ptr %9
  %11 = add i32 %10, 5
  %12 = trunc i32 %11 to i16
  ret i16 %12
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$EBND() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
