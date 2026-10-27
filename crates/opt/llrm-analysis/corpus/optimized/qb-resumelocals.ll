target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"DEN%" = internal global [2 x i8] zeroinitializer
@$string3 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\03\00", ptr getelementptr (i8, ptr @$string3, i16 4), [4 x i8] c"err\00" }>
@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c"next" }>
@$string5 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string5, i16 4), [6 x i8] c"again\00" }>

define internal cc 1000 void @__main() addrspace(1) noinline "llrm-fp-eval"="x87-extended" noreturn personality ptr addrspace(1) @llrm.qb.personality {
frame:
  %site = alloca i16
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %site)
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 true)
  br label %b3

b3:
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @NXT()
          to label %b4 unwind label %landing

b4:
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @AGN()
          to label %b5 unwind label %landing

b5:
  call cc 1005 addrspace(1) void @llrm.qb.B$CEND()
  unreachable

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %0 = extractvalue { ptr, i32 } %landed, 1
  %1 = trunc i32 %0 to i16
  %resumed = call fastcc addrspace(1) i16 @__main$handler(i16 %1)
  %2 = icmp eq i16 %resumed, 0
  %3 = load i16, ptr %site
  br i1 %2, label %again, label %next

again:
  %4 = icmp eq i16 %3, 0
  br i1 %4, label %b3, label %b4

next:
  %5 = icmp eq i16 %3, 0
  br i1 %5, label %b4, label %b5
}

define cc 1000 void @NXT() addrspace(1) noinline "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
frame:
  %0 = alloca i16
  %site = alloca i16
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %0)
  store i16 0, ptr %0
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %site)
  store i16 7, ptr %0, !tbaa !2
  br label %b2

b2:
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5)
          to label %7 unwind label %landing

b3:
  store i16 1, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4)
          to label %8 unwind label %landing

b4:
  ret void

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %1 = extractvalue { ptr, i32 } %landed, 1
  %2 = trunc i32 %1 to i16
  %resumed = call fastcc addrspace(1) i16 @__main$handler(i16 %2)
  %3 = icmp eq i16 %resumed, 0
  %4 = load i16, ptr %site
  br i1 %3, label %again, label %next

again:
  %5 = icmp eq i16 %4, 0
  br i1 %5, label %b2, label %b3

next:
  %6 = icmp eq i16 %4, 0
  br i1 %6, label %b3, label %b4

7:
  unreachable

8:
  %9 = load i16, ptr %0, !tbaa !2
  store i16 1, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %9)
          to label %b4 unwind label %landing
}

define cc 1000 void @AGN() addrspace(1) noinline "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
frame:
  %0 = alloca i16
  %1 = alloca i16
  %site = alloca i16
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %0)
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %1)
  store i16 0, ptr %0
  store i16 0, ptr %1
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %site)
  store i16 20, ptr %1, !tbaa !2
  br label %b2

b2:
  %2 = load i16, ptr @"DEN%", !tbaa !2
  %3 = icmp eq i16 %2, 0
  br i1 %3, label %b3, label %b4

b3:
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %15 unwind label %landing

b4:
  %4 = sdiv i16 10, %2
  store i16 %4, ptr %0, !tbaa !2
  br label %b5

b5:
  store i16 1, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5)
          to label %11 unwind label %landing

b6:
  ret void

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %5 = extractvalue { ptr, i32 } %landed, 1
  %6 = trunc i32 %5 to i16
  %resumed = call fastcc addrspace(1) i16 @__main$handler(i16 %6)
  %7 = icmp eq i16 %resumed, 0
  %8 = load i16, ptr %site
  br i1 %7, label %again, label %next

again:
  %9 = icmp eq i16 %8, 0
  br i1 %9, label %b2, label %b5

next:
  %10 = icmp eq i16 %8, 0
  br i1 %10, label %b5, label %b6

11:
  %12 = load i16, ptr %1, !tbaa !2
  store i16 1, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %12)
          to label %13 unwind label %landing

13:
  %14 = load i16, ptr %0, !tbaa !2
  store i16 1, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %14)
          to label %b6 unwind label %landing

15:
  unreachable
}

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1) noreturn

declare cc 1005 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 i16 @llrm.qb.B$FERR() addrspace(1) nounwind

declare cc 1005 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1005 void @llrm.qb.B$CEND() addrspace(1) nounwind noreturn

declare cc 1005 void @llrm.qb.B$PSI2(i16) addrspace(1)

define internal fastcc range(i16 0, 2) i16 @__main$handler(i16 %0) addrspace(1) "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
entry:
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string3)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %0)
  %1 = icmp eq i16 %0, 5
  br i1 %1, label %common.ret, label %b10

common.ret:
  %common.op = phi i16 [ 0, %b10 ], [ 1, %entry ]
  tail call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 true)
  ret i16 %common.op

b10:
  store i16 2, ptr @"DEN%", !tbaa !2
  br label %common.ret
}

declare i32 @llrm.qb.personality(...) addrspace(1)

declare cc 1000 void @llrm.qb.onerror(i1) addrspace(1) nounwind frame_bound

declare cc 1000 void @llrm.qb.frame(ptr) addrspace(1) nounwind

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
