target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"X%" = internal global [2 x i8] zeroinitializer
@$arguments$2 = internal global [2 x i8] zeroinitializer
@$string4 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string4, i16 4), [2 x i8] c"ok" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  store i16 5, ptr @"X%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SCLR(i16 4096, i16 -1)
  %0 = load i16, ptr @"X%", !tbaa !2
  %1 = call cc 1000 addrspace(1) ptr @llrm.qb.B$STI2(i16 %0)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr %1)
  store i16 10, ptr @$arguments$2, !tbaa !2
  call cc 1000 addrspace(1) void @DEEP(ptr @$arguments$2)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string4)
  call cc 1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

define cc 1000 void @DEEP(ptr dereferenceable(2) %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = load i16, ptr %0
  %3 = icmp sgt i16 %2, 0
  br i1 %3, label %b2, label %b3

b2:
  %4 = load i16, ptr %0
  %5 = sub i16 %4, 1
  store i16 %5, ptr %1, !tbaa !2
  call cc 1000 addrspace(1) void @DEEP(ptr %1)
  br label %b4

b3:
  br label %b4

b4:
  ret void
}

declare cc 1000 void @llrm.qb.B$SCLR(i16, i16) addrspace(1)

declare cc 1000 ptr @llrm.qb.B$STI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$CEND() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
