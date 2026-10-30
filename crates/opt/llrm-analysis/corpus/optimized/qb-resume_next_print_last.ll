target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"K%" = internal global [2 x i8] zeroinitializer
@$string3 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string3, i16 4), [4 x i8] c"done" }>
@"J%" = internal global [2 x i8] zeroinitializer
@$string4 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\06\00", ptr getelementptr (i8, ptr @$string4, i16 4), [6 x i8] c"w done" }>
@"I%" = internal global [2 x i8] zeroinitializer
@$string5 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\06\00", ptr getelementptr (i8, ptr @$string5, i16 4), [6 x i8] c"d done" }>
@"N%" = internal global [2 x i8] zeroinitializer
@$string6 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string6, i16 4), [2 x i8] c"k\00" }>
@$string7 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\06\00", ptr getelementptr (i8, ptr @$string7, i16 4), [6 x i8] c"a done" }>
@$string8 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\03\00", ptr getelementptr (i8, ptr @$string8, i16 4), [4 x i8] c"err\00" }>
@$data.2 = internal global i16 0
@$data.3 = internal global i16 0

define internal cc 1000 void @__main() addrspace(1) noinline "llrm-fp-eval"="x87-extended" noreturn personality ptr addrspace(1) @llrm.qb.personality {
frame:
  %site = alloca i16
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %site)
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 true)
  store i16 1, ptr @"K%", !tbaa !2
  br label %b4

b4:
  %0 = phi i16 [ %6, %b11 ], [ 1, %frame ]
  %1 = icmp slt i16 %0, 4
  br i1 %1, label %b7, label %b8

b7:
  %2 = add nsw i16 %0, -1
  %3 = icmp eq i16 %2, 0
  br i1 %3, label %b9, label %b10

b8:
  store i16 1, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string3)
          to label %b12 unwind label %landing

b9:
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %49 unwind label %landing

b10:
  %4 = sdiv i16 10, %2
  store i16 0, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %4)
          to label %b11 unwind label %landing

b11:
  %5 = load i16, ptr @"K%", !tbaa !2
  %6 = add nsw i16 %5, 1
  store i16 %6, ptr @"K%", !tbaa !2
  br label %b4

b12:
  store i16 0, ptr @"J%", !tbaa !2
  br label %b14

b14:
  %7 = phi i16 [ %13, %b20 ], [ 0, %b12 ]
  %8 = icmp slt i16 %7, 3
  br i1 %8, label %b15, label %b16

b15:
  %9 = add nsw i16 %7, 1
  store i16 %9, ptr @"J%", !tbaa !2
  %10 = add i16 %7, -1
  %11 = icmp eq i16 %10, 0
  br i1 %11, label %b18, label %b19

b16:
  store i16 3, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string4)
          to label %b24 unwind label %landing

b18:
  store i16 2, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %50 unwind label %landing

b19:
  %12 = sdiv i16 10, %10
  store i16 2, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %12)
          to label %b20 unwind label %landing

b20:
  %13 = load i16, ptr @"J%", !tbaa !2
  br label %b14

b24:
  %14 = phi i16 [ %19, %b29 ], [ 0, %b16 ], [ 0, %42 ]
  %15 = add nsw i16 %14, 1
  store i16 %15, ptr @"I%", !tbaa !2
  %16 = add i16 %14, -1
  %17 = icmp eq i16 %16, 0
  br i1 %17, label %b27, label %b28

b25:
  store i16 5, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string5)
          to label %b30 unwind label %landing

b27:
  store i16 4, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %51 unwind label %landing

b28:
  %18 = sdiv i16 10, %16
  store i16 4, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %18)
          to label %b29 unwind label %landing

b29:
  %19 = load i16, ptr @"I%", !tbaa !2
  %20 = icmp slt i16 %19, 3
  br i1 %20, label %b24, label %b25

b30:
  store i16 0, ptr @"N%", !tbaa !2
  store i16 1, ptr @"K%", !tbaa !2
  store i16 3, ptr @$data.2, !tbaa !2
  store i16 1, ptr @$data.3, !tbaa !2
  br label %b32

b32:
  %21 = phi i16 [ %35, %b40 ], [ 1, %b30 ]
  %22 = phi i16 [ %34, %b40 ], [ 1, %b30 ]
  %23 = icmp sgt i16 %22, -1
  %24 = load i16, ptr @$data.2, !tbaa !2
  br i1 %23, label %b33, label %b34

b33:
  %25 = icmp sgt i16 %21, %24
  br i1 %25, label %b36, label %b35

b34:
  %26 = icmp slt i16 %21, %24
  br i1 %26, label %b36, label %b35

b35:
  store i16 6, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string6)
          to label %54 unwind label %landing

b36:
  store i16 8, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string7)
          to label %52 unwind label %landing

b37:
  %27 = load i16, ptr @"K%", !tbaa !2
  %28 = add i16 %27, -2
  %29 = icmp eq i16 %28, 0
  br i1 %29, label %b38, label %b39

b38:
  store i16 7, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %56 unwind label %landing

b39:
  %30 = load i16, ptr @"N%", !tbaa !2
  %31 = sdiv i16 10, %28
  %32 = add i16 %31, %30
  store i16 %32, ptr @"N%", !tbaa !2
  br label %b40

b40:
  %33 = phi i16 [ %47, %46 ], [ %27, %b39 ]
  %34 = load i16, ptr @$data.3, !tbaa !2
  %35 = add nsw i16 %34, %33
  store i16 %35, ptr @"K%", !tbaa !2
  br label %b32

b41:
  call cc 1005 addrspace(1) void @llrm.qb.B$CEND()
  unreachable

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %36 = extractvalue { ptr, i32 } %landed, 1
  %37 = trunc i32 %36 to i16
  %resumed = call fastcc addrspace(1) i16 @__main$handler(i16 %37)
  %38 = load i16, ptr %site
  switch table i16 %38, label %39 [
    i16 0, label %39
    i16 1, label %40
    i16 2, label %41
    i16 3, label %42
    i16 4, label %43
    i16 5, label %44
    i16 6, label %45
    i16 7, label %46
    i16 8, label %48
  ]

39:
  br label %b11

40:
  br label %b12

41:
  br label %b20

42:
  br label %b24

43:
  br label %b29

44:
  br label %b30

45:
  br label %b37

46:
  %47 = load i16, ptr @"K%", !tbaa !2
  br label %b40

48:
  br label %b41

49:
  unreachable

50:
  unreachable

51:
  unreachable

52:
  %53 = load i16, ptr @"N%", !tbaa !2
  store i16 8, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %53)
          to label %b41 unwind label %landing

54:
  %55 = load i16, ptr @"K%", !tbaa !2
  store i16 6, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %55)
          to label %b37 unwind label %landing

56:
  unreachable
}

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1) noreturn

declare cc 1005 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 i16 @llrm.qb.B$FERR() addrspace(1) nounwind

declare cc 1005 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc 1005 void @llrm.qb.B$CEND() addrspace(1) nounwind noreturn

define internal fastcc i16 @__main$handler(i16 %0) addrspace(1) "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
entry:
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string8)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %0)
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
