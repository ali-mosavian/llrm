target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [252 x i8] c"\01\00\00\00\01\00\00\00\01\00\00\00\00\00\01\00\00\00\03\00\00\00\02\00\00\00\00\00\01\00\00\00\04\00\00\00\03\00\00\00\00\00\01\00\00\00\05\00\00\00\04\00\00\00\00\00\01\00\00\00\02\00\00\00\05\00\00\00\00\00\01\00\00\00\07\00\00\00\09\00\00\00\00\00\01\00\00\00\08\00\00\00\0B\00\00\00\00\00\01\00\00\00\0A\00\00\00\0C\00\00\00\00\00\01\00\00\00\0C\00\00\00\0D\00\00\00\00\00\01\00\00\00\0D\00\00\00\0E\00\00\00\00\00\02\00\00\00\01\00\00\00\01\00\00\00\00\00\02\00\00\00\02\00\00\00\02\00\00\00\00\00\02\00\00\00\03\00\00\00\03\00\00\00\00\00\02\00\00\00\04\00\00\00\07\00\00\00\00\00\03\00\00\00\01\00\00\00\01\00\00\00\00\00\03\00\00\00\02\00\00\00\02\00\00\00\00\00\03\00\00\00\05\00\00\00\09\00\00\00\00\00\03\00\00\00\06\00\00\00\0F\00\00\00\00\00"
@"DEN%" = internal global [2 x i8] zeroinitializer
@$string3 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\03\00", ptr getelementptr (i8, ptr @$string3, i16 4), [4 x i8] c"err\00" }>
@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c"next" }>
@$string5 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string5, i16 4), [6 x i8] c"again\00" }>
@$QB$ACTIVE = internal global i16 0
@$QB$ERL = internal global i16 0

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
  store i16 3, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string3)
          to label %6 unwind label %landing

b3:
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @NXT()
          to label %10 unwind label %landing

b4:
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @AGN()
          to label %11 unwind label %landing

b5:
  store i16 2, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$CEND()
          to label %12 unwind label %landing

b7:
  store i16 4, ptr %site
  %0 = invoke cc 1000 addrspace(1) i16 @llrm.qb.B$FERR()
          to label %13 unwind label %landing

b8:
  store i16 5, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 20)
          to label %15 unwind label %landing

b9:
  br label %b10

b10:
  store i16 2, ptr @"DEN%", !tbaa !2
  br label %b12

b12:
  store i16 6, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 20)
          to label %16 unwind label %landing

b13:
  ret void

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %1 = extractvalue { ptr, i32 } %landed, 1
  %2 = trunc i32 %1 to i16
  %resumed = call addrspace(1) i16 @__main$handler(i16 %2, i16 0)
  switch i16 %resumed, label %5 [
    i16 0, label %again
    i16 1, label %next
  ]

again:
  %3 = load i16, ptr %site
  switch i16 %3, label %17 [
    i16 0, label %b3
    i16 1, label %b4
    i16 2, label %b5
    i16 3, label %b2
    i16 4, label %b7
    i16 5, label %b8
    i16 6, label %b12
  ]

next:
  %4 = load i16, ptr %site
  switch i16 %4, label %18 [
    i16 0, label %b4
    i16 1, label %b5
    i16 2, label %b2
    i16 3, label %b7
    i16 4, label %b8
    i16 5, label %b10
    i16 6, label %b13
  ]

5:
  unreachable

6:
  store i16 3, ptr %site
  %7 = invoke cc 1000 addrspace(1) i16 @llrm.qb.B$FERR()
          to label %8 unwind label %landing

8:
  store i16 3, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %7)
          to label %9 unwind label %landing

9:
  br label %b7

10:
  br label %b4

11:
  br label %b5

12:
  unreachable

13:
  %14 = icmp eq i16 %0, 5
  br i1 %14, label %b8, label %b9

15:
  unreachable

16:
  unreachable

17:
  unreachable

18:
  unreachable
}

define cc 1000 void @NXT() addrspace(1) noinline "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
frame:
  %0 = alloca i16
  %site = alloca i16
  call addrspace(1) void @llrm.qb.frame(ptr %0)
  store i16 0, ptr %0
  call addrspace(1) void @llrm.qb.frame(ptr %site)
  br label %b1

b1:
  store i16 7, ptr %0, !tbaa !2
  br label %b2

b2:
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5)
          to label %6 unwind label %landing

b3:
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4)
          to label %7 unwind label %landing

b4:
  ret void

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %1 = extractvalue { ptr, i32 } %landed, 1
  %2 = trunc i32 %1 to i16
  %resumed = call addrspace(1) i16 @__main$handler(i16 %2, i16 0)
  switch i16 %resumed, label %5 [
    i16 0, label %again
    i16 1, label %next
  ]

again:
  %3 = load i16, ptr %site
  switch i16 %3, label %10 [
    i16 0, label %b2
    i16 1, label %b3
  ]

next:
  %4 = load i16, ptr %site
  switch i16 %4, label %11 [
    i16 0, label %b3
    i16 1, label %b4
  ]

5:
  unreachable

6:
  unreachable

7:
  %8 = load i16, ptr %0, !tbaa !2
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %8)
          to label %9 unwind label %landing

9:
  br label %b4

10:
  unreachable

11:
  unreachable
}

define cc 1000 void @AGN() addrspace(1) noinline "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
frame:
  %0 = alloca i16
  %1 = alloca i16
  %site = alloca i16
  call addrspace(1) void @llrm.qb.frame(ptr %0)
  call addrspace(1) void @llrm.qb.frame(ptr %1)
  store i16 0, ptr %0
  store i16 0, ptr %1
  call addrspace(1) void @llrm.qb.frame(ptr %site)
  br label %b1

b1:
  store i16 20, ptr %1, !tbaa !2
  br label %b2

b2:
  %2 = load i16, ptr @"DEN%", !tbaa !2
  %3 = sext i16 %2 to i32
  %4 = icmp eq i32 %3, 0
  br i1 %4, label %b3, label %b4

b3:
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %20 unwind label %landing

b4:
  %5 = icmp eq i32 %3, 0
  br i1 %5, label %trap, label %11

b5:
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5)
          to label %15 unwind label %landing

b6:
  ret void

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %6 = extractvalue { ptr, i32 } %landed, 1
  %7 = trunc i32 %6 to i16
  %resumed = call addrspace(1) i16 @__main$handler(i16 %7, i16 0)
  switch i16 %resumed, label %10 [
    i16 0, label %again
    i16 1, label %next
  ]

again:
  %8 = load i16, ptr %site
  switch i16 %8, label %21 [
    i16 0, label %b2
    i16 1, label %b5
  ]

next:
  %9 = load i16, ptr %site
  switch i16 %9, label %22 [
    i16 0, label %b5
    i16 1, label %b6
  ]

10:
  unreachable

trap:
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %14 unwind label %landing

11:
  %12 = sdiv i32 10, %3
  %13 = trunc i32 %12 to i16
  store i16 %13, ptr %0, !tbaa !2
  br label %b5

14:
  unreachable

15:
  %16 = load i16, ptr %1, !tbaa !2
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %16)
          to label %17 unwind label %landing

17:
  %18 = load i16, ptr %0, !tbaa !2
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %18)
          to label %19 unwind label %landing

19:
  br label %b6

20:
  unreachable

21:
  unreachable

22:
  unreachable
}

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 i16 @llrm.qb.B$FERR() addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$CEND() addrspace(1)

declare cc 1000 void @llrm.qb.B$PSI2(i16) addrspace(1)

define internal i16 @__main$handler(i16 %0, i16 %1) addrspace(1) "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
entry:
  store i16 %1, ptr @$QB$ERL
  %2 = load i16, ptr @$QB$ACTIVE
  switch i16 %2, label %4 [
    i16 1, label %b2
  ]

b2:
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string3)
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %0)
  br label %b7

b7:
  %3 = icmp eq i16 %0, 5
  br i1 %3, label %b8, label %b9

b9:
  br label %b10

b8:
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 true)
  ret i16 1

b10:
  store i16 2, ptr @"DEN%", !tbaa !2
  br label %b12

b12:
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 true)
  ret i16 0

4:
  unreachable

5:
  unreachable

6:
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
