target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [4 x i8] zeroinitializer
@$qb$statementTable = internal constant [504 x i8] c"\01\00\00\00\01\00\00\00\01\00\00\00\00\00\01\00\00\00\04\00\00\00\02\00\00\00\00\00\01\00\00\00\05\00\00\00\03\00\00\00\00\00\01\00\00\00\06\00\00\00\04\00\00\00\00\00\01\00\00\00\07\00\00\00\0A\00\00\00\00\00\01\00\00\00\08\00\00\00\0B\00\00\00\00\00\01\00\00\00\09\00\00\00\0D\00\00\00\00\00\01\00\00\00\0A\00\00\00\11\00\00\00\00\00\01\00\00\00\0B\00\00\00\15\00\00\00\00\00\01\00\00\00\0C\00\00\00\16\00\00\00\00\00\01\00\00\00\0D\00\00\00\17\00\00\00\00\00\01\00\00\00\0E\00\00\00\1B\00\00\00\00\00\01\00\00\00\0F\00\00\00\22\00\00\00\00\00\01\00\00\00\10\00\00\00#\00\00\00\00\00\01\00\00\00\11\00\00\00$\00\00\00\00\00\01\00\00\00\02\00\00\00&\00\00\00\00\00\01\00\00\00\12\00\00\00*\00\00\00\00\00\01\00\00\00\13\00\00\00+\00\00\00\00\00\01\00\00\00\14\00\00\00,\00\00\00\00\00\01\00\00\00\18\00\00\007\00\00\00\00\00\01\00\00\00\1A\00\00\00=\00\00\00\00\00\01\00\00\00\1B\00\00\00A\00\00\00\00\00\01\00\00\00\19\00\00\00E\00\00\00\00\00\01\00\00\00\1C\00\00\00K\00\00\00\00\00\01\00\00\00\1D\00\00\00L\00\00\00\00\00\01\00\00\00\1E\00\00\00M\00\00\00\00\00\01\00\00\00\1F\00\00\00N\00\00\00\00\00\01\00\00\00 \00\00\00T\00\00\00\00\00\01\00\00\00!\00\00\00V\00\00\00\00\00\01\00\00\00\03\00\00\00W\00\00\00\00\00\01\00\00\00#\00\00\00Y\00\00\00\00\00\01\00\00\00%\00\00\00[\00\00\00\00\00\01\00\00\00'\00\00\00\\\00\00\00\00\00\01\00\00\00)\00\00\00^\00\00\00\00\00\01\00\00\00*\00\00\00_\00\00\00\00\00\01\00\00\00$\00\00\00`\00\00\00\00\00"
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
  br label %b4

b2:
  store i16 8, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string7)
          to label %56 unwind label %landing

b3:
  store i16 15, ptr %site
  %0 = invoke cc 1000 addrspace(1) i16 @llrm.qb.B$FERR()
          to label %33 unwind label %landing

b4:
  store i16 1, ptr @"WHERE%", !tbaa !2
  br label %b5

b5:
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 52)
          to label %34 unwind label %landing

b6:
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string3)
          to label %35 unwind label %landing

b7:
  store i16 2, ptr @"WHERE%", !tbaa !2
  br label %b8

b8:
  store i16 -1, ptr @"K%", !tbaa !2
  br label %b9

b9:
  %1 = load i16, ptr @"K%", !tbaa !2
  store i16 2, ptr %site
  %2 = invoke cc 1000 addrspace(1) ptr @llrm.qb.B$SPAC(i16 %1)
          to label %41 unwind label %landing

b10:
  store i16 3, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4)
          to label %43 unwind label %landing

b11:
  store i16 3, ptr @"WHERE%", !tbaa !2
  br label %b12

b12:
  store i16 300, ptr @"K%", !tbaa !2
  br label %b13

b13:
  %3 = load i16, ptr @"K%", !tbaa !2
  store i16 4, ptr %site
  %4 = invoke cc 1000 addrspace(1) ptr @llrm.qb.B$FCHR(i16 %3)
          to label %46 unwind label %landing

b14:
  store i16 5, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5)
          to label %48 unwind label %landing

b15:
  store i16 4, ptr @"WHERE%", !tbaa !2
  br label %b16

b16:
  store i16 6, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 7)
          to label %54 unwind label %landing

b17:
  store i16 7, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string6)
          to label %55 unwind label %landing

b18:
  store i16 5, ptr @"WHERE%", !tbaa !2
  br label %b19

b19:
  store i16 0, ptr @"SUM%", !tbaa !2
  br label %b20

b20:
  store i16 1, ptr @"I%", !tbaa !2
  store i16 5, ptr @$data, !tbaa !2
  %5 = getelementptr i8, ptr @$data, i16 2
  store i16 1, ptr %5, !tbaa !2
  br label %b21

b21:
  %6 = getelementptr i8, ptr @$data, i16 2
  %7 = load i16, ptr %6, !tbaa !2
  %8 = icmp sge i16 %7, 0
  br i1 %8, label %b22, label %b23

b22:
  %9 = load i16, ptr @"I%", !tbaa !2
  %10 = load i16, ptr @$data, !tbaa !2
  %11 = icmp sle i16 %9, %10
  br i1 %11, label %b24, label %b25

b23:
  %12 = load i16, ptr @"I%", !tbaa !2
  %13 = load i16, ptr @$data, !tbaa !2
  %14 = icmp sge i16 %12, %13
  br i1 %14, label %b24, label %b25

b24:
  %15 = load i16, ptr @"I%", !tbaa !2
  %16 = sub i16 3, %15
  store i16 9, ptr %site
  %17 = invoke cc 1000 addrspace(1) ptr @llrm.qb.B$LEFT(ptr @$string8, i16 %16)
          to label %64 unwind label %landing

b25:
  store i16 10, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string9)
          to label %59 unwind label %landing

b26:
  %18 = load i16, ptr @"SUM%", !tbaa !2
  %19 = load i16, ptr @"I%", !tbaa !2
  %20 = add i16 %18, %19
  store i16 %20, ptr @"SUM%", !tbaa !2
  br label %b27

b27:
  %21 = load i16, ptr @"I%", !tbaa !2
  %22 = getelementptr i8, ptr @$data, i16 2
  %23 = load i16, ptr %22, !tbaa !2
  %24 = add nsw i16 %21, %23
  store i16 %24, ptr @"I%", !tbaa !2
  br label %b21

b28:
  store i32 305419896, ptr @"A&", !tbaa !2
  br label %b29

b29:
  store i16 6, ptr @"WHERE%", !tbaa !2
  br label %b30

b30:
  store i16 11, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 9)
          to label %66 unwind label %landing

b31:
  store i16 12, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string10)
          to label %67 unwind label %landing

b32:
  store i16 13, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string11)
          to label %72 unwind label %landing

b33:
  store i16 14, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$CEND()
          to label %73 unwind label %landing

b35:
  %25 = load i16, ptr @"WHERE%", !tbaa !2
  %26 = icmp eq i16 %25, 3
  br i1 %26, label %b37, label %b38

b36:
  ret void

b37:
  store i16 65, ptr @"K%", !tbaa !2
  br label %b39

b38:
  %27 = icmp eq i16 %25, 4
  br i1 %27, label %b41, label %b42

b39:
  store i16 16, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 20)
          to label %74 unwind label %landing

b41:
  store i16 17, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 20)
          to label %75 unwind label %landing

b42:
  store i16 18, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 20)
          to label %76 unwind label %landing

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %28 = extractvalue { ptr, i32 } %landed, 1
  %29 = trunc i32 %28 to i16
  %resumed = call addrspace(1) i16 @__main$handler(i16 %29, i16 0)
  switch i16 %resumed, label %32 [
    i16 0, label %again
    i16 1, label %next
    i16 2, label %b2
  ]

again:
  %30 = load i16, ptr %site
  switch i16 %30, label %77 [
    i16 0, label %b5
    i16 1, label %b6
    i16 2, label %b9
    i16 3, label %b10
    i16 4, label %b13
    i16 5, label %b14
    i16 6, label %b16
    i16 7, label %b17
    i16 8, label %b2
    i16 9, label %b24
    i16 10, label %b25
    i16 11, label %b30
    i16 12, label %b31
    i16 13, label %b32
    i16 14, label %b33
    i16 15, label %b3
    i16 16, label %b39
    i16 17, label %b41
    i16 18, label %b42
  ]

next:
  %31 = load i16, ptr %site
  switch i16 %31, label %78 [
    i16 0, label %b6
    i16 1, label %b7
    i16 2, label %b10
    i16 3, label %b11
    i16 4, label %b14
    i16 5, label %b15
    i16 6, label %b17
    i16 7, label %b2
    i16 8, label %b18
    i16 9, label %b26
    i16 10, label %b28
    i16 11, label %b31
    i16 12, label %b32
    i16 13, label %b33
    i16 14, label %b3
    i16 15, label %b35
    i16 16, label %b41
    i16 17, label %b42
    i16 18, label %b36
  ]

32:
  unreachable

33:
  store i16 %0, ptr @"CAUGHT%", !tbaa !2
  br label %b35

34:
  unreachable

35:
  %36 = load i16, ptr @"CAUGHT%", !tbaa !2
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %36)
          to label %37 unwind label %landing

37:
  store i16 1, ptr %site
  %38 = invoke cc 1000 addrspace(1) i16 @llrm.qb.B$FERR()
          to label %39 unwind label %landing

39:
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %38)
          to label %40 unwind label %landing

40:
  br label %b7

41:
  store i16 2, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr %2, ptr @S$)
          to label %42 unwind label %landing

42:
  br label %b10

43:
  %44 = load i16, ptr @"CAUGHT%", !tbaa !2
  store i16 3, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %44)
          to label %45 unwind label %landing

45:
  br label %b11

46:
  store i16 4, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr %4, ptr @S$)
          to label %47 unwind label %landing

47:
  br label %b14

48:
  %49 = load i16, ptr @"CAUGHT%", !tbaa !2
  store i16 5, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %49)
          to label %50 unwind label %landing

50:
  store i16 5, ptr %site
  %51 = invoke cc 1000 addrspace(1) i16 @llrm.qb.B$FASC(ptr @S$)
          to label %52 unwind label %landing

52:
  store i16 5, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %51)
          to label %53 unwind label %landing

53:
  br label %b15

54:
  unreachable

55:
  br label %b2

56:
  %57 = load i16, ptr @"CAUGHT%", !tbaa !2
  store i16 8, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %57)
          to label %58 unwind label %landing

58:
  br label %b18

59:
  %60 = load i16, ptr @"CAUGHT%", !tbaa !2
  store i16 10, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %60)
          to label %61 unwind label %landing

61:
  %62 = load i16, ptr @"SUM%", !tbaa !2
  store i16 10, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %62)
          to label %63 unwind label %landing

63:
  br label %b28

64:
  store i16 9, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr %17, ptr @S$)
          to label %65 unwind label %landing

65:
  br label %b26

66:
  unreachable

67:
  %68 = load i32, ptr @"A&", !tbaa !2
  store i16 12, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %68)
          to label %69 unwind label %landing

69:
  %70 = load i16, ptr @"CAUGHT%", !tbaa !2
  store i16 12, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %70)
          to label %71 unwind label %landing

71:
  br label %b32

72:
  br label %b33

73:
  unreachable

74:
  unreachable

75:
  unreachable

76:
  unreachable

77:
  unreachable

78:
  unreachable
}

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1000 i16 @llrm.qb.B$FERR() addrspace(1)

declare cc 1000 void @llrm.qb.B$PSI2(i16) addrspace(1)

declare cc 1000 ptr @llrm.qb.B$SPAC(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$SASS(ptr, ptr) addrspace(1)

declare cc 1000 ptr @llrm.qb.B$FCHR(i16) addrspace(1)

declare cc 1000 i16 @llrm.qb.B$FASC(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc 1000 ptr @llrm.qb.B$LEFT(ptr, i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSI4(i32) addrspace(1)

declare cc 1000 void @llrm.qb.B$CEND() addrspace(1)

define internal i16 @__main$handler(i16 %0, i16 %1) addrspace(1) "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
entry:
  store i16 %1, ptr @$QB$ERL
  %2 = load i16, ptr @$QB$ACTIVE
  switch i16 %2, label %6 [
    i16 1, label %b3
  ]

b3:
  store i16 %0, ptr @"CAUGHT%", !tbaa !2
  br label %b35

b35:
  %3 = load i16, ptr @"WHERE%", !tbaa !2
  %4 = icmp eq i16 %3, 3
  br i1 %4, label %b37, label %b38

b38:
  %5 = icmp eq i16 %3, 4
  br i1 %5, label %b41, label %b42

b37:
  store i16 65, ptr @"K%", !tbaa !2
  br label %b39

b39:
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 true)
  ret i16 0

b41:
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 true)
  ret i16 2

b42:
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 true)
  ret i16 1

6:
  unreachable

7:
  unreachable

8:
  unreachable

9:
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
