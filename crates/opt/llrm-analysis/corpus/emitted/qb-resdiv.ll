target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [238 x i8] c"\01\00\00\00\01\00\00\00\01\00\00\00\00\00\01\00\00\00\03\00\00\00\02\00\00\00\00\00\01\00\00\00\04\00\00\00\05\00\00\00\00\00\01\00\00\00\05\00\00\00\08\00\00\00\00\00\01\00\00\00\06\00\00\00\0A\00\00\00\00\00\01\00\00\00\02\00\00\00\0B\00\00\00\00\00\01\00\00\00\08\00\00\00\11\00\00\00\00\00\01\00\00\00\09\00\00\00\12\00\00\00\00\00\02\00\00\00\01\00\00\00\01\00\00\00\00\00\02\00\00\00\02\00\00\00\01\00\00\00\84\03\02\00\00\00\03\00\00\00\02\00\00\00\89\03\02\00\00\00\04\00\00\00\04\00\00\00\8C\03\02\00\00\00\05\00\00\00\0A\00\00\00\8C\03\03\00\00\00\01\00\00\00\01\00\00\00\00\00\03\00\00\00\02\00\00\00\01\00\00\00\93\03\03\00\00\00\03\00\00\00\08\00\00\00\98\03\03\00\00\00\06\00\00\00\0B\00\00\00\98\03"
@$arguments$1 = internal global [4 x i8] zeroinitializer
@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c"done" }>
@$string5 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\03\00", ptr getelementptr (i8, ptr @$string5, i16 4), [4 x i8] c"err\00" }>
@$string6 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\06\00", ptr getelementptr (i8, ptr @$string6, i16 4), [6 x i8] c"caller" }>
@$QB$ACTIVE = internal global i16 0
@$QB$ERL = internal global i16 0
@$QB$ERL$CALLER = internal constant [2 x i16] [i16 905, i16 908]
@"$QB$ERL$INNER%" = internal constant [1 x i16] [i16 915]

define internal cc 1000 void @__main() addrspace(1) noinline "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
frame:
  %site = alloca i16
  call addrspace(1) void @llrm.qb.frame(ptr %site)
  br label %b1

b1:
  store i16 1, ptr @$QB$ACTIVE
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 true)
  br label %b3

b2:
  store i16 4, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5)
          to label %7 unwind label %landing

b3:
  store i16 0, ptr @$arguments$1, !tbaa !2
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @CALLER(ptr @$arguments$1)
          to label %13 unwind label %landing

b4:
  %0 = getelementptr i8, ptr @$arguments$1, i16 2
  store i16 2, ptr %0, !tbaa !2
  %1 = getelementptr i8, ptr @$arguments$1, i16 2
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @CALLER(ptr %1)
          to label %14 unwind label %landing

b5:
  store i16 2, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string4)
          to label %15 unwind label %landing

b6:
  store i16 3, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$CEND()
          to label %16 unwind label %landing

b8:
  store i16 5, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 20)
          to label %17 unwind label %landing

b9:
  ret void

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %2 = extractvalue { ptr, i32 } %landed, 1
  %3 = trunc i32 %2 to i16
  %resumed = call addrspace(1) i16 @__main$handler(i16 %3, i16 0)
  switch i16 %resumed, label %6 [
    i16 0, label %again
    i16 1, label %next
  ]

again:
  %4 = load i16, ptr %site
  switch i16 %4, label %18 [
    i16 0, label %b3
    i16 1, label %b4
    i16 2, label %b5
    i16 3, label %b6
    i16 4, label %b2
    i16 5, label %b8
  ]

next:
  %5 = load i16, ptr %site
  switch i16 %5, label %19 [
    i16 0, label %b4
    i16 1, label %b5
    i16 2, label %b6
    i16 3, label %b2
    i16 4, label %b8
    i16 5, label %b9
  ]

6:
  unreachable

7:
  store i16 4, ptr %site
  %8 = invoke cc 1000 addrspace(1) i16 @llrm.qb.B$FERR()
          to label %9 unwind label %landing

9:
  store i16 4, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %8)
          to label %10 unwind label %landing

10:
  %erl = load i16, ptr @$QB$ERL
  %11 = zext i16 %erl to i32
  store i16 4, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %11)
          to label %12 unwind label %landing

12:
  br label %b8

13:
  br label %b4

14:
  br label %b5

15:
  br label %b6

16:
  unreachable

17:
  unreachable

18:
  unreachable

19:
  unreachable
}

define cc 1000 void @CALLER(ptr dereferenceable(2) %0) addrspace(1) noinline "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
frame:
  %1 = alloca i16
  %2 = alloca i32
  %site = alloca i16
  call addrspace(1) void @llrm.qb.frame(ptr %1)
  call addrspace(1) void @llrm.qb.frame(ptr %2)
  store i16 0, ptr %1
  store i32 0, ptr %2
  call addrspace(1) void @llrm.qb.frame(ptr %site)
  br label %b1

b1:
  br label %b2

b2:
  store i32 100, ptr %2, !tbaa !2
  br label %b3

b3:
  store i16 0, ptr %site
  %3 = invoke cc 1000 addrspace(1) i16 @"INNER%"(ptr %0)
          to label %11 unwind label %landing

b4:
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string6)
          to label %12 unwind label %landing

b5:
  ret void

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %4 = extractvalue { ptr, i32 } %landed, 1
  %5 = trunc i32 %4 to i16
  %6 = load i16, ptr %site
  %7 = getelementptr [2 x i16], ptr @$QB$ERL$CALLER, i16 0, i16 %6
  %erl = load i16, ptr %7
  %resumed = call addrspace(1) i16 @__main$handler(i16 %5, i16 %erl)
  switch i16 %resumed, label %10 [
    i16 0, label %again
    i16 1, label %next
  ]

again:
  %8 = load i16, ptr %site
  switch i16 %8, label %17 [
    i16 0, label %b3
    i16 1, label %b4
  ]

next:
  %9 = load i16, ptr %site
  switch i16 %9, label %18 [
    i16 0, label %b4
    i16 1, label %b5
  ]

10:
  unreachable

11:
  store i16 %3, ptr %1, !tbaa !2
  br label %b4

12:
  %13 = load i32, ptr %2, !tbaa !2
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %13)
          to label %14 unwind label %landing

14:
  %15 = load i16, ptr %1, !tbaa !2
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %15)
          to label %16 unwind label %landing

16:
  br label %b5

17:
  unreachable

18:
  unreachable
}

define cc 1000 i16 @"INNER%"(ptr dereferenceable(2) %0) addrspace(1) noinline "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
frame:
  %1 = alloca i16
  %2 = alloca i16
  %site = alloca i16
  call addrspace(1) void @llrm.qb.frame(ptr %1)
  store i16 0, ptr %1
  store i16 0, ptr %2
  call addrspace(1) void @llrm.qb.frame(ptr %site)
  br label %b1

b1:
  br label %b2

b2:
  %3 = load i16, ptr %0
  %4 = sext i16 %3 to i32
  %5 = icmp eq i32 %4, 0
  br i1 %5, label %b4, label %b5

b3:
  %6 = load i16, ptr %1, !tbaa !2
  %7 = add i16 %6, 1
  store i16 %7, ptr %2, !tbaa !2
  br label %b6

b4:
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %21 unwind label %landing

b5:
  %8 = icmp eq i32 %4, 0
  br i1 %8, label %trap, label %17

b6:
  br label %b7

b7:
  %9 = load i16, ptr %2, !tbaa !2
  ret i16 %9

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %10 = extractvalue { ptr, i32 } %landed, 1
  %11 = trunc i32 %10 to i16
  %12 = load i16, ptr %site
  %13 = getelementptr [1 x i16], ptr @"$QB$ERL$INNER%", i16 0, i16 %12
  %erl = load i16, ptr %13
  %resumed = call addrspace(1) i16 @__main$handler(i16 %11, i16 %erl)
  switch i16 %resumed, label %16 [
    i16 0, label %again
    i16 1, label %next
  ]

again:
  %14 = load i16, ptr %site
  switch i16 %14, label %22 [
    i16 0, label %b2
  ]

next:
  %15 = load i16, ptr %site
  switch i16 %15, label %23 [
    i16 0, label %b3
  ]

16:
  unreachable

trap:
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %20 unwind label %landing

17:
  %18 = sdiv i32 10, %4
  %19 = trunc i32 %18 to i16
  store i16 %19, ptr %1, !tbaa !2
  br label %b3

20:
  unreachable

21:
  unreachable

22:
  unreachable

23:
  unreachable
}

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 i16 @llrm.qb.B$FERR() addrspace(1)

declare cc 1000 void @llrm.qb.B$PSI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI4(i32) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$CEND() addrspace(1)

declare cc 1000 void @llrm.qb.B$PSI4(i32) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

define internal i16 @__main$handler(i16 %0, i16 %1) addrspace(1) "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
entry:
  store i16 %1, ptr @$QB$ERL
  %2 = load i16, ptr @$QB$ACTIVE
  switch i16 %2, label %4 [
    i16 1, label %b2
  ]

b2:
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %0)
  %3 = zext i16 %1 to i32
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %3)
  br label %b8

b8:
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 true)
  ret i16 1

4:
  unreachable

5:
  unreachable
}

declare i32 @llrm.qb.personality(...) addrspace(1)

declare cc 1000 void @llrm.qb.onerror(i1) addrspace(1) nounwind frame_bound

declare cc 1000 void @llrm.qb.frame(ptr) addrspace(1) nounwind

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
