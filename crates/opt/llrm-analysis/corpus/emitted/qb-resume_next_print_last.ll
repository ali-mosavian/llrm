target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [8 x i8] zeroinitializer
@$qb$statementTable = internal constant [378 x i8] c"\01\00\00\00\01\00\00\00\01\00\00\00\00\00\01\00\00\00\03\00\00\00\02\00\00\00\00\00\01\00\00\00\07\00\00\00\0D\00\00\00\00\00\01\00\00\00\0B\00\00\00\15\00\00\00\00\00\01\00\00\00\08\00\00\00\19\00\00\00\00\00\01\00\00\00\0C\00\00\00\1B\00\00\00\00\00\01\00\00\00\0D\00\00\00\1C\00\00\00\00\00\01\00\00\00\0F\00\00\00\1E\00\00\00\00\00\01\00\00\00\11\00\00\00!\00\00\00\00\00\01\00\00\00\14\00\00\00)\00\00\00\00\00\01\00\00\00\10\00\00\00)\00\00\00\00\00\01\00\00\00\15\00\00\00+\00\00\00\00\00\01\00\00\00\16\00\00\00,\00\00\00\00\00\01\00\00\00\18\00\00\00,\00\00\00\00\00\01\00\00\00\1A\00\00\00/\00\00\00\00\00\01\00\00\00\1D\00\00\007\00\00\00\00\00\01\00\00\00\19\00\00\009\00\00\00\00\00\01\00\00\00\1E\00\00\00;\00\00\00\00\00\01\00\00\00\1F\00\00\00<\00\00\00\00\00\01\00\00\00#\00\00\00G\00\00\00\00\00\01\00\00\00%\00\00\00K\00\00\00\00\00\01\00\00\00(\00\00\00U\00\00\00\00\00\01\00\00\00$\00\00\00Y\00\00\00\00\00\01\00\00\00)\00\00\00]\00\00\00\00\00\01\00\00\00\02\00\00\00^\00\00\00\00\00\01\00\00\00+\00\00\00b\00\00\00\00\00\01\00\00\00,\00\00\00c\00\00\00\00\00"
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
  store i16 10, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string8)
          to label %65 unwind label %landing

b3:
  store i16 1, ptr @"K%", !tbaa !2
  store i16 3, ptr @$data, !tbaa !2
  %0 = getelementptr i8, ptr @$data, i16 2
  store i16 1, ptr %0, !tbaa !2
  br label %b4

b4:
  %1 = getelementptr i8, ptr @$data, i16 2
  %2 = load i16, ptr %1, !tbaa !2
  %3 = icmp sge i16 %2, 0
  br i1 %3, label %b5, label %b6

b5:
  %4 = load i16, ptr @"K%", !tbaa !2
  %5 = load i16, ptr @$data, !tbaa !2
  %6 = icmp sle i16 %4, %5
  br i1 %6, label %b7, label %b8

b6:
  %7 = load i16, ptr @"K%", !tbaa !2
  %8 = load i16, ptr @$data, !tbaa !2
  %9 = icmp sge i16 %7, %8
  br i1 %9, label %b7, label %b8

b7:
  %10 = load i16, ptr @"K%", !tbaa !2
  %11 = sub i16 %10, 1
  %12 = sext i16 %11 to i32
  %13 = icmp eq i32 %12, 0
  br i1 %13, label %b9, label %b10

b8:
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string3)
          to label %69 unwind label %landing

b9:
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %75 unwind label %landing

b10:
  %14 = icmp eq i32 %12, 0
  br i1 %14, label %trap, label %70

b11:
  %15 = load i16, ptr @"K%", !tbaa !2
  %16 = getelementptr i8, ptr @$data, i16 2
  %17 = load i16, ptr %16, !tbaa !2
  %18 = add nsw i16 %15, %17
  store i16 %18, ptr @"K%", !tbaa !2
  br label %b4

b12:
  store i16 0, ptr @"J%", !tbaa !2
  br label %b13

b13:
  br label %b14

b14:
  %19 = load i16, ptr @"J%", !tbaa !2
  %20 = icmp slt i16 %19, 3
  br i1 %20, label %b15, label %b16

b15:
  %21 = load i16, ptr @"J%", !tbaa !2
  %22 = add i16 %21, 1
  store i16 %22, ptr @"J%", !tbaa !2
  br label %b17

b16:
  store i16 3, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string4)
          to label %76 unwind label %landing

b17:
  %23 = load i16, ptr @"J%", !tbaa !2
  %24 = sub i16 %23, 2
  %25 = sext i16 %24 to i32
  %26 = icmp eq i32 %25, 0
  br i1 %26, label %b18, label %b19

b18:
  store i16 2, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %82 unwind label %landing

b19:
  %27 = icmp eq i32 %25, 0
  br i1 %27, label %trap1, label %77

b20:
  br label %b14

b21:
  store i16 0, ptr @"I%", !tbaa !2
  br label %b22

b22:
  br label %b24

b23:
  %28 = load i16, ptr @"I%", !tbaa !2
  %29 = icmp slt i16 %28, 3
  br i1 %29, label %b24, label %b25

b24:
  %30 = load i16, ptr @"I%", !tbaa !2
  %31 = add i16 %30, 1
  store i16 %31, ptr @"I%", !tbaa !2
  br label %b26

b25:
  store i16 5, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string5)
          to label %83 unwind label %landing

b26:
  %32 = load i16, ptr @"I%", !tbaa !2
  %33 = sub i16 %32, 2
  %34 = sext i16 %33 to i32
  %35 = icmp eq i32 %34, 0
  br i1 %35, label %b27, label %b28

b27:
  store i16 4, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %89 unwind label %landing

b28:
  %36 = icmp eq i32 %34, 0
  br i1 %36, label %trap2, label %84

b29:
  br label %b23

b30:
  store i16 0, ptr @"N%", !tbaa !2
  br label %b31

b31:
  store i16 1, ptr @"K%", !tbaa !2
  %37 = getelementptr i8, ptr @$data, i16 4
  store i16 3, ptr %37, !tbaa !2
  %38 = getelementptr i8, ptr @$data, i16 6
  store i16 1, ptr %38, !tbaa !2
  br label %b32

b32:
  %39 = getelementptr i8, ptr @$data, i16 6
  %40 = load i16, ptr %39, !tbaa !2
  %41 = icmp sge i16 %40, 0
  br i1 %41, label %b33, label %b34

b33:
  %42 = load i16, ptr @"K%", !tbaa !2
  %43 = getelementptr i8, ptr @$data, i16 4
  %44 = load i16, ptr %43, !tbaa !2
  %45 = icmp sle i16 %42, %44
  br i1 %45, label %b35, label %b36

b34:
  %46 = load i16, ptr @"K%", !tbaa !2
  %47 = getelementptr i8, ptr @$data, i16 4
  %48 = load i16, ptr %47, !tbaa !2
  %49 = icmp sge i16 %46, %48
  br i1 %49, label %b35, label %b36

b35:
  store i16 6, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string6)
          to label %93 unwind label %landing

b36:
  store i16 8, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string7)
          to label %90 unwind label %landing

b37:
  %50 = load i16, ptr @"N%", !tbaa !2
  %51 = load i16, ptr @"K%", !tbaa !2
  %52 = sub i16 %51, 2
  %53 = sext i16 %52 to i32
  %54 = icmp eq i32 %53, 0
  br i1 %54, label %b38, label %b39

b38:
  store i16 7, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %101 unwind label %landing

b39:
  %55 = icmp eq i32 %53, 0
  br i1 %55, label %trap3, label %96

b40:
  %56 = load i16, ptr @"K%", !tbaa !2
  %57 = getelementptr i8, ptr @$data, i16 6
  %58 = load i16, ptr %57, !tbaa !2
  %59 = add nsw i16 %56, %58
  store i16 %59, ptr @"K%", !tbaa !2
  br label %b32

b41:
  store i16 9, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$CEND()
          to label %102 unwind label %landing

b43:
  store i16 11, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 20)
          to label %103 unwind label %landing

b44:
  ret void

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %60 = extractvalue { ptr, i32 } %landed, 1
  %61 = trunc i32 %60 to i16
  %resumed = call addrspace(1) i16 @__main$handler(i16 %61, i16 0)
  switch i16 %resumed, label %64 [
    i16 0, label %again
    i16 1, label %next
  ]

again:
  %62 = load i16, ptr %site
  switch i16 %62, label %104 [
    i16 0, label %b7
    i16 1, label %b8
    i16 2, label %b17
    i16 3, label %b16
    i16 4, label %b26
    i16 5, label %b25
    i16 6, label %b35
    i16 7, label %b37
    i16 8, label %b36
    i16 9, label %b41
    i16 10, label %b2
    i16 11, label %b43
  ]

next:
  %63 = load i16, ptr %site
  switch i16 %63, label %105 [
    i16 0, label %b11
    i16 1, label %b12
    i16 2, label %b20
    i16 3, label %b21
    i16 4, label %b29
    i16 5, label %b30
    i16 6, label %b37
    i16 7, label %b40
    i16 8, label %b41
    i16 9, label %b2
    i16 10, label %b43
    i16 11, label %b44
  ]

64:
  unreachable

65:
  store i16 10, ptr %site
  %66 = invoke cc 1000 addrspace(1) i16 @llrm.qb.B$FERR()
          to label %67 unwind label %landing

67:
  store i16 10, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %66)
          to label %68 unwind label %landing

68:
  br label %b43

69:
  br label %b12

trap:
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %73 unwind label %landing

70:
  %71 = sdiv i32 10, %12
  %72 = trunc i32 %71 to i16
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %72)
          to label %74 unwind label %landing

73:
  unreachable

74:
  br label %b11

75:
  unreachable

76:
  br label %b21

trap1:
  store i16 2, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %80 unwind label %landing

77:
  %78 = sdiv i32 10, %25
  %79 = trunc i32 %78 to i16
  store i16 2, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %79)
          to label %81 unwind label %landing

80:
  unreachable

81:
  br label %b20

82:
  unreachable

83:
  br label %b30

trap2:
  store i16 4, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %87 unwind label %landing

84:
  %85 = sdiv i32 10, %34
  %86 = trunc i32 %85 to i16
  store i16 4, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %86)
          to label %88 unwind label %landing

87:
  unreachable

88:
  br label %b29

89:
  unreachable

90:
  %91 = load i16, ptr @"N%", !tbaa !2
  store i16 8, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %91)
          to label %92 unwind label %landing

92:
  br label %b41

93:
  %94 = load i16, ptr @"K%", !tbaa !2
  store i16 6, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %94)
          to label %95 unwind label %landing

95:
  br label %b37

trap3:
  store i16 7, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %100 unwind label %landing

96:
  %97 = sdiv i32 10, %53
  %98 = trunc i32 %97 to i16
  %99 = add i16 %50, %98
  store i16 %99, ptr @"N%", !tbaa !2
  br label %b40

100:
  unreachable

101:
  unreachable

102:
  unreachable

103:
  unreachable

104:
  unreachable

105:
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
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string8)
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %0)
  br label %b43

b43:
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
