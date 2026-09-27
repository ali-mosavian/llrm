target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca [8 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 8, i1 false)
  %1 = getelementptr inbounds i16, ptr %0, i16 0
  store i16 1, ptr %1, !tbaa !2
  %2 = getelementptr inbounds i16, ptr %0, i16 1
  store i16 2, ptr %2, !tbaa !2
  %3 = getelementptr inbounds i16, ptr %0, i16 2
  store i16 3, ptr %3, !tbaa !2
  %4 = getelementptr inbounds i16, ptr %0, i16 3
  store i16 4, ptr %4, !tbaa !2
  %5 = getelementptr i8, ptr @$str1, i16 6
  br label %b2

b2:
  %6 = phi ptr [ %5, %b1 ], [ %11, %b3 ]
  %7 = phi i16 [ 0, %b1 ], [ %17, %b3 ]
  %8 = icmp ult i16 %7, 4
  br i1 %8, label %b3, label %b5

b3:
  %9 = getelementptr i8, ptr %6, i16 -4
  %10 = load i16, ptr %9
  %11 = call addrspace(1) ptr @N$BGRW(ptr %6, i16 1, i16 2)
  %12 = shl i16 %10, 1
  %13 = getelementptr i8, ptr %11, i16 %12
  %14 = getelementptr inbounds i16, ptr %0, i16 %7
  %15 = load i16, ptr %14, !tbaa !2
  %16 = shl i16 %15, 1
  store i16 %16, ptr %13
  %17 = add i16 %7, 1
  br label %b2

b5:
  %18 = getelementptr i8, ptr %6, i16 -4
  %19 = load i16, ptr %18
  br label %b7

b6:
  call addrspace(1) void @N$BDRP(ptr %6)
  call addrspace(1) void @N$BDRP(ptr null)
  ret i16 %20

b7:
  %20 = phi i16 [ 0, %b5 ], [ %27, %b8 ]
  %21 = phi i16 [ 0, %b5 ], [ %28, %b8 ]
  %22 = icmp ult i16 %21, %19
  br i1 %22, label %b8, label %b6

b8:
  %23 = shl i16 %21, 1
  %24 = getelementptr i8, ptr %6, i16 %23
  %25 = load i16, ptr %24
  %26 = add i16 %25, 1
  %27 = add i16 %20, %26
  %28 = add i16 %21, 1
  br label %b7
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare void @N$BDRP(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
