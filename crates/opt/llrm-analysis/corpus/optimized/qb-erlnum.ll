target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@S$ = internal global [4 x i8] zeroinitializer
@$string3 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string3, i16 4), [4 x i8] c"DONE" }>
@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c"ERL=" }>
@$QB$ERL$__main = internal constant [7 x i16] [i16 0, i16 100, i16 100, i16 200, i16 200, i16 200, i16 200]

define internal cc 1000 void @__main() addrspace(1) noinline "llrm-fp-eval"="x87-extended" noreturn personality ptr addrspace(1) @llrm.qb.personality {
frame:
  %site = alloca i16
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %site)
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 true)
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %13 unwind label %landing

b3:
  store i16 3, ptr %site
  %0 = invoke cc 1005 addrspace(1) ptr @llrm.qb.B$FCHR(i16 300)
          to label %15 unwind label %landing

b10:
  store i16 4, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string3)
          to label %b11 unwind label %landing

b11:
  call cc 1005 addrspace(1) void @llrm.qb.B$CEND()
  unreachable

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %1 = extractvalue { ptr, i32 } %landed, 1
  %2 = trunc i32 %1 to i16
  %3 = load i16, ptr %site
  %4 = getelementptr [2 x i8], ptr @$QB$ERL$__main, i16 %3
  %erl = load i16, ptr %4
  %resumed = call fastcc addrspace(1) i16 @__main$handler(i16 %2, i16 %erl)
  %5 = load i16, ptr %site
  switch table i16 %5, label %6 [
    i16 0, label %6
    i16 1, label %7
    i16 2, label %9
    i16 3, label %10
    i16 4, label %11
  ]

6:
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 12)
          to label %12 unwind label %landing

7:
  store i16 2, ptr %site
  %8 = invoke cc 1005 addrspace(1) ptr @llrm.qb.B$SPAC(i16 -1)
          to label %14 unwind label %landing

9:
  br label %b3

10:
  br label %b10

11:
  br label %b11

12:
  unreachable

13:
  unreachable

14:
  store i16 2, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$SASS(ptr %8, ptr @S$)
          to label %b3 unwind label %landing

15:
  store i16 3, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$SASS(ptr %0, ptr @S$)
          to label %b10 unwind label %landing
}

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1) noreturn

declare cc 1005 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 i16 @llrm.qb.B$FERR() addrspace(1) nounwind

declare cc 1005 void @llrm.qb.B$PSI2(i16) addrspace(1)

declare cc 1005 void @llrm.qb.B$PEI4(i32) addrspace(1)

declare cc 1005 ptr @llrm.qb.B$SPAC(i16) addrspace(1)

declare cc 1005 void @llrm.qb.B$SASS(ptr, ptr) addrspace(1)

declare cc 1005 ptr @llrm.qb.B$FCHR(i16) addrspace(1) "llrm-fp-eval"="x87-extended"

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc 1005 void @llrm.qb.B$CEND() addrspace(1) nounwind noreturn

define internal fastcc i16 @__main$handler(i16 %0, i16 %1) addrspace(1) "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
entry:
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4)
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
