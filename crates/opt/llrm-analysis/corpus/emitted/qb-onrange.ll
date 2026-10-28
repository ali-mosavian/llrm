target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [280 x i8] c"\01\00\00\00\01\00\00\00\01\00\00\00\00\00\01\00\00\00\03\00\00\00\02\00\00\00\00\00\01\00\00\00\04\00\00\00\03\00\00\00\00\00\01\00\00\00\05\00\00\00\04\00\00\00\00\00\01\00\00\00\0B\00\00\00\0A\00\00\00\00\00\01\00\00\00\0D\00\00\00\0C\00\00\00\00\00\01\00\00\00\0A\00\00\00\0D\00\00\00\00\00\01\00\00\00\0F\00\00\00\0F\00\00\00\00\00\01\00\00\00\02\00\00\00\10\00\00\00\00\00\01\00\00\00\11\00\00\00\14\00\00\00\00\00\01\00\00\00\12\00\00\00\17\00\00\00\00\00\01\00\00\00\13\00\00\00\19\00\00\00\00\00\01\00\00\00\16\00\00\00\1B\00\00\00\00\00\01\00\00\00\15\00\00\00\1C\00\00\00\00\00\01\00\00\00\18\00\00\00\1D\00\00\00\00\00\01\00\00\00\19\00\00\00\1E\00\00\00\00\00\02\00\00\00\01\00\00\00\01\00\00\00\00\00\02\00\00\00\02\00\00\00\03\00\00\00\00\00\03\00\00\00\01\00\00\00\01\00\00\00\00\00\03\00\00\00\02\00\00\00\03\00\00\00\00\00"
@"C%" = internal global [2 x i8] zeroinitializer
@"X%" = internal global [2 x i8] zeroinitializer
@$string3 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string3, i16 4), [6 x i8] c"after\00" }>
@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\03\00", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c"err\00" }>
@$string5 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string5, i16 4), [4 x i8] c"loop" }>
@$string6 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string6, i16 4), [2 x i8] c"a\00" }>
@$string7 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string7, i16 4), [2 x i8] c"b\00" }>
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
  store i16 5, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4)
          to label %14 unwind label %landing

b3:
  store i16 0, ptr @"C%", !tbaa !2
  br label %b4

b4:
  store i16 300, ptr @"X%", !tbaa !2
  br label %b5

b5:
  %0 = load i16, ptr @"X%", !tbaa !2
  %1 = icmp slt i16 %0, 0
  br i1 %1, label %b6, label %b7

b6:
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5)
          to label %19 unwind label %landing

b7:
  %2 = icmp sgt i16 %0, 255
  br i1 %2, label %b8, label %b9

b8:
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5)
          to label %18 unwind label %landing

b9:
  %3 = icmp eq i16 %0, 1
  br i1 %3, label %b11, label %b12

b10:
  store i16 3, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string3)
          to label %22 unwind label %landing

b11:
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @A()
          to label %20 unwind label %landing

b12:
  %4 = icmp eq i16 %0, 2
  br i1 %4, label %b13, label %b14

b13:
  store i16 2, ptr %site
  invoke cc 1000 addrspace(1) void @B()
          to label %21 unwind label %landing

b14:
  br label %b10

b15:
  store i16 4, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$CEND()
          to label %23 unwind label %landing

b17:
  %5 = load i16, ptr @"C%", !tbaa !2
  %6 = add i16 %5, 1
  store i16 %6, ptr @"C%", !tbaa !2
  br label %b18

b18:
  %7 = load i16, ptr @"C%", !tbaa !2
  %8 = icmp sgt i16 %7, 3
  br i1 %8, label %b19, label %b20

b19:
  store i16 6, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string5)
          to label %24 unwind label %landing

b20:
  br label %b21

b21:
  store i16 1, ptr @"X%", !tbaa !2
  br label %b24

b22:
  store i16 7, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$CEND()
          to label %25 unwind label %landing

b24:
  store i16 8, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 20)
          to label %26 unwind label %landing

b25:
  ret void

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %9 = extractvalue { ptr, i32 } %landed, 1
  %10 = trunc i32 %9 to i16
  %resumed = call addrspace(1) i16 @__main$handler(i16 %10, i16 0)
  switch i16 %resumed, label %13 [
    i16 0, label %again
    i16 1, label %next
  ]

again:
  %11 = load i16, ptr %site
  switch i16 %11, label %27 [
    i16 0, label %b5
    i16 1, label %b11
    i16 2, label %b13
    i16 3, label %b10
    i16 4, label %b15
    i16 5, label %b2
    i16 6, label %b19
    i16 7, label %b22
    i16 8, label %b24
  ]

next:
  %12 = load i16, ptr %site
  switch i16 %12, label %28 [
    i16 0, label %b11
    i16 1, label %b13
    i16 2, label %b10
    i16 3, label %b15
    i16 4, label %b2
    i16 5, label %b17
    i16 6, label %b22
    i16 7, label %b21
    i16 8, label %b25
  ]

13:
  unreachable

14:
  store i16 5, ptr %site
  %15 = invoke cc 1000 addrspace(1) i16 @llrm.qb.B$FERR()
          to label %16 unwind label %landing

16:
  store i16 5, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %15)
          to label %17 unwind label %landing

17:
  br label %b17

18:
  unreachable

19:
  unreachable

20:
  br label %b10

21:
  br label %b10

22:
  br label %b15

23:
  unreachable

24:
  br label %b22

25:
  unreachable

26:
  unreachable

27:
  unreachable

28:
  unreachable
}

define internal cc 1000 void @A() addrspace(1) noinline "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
frame:
  %site = alloca i16
  call addrspace(1) void @llrm.qb.frame(ptr %site)
  br label %b1

b1:
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string6)
          to label %5 unwind label %landing

b2:
  ret void

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %0 = extractvalue { ptr, i32 } %landed, 1
  %1 = trunc i32 %0 to i16
  %resumed = call addrspace(1) i16 @__main$handler(i16 %1, i16 0)
  switch i16 %resumed, label %4 [
    i16 0, label %again
    i16 1, label %next
  ]

again:
  %2 = load i16, ptr %site
  switch i16 %2, label %6 [
    i16 0, label %b1
  ]

next:
  %3 = load i16, ptr %site
  switch i16 %3, label %7 [
    i16 0, label %b2
  ]

4:
  unreachable

5:
  br label %b2

6:
  unreachable

7:
  unreachable
}

define internal cc 1000 void @B() addrspace(1) noinline "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
frame:
  %site = alloca i16
  call addrspace(1) void @llrm.qb.frame(ptr %site)
  br label %b1

b1:
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string7)
          to label %5 unwind label %landing

b2:
  ret void

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %0 = extractvalue { ptr, i32 } %landed, 1
  %1 = trunc i32 %0 to i16
  %resumed = call addrspace(1) i16 @__main$handler(i16 %1, i16 0)
  switch i16 %resumed, label %4 [
    i16 0, label %again
    i16 1, label %next
  ]

again:
  %2 = load i16, ptr %site
  switch i16 %2, label %6 [
    i16 0, label %b1
  ]

next:
  %3 = load i16, ptr %site
  switch i16 %3, label %7 [
    i16 0, label %b2
  ]

4:
  unreachable

5:
  br label %b2

6:
  unreachable

7:
  unreachable
}

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 i16 @llrm.qb.B$FERR() addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$CEND() addrspace(1)

define internal i16 @__main$handler(i16 %0, i16 %1) addrspace(1) "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
entry:
  store i16 %1, ptr @$QB$ERL
  %2 = load i16, ptr @$QB$ACTIVE
  switch i16 %2, label %7 [
    i16 1, label %b2
  ]

b2:
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4)
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %0)
  br label %b17

b17:
  %3 = load i16, ptr @"C%", !tbaa !2
  %4 = add i16 %3, 1
  store i16 %4, ptr @"C%", !tbaa !2
  br label %b18

b18:
  %5 = load i16, ptr @"C%", !tbaa !2
  %6 = icmp sgt i16 %5, 3
  br i1 %6, label %b19, label %b20

b20:
  br label %b21

b19:
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string5)
  br label %b22

b21:
  store i16 1, ptr @"X%", !tbaa !2
  br label %b24

b22:
  call cc 1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable

b24:
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 true)
  ret i16 0

7:
  unreachable

8:
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
