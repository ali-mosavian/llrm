target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"CAUGHT%" = internal global [2 x i8] zeroinitializer
@"WHERE%" = internal global [2 x i8] zeroinitializer
@"K%" = internal global [2 x i8] zeroinitializer
@"I%" = internal global [2 x i8] zeroinitializer
@"SUM%" = internal global [2 x i8] zeroinitializer
@"A&" = internal global [4 x i8] zeroinitializer
@S$ = internal global [4 x i8] zeroinitializer
@$string3 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\06\00", ptr getelementptr (i8, ptr @$string3, i16 4), [6 x i8] c"ERROR=" }>
@$string4 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\06\00", ptr getelementptr (i8, ptr @$string4, i16 4), [6 x i8] c"SPACE=" }>
@$string5 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\06\00", ptr getelementptr (i8, ptr @$string5, i16 4), [6 x i8] c"RETRY=" }>
@$string6 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string6, i16 4), [8 x i8] c"SKIPPED\00" }>
@$string7 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\06\00", ptr getelementptr (i8, ptr @$string7, i16 4), [6 x i8] c"LABEL=" }>
@$string8 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\03\00", ptr getelementptr (i8, ptr @$string8, i16 4), [4 x i8] c"abc\00" }>
@$string9 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string9, i16 4), [6 x i8] c"LOOP=\00" }>
@$string10 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string10, i16 4), [6 x i8] c"KEPT=\00" }>
@$string11 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string11, i16 4), [4 x i8] c"DONE" }>
@$data.0 = internal global i16 0
@$data.1 = internal global i16 0

define internal cc 1000 void @__main() addrspace(1) noinline "llrm-fp-eval"="x87-extended" noreturn personality ptr addrspace(1) @llrm.qb.personality {
frame:
  %site = alloca i16
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %site)
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 true)
  store i16 1, ptr @"WHERE%", !tbaa !2
  br label %b5

b2:
  store i16 8, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string7)
          to label %72 unwind label %landing

b5:
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 52)
          to label %57 unwind label %landing

b6:
  store i16 1, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string3)
          to label %58 unwind label %landing

b7:
  store i16 2, ptr @"WHERE%", !tbaa !2
  store i16 -1, ptr @"K%", !tbaa !2
  br label %b9

b9:
  %0 = phi i16 [ %28, %27 ], [ -1, %b7 ]
  store i16 2, ptr %site
  %1 = invoke cc 1005 addrspace(1) ptr @llrm.qb.B$SPAC(i16 %0)
          to label %62 unwind label %landing

b10:
  store i16 3, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4)
          to label %63 unwind label %landing

b11:
  store i16 3, ptr @"WHERE%", !tbaa !2
  store i16 300, ptr @"K%", !tbaa !2
  br label %b13

b13:
  %2 = phi i16 [ %31, %30 ], [ 300, %b11 ]
  store i16 4, ptr %site
  %3 = invoke cc 1005 addrspace(1) ptr @llrm.qb.B$FCHR(i16 %2)
          to label %65 unwind label %landing

b14:
  store i16 5, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5)
          to label %66 unwind label %landing

b15:
  store i16 4, ptr @"WHERE%", !tbaa !2
  br label %b16

b16:
  store i16 6, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 7)
          to label %71 unwind label %landing

b17:
  store i16 7, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string6)
          to label %b2 unwind label %landing

b18:
  store i16 5, ptr @"WHERE%", !tbaa !2
  store i16 0, ptr @"SUM%", !tbaa !2
  store i16 1, ptr @"I%", !tbaa !2
  store i16 5, ptr @$data.0, !tbaa !2
  store i16 1, ptr @$data.1, !tbaa !2
  br label %b21

b21:
  %4 = phi i16 [ %17, %b26 ], [ 1, %b18 ]
  %5 = phi i16 [ %16, %b26 ], [ 1, %b18 ]
  %6 = icmp sgt i16 %5, -1
  %7 = load i16, ptr @$data.0, !tbaa !2
  br i1 %6, label %b22, label %b23

b22:
  %8 = icmp sgt i16 %4, %7
  br i1 %8, label %b25, label %b24

b23:
  %9 = icmp slt i16 %4, %7
  br i1 %9, label %b25, label %b24

b24:
  %10 = phi i16 [ %37, %36 ], [ %4, %b22 ], [ %4, %b23 ]
  %11 = sub i16 3, %10
  store i16 9, ptr %site
  %12 = invoke cc 1005 addrspace(1) ptr @llrm.qb.B$LEFT(ptr @$string8, i16 %11)
          to label %78 unwind label %landing

b25:
  store i16 10, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string9)
          to label %74 unwind label %landing

b26:
  %13 = load i16, ptr @"SUM%", !tbaa !2
  %14 = load i16, ptr @"I%", !tbaa !2
  %15 = add i16 %14, %13
  store i16 %15, ptr @"SUM%", !tbaa !2
  %16 = load i16, ptr @$data.1, !tbaa !2
  %17 = add nsw i16 %16, %14
  store i16 %17, ptr @"I%", !tbaa !2
  br label %b21

b28:
  store i32 305419896, ptr @"A&", !tbaa !2
  store i16 6, ptr @"WHERE%", !tbaa !2
  br label %b30

b30:
  store i16 11, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 9)
          to label %79 unwind label %landing

b31:
  store i16 12, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string10)
          to label %80 unwind label %landing

b32:
  store i16 13, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string11)
          to label %b33 unwind label %landing

b33:
  call cc 1005 addrspace(1) void @llrm.qb.B$CEND()
  unreachable

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %18 = extractvalue { ptr, i32 } %landed, 1
  %19 = trunc i32 %18 to i16
  %resumed = call fastcc addrspace(1) i16 @__main$handler(i16 %19)
  %20 = icmp eq i16 %resumed, 2
  br i1 %20, label %b2, label %21

21:
  %22 = icmp eq i16 %resumed, 1
  br i1 %22, label %next, label %23

23:
  %24 = load i16, ptr %site
  switch table i16 %24, label %25 [
    i16 0, label %25
    i16 1, label %26
    i16 2, label %27
    i16 3, label %29
    i16 4, label %30
    i16 5, label %32
    i16 6, label %33
    i16 7, label %34
    i16 8, label %35
    i16 9, label %36
    i16 10, label %38
    i16 11, label %39
    i16 12, label %40
    i16 13, label %41
  ]

25:
  br label %b5

26:
  br label %b6

27:
  %28 = load i16, ptr @"K%", !tbaa !2
  br label %b9

29:
  br label %b10

30:
  %31 = load i16, ptr @"K%", !tbaa !2
  br label %b13

32:
  br label %b14

33:
  br label %b16

34:
  br label %b17

35:
  br label %b2

36:
  %37 = load i16, ptr @"I%", !tbaa !2
  br label %b24

38:
  br label %b25

39:
  br label %b30

40:
  br label %b31

41:
  br label %b32

next:
  %42 = load i16, ptr %site
  switch table i16 %42, label %43 [
    i16 0, label %43
    i16 1, label %44
    i16 2, label %45
    i16 3, label %46
    i16 4, label %47
    i16 5, label %48
    i16 6, label %49
    i16 7, label %50
    i16 8, label %51
    i16 9, label %52
    i16 10, label %53
    i16 11, label %54
    i16 12, label %55
    i16 13, label %56
  ]

43:
  br label %b6

44:
  br label %b7

45:
  br label %b10

46:
  br label %b11

47:
  br label %b14

48:
  br label %b15

49:
  br label %b17

50:
  br label %b2

51:
  br label %b18

52:
  br label %b26

53:
  br label %b28

54:
  br label %b31

55:
  br label %b32

56:
  br label %b33

57:
  unreachable

58:
  %59 = load i16, ptr @"CAUGHT%", !tbaa !2
  store i16 1, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %59)
          to label %60 unwind label %landing

60:
  %61 = call cc 1000 addrspace(1) i16 @llrm.qb.B$FERR()
  store i16 1, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %61)
          to label %b7 unwind label %landing

62:
  store i16 2, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$SASS(ptr %1, ptr @S$)
          to label %b10 unwind label %landing

63:
  %64 = load i16, ptr @"CAUGHT%", !tbaa !2
  store i16 3, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %64)
          to label %b11 unwind label %landing

65:
  store i16 4, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$SASS(ptr %3, ptr @S$)
          to label %b14 unwind label %landing

66:
  %67 = load i16, ptr @"CAUGHT%", !tbaa !2
  store i16 5, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %67)
          to label %68 unwind label %landing

68:
  store i16 5, ptr %site
  %69 = invoke cc 1005 addrspace(1) i16 @llrm.qb.B$FASC(ptr @S$)
          to label %70 unwind label %landing

70:
  store i16 5, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %69)
          to label %b15 unwind label %landing

71:
  unreachable

72:
  %73 = load i16, ptr @"CAUGHT%", !tbaa !2
  store i16 8, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %73)
          to label %b18 unwind label %landing

74:
  %75 = load i16, ptr @"CAUGHT%", !tbaa !2
  store i16 10, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %75)
          to label %76 unwind label %landing

76:
  %77 = load i16, ptr @"SUM%", !tbaa !2
  store i16 10, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %77)
          to label %b28 unwind label %landing

78:
  store i16 9, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$SASS(ptr %12, ptr @S$)
          to label %b26 unwind label %landing

79:
  unreachable

80:
  %81 = load i32, ptr @"A&", !tbaa !2
  store i16 12, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI4(i32 %81)
          to label %82 unwind label %landing

82:
  %83 = load i16, ptr @"CAUGHT%", !tbaa !2
  store i16 12, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %83)
          to label %b32 unwind label %landing
}

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1) noreturn

declare cc 1005 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1005 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1000 i16 @llrm.qb.B$FERR() addrspace(1) nounwind

declare cc 1005 void @llrm.qb.B$PSI2(i16) addrspace(1)

declare cc 1005 ptr @llrm.qb.B$SPAC(i16) addrspace(1)

declare cc 1005 void @llrm.qb.B$SASS(ptr, ptr) addrspace(1)

declare cc 1005 ptr @llrm.qb.B$FCHR(i16) addrspace(1) "llrm-fp-eval"="x87-extended"

declare cc 1005 i16 @llrm.qb.B$FASC(ptr releases) addrspace(1) alwaysinline "llrm-fp-eval"="x87-extended"

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc 1005 ptr @llrm.qb.B$LEFT(ptr, i16) addrspace(1)

declare cc 1005 void @llrm.qb.B$PSI4(i32) addrspace(1)

declare cc 1005 void @llrm.qb.B$CEND() addrspace(1) nounwind noreturn

define internal fastcc range(i16 0, 3) i16 @__main$handler(i16 %0) addrspace(1) "llrm-fp-eval"="x87-extended" nounwind personality ptr addrspace(1) @llrm.qb.personality {
entry:
  store i16 %0, ptr @"CAUGHT%", !tbaa !2
  %1 = load i16, ptr @"WHERE%", !tbaa !2
  %2 = icmp eq i16 %1, 4
  br i1 %2, label %common.ret, label %3

3:
  %4 = icmp eq i16 %1, 3
  br i1 %4, label %b37, label %b42

b37:
  store i16 65, ptr @"K%", !tbaa !2
  br label %common.ret

common.ret:
  %common.op = phi i16 [ 0, %b37 ], [ 1, %b42 ], [ 2, %entry ]
  tail call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 true)
  ret i16 %common.op

b42:
  br label %common.ret
}

declare i32 @llrm.qb.personality(...) addrspace(1)

declare cc 1000 void @llrm.qb.onerror(i1) addrspace(1) nounwind frame_bound

declare cc 1000 void @llrm.qb.frame(ptr) addrspace(1) nounwind

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
