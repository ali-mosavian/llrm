target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [168 x i8] c"\01\00\00\00\01\00\00\00\01\00\00\00\00\00\01\00\00\00\05\00\00\00\02\00\00\00\00\00\01\00\00\00\02\00\00\00\03\00\00\00d\00\01\00\00\00\07\00\00\00\04\00\00\00d\00\01\00\00\00\08\00\00\00\06\00\00\00d\00\01\00\00\00\03\00\00\00\0A\00\00\00\C8\00\01\00\00\00\09\00\00\00\0B\00\00\00\C8\00\01\00\00\00\0A\00\00\00\0F\00\00\00\C8\00\01\00\00\00\0B\00\00\00\11\00\00\00\C8\00\01\00\00\00\04\00\00\00\12\00\00\00\C8\00\01\00\00\00\0D\00\00\00\18\00\00\00\C8\00\01\00\00\00\0E\00\00\00\19\00\00\00\C8\00"
@S$ = internal global [4 x i8] zeroinitializer
@"K%" = internal global [2 x i8] zeroinitializer
@$string3 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string3, i16 4), [4 x i8] c"DONE" }>
@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c"ERL=" }>
@$QB$ACTIVE = internal global i16 0
@$QB$ERL = internal global i16 0
@$QB$ERL$__main = internal constant [8 x i16] [i16 0, i16 100, i16 100, i16 200, i16 200, i16 200, i16 200, i16 200]

define internal cc 1000 void @__main() addrspace(1) noinline "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
frame:
  %site = alloca i16
  call addrspace(1) void @llrm.qb.frame(ptr %site)
  br label %b1

b1:
  store i16 1, ptr @$QB$ACTIVE
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 true)
  br label %b5

b2:
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 12)
          to label %11 unwind label %landing

b3:
  store i16 300, ptr @"K%", !tbaa !2
  br label %b9

b4:
  store i16 6, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4)
          to label %12 unwind label %landing

b5:
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %18 unwind label %landing

b7:
  store i16 -1, ptr @"K%", !tbaa !2
  br label %b8

b8:
  %0 = load i16, ptr @"K%", !tbaa !2
  store i16 2, ptr %site
  %1 = invoke cc 1000 addrspace(1) ptr @llrm.qb.B$SPAC(i16 %0)
          to label %19 unwind label %landing

b9:
  %2 = load i16, ptr @"K%", !tbaa !2
  store i16 3, ptr %site
  %3 = invoke cc 1000 addrspace(1) ptr @llrm.qb.B$FCHR(i16 %2)
          to label %21 unwind label %landing

b10:
  store i16 4, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string3)
          to label %23 unwind label %landing

b11:
  store i16 5, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$CEND()
          to label %24 unwind label %landing

b13:
  store i16 7, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 20)
          to label %25 unwind label %landing

b14:
  ret void

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %4 = extractvalue { ptr, i32 } %landed, 1
  %5 = trunc i32 %4 to i16
  %6 = load i16, ptr %site
  %7 = getelementptr [8 x i16], ptr @$QB$ERL$__main, i16 0, i16 %6
  %erl = load i16, ptr %7
  %resumed = call addrspace(1) i16 @__main$handler(i16 %5, i16 %erl)
  switch i16 %resumed, label %10 [
    i16 0, label %again
    i16 1, label %next
  ]

again:
  %8 = load i16, ptr %site
  switch i16 %8, label %26 [
    i16 0, label %b5
    i16 1, label %b2
    i16 2, label %b8
    i16 3, label %b9
    i16 4, label %b10
    i16 5, label %b11
    i16 6, label %b4
    i16 7, label %b13
  ]

next:
  %9 = load i16, ptr %site
  switch i16 %9, label %27 [
    i16 0, label %b2
    i16 1, label %b7
    i16 2, label %b3
    i16 3, label %b10
    i16 4, label %b11
    i16 5, label %b4
    i16 6, label %b13
    i16 7, label %b14
  ]

10:
  unreachable

11:
  unreachable

12:
  store i16 6, ptr %site
  %13 = invoke cc 1000 addrspace(1) i16 @llrm.qb.B$FERR()
          to label %14 unwind label %landing

14:
  store i16 6, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %13)
          to label %15 unwind label %landing

15:
  %erl1 = load i16, ptr @$QB$ERL
  %16 = zext i16 %erl1 to i32
  store i16 6, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %16)
          to label %17 unwind label %landing

17:
  br label %b13

18:
  unreachable

19:
  store i16 2, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr %1, ptr @S$)
          to label %20 unwind label %landing

20:
  br label %b3

21:
  store i16 3, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr %3, ptr @S$)
          to label %22 unwind label %landing

22:
  br label %b10

23:
  br label %b11

24:
  unreachable

25:
  unreachable

26:
  unreachable

27:
  unreachable
}

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 i16 @llrm.qb.B$FERR() addrspace(1)

declare cc 1000 void @llrm.qb.B$PSI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI4(i32) addrspace(1)

declare cc 1000 ptr @llrm.qb.B$SPAC(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$SASS(ptr, ptr) addrspace(1)

declare cc 1000 ptr @llrm.qb.B$FCHR(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$CEND() addrspace(1)

define internal i16 @__main$handler(i16 %0, i16 %1) addrspace(1) "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
entry:
  store i16 %1, ptr @$QB$ERL
  %2 = load i16, ptr @$QB$ACTIVE
  switch i16 %2, label %4 [
    i16 1, label %b4
  ]

b4:
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %0)
  %3 = zext i16 %1 to i32
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %3)
  br label %b13

b13:
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
