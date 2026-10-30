target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"

define cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = tail call cc 1005 addrspace(1) ptr @N$BGRW(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6), i16 1, i16 2)
  store i16 2, ptr %0
  %1 = getelementptr i8, ptr %0, i16 -4
  %2 = load i16, ptr %1
  %3 = tail call cc 1005 addrspace(1) ptr @N$BGRW(ptr %0, i16 1, i16 2)
  %4 = shl i16 %2, 1
  %5 = getelementptr inbounds i8, ptr %3, i16 %4
  store i16 4, ptr %5
  %6 = getelementptr i8, ptr %3, i16 -4
  %7 = load i16, ptr %6
  %8 = tail call cc 1005 addrspace(1) ptr @N$BGRW(ptr %3, i16 1, i16 2)
  %9 = shl i16 %7, 1
  %10 = getelementptr inbounds i8, ptr %8, i16 %9
  store i16 6, ptr %10
  %11 = getelementptr i8, ptr %8, i16 -4
  %12 = load i16, ptr %11
  %13 = tail call cc 1005 addrspace(1) ptr @N$BGRW(ptr %8, i16 1, i16 2)
  %14 = shl i16 %12, 1
  %15 = getelementptr inbounds i8, ptr %13, i16 %14
  store i16 8, ptr %15
  %16 = getelementptr i8, ptr %13, i16 -4
  %17 = load i16, ptr %16
  %18 = icmp eq i16 %17, 0
  br i1 %18, label %b10, label %19

19:
  br label %b8

b8:
  %20 = phi ptr [ %13, %19 ], [ %27, %b8 ]
  %21 = phi i16 [ %17, %19 ], [ %26, %b8 ]
  %22 = phi i16 [ %25, %b8 ], [ 0, %19 ]
  %23 = load i16, ptr %20
  %24 = add i16 %22, 1
  %25 = add i16 %24, %23
  %26 = add i16 %21, -1
  %27 = getelementptr i8, ptr %20, i16 2
  %exitcond = icmp eq i16 %26, 0
  br i1 %exitcond, label %b10, label %b8

b10:
  %28 = phi i16 [ 0, %b1 ], [ %25, %b8 ]
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %13)
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  ret i16 %28
}

declare cc 1005 ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
