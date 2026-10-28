target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [4 x i8] zeroinitializer
@$qb$statementTable = internal constant [126 x i8] c"\01\00\00\00\01\00\00\00\01\00\00\00\00\00\01\00\00\00\03\00\00\00\05\00\00\00\00\00\01\00\00\00\04\00\00\00\06\00\00\00\00\00\01\00\00\00\0A\00\00\00\11\00\00\00\00\00\01\00\00\00\0B\00\00\00\13\00\00\00\00\00\01\00\00\00\0C\00\00\00\15\00\00\00\00\00\01\00\00\00\02\00\00\00\16\00\00\00\00\00\01\00\00\00\0E\00\00\00\18\00\00\00\00\00\01\00\00\00\10\00\00\00\1A\00\00\00\00\00"
@"CAUGHT%" = internal global [2 x i8] zeroinitializer
@"NUMERATOR&" = internal global [4 x i8] zeroinitializer
@"DIVISOR&" = internal global [4 x i8] zeroinitializer
@"RESULT&" = internal global [4 x i8] zeroinitializer
@$string3 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string3, i16 4), [4 x i8] c"DONE" }>
@$qb$readData = internal constant [6 x i8] c" 7, 0\00"
@$QB$ACTIVE = internal global i16 0
@$QB$ERL = internal global i16 0

define internal cc 1000 void @__main() addrspace(1) noinline "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
frame:
  %site = alloca i16
  call addrspace(1) void @llrm.qb.frame(ptr %site)
  br label %b1

b1:
  %0 = addrspacecast ptr @"NUMERATOR&" to ptr addrspace(1)
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$RDI4(ptr addrspace(1) %0)
          to label %15 unwind label %landing

b2:
  store i16 5, ptr %site
  %1 = invoke cc 1000 addrspace(1) i16 @llrm.qb.B$FERR()
          to label %18 unwind label %landing

b3:
  store i16 1, ptr @$QB$ACTIVE
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 true)
  br label %b4

b4:
  %2 = load i32, ptr @"NUMERATOR&", !tbaa !2
  %3 = load i32, ptr @"DIVISOR&", !tbaa !2
  %4 = icmp eq i32 %3, 0
  br i1 %4, label %b5, label %b6

b5:
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %22 unwind label %landing

b6:
  %5 = icmp eq i32 %3, -1
  br i1 %5, label %b7, label %b8

b7:
  %6 = sub i32 0, %2
  store i32 %6, ptr @$data, !tbaa !2
  br label %b9

b8:
  %7 = icmp eq i32 %3, 0
  br i1 %7, label %trap, label %19

b9:
  %8 = load i32, ptr @$data, !tbaa !2
  store i32 %8, ptr @"RESULT&", !tbaa !2
  br label %b10

b10:
  %9 = load i16, ptr @"CAUGHT%", !tbaa !2
  store i16 2, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %9)
          to label %23 unwind label %landing

b11:
  store i16 3, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string3)
          to label %24 unwind label %landing

b12:
  store i16 4, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$CEND()
          to label %25 unwind label %landing

b14:
  store i16 6, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 20)
          to label %26 unwind label %landing

b15:
  store i16 6, ptr %site
  invoke cc 1000 addrspace(1) void @"llrm.qb.$QB$DATA:0"()
          to label %27 unwind label %landing

b16:
  ret void

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %10 = extractvalue { ptr, i32 } %landed, 1
  %11 = trunc i32 %10 to i16
  %resumed = call addrspace(1) i16 @__main$handler(i16 %11, i16 0)
  switch i16 %resumed, label %14 [
    i16 0, label %again
    i16 1, label %next
  ]

again:
  %12 = load i16, ptr %site
  switch i16 %12, label %28 [
    i16 0, label %b1
    i16 1, label %b4
    i16 2, label %b10
    i16 3, label %b11
    i16 4, label %b12
    i16 5, label %b2
    i16 6, label %b14
  ]

next:
  %13 = load i16, ptr %site
  switch i16 %13, label %29 [
    i16 0, label %b3
    i16 1, label %b10
    i16 2, label %b11
    i16 3, label %b12
    i16 4, label %b2
    i16 5, label %b14
    i16 6, label %b16
  ]

14:
  unreachable

15:
  %16 = addrspacecast ptr @"DIVISOR&" to ptr addrspace(1)
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$RDI4(ptr addrspace(1) %16)
          to label %17 unwind label %landing

17:
  br label %b3

18:
  store i16 %1, ptr @"CAUGHT%", !tbaa !2
  br label %b14

trap:
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %21 unwind label %landing

19:
  %20 = sdiv i32 %2, %3
  store i32 %20, ptr @$data, !tbaa !2
  br label %b9

21:
  unreachable

22:
  unreachable

23:
  br label %b11

24:
  br label %b12

25:
  unreachable

26:
  unreachable

27:
  br label %b16

28:
  unreachable

29:
  unreachable
}

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$RDI4(ptr addrspace(1)) addrspace(1)

declare cc 1000 i16 @llrm.qb.B$FERR() addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$CEND() addrspace(1)

declare cc 1000 void @"llrm.qb.$QB$DATA:0"() addrspace(1)

define internal i16 @__main$handler(i16 %0, i16 %1) addrspace(1) "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
entry:
  store i16 %1, ptr @$QB$ERL
  %2 = load i16, ptr @$QB$ACTIVE
  switch i16 %2, label %3 [
    i16 1, label %b2
  ]

b2:
  store i16 %0, ptr @"CAUGHT%", !tbaa !2
  br label %b14

b14:
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 true)
  ret i16 1

3:
  unreachable

4:
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
