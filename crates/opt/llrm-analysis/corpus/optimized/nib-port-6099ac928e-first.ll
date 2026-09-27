target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [12 x i8] c"\08\00\05\00\05\00metal\00"
@$str2 = internal constant [9 x i8] c"\08\00\02\00\02\00ok\00"

define internal i8 @first(ptr %0) addrspace(1) {
b1:
  %1 = getelementptr i8, ptr %0, i16 -4
  %2 = load i16, ptr %1
  %3 = icmp ugt i16 %2, 0
  br i1 %3, label %b3, label %b5

b3:
  %4 = getelementptr i8, ptr %0, i16 0
  %5 = load i8, ptr %4
  call addrspace(1) void @N$BDRP(ptr %0)
  ret i8 %5

b5:
  call addrspace(1) void @N$BDRP(ptr %0)
  ret i8 0
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = getelementptr i8, ptr @$str1, i16 6
  %1 = getelementptr i8, ptr %0, i16 -4
  %2 = load i16, ptr %1
  %3 = icmp ugt i16 %2, 0
  br i1 %3, label %4, label %7

4:
  %5 = getelementptr i8, ptr %0, i16 0
  %6 = load i8, ptr %5
  call addrspace(1) void @N$BDRP(ptr %0)
  br label %8

7:
  call addrspace(1) void @N$BDRP(ptr %0)
  br label %8

8:
  %9 = phi i8 [ %6, %4 ], [ 0, %7 ]
  %10 = icmp eq i8 %9, 109
  br i1 %10, label %b2, label %b3

b2:
  %11 = getelementptr i8, ptr @$str2, i16 6
  call addrspace(1) void @N$PS(ptr %11)
  call addrspace(1) void @N$PN()
  ret i16 0

b3:
  ret i16 1
}

declare void @N$BDRP(ptr) addrspace(1)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PN() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
