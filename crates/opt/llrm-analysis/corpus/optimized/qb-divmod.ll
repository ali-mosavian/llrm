target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"A&" = internal global [4 x i8] zeroinitializer
@"B&" = internal global [4 x i8] zeroinitializer
@"R&" = internal global [4 x i8] zeroinitializer
@"CAUGHT%" = internal global [2 x i8] zeroinitializer
@$string3 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string3, i16 4), [6 x i8] c"DIV1=\00" }>
@$string4 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string4, i16 4), [6 x i8] c"MOD1=\00" }>
@$string5 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string5, i16 4), [6 x i8] c"DIV2=\00" }>
@$string6 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string6, i16 4), [6 x i8] c"MOD2=\00" }>
@$string7 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string7, i16 4), [6 x i8] c"DIV3=\00" }>
@$string8 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string8, i16 4), [6 x i8] c"MOD3=\00" }>
@$string9 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string9, i16 4), [6 x i8] c"DIV4=\00" }>
@$string10 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string10, i16 4), [6 x i8] c"MOD4=\00" }>
@$string11 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string11, i16 4), [8 x i8] c"DIVBIG=\00" }>
@$string12 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string12, i16 4), [8 x i8] c"MODBIG=\00" }>
@$string13 = internal constant <{ [2 x i8], ptr, [10 x i8] }> <{ [2 x i8] c"\09\00", ptr getelementptr (i8, ptr @$string13, i16 4), [10 x i8] c"MULSMALL=\00" }>
@$string14 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string14, i16 4), [8 x i8] c"SHDIV1=\00" }>
@$string15 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string15, i16 4), [8 x i8] c"SHMOD1=\00" }>
@$string16 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string16, i16 4), [8 x i8] c"SHDIV2=\00" }>
@$string17 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string17, i16 4), [8 x i8] c"SHMOD2=\00" }>
@$string18 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string18, i16 4), [8 x i8] c"SHDIV3=\00" }>
@$string19 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string19, i16 4), [8 x i8] c"SHMOD3=\00" }>
@$string20 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string20, i16 4), [8 x i8] c"SHDIV4=\00" }>
@$string21 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string21, i16 4), [8 x i8] c"SHMOD4=\00" }>
@$string22 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string22, i16 4), [8 x i8] c"MULOVF=\00" }>
@$string23 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string23, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) noinline "llrm-fp-eval"="x87-extended" noreturn personality ptr addrspace(1) @llrm.qb.personality {
frame:
  %site = alloca i16
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %site)
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 true)
  store i32 7, ptr @"A&", !tbaa !2
  store i32 2, ptr @"B&", !tbaa !2
  store i16 0, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string3)
          to label %29 unwind label %landing

b6:
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %37 unwind label %landing

b8:
  %0 = sub i32 0, %30
  br label %b10

b10:
  %sink = phi i32 [ %0, %b8 ], [ %36, %35 ]
  store i16 0, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %sink)
          to label %b11 unwind label %landing

b11:
  store i16 1, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4)
          to label %38 unwind label %landing

b12:
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %46 unwind label %landing

b16:
  %sink1 = phi i32 [ %45, %43 ], [ 0, %38 ]
  store i16 1, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %sink1)
          to label %b17 unwind label %landing

b17:
  store i32 -7, ptr @"A&", !tbaa !2
  store i16 2, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5)
          to label %47 unwind label %landing

b19:
  store i16 2, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %55 unwind label %landing

b21:
  %1 = sub i32 0, %48
  br label %b23

b23:
  %sink2 = phi i32 [ %1, %b21 ], [ %54, %53 ]
  store i16 2, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %sink2)
          to label %b24 unwind label %landing

b24:
  store i16 3, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string6)
          to label %56 unwind label %landing

b25:
  store i16 3, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %64 unwind label %landing

b29:
  %sink3 = phi i32 [ %63, %61 ], [ 0, %56 ]
  store i16 3, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %sink3)
          to label %b30 unwind label %landing

b30:
  store i32 -2, ptr @"B&", !tbaa !2
  store i16 4, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string7)
          to label %65 unwind label %landing

b32:
  store i16 4, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %73 unwind label %landing

b34:
  %2 = sub i32 0, %66
  br label %b36

b36:
  %sink4 = phi i32 [ %2, %b34 ], [ %72, %71 ]
  store i16 4, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %sink4)
          to label %b37 unwind label %landing

b37:
  store i16 5, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string8)
          to label %74 unwind label %landing

b38:
  store i16 5, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %82 unwind label %landing

b42:
  %sink5 = phi i32 [ %81, %79 ], [ 0, %74 ]
  store i16 5, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %sink5)
          to label %b43 unwind label %landing

b43:
  store i32 7, ptr @"A&", !tbaa !2
  store i16 6, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string9)
          to label %83 unwind label %landing

b45:
  store i16 6, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %91 unwind label %landing

b47:
  %3 = sub i32 0, %84
  br label %b49

b49:
  %sink6 = phi i32 [ %3, %b47 ], [ %90, %89 ]
  store i16 6, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %sink6)
          to label %b50 unwind label %landing

b50:
  store i16 7, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string10)
          to label %92 unwind label %landing

b51:
  store i16 7, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %100 unwind label %landing

b55:
  %sink7 = phi i32 [ %99, %97 ], [ 0, %92 ]
  store i16 7, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %sink7)
          to label %b56 unwind label %landing

b56:
  store i32 305419896, ptr @"A&", !tbaa !2
  store i32 252645135, ptr @"B&", !tbaa !2
  store i16 8, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string11)
          to label %101 unwind label %landing

b59:
  store i16 8, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %109 unwind label %landing

b61:
  %4 = sub i32 0, %102
  br label %b63

b63:
  %sink8 = phi i32 [ %4, %b61 ], [ %108, %107 ]
  store i16 8, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %sink8)
          to label %b64 unwind label %landing

b64:
  store i16 9, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string12)
          to label %110 unwind label %landing

b65:
  store i16 9, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %118 unwind label %landing

b69:
  %sink9 = phi i32 [ %117, %115 ], [ 0, %110 ]
  store i16 9, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %sink9)
          to label %b70 unwind label %landing

b70:
  store i16 10, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string13)
          to label %119 unwind label %landing

b71:
  store i32 -7, ptr @"R&", !tbaa !2
  store i16 11, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string14)
          to label %122 unwind label %landing

b73:
  store i16 12, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string15)
          to label %125 unwind label %landing

b74:
  store i16 13, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string16)
          to label %128 unwind label %landing

b75:
  store i16 14, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string17)
          to label %131 unwind label %landing

b76:
  store i32 -1000000, ptr @"R&", !tbaa !2
  store i16 15, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string18)
          to label %134 unwind label %landing

b78:
  store i16 16, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string19)
          to label %137 unwind label %landing

b79:
  store i32 1000000, ptr @"R&", !tbaa !2
  store i16 17, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string20)
          to label %140 unwind label %landing

b81:
  store i16 18, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string21)
          to label %143 unwind label %landing

b82:
  store i32 305419896, ptr @"A&", !tbaa !2
  store i32 252645135, ptr @"B&", !tbaa !2
  store i16 0, ptr @"CAUGHT%", !tbaa !2
  store i32 992614664, ptr @"R&", !tbaa !2
  store i16 19, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string22)
          to label %146 unwind label %landing

b87:
  store i16 20, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string23)
          to label %b88 unwind label %landing

b88:
  call cc 1005 addrspace(1) void @llrm.qb.B$CEND()
  unreachable

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %5 = extractvalue { ptr, i32 } %landed, 1
  %6 = trunc i32 %5 to i16
  %resumed = call fastcc addrspace(1) i16 @__main$handler(i16 %6)
  %7 = load i16, ptr %site
  switch table i16 %7, label %8 [
    i16 0, label %8
    i16 1, label %9
    i16 2, label %10
    i16 3, label %11
    i16 4, label %12
    i16 5, label %13
    i16 6, label %14
    i16 7, label %15
    i16 8, label %16
    i16 9, label %17
    i16 10, label %18
    i16 11, label %19
    i16 12, label %20
    i16 13, label %21
    i16 14, label %22
    i16 15, label %23
    i16 16, label %24
    i16 17, label %25
    i16 18, label %26
    i16 19, label %27
    i16 20, label %28
  ]

8:
  br label %b11

9:
  br label %b17

10:
  br label %b24

11:
  br label %b30

12:
  br label %b37

13:
  br label %b43

14:
  br label %b50

15:
  br label %b56

16:
  br label %b64

17:
  br label %b70

18:
  br label %b71

19:
  br label %b73

20:
  br label %b74

21:
  br label %b75

22:
  br label %b76

23:
  br label %b78

24:
  br label %b79

25:
  br label %b81

26:
  br label %b82

27:
  br label %b87

28:
  br label %b88

29:
  %30 = load i32, ptr @"A&", !tbaa !2
  %31 = load i32, ptr @"B&", !tbaa !2
  %32 = icmp eq i32 %31, -1
  br i1 %32, label %b8, label %33

33:
  %34 = icmp eq i32 %31, 0
  br i1 %34, label %b6, label %35

35:
  %36 = sdiv i32 %30, %31
  br label %b10

37:
  unreachable

38:
  %39 = load i32, ptr @"B&", !tbaa !2
  %40 = icmp eq i32 %39, -1
  br i1 %40, label %b16, label %41

41:
  %42 = icmp eq i32 %39, 0
  br i1 %42, label %b12, label %43

43:
  %44 = load i32, ptr @"A&", !tbaa !2
  %45 = srem i32 %44, %39
  br label %b16

46:
  unreachable

47:
  %48 = load i32, ptr @"A&", !tbaa !2
  %49 = load i32, ptr @"B&", !tbaa !2
  %50 = icmp eq i32 %49, -1
  br i1 %50, label %b21, label %51

51:
  %52 = icmp eq i32 %49, 0
  br i1 %52, label %b19, label %53

53:
  %54 = sdiv i32 %48, %49
  br label %b23

55:
  unreachable

56:
  %57 = load i32, ptr @"B&", !tbaa !2
  %58 = icmp eq i32 %57, -1
  br i1 %58, label %b29, label %59

59:
  %60 = icmp eq i32 %57, 0
  br i1 %60, label %b25, label %61

61:
  %62 = load i32, ptr @"A&", !tbaa !2
  %63 = srem i32 %62, %57
  br label %b29

64:
  unreachable

65:
  %66 = load i32, ptr @"A&", !tbaa !2
  %67 = load i32, ptr @"B&", !tbaa !2
  %68 = icmp eq i32 %67, -1
  br i1 %68, label %b34, label %69

69:
  %70 = icmp eq i32 %67, 0
  br i1 %70, label %b32, label %71

71:
  %72 = sdiv i32 %66, %67
  br label %b36

73:
  unreachable

74:
  %75 = load i32, ptr @"B&", !tbaa !2
  %76 = icmp eq i32 %75, -1
  br i1 %76, label %b42, label %77

77:
  %78 = icmp eq i32 %75, 0
  br i1 %78, label %b38, label %79

79:
  %80 = load i32, ptr @"A&", !tbaa !2
  %81 = srem i32 %80, %75
  br label %b42

82:
  unreachable

83:
  %84 = load i32, ptr @"A&", !tbaa !2
  %85 = load i32, ptr @"B&", !tbaa !2
  %86 = icmp eq i32 %85, -1
  br i1 %86, label %b47, label %87

87:
  %88 = icmp eq i32 %85, 0
  br i1 %88, label %b45, label %89

89:
  %90 = sdiv i32 %84, %85
  br label %b49

91:
  unreachable

92:
  %93 = load i32, ptr @"B&", !tbaa !2
  %94 = icmp eq i32 %93, -1
  br i1 %94, label %b55, label %95

95:
  %96 = icmp eq i32 %93, 0
  br i1 %96, label %b51, label %97

97:
  %98 = load i32, ptr @"A&", !tbaa !2
  %99 = srem i32 %98, %93
  br label %b55

100:
  unreachable

101:
  %102 = load i32, ptr @"B&", !tbaa !2
  %103 = load i32, ptr @"A&", !tbaa !2
  %104 = icmp eq i32 %103, -1
  br i1 %104, label %b61, label %105

105:
  %106 = icmp eq i32 %103, 0
  br i1 %106, label %b59, label %107

107:
  %108 = sdiv i32 %102, %103
  br label %b63

109:
  unreachable

110:
  %111 = load i32, ptr @"A&", !tbaa !2
  %112 = icmp eq i32 %111, -1
  br i1 %112, label %b69, label %113

113:
  %114 = icmp eq i32 %111, 0
  br i1 %114, label %b65, label %115

115:
  %116 = load i32, ptr @"B&", !tbaa !2
  %117 = srem i32 %116, %111
  br label %b69

118:
  unreachable

119:
  %120 = load i32, ptr @"A&", !tbaa !2
  %121 = mul i32 %120, 3
  store i16 10, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %121)
          to label %b71 unwind label %landing

122:
  %123 = load i32, ptr @"R&", !tbaa !2
  %124 = sdiv i32 %123, 2
  store i16 11, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %124)
          to label %b73 unwind label %landing

125:
  %126 = load i32, ptr @"R&", !tbaa !2
  %127 = srem i32 %126, 2
  store i16 12, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %127)
          to label %b74 unwind label %landing

128:
  %129 = load i32, ptr @"R&", !tbaa !2
  %130 = sdiv i32 %129, 512
  store i16 13, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %130)
          to label %b75 unwind label %landing

131:
  %132 = load i32, ptr @"R&", !tbaa !2
  %133 = srem i32 %132, 512
  store i16 14, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %133)
          to label %b76 unwind label %landing

134:
  %135 = load i32, ptr @"R&", !tbaa !2
  %136 = sdiv i32 %135, 512
  store i16 15, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %136)
          to label %b78 unwind label %landing

137:
  %138 = load i32, ptr @"R&", !tbaa !2
  %139 = srem i32 %138, 512
  store i16 16, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %139)
          to label %b79 unwind label %landing

140:
  %141 = load i32, ptr @"R&", !tbaa !2
  %142 = sdiv i32 %141, 512
  store i16 17, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %142)
          to label %b81 unwind label %landing

143:
  %144 = load i32, ptr @"R&", !tbaa !2
  %145 = srem i32 %144, 512
  store i16 18, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %145)
          to label %b82 unwind label %landing

146:
  %147 = load i16, ptr @"CAUGHT%", !tbaa !2
  store i16 19, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %147)
          to label %b87 unwind label %landing
}

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1) noreturn

declare cc 1000 i16 @llrm.qb.B$FERR() addrspace(1) nounwind

declare cc 1005 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1005 void @llrm.qb.B$PEI4(i32) addrspace(1)

declare cc 1005 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc 1005 void @llrm.qb.B$CEND() addrspace(1) nounwind noreturn

define internal fastcc i16 @__main$handler(i16 %0) addrspace(1) "llrm-fp-eval"="x87-extended" nounwind personality ptr addrspace(1) @llrm.qb.personality {
entry:
  store i16 %0, ptr @"CAUGHT%", !tbaa !2
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
