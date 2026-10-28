target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$arguments$1 = internal global [4 x i8] zeroinitializer
@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c"done" }>
@$string5 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\03\00", ptr getelementptr (i8, ptr @$string5, i16 4), [4 x i8] c"err\00" }>
@$string6 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\06\00", ptr getelementptr (i8, ptr @$string6, i16 4), [6 x i8] c"caller" }>
@$QB$ERL$CALLER = internal constant [2 x i16] [i16 905, i16 908]
@"$QB$ERL$INNER%" = internal constant [2 x i16] [i16 910, i16 915]

define internal cc 1000 void @__main() addrspace(1) noinline "llrm-fp-eval"="x87-extended" noreturn personality ptr addrspace(1) @llrm.qb.personality {
frame:
  %site = alloca i16
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %site)
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 true)
  store i16 0, ptr @$arguments$1, !tbaa !2
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @CALLER(ptr @$arguments$1)
          to label %b4 unwind label %landing

b4:
  store i16 2, ptr getelementptr inbounds nuw (i8, ptr @$arguments$1, i16 2), !tbaa !2
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @CALLER(ptr getelementptr inbounds nuw (i8, ptr @$arguments$1, i16 2))
          to label %b5 unwind label %landing

b5:
  store i16 2, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string4)
          to label %b6 unwind label %landing

b6:
  call cc 1005 addrspace(1) void @llrm.qb.B$CEND()
  unreachable

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %0 = extractvalue { ptr, i32 } %landed, 1
  %1 = trunc i32 %0 to i16
  %resumed = call fastcc addrspace(1) i16 @__main$handler(i16 %1, i16 0)
  %2 = load i16, ptr %site
  %3 = icmp eq i16 %2, 0
  br i1 %3, label %b4, label %4

4:
  %5 = icmp eq i16 %2, 1
  br i1 %5, label %b5, label %6

6:
  br label %b6
}

define cc 1000 void @CALLER(ptr dereferenceable(2) nocapture readonly %0) addrspace(1) noinline "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
frame:
  %1 = alloca i16
  %2 = alloca i32
  %site = alloca i16
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %1)
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %2)
  store i16 0, ptr %1
  store i32 0, ptr %2
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %site)
  store i32 100, ptr %2, !tbaa !2
  store i16 0, ptr %site
  %3 = invoke cc 1000 addrspace(1) i16 @"INNER%"(ptr nonnull %0)
          to label %10 unwind label %landing

b4:
  store i16 1, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string6)
          to label %11 unwind label %landing

b5:
  ret void

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %4 = extractvalue { ptr, i32 } %landed, 1
  %5 = trunc i32 %4 to i16
  %6 = load i16, ptr %site
  %7 = getelementptr [2 x i8], ptr @$QB$ERL$CALLER, i16 %6
  %erl = load i16, ptr %7
  %resumed = call fastcc addrspace(1) i16 @__main$handler(i16 %5, i16 %erl)
  %8 = load i16, ptr %site
  %9 = icmp eq i16 %8, 0
  br i1 %9, label %b4, label %b5

10:
  store i16 %3, ptr %1, !tbaa !2
  br label %b4

11:
  %12 = load i32, ptr %2, !tbaa !2
  store i16 1, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI4(i32 %12)
          to label %13 unwind label %landing

13:
  %14 = load i16, ptr %1, !tbaa !2
  store i16 1, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %14)
          to label %b5 unwind label %landing
}

define cc 1000 i16 @"INNER%"(ptr dereferenceable(2) nocapture readonly %0) addrspace(1) noinline "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
frame:
  %1 = alloca i16
  %site = alloca i16
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %1)
  store i16 0, ptr %1
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %site)
  %2 = load i16, ptr %0
  %3 = icmp eq i16 %2, 0
  br i1 %3, label %b5, label %b10

b3:
  %4 = load i16, ptr %0
  %5 = icmp eq i16 %4, 0
  br i1 %5, label %b9, label %b10

b4:
  %6 = phi i16 [ %17, %16 ], [ %9, %b10 ]
  %7 = add i16 %6, 1
  ret i16 %7

b5:
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %19 unwind label %landing

b9:
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %18 unwind label %landing

b10:
  %8 = phi i16 [ %4, %b3 ], [ %2, %frame ]
  %9 = sdiv i16 10, %8
  store i16 %9, ptr %1, !tbaa !2
  br label %b4

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %10 = extractvalue { ptr, i32 } %landed, 1
  %11 = trunc i32 %10 to i16
  %12 = load i16, ptr %site
  %13 = getelementptr [2 x i8], ptr @"$QB$ERL$INNER%", i16 %12
  %erl = load i16, ptr %13
  %resumed = call fastcc addrspace(1) i16 @__main$handler(i16 %11, i16 %erl)
  %14 = load i16, ptr %site
  %15 = icmp eq i16 %14, 0
  br i1 %15, label %b3, label %16

16:
  %17 = load i16, ptr %1, !tbaa !2
  br label %b4

18:
  unreachable

19:
  unreachable
}

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1) noreturn

declare cc 1005 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 i16 @llrm.qb.B$FERR() addrspace(1) nounwind

declare cc 1005 void @llrm.qb.B$PSI2(i16) addrspace(1)

declare cc 1005 void @llrm.qb.B$PEI4(i32) addrspace(1)

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc 1005 void @llrm.qb.B$CEND() addrspace(1) nounwind noreturn

declare cc 1005 void @llrm.qb.B$PSI4(i32) addrspace(1)

declare cc 1005 void @llrm.qb.B$PEI2(i16) addrspace(1)

define internal fastcc i16 @__main$handler(i16 %0, i16 %1) addrspace(1) "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
entry:
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %0)
  %2 = zext i16 %1 to i32
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %2)
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
