target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$arguments$1 = internal global [2 x i8] zeroinitializer

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  store i16 50, ptr @$arguments$1, !tbaa !2
  %0 = tail call cc 1000 addrspace(1) i16 @"DEEP%"(ptr @$arguments$1)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %0)
  ret void
}

define cc 1000 i16 @"DEEP%"(ptr dereferenceable(2) nocapture readonly %0) addrspace(1) "llrm-fp-eval"="x87-extended" memory(argmem: read) nounwind nofree nosync {
b1:
  %1 = alloca i16
  %2 = load i16, ptr %0
  %3 = icmp eq i16 %2, 0
  br i1 %3, label %b5, label %b3

b3:
  %4 = add i16 %2, -1
  store i16 %4, ptr %1, !tbaa !2
  %5 = call cc 1000 addrspace(1) i16 @"DEEP%"(ptr nonnull %1)
  %6 = add i16 %5, 1
  br label %b5

b5:
  %7 = phi i16 [ %6, %b3 ], [ 0, %b1 ]
  ret i16 %7
}

declare cc 1005 void @llrm.qb.B$PEI2(i16) addrspace(1) nocallback

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
