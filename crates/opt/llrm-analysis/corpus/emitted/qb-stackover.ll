target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@$string3 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string3, i16 4), [6 x i8] c"start\00" }>
@$arguments$3 = internal global [2 x i8] zeroinitializer
@$string5 = internal constant <{ [2 x i8], ptr, [12 x i8] }> <{ [2 x i8] c"\0B\00", ptr getelementptr (i8, ptr @$string5, i16 4), [12 x i8] c"unreachable\00" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string3)
  store i16 30000, ptr @$arguments$3, !tbaa !2
  %0 = call cc 1000 addrspace(1) i16 @"DEEP%"(ptr @$arguments$3)
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %0)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string5)
  ret void
}

define cc 1000 i16 @"DEEP%"(ptr dereferenceable(2) %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = load i16, ptr %0
  %4 = icmp eq i16 %3, 0
  br i1 %4, label %b2, label %b3

b2:
  store i16 0, ptr %2, !tbaa !2
  br label %b4

b3:
  %5 = load i16, ptr %0
  %6 = sub i16 %5, 1
  store i16 %6, ptr %1, !tbaa !2
  %7 = call cc 1000 addrspace(1) i16 @"DEEP%"(ptr %1)
  %8 = add i16 1, %7
  store i16 %8, ptr %2, !tbaa !2
  br label %b4

b4:
  br label %b5

b5:
  %9 = load i16, ptr %2, !tbaa !2
  ret i16 %9
}

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
