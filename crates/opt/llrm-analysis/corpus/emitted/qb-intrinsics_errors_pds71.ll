target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [546 x i8] c"\01\00\00\00\01\00\00\00\01\00\00\00\00\00\01\00\00\00\03\00\00\00\02\00\00\00\00\00\01\00\00\00\04\00\00\00\05\00\00\00\00\00\01\00\00\00\05\00\00\00\06\00\00\00\00\00\01\00\00\00\06\00\00\00\07\00\00\00\00\00\01\00\00\00\07\00\00\00\0A\00\00\00\00\00\01\00\00\00\08\00\00\00\10\00\00\00\00\00\01\00\00\00\09\00\00\00\11\00\00\00\00\00\01\00\00\00\0A\00\00\00\12\00\00\00\00\00\01\00\00\00\0B\00\00\00\17\00\00\00\00\00\01\00\00\00\0C\00\00\00\1D\00\00\00\00\00\01\00\00\00\0D\00\00\00\1E\00\00\00\00\00\01\00\00\00\0E\00\00\00\1F\00\00\00\00\00\01\00\00\00\0F\00\00\00$\00\00\00\00\00\01\00\00\00\10\00\00\00*\00\00\00\00\00\01\00\00\00\11\00\00\00+\00\00\00\00\00\01\00\00\00\12\00\00\00,\00\00\00\00\00\01\00\00\00\13\00\00\001\00\00\00\00\00\01\00\00\00\14\00\00\007\00\00\00\00\00\01\00\00\00\15\00\00\008\00\00\00\00\00\01\00\00\00\16\00\00\009\00\00\00\00\00\01\00\00\00\17\00\00\00>\00\00\00\00\00\01\00\00\00\18\00\00\00D\00\00\00\00\00\01\00\00\00\19\00\00\00E\00\00\00\00\00\01\00\00\00\1A\00\00\00F\00\00\00\00\00\01\00\00\00\1B\00\00\00G\00\00\00\00\00\01\00\00\00\1C\00\00\00K\00\00\00\00\00\01\00\00\00\1D\00\00\00Q\00\00\00\00\00\01\00\00\00\1E\00\00\00R\00\00\00\00\00\01\00\00\00\1F\00\00\00S\00\00\00\00\00\01\00\00\00 \00\00\00U\00\00\00\00\00\01\00\00\00!\00\00\00Y\00\00\00\00\00\01\00\00\00\22\00\00\00_\00\00\00\00\00\01\00\00\00#\00\00\00a\00\00\00\00\00\01\00\00\00\02\00\00\00b\00\00\00\00\00\01\00\00\00%\00\00\00d\00\00\00\00\00\01\00\00\00&\00\00\00e\00\00\00\00\00\02\00\00\00\01\00\00\00\01\00\00\00\00\00\02\00\00\00\02\00\00\00\06\00\00\00\00\00"
@"CAUGHT%" = internal global [2 x i8] zeroinitializer
@P$ = internal global [4 x i8] zeroinitializer
@E$ = internal global [4 x i8] zeroinitializer
@"V%" = internal global [2 x i8] zeroinitializer
@"N%" = internal global [2 x i8] zeroinitializer
@$string3 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\03\00", ptr getelementptr (i8, ptr @$string3, i16 4), [4 x i8] c"abc\00" }>
@$string4 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\06\00", ptr getelementptr (i8, ptr @$string4, i16 4), [6 x i8] c"EMPTY=" }>
@$arguments$10 = internal global [6 x i8] zeroinitializer
@$string6 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string6, i16 4), [6 x i8] c"ZERO=\00" }>
@$string7 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string7, i16 4), [6 x i8] c"PAST=\00" }>
@$string8 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string8, i16 4), [6 x i8] c"LAST=\00" }>
@$string9 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string9, i16 4), [4 x i8] c"NEG=" }>
@$string10 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string10, i16 4), [4 x i8] c"BIG=" }>
@$string11 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string11, i16 4), [8 x i8] c"NEGCHR=\00" }>
@$string12 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string12, i16 4), [4 x i8] c"DONE" }>
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
  store i16 17, ptr %site
  %0 = invoke cc 1000 addrspace(1) i16 @llrm.qb.B$FERR()
          to label %19 unwind label %landing

b3:
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string3, ptr @P$)
          to label %20 unwind label %landing

b4:
  store i16 0, ptr @"CAUGHT%", !tbaa !2
  br label %b5

b5:
  store i16 0, ptr @"V%", !tbaa !2
  br label %b6

b6:
  store i16 1, ptr %site
  %1 = invoke cc 1000 addrspace(1) i16 @llrm.qb.B$FASC(ptr @E$)
          to label %21 unwind label %landing

b7:
  store i16 2, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4)
          to label %22 unwind label %landing

b8:
  store i16 0, ptr @"CAUGHT%", !tbaa !2
  br label %b9

b9:
  store i16 0, ptr @"V%", !tbaa !2
  br label %b10

b10:
  store i16 0, ptr @$arguments$10, !tbaa !2
  store i16 3, ptr %site
  %2 = invoke cc 1000 addrspace(1) i16 @"PROBE%"(ptr @P$, ptr @$arguments$10)
          to label %27 unwind label %landing

b11:
  store i16 4, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string6)
          to label %28 unwind label %landing

b12:
  store i16 0, ptr @"CAUGHT%", !tbaa !2
  br label %b13

b13:
  store i16 0, ptr @"V%", !tbaa !2
  br label %b14

b14:
  %3 = getelementptr i8, ptr @$arguments$10, i16 2
  store i16 4, ptr %3, !tbaa !2
  %4 = getelementptr i8, ptr @$arguments$10, i16 2
  store i16 5, ptr %site
  %5 = invoke cc 1000 addrspace(1) i16 @"PROBE%"(ptr @P$, ptr %4)
          to label %33 unwind label %landing

b15:
  store i16 6, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string7)
          to label %34 unwind label %landing

b16:
  store i16 0, ptr @"CAUGHT%", !tbaa !2
  br label %b17

b17:
  store i16 0, ptr @"V%", !tbaa !2
  br label %b18

b18:
  %6 = getelementptr i8, ptr @$arguments$10, i16 4
  store i16 3, ptr %6, !tbaa !2
  %7 = getelementptr i8, ptr @$arguments$10, i16 4
  store i16 7, ptr %site
  %8 = invoke cc 1000 addrspace(1) i16 @"PROBE%"(ptr @P$, ptr %7)
          to label %39 unwind label %landing

b19:
  store i16 8, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string8)
          to label %40 unwind label %landing

b20:
  store i16 0, ptr @"CAUGHT%", !tbaa !2
  br label %b21

b21:
  store i16 0, ptr @"V%", !tbaa !2
  br label %b22

b22:
  store i16 9, ptr %site
  %9 = invoke cc 1000 addrspace(1) ptr @llrm.qb.B$FMID(ptr @P$, i16 2, i16 -1)
          to label %45 unwind label %landing

b23:
  store i16 10, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string9)
          to label %48 unwind label %landing

b24:
  store i16 0, ptr @"CAUGHT%", !tbaa !2
  br label %b25

b25:
  store i16 0, ptr @"V%", !tbaa !2
  br label %b26

b26:
  store i16 256, ptr @"N%", !tbaa !2
  br label %b27

b27:
  %10 = load i16, ptr @"N%", !tbaa !2
  store i16 11, ptr %site
  %11 = invoke cc 1000 addrspace(1) ptr @llrm.qb.B$FCHR(i16 %10)
          to label %53 unwind label %landing

b28:
  store i16 12, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string10)
          to label %56 unwind label %landing

b29:
  store i16 0, ptr @"CAUGHT%", !tbaa !2
  br label %b30

b30:
  store i16 0, ptr @"V%", !tbaa !2
  br label %b31

b31:
  store i16 -1, ptr @"N%", !tbaa !2
  br label %b32

b32:
  %12 = load i16, ptr @"N%", !tbaa !2
  store i16 13, ptr %site
  %13 = invoke cc 1000 addrspace(1) ptr @llrm.qb.B$FCHR(i16 %12)
          to label %61 unwind label %landing

b33:
  store i16 14, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string11)
          to label %64 unwind label %landing

b34:
  store i16 15, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string12)
          to label %69 unwind label %landing

b35:
  store i16 16, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$CEND()
          to label %70 unwind label %landing

b37:
  store i16 18, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 20)
          to label %71 unwind label %landing

b38:
  ret void

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %14 = extractvalue { ptr, i32 } %landed, 1
  %15 = trunc i32 %14 to i16
  %resumed = call addrspace(1) i16 @__main$handler(i16 %15, i16 0)
  switch i16 %resumed, label %18 [
    i16 0, label %again
    i16 1, label %next
  ]

again:
  %16 = load i16, ptr %site
  switch i16 %16, label %72 [
    i16 0, label %b3
    i16 1, label %b6
    i16 2, label %b7
    i16 3, label %b10
    i16 4, label %b11
    i16 5, label %b14
    i16 6, label %b15
    i16 7, label %b18
    i16 8, label %b19
    i16 9, label %b22
    i16 10, label %b23
    i16 11, label %b27
    i16 12, label %b28
    i16 13, label %b32
    i16 14, label %b33
    i16 15, label %b34
    i16 16, label %b35
    i16 17, label %b2
    i16 18, label %b37
  ]

next:
  %17 = load i16, ptr %site
  switch i16 %17, label %73 [
    i16 0, label %b4
    i16 1, label %b7
    i16 2, label %b8
    i16 3, label %b11
    i16 4, label %b12
    i16 5, label %b15
    i16 6, label %b16
    i16 7, label %b19
    i16 8, label %b20
    i16 9, label %b23
    i16 10, label %b24
    i16 11, label %b28
    i16 12, label %b29
    i16 13, label %b33
    i16 14, label %b34
    i16 15, label %b35
    i16 16, label %b2
    i16 17, label %b37
    i16 18, label %b38
  ]

18:
  unreachable

19:
  store i16 %0, ptr @"CAUGHT%", !tbaa !2
  br label %b37

20:
  br label %b4

21:
  store i16 %1, ptr @"V%", !tbaa !2
  br label %b7

22:
  %23 = load i16, ptr @"CAUGHT%", !tbaa !2
  store i16 2, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %23)
          to label %24 unwind label %landing

24:
  %25 = load i16, ptr @"V%", !tbaa !2
  store i16 2, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %25)
          to label %26 unwind label %landing

26:
  br label %b8

27:
  store i16 %2, ptr @"V%", !tbaa !2
  br label %b11

28:
  %29 = load i16, ptr @"CAUGHT%", !tbaa !2
  store i16 4, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %29)
          to label %30 unwind label %landing

30:
  %31 = load i16, ptr @"V%", !tbaa !2
  store i16 4, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %31)
          to label %32 unwind label %landing

32:
  br label %b12

33:
  store i16 %5, ptr @"V%", !tbaa !2
  br label %b15

34:
  %35 = load i16, ptr @"CAUGHT%", !tbaa !2
  store i16 6, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %35)
          to label %36 unwind label %landing

36:
  %37 = load i16, ptr @"V%", !tbaa !2
  store i16 6, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %37)
          to label %38 unwind label %landing

38:
  br label %b16

39:
  store i16 %8, ptr @"V%", !tbaa !2
  br label %b19

40:
  %41 = load i16, ptr @"CAUGHT%", !tbaa !2
  store i16 8, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %41)
          to label %42 unwind label %landing

42:
  %43 = load i16, ptr @"V%", !tbaa !2
  store i16 8, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %43)
          to label %44 unwind label %landing

44:
  br label %b20

45:
  store i16 9, ptr %site
  %46 = invoke cc 1000 addrspace(1) i16 @llrm.qb.B$FASC(ptr %9)
          to label %47 unwind label %landing

47:
  store i16 %46, ptr @"V%", !tbaa !2
  br label %b23

48:
  %49 = load i16, ptr @"CAUGHT%", !tbaa !2
  store i16 10, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %49)
          to label %50 unwind label %landing

50:
  %51 = load i16, ptr @"V%", !tbaa !2
  store i16 10, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %51)
          to label %52 unwind label %landing

52:
  br label %b24

53:
  store i16 11, ptr %site
  %54 = invoke cc 1000 addrspace(1) i16 @llrm.qb.B$FASC(ptr %11)
          to label %55 unwind label %landing

55:
  store i16 %54, ptr @"V%", !tbaa !2
  br label %b28

56:
  %57 = load i16, ptr @"CAUGHT%", !tbaa !2
  store i16 12, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %57)
          to label %58 unwind label %landing

58:
  %59 = load i16, ptr @"V%", !tbaa !2
  store i16 12, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %59)
          to label %60 unwind label %landing

60:
  br label %b29

61:
  store i16 13, ptr %site
  %62 = invoke cc 1000 addrspace(1) i16 @llrm.qb.B$FASC(ptr %13)
          to label %63 unwind label %landing

63:
  store i16 %62, ptr @"V%", !tbaa !2
  br label %b33

64:
  %65 = load i16, ptr @"CAUGHT%", !tbaa !2
  store i16 14, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %65)
          to label %66 unwind label %landing

66:
  %67 = load i16, ptr @"V%", !tbaa !2
  store i16 14, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %67)
          to label %68 unwind label %landing

68:
  br label %b34

69:
  br label %b35

70:
  unreachable

71:
  unreachable

72:
  unreachable

73:
  unreachable
}

define cc 1000 i16 @"PROBE%"(ptr dereferenceable(4) %0, ptr dereferenceable(2) %1) addrspace(1) noinline "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
frame:
  %2 = alloca i16
  %site = alloca i16
  call addrspace(1) void @llrm.qb.frame(ptr %2)
  store i16 0, ptr %2
  call addrspace(1) void @llrm.qb.frame(ptr %site)
  br label %b1

b1:
  %3 = load i16, ptr %1
  store i16 0, ptr %site
  %4 = invoke cc 1000 addrspace(1) ptr @llrm.qb.B$FMID(ptr %0, i16 %3, i16 1)
          to label %11 unwind label %landing

b2:
  br label %b3

b3:
  %5 = load i16, ptr %2, !tbaa !2
  ret i16 %5

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
  switch i16 %8, label %14 [
    i16 0, label %b1
  ]

next:
  %9 = load i16, ptr %site
  switch i16 %9, label %15 [
    i16 0, label %b2
  ]

10:
  unreachable

11:
  store i16 0, ptr %site
  %12 = invoke cc 1000 addrspace(1) i16 @llrm.qb.B$FASC(ptr %4)
          to label %13 unwind label %landing

13:
  store i16 %12, ptr %2, !tbaa !2
  br label %b2

14:
  unreachable

15:
  unreachable
}

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1)

declare cc 1000 i16 @llrm.qb.B$FERR() addrspace(1)

declare cc 1000 void @llrm.qb.B$SASS(ptr, ptr) addrspace(1)

declare cc 1000 i16 @llrm.qb.B$FASC(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1000 ptr @llrm.qb.B$FMID(ptr, i16, i16) addrspace(1)

declare cc 1000 ptr @llrm.qb.B$FCHR(i16) addrspace(1)

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
  store i16 %0, ptr @"CAUGHT%", !tbaa !2
  br label %b37

b37:
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
