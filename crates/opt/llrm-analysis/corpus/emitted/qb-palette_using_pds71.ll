target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [8 x i8] zeroinitializer
@$qb$statementTable = internal constant [448 x i8] c"\01\00\00\00\01\00\00\00\01\00\00\00\00\00\01\00\00\00\03\00\00\00\02\00\00\00\00\00\01\00\00\00\07\00\00\00\0D\00\00\00\00\00\01\00\00\00\09\00\00\00\1E\00\00\00\00\00\01\00\00\00\08\00\00\00\22\00\00\00\00\00\01\00\00\00\0A\00\00\00%\00\00\00\00\00\01\00\00\00\0B\00\00\00&\00\00\00\00\00\01\00\00\00\0C\00\00\00'\00\00\00\00\00\01\00\00\00\0D\00\00\00(\00\00\00\00\00\01\00\00\00\0E\00\00\00)\00\00\00\00\00\01\00\00\00\0F\00\00\00,\00\00\00\00\00\01\00\00\00\10\00\00\00-\00\00\00\00\00\01\00\00\00\14\00\00\008\00\00\00\00\00\01\00\00\00\16\00\00\00;\00\00\00\00\00\01\00\00\00\17\00\00\00>\00\00\00\00\00\01\00\00\00\18\00\00\00A\00\00\00\00\00\01\00\00\00\19\00\00\00D\00\00\00\00\00\01\00\00\00\1A\00\00\00L\00\00\00\00\00\01\00\00\00\15\00\00\00P\00\00\00\00\00\01\00\00\00\1B\00\00\00Q\00\00\00\00\00\01\00\00\00\1C\00\00\00T\00\00\00\00\00\01\00\00\00\1D\00\00\00W\00\00\00\00\00\01\00\00\00\1E\00\00\00Z\00\00\00\00\00\01\00\00\00\1F\00\00\00b\00\00\00\00\00\01\00\00\00 \00\00\00e\00\00\00\00\00\01\00\00\00!\00\00\00h\00\00\00\00\00\01\00\00\00\22\00\00\00i\00\00\00\00\00\01\00\00\00#\00\00\00l\00\00\00\00\00\01\00\00\00$\00\00\00m\00\00\00\00\00\01\00\00\00\02\00\00\00n\00\00\00\00\00\01\00\00\00&\00\00\00r\00\00\00\00\00\01\00\00\00'\00\00\00s\00\00\00\00\00"
@"P&" = internal global [1024 x i8] zeroinitializer, align 2
@P$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"P&" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"P&", [6 x i8] c"\04\00\00\01\00\00" }>
@"Q&" = internal global [16 x i8] zeroinitializer, align 2
@Q$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"Q&" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"Q&", [6 x i8] c"\04\00\04\00\00\00" }>
@"I%" = internal global [2 x i8] zeroinitializer
@"A%" = internal global [2 x i8] zeroinitializer
@"B%" = internal global [2 x i8] zeroinitializer
@"C%" = internal global [2 x i8] zeroinitializer
@$string5 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string5, i16 4), [4 x i8] c"kept" }>
@$string6 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\06\00", ptr getelementptr (i8, ptr @$string6, i16 4), [6 x i8] c"caught" }>
@$QB$ACTIVE = internal global i16 0
@$QB$ERL = internal global i16 0

define internal cc 1000 void @__main() addrspace(1) noinline "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
frame:
  %site = alloca i16
  call addrspace(1) void @llrm.qb.frame(ptr %site)
  br label %b1

b1:
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$CSCN(i16 1, i16 13, i16 2)
          to label %79 unwind label %landing

b2:
  store i16 9, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string6)
          to label %80 unwind label %landing

b3:
  store i16 0, ptr @"I%", !tbaa !2
  store i16 255, ptr @$data, !tbaa !2
  %0 = getelementptr i8, ptr @$data, i16 2
  store i16 1, ptr %0, !tbaa !2
  br label %b4

b4:
  %1 = getelementptr i8, ptr @$data, i16 2
  %2 = load i16, ptr %1, !tbaa !2
  %3 = icmp sge i16 %2, 0
  br i1 %3, label %b5, label %b6

b5:
  %4 = load i16, ptr @"I%", !tbaa !2
  %5 = load i16, ptr @$data, !tbaa !2
  %6 = icmp sle i16 %4, %5
  br i1 %6, label %b7, label %b8

b6:
  %7 = load i16, ptr @"I%", !tbaa !2
  %8 = load i16, ptr @$data, !tbaa !2
  %9 = icmp sge i16 %7, %8
  br i1 %9, label %b7, label %b8

b7:
  %10 = load i16, ptr @"I%", !tbaa !2
  %11 = load i16, ptr @"I%", !tbaa !2
  %12 = and i16 %11, 63
  %13 = load i16, ptr @"I%", !tbaa !2
  %14 = mul i16 %13, 3
  %15 = and i16 %14, 63
  %16 = sext i16 %15 to i32
  %17 = mul i32 256, %16
  %18 = sext i16 %12 to i32
  %19 = add i32 %18, %17
  %20 = load i16, ptr @"I%", !tbaa !2
  %21 = mul i16 %20, 5
  %22 = and i16 %21, 63
  %23 = sext i16 %22 to i32
  %24 = mul i32 65536, %23
  %25 = add i32 %19, %24
  %26 = sub i16 %10, 0
  %27 = getelementptr inbounds i32, ptr @"P&", i16 %26
  store i32 %25, ptr %27, !tbaa !2
  br label %b9

b8:
  %28 = getelementptr inbounds i32, ptr @"P&", i16 7
  store i32 -1, ptr %28, !tbaa !2
  br label %b10

b9:
  %29 = load i16, ptr @"I%", !tbaa !2
  %30 = getelementptr i8, ptr @$data, i16 2
  %31 = load i16, ptr %30, !tbaa !2
  %32 = add nsw i16 %29, %31
  store i16 %32, ptr @"I%", !tbaa !2
  br label %b4

b10:
  call void @llrm.ia16.out.i8(i16 968, i8 7)
  br label %b11

b11:
  call void @llrm.ia16.out.i8(i16 969, i8 1)
  br label %b12

b12:
  call void @llrm.ia16.out.i8(i16 969, i8 2)
  br label %b13

b13:
  call void @llrm.ia16.out.i8(i16 969, i8 3)
  br label %b14

b14:
  %33 = getelementptr inbounds i32, ptr @"P&", i16 0
  %34 = addrspacecast ptr %33 to ptr addrspace(1)
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PALU(ptr addrspace(1) %34, ptr @P$descriptor)
          to label %84 unwind label %landing

b15:
  store i16 1, ptr @$QB$ACTIVE
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 true)
  br label %b16

b16:
  store i16 0, ptr @"I%", !tbaa !2
  %35 = getelementptr i8, ptr @$data, i16 4
  store i16 255, ptr %35, !tbaa !2
  %36 = getelementptr i8, ptr @$data, i16 6
  store i16 51, ptr %36, !tbaa !2
  br label %b17

b17:
  %37 = getelementptr i8, ptr @$data, i16 6
  %38 = load i16, ptr %37, !tbaa !2
  %39 = icmp sge i16 %38, 0
  br i1 %39, label %b18, label %b19

b18:
  %40 = load i16, ptr @"I%", !tbaa !2
  %41 = getelementptr i8, ptr @$data, i16 4
  %42 = load i16, ptr %41, !tbaa !2
  %43 = icmp sle i16 %40, %42
  br i1 %43, label %b20, label %b21

b19:
  %44 = load i16, ptr @"I%", !tbaa !2
  %45 = getelementptr i8, ptr @$data, i16 4
  %46 = load i16, ptr %45, !tbaa !2
  %47 = icmp sge i16 %44, %46
  br i1 %47, label %b20, label %b21

b20:
  %48 = load i16, ptr @"I%", !tbaa !2
  %49 = trunc i16 %48 to i8
  call void @llrm.ia16.out.i8(i16 967, i8 %49)
  br label %b22

b21:
  call void @llrm.ia16.out.i8(i16 967, i8 7)
  br label %b27

b22:
  %50 = call i8 @llrm.ia16.in.i8(i16 969)
  %51 = zext i8 %50 to i16
  store i16 %51, ptr @"A%", !tbaa !2
  br label %b23

b23:
  %52 = call i8 @llrm.ia16.in.i8(i16 969)
  %53 = zext i8 %52 to i16
  store i16 %53, ptr @"B%", !tbaa !2
  br label %b24

b24:
  %54 = call i8 @llrm.ia16.in.i8(i16 969)
  %55 = zext i8 %54 to i16
  store i16 %55, ptr @"C%", !tbaa !2
  br label %b25

b25:
  %56 = load i16, ptr @"I%", !tbaa !2
  store i16 2, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %56)
          to label %85 unwind label %landing

b26:
  %57 = load i16, ptr @"I%", !tbaa !2
  %58 = getelementptr i8, ptr @$data, i16 6
  %59 = load i16, ptr %58, !tbaa !2
  %60 = add nsw i16 %57, %59
  store i16 %60, ptr @"I%", !tbaa !2
  br label %b17

b27:
  %61 = call i8 @llrm.ia16.in.i8(i16 969)
  %62 = zext i8 %61 to i16
  store i16 %62, ptr @"A%", !tbaa !2
  br label %b28

b28:
  %63 = call i8 @llrm.ia16.in.i8(i16 969)
  %64 = zext i8 %63 to i16
  store i16 %64, ptr @"B%", !tbaa !2
  br label %b29

b29:
  %65 = call i8 @llrm.ia16.in.i8(i16 969)
  %66 = zext i8 %65 to i16
  store i16 %66, ptr @"C%", !tbaa !2
  br label %b30

b30:
  store i16 3, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5)
          to label %92 unwind label %landing

b31:
  %67 = getelementptr inbounds i32, ptr @"P&", i16 200
  %68 = addrspacecast ptr %67 to ptr addrspace(1)
  store i16 4, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PALU(ptr addrspace(1) %68, ptr @P$descriptor)
          to label %99 unwind label %landing

b32:
  %69 = getelementptr inbounds i32, ptr @"Q&", i16 0
  %70 = addrspacecast ptr %69 to ptr addrspace(1)
  store i16 5, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PALU(ptr addrspace(1) %70, ptr @Q$descriptor)
          to label %100 unwind label %landing

b33:
  %71 = getelementptr inbounds i32, ptr @"P&", i16 9
  store i32 64, ptr %71, !tbaa !2
  br label %b34

b34:
  %72 = getelementptr inbounds i32, ptr @"P&", i16 0
  %73 = addrspacecast ptr %72 to ptr addrspace(1)
  store i16 6, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PALU(ptr addrspace(1) %73, ptr @P$descriptor)
          to label %101 unwind label %landing

b35:
  store i16 7, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$CSCN(i16 1, i16 0, i16 2)
          to label %102 unwind label %landing

b36:
  store i16 8, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$CEND()
          to label %103 unwind label %landing

b38:
  store i16 10, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 20)
          to label %104 unwind label %landing

b39:
  ret void

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %74 = extractvalue { ptr, i32 } %landed, 1
  %75 = trunc i32 %74 to i16
  %resumed = call addrspace(1) i16 @__main$handler(i16 %75, i16 0)
  switch i16 %resumed, label %78 [
    i16 0, label %again
    i16 1, label %next
  ]

again:
  %76 = load i16, ptr %site
  switch i16 %76, label %105 [
    i16 0, label %b1
    i16 1, label %b14
    i16 2, label %b25
    i16 3, label %b30
    i16 4, label %b31
    i16 5, label %b32
    i16 6, label %b34
    i16 7, label %b35
    i16 8, label %b36
    i16 9, label %b2
    i16 10, label %b38
  ]

next:
  %77 = load i16, ptr %site
  switch i16 %77, label %106 [
    i16 0, label %b3
    i16 1, label %b15
    i16 2, label %b26
    i16 3, label %b31
    i16 4, label %b32
    i16 5, label %b33
    i16 6, label %b35
    i16 7, label %b36
    i16 8, label %b2
    i16 9, label %b38
    i16 10, label %b39
  ]

78:
  unreachable

79:
  br label %b3

80:
  store i16 9, ptr %site
  %81 = invoke cc 1000 addrspace(1) i16 @llrm.qb.B$FERR()
          to label %82 unwind label %landing

82:
  store i16 9, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %81)
          to label %83 unwind label %landing

83:
  br label %b38

84:
  br label %b15

85:
  %86 = load i16, ptr @"A%", !tbaa !2
  store i16 2, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %86)
          to label %87 unwind label %landing

87:
  %88 = load i16, ptr @"B%", !tbaa !2
  store i16 2, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %88)
          to label %89 unwind label %landing

89:
  %90 = load i16, ptr @"C%", !tbaa !2
  store i16 2, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %90)
          to label %91 unwind label %landing

91:
  br label %b26

92:
  %93 = load i16, ptr @"A%", !tbaa !2
  store i16 3, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %93)
          to label %94 unwind label %landing

94:
  %95 = load i16, ptr @"B%", !tbaa !2
  store i16 3, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %95)
          to label %96 unwind label %landing

96:
  %97 = load i16, ptr @"C%", !tbaa !2
  store i16 3, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %97)
          to label %98 unwind label %landing

98:
  br label %b31

99:
  br label %b32

100:
  br label %b33

101:
  br label %b35

102:
  br label %b36

103:
  unreachable

104:
  unreachable

105:
  unreachable

106:
  unreachable
}

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$CSCN(i16, i16, i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 i16 @llrm.qb.B$FERR() addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare void @llrm.ia16.out.i8(i16, i8) nocallback nofree nounwind willreturn memory(readwrite)

declare cc 1000 void @llrm.qb.B$PALU(ptr addrspace(1), ptr) addrspace(1)

declare i8 @llrm.ia16.in.i8(i16) nocallback nofree nounwind willreturn memory(readwrite)

declare cc 1000 void @llrm.qb.B$PSI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$CEND() addrspace(1)

define internal i16 @__main$handler(i16 %0, i16 %1) addrspace(1) "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
entry:
  store i16 %1, ptr @$QB$ERL
  %2 = load i16, ptr @$QB$ACTIVE
  switch i16 %2, label %3 [
    i16 1, label %b2
  ]

b2:
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string6)
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %0)
  br label %b38

b38:
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
