target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [112 x i8] c"\01\00\00\00\01\00\00\00\01\00\00\00\00\00\01\00\00\00\03\00\00\00\02\00\00\00\00\00\01\00\00\00\06\00\00\00\09\00\00\00\00\00\01\00\00\00\07\00\00\00\0D\00\00\00\00\00\01\00\00\00\02\00\00\00\0E\00\00\00\00\00\01\00\00\00\09\00\00\00\12\00\00\00\00\00\01\00\00\00\0A\00\00\00\13\00\00\00\00\00\01\00\00\00\0B\00\00\00\14\00\00\00\00\00"
@"DEN%" = internal global [2 x i8] zeroinitializer
@"R%" = internal global [2 x i8] zeroinitializer
@$string3 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string3, i16 4), [2 x i8] c"r\00" }>
@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\03\00", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c"err\00" }>
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
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4)
          to label %9 unwind label %landing

b3:
  %0 = load i16, ptr @"DEN%", !tbaa !2
  %1 = sext i16 %0 to i32
  %2 = icmp eq i32 %1, 0
  br i1 %2, label %b4, label %b5

b4:
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %17 unwind label %landing

b5:
  %3 = icmp eq i32 %1, 0
  br i1 %3, label %trap, label %13

b6:
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string3)
          to label %18 unwind label %landing

b7:
  store i16 2, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$CEND()
          to label %21 unwind label %landing

b9:
  store i16 2, ptr @"DEN%", !tbaa !2
  br label %b10

b10:
  store i16 4, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 20)
          to label %22 unwind label %landing

b11:
  ret void

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %4 = extractvalue { ptr, i32 } %landed, 1
  %5 = trunc i32 %4 to i16
  %resumed = call addrspace(1) i16 @__main$handler(i16 %5, i16 0)
  switch i16 %resumed, label %8 [
    i16 0, label %again
    i16 1, label %next
  ]

again:
  %6 = load i16, ptr %site
  switch i16 %6, label %23 [
    i16 0, label %b3
    i16 1, label %b6
    i16 2, label %b7
    i16 3, label %b2
    i16 4, label %b10
  ]

next:
  %7 = load i16, ptr %site
  switch i16 %7, label %24 [
    i16 0, label %b6
    i16 1, label %b7
    i16 2, label %b2
    i16 3, label %b9
    i16 4, label %b11
  ]

8:
  unreachable

9:
  store i16 3, ptr %site
  %10 = invoke cc 1000 addrspace(1) i16 @llrm.qb.B$FERR()
          to label %11 unwind label %landing

11:
  store i16 3, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %10)
          to label %12 unwind label %landing

12:
  br label %b9

trap:
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %16 unwind label %landing

13:
  %14 = sdiv i32 10, %1
  %15 = trunc i32 %14 to i16
  store i16 %15, ptr @"R%", !tbaa !2
  br label %b6

16:
  unreachable

17:
  unreachable

18:
  %19 = load i16, ptr @"R%", !tbaa !2
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %19)
          to label %20 unwind label %landing

20:
  br label %b7

21:
  unreachable

22:
  unreachable

23:
  unreachable

24:
  unreachable
}

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 i16 @llrm.qb.B$FERR() addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$CEND() addrspace(1)

define internal i16 @__main$handler(i16 %0, i16 %1) addrspace(1) "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
entry:
  store i16 %1, ptr @$QB$ERL
  %2 = load i16, ptr @$QB$ACTIVE
  switch i16 %2, label %3 [
    i16 1, label %b2
  ]

b2:
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4)
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %0)
  br label %b9

b9:
  store i16 2, ptr @"DEN%", !tbaa !2
  br label %b10

b10:
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 true)
  ret i16 0

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
