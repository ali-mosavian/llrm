target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@$string3 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string3, i16 4), [6 x i8] c"start\00" }>
@"Y%" = internal global [2 x i8] zeroinitializer
@"X%" = internal global [2 x i8] zeroinitializer
@$string4 = internal constant <{ [2 x i8], ptr, [12 x i8] }> <{ [2 x i8] c"\0B\00", ptr getelementptr (i8, ptr @$string4, i16 4), [12 x i8] c"not reached\00" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string3)
  store i16 0, ptr @"Y%", !tbaa !2
  %0 = load i16, ptr @"Y%", !tbaa !2
  %1 = sext i16 %0 to i32
  %2 = icmp eq i32 %1, 0
  br i1 %2, label %b2, label %b3

b2:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11) cold
  unreachable

b3:
  %3 = sdiv i32 5, %1
  %4 = trunc i32 %3 to i16
  store i16 %4, ptr @"X%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string4)
  ret void
}

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
