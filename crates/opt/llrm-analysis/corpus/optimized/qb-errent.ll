target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"CAUGHT%" = internal global [2 x i8] zeroinitializer
@"NUMERATOR&" = internal global [4 x i8] zeroinitializer
@"DIVISOR&" = internal global [4 x i8] zeroinitializer
@$string3 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string3, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) noinline "llrm-fp-eval"="x87-extended" noreturn personality ptr addrspace(1) @llrm.qb.personality {
frame:
  %site = alloca i16
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %site)
  store i16 0, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$RDI4(ptr addrspace(1) addrspacecast (ptr @"NUMERATOR&" to ptr addrspace(1)))
          to label %10 unwind label %landing

b3:
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 true)
  %0 = load i32, ptr @"DIVISOR&", !tbaa !2
  %1 = icmp eq i32 %0, 0
  br i1 %1, label %b5, label %b10

b5:
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %11 unwind label %landing

b10:
  %2 = load i16, ptr @"CAUGHT%", !tbaa !2
  store i16 2, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %2)
          to label %b11 unwind label %landing

b11:
  store i16 3, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string3)
          to label %b12 unwind label %landing

b12:
  call cc 1005 addrspace(1) void @llrm.qb.B$CEND()
  unreachable

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %3 = extractvalue { ptr, i32 } %landed, 1
  %4 = trunc i32 %3 to i16
  %resumed = call fastcc addrspace(1) i16 @__main$handler(i16 %4)
  %5 = load i16, ptr %site
  switch table i16 %5, label %6 [
    i16 0, label %6
    i16 1, label %7
    i16 2, label %8
    i16 3, label %9
  ]

6:
  br label %b3

7:
  br label %b10

8:
  br label %b11

9:
  br label %b12

10:
  store i16 0, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$RDI4(ptr addrspace(1) addrspacecast (ptr @"DIVISOR&" to ptr addrspace(1)))
          to label %b3 unwind label %landing

11:
  unreachable
}

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1) noreturn

declare cc 1005 void @llrm.qb.B$RDI4(ptr addrspace(1) nocapture noretain) addrspace(1)

declare cc 1000 i16 @llrm.qb.B$FERR() addrspace(1) nounwind

declare cc 1005 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc 1005 void @llrm.qb.B$CEND() addrspace(1) nounwind noreturn

define internal fastcc i16 @__main$handler(i16 %0) addrspace(1) "llrm-fp-eval"="x87-extended" nounwind personality ptr addrspace(1) @llrm.qb.personality {
entry:
  store i16 %0, ptr @"CAUGHT%", !tbaa !2
  tail call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 true)
  ret i16 poison
}

declare i32 @llrm.qb.personality(...) addrspace(1)

declare cc 1000 void @llrm.qb.onerror(i1) addrspace(1) nounwind frame_bound

declare cc 1000 void @llrm.qb.frame(ptr) addrspace(1) nounwind

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
