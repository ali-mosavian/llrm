target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [140 x i8] c"\01\00\00\00\01\00\00\00\01\00\00\00\00\00\01\00\00\00\03\00\00\00\02\00\00\00\00\00\01\00\00\00\04\00\00\00\03\00\00\00\00\00\01\00\00\00\05\00\00\00\06\00\00\00\00\00\01\00\00\00\06\00\00\00\08\00\00\00\00\00\01\00\00\00\07\00\00\00\0A\00\00\00\00\00\01\00\00\00\02\00\00\00\0B\00\00\00\00\00\01\00\00\00\09\00\00\00\0F\00\00\00\00\00\01\00\00\00\0A\00\00\00\11\00\00\00\00\00\01\00\00\00\0B\00\00\00\12\00\00\00\00\00"
@"VALUE%" = internal global [2 x i8] zeroinitializer
@$string3 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string3, i16 4), [4 x i8] c"DONE" }>
@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\03\00", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c"ERR\00" }>
@$string5 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string5, i16 4), [4 x i8] c"DONE" }>
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
          to label %8 unwind label %landing

b3:
  store i16 32767, ptr @"VALUE%", !tbaa !2
  br label %b4

b4:
  %0 = load i16, ptr @"VALUE%", !tbaa !2
  %1 = add i16 %0, 1
  store i16 %1, ptr @"VALUE%", !tbaa !2
  br label %b5

b5:
  %2 = load i16, ptr @"VALUE%", !tbaa !2
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %2)
          to label %12 unwind label %landing

b6:
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string3)
          to label %13 unwind label %landing

b7:
  store i16 2, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$CEND()
          to label %14 unwind label %landing

b9:
  store i16 4, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string5)
          to label %15 unwind label %landing

b10:
  store i16 5, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$CEND()
          to label %16 unwind label %landing

b11:
  ret void

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %3 = extractvalue { ptr, i32 } %landed, 1
  %4 = trunc i32 %3 to i16
  %resumed = call addrspace(1) i16 @__main$handler(i16 %4, i16 0)
  switch i16 %resumed, label %7 [
    i16 0, label %again
    i16 1, label %next
  ]

again:
  %5 = load i16, ptr %site
  switch i16 %5, label %17 [
    i16 0, label %b5
    i16 1, label %b6
    i16 2, label %b7
    i16 3, label %b2
    i16 4, label %b9
    i16 5, label %b10
  ]

next:
  %6 = load i16, ptr %site
  switch i16 %6, label %18 [
    i16 0, label %b6
    i16 1, label %b7
    i16 2, label %b2
    i16 3, label %b9
    i16 4, label %b10
    i16 5, label %b11
  ]

7:
  unreachable

8:
  store i16 3, ptr %site
  %9 = invoke cc 1000 addrspace(1) i16 @llrm.qb.B$FERR()
          to label %10 unwind label %landing

10:
  store i16 3, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %9)
          to label %11 unwind label %landing

11:
  br label %b9

12:
  br label %b6

13:
  br label %b7

14:
  unreachable

15:
  br label %b10

16:
  unreachable

17:
  unreachable

18:
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
  switch i16 %2, label %3 [
    i16 1, label %b2
  ]

b2:
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4)
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %0)
  br label %b9

b9:
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string5)
  br label %b10

b10:
  call cc 1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable

3:
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
