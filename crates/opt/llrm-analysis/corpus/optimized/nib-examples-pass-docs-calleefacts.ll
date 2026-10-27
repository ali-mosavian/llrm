target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"

define cc 1005 void @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = tail call cc 1005 addrspace(1) ptr @N$BGRW(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6), i16 3, i16 2)
  store i16 1, ptr %0
  %1 = getelementptr inbounds nuw i8, ptr %0, i16 2
  store i16 2, ptr %1
  %2 = getelementptr inbounds nuw i8, ptr %0, i16 4
  store i16 3, ptr %2
  %3 = getelementptr i8, ptr %0, i16 -4
  %4 = load i16, ptr %3
  %5 = icmp eq i16 %4, 1
  br i1 %5, label %b5, label %6

6:
  %7 = icmp eq i16 %4, 0
  br i1 %7, label %b3, label %b4

b3:
  tail call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b4:
  %8 = icmp ugt i16 %4, 2
  br i1 %8, label %9, label %b7

b5:
  tail call cc 1005 addrspace(1) void @N$EBND()
  unreachable

9:
  tail call cc 1005 addrspace(1) void @N$PI2(i16 8)
  tail call cc 1005 addrspace(1) void @N$PN()
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %0)
  ret void

b7:
  tail call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

declare cc 1005 ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
