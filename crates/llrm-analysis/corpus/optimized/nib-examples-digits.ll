target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [12 x i8] c"\08\00\05\00\05\00empty\00"
@$str2 = internal constant [18 x i8] c"\08\00\0B\00\0B\00not a digit\00"
@$str3 = internal constant [14 x i8] c"\08\00\07\00\07\00too big\00"
@$str4 = internal constant [11 x i8] c"\08\00\04\00\04\001234\00"
@$str5 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str6 = internal constant [11 x i8] c"\08\00\04\00\04\0012x4\00"
@$str7 = internal constant [12 x i8] c"\08\00\05\00\05\0099999\00"
@$str8 = internal constant [18 x i8] c"\08\00\0B\00\0B\00first even \00"
@$str9 = internal constant [20 x i8] c"\08\00\0D\00\0D\00no even value\00"

define internal void @digit(ptr addrspace(1) %0, i8 %1) addrspace(1) memory(argmem: write) willreturn {
b1:
  %2 = icmp ult i8 %1, 48
  %3 = sext i1 %2 to i8
  br i1 %2, label %b3, label %b2

b2:
  %4 = icmp ugt i8 %1, 57
  %5 = sext i1 %4 to i8
  br label %b3

b3:
  %6 = phi i8 [ %3, %b1 ], [ %5, %b2 ]
  %7 = icmp ne i8 %6, 0
  br i1 %7, label %b4, label %b5

b4:
  store i8 1, ptr addrspace(1) %0
  %8 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 1, ptr addrspace(1) %8
  %9 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 0, ptr addrspace(1) %9
  ret void

b5:
  %10 = zext i8 %1 to i16
  %11 = add i16 %10, -48
  %12 = trunc i16 %11 to i8
  store i8 0, ptr addrspace(1) %0
  %13 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 %12, ptr addrspace(1) %13
  ret void
}

define internal void @parse(ptr addrspace(1) %0, ptr %1) addrspace(1) {
b1:
  %2 = alloca [6 x i8]
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 6, i1 false)
  %3 = getelementptr i8, ptr %1, i16 -4
  %4 = load i16, ptr %3
  %5 = icmp eq i16 %4, 0
  br i1 %5, label %b2, label %b3

b2:
  store i8 1, ptr addrspace(1) %0
  %6 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 0, ptr addrspace(1) %6
  call addrspace(1) void @N$BDRP(ptr %1)
  ret void

b3:
  %7 = addrspacecast ptr %2 to ptr addrspace(1)
  %8 = getelementptr inbounds i8, ptr %2, i16 2
  %9 = getelementptr i8, ptr addrspace(1) %7, i16 2
  %10 = getelementptr i8, ptr addrspace(1) %7, i16 4
  br label %b5

b5:
  %11 = phi i32 [ 0, %b3 ], [ %42, %b13 ]
  %12 = phi i16 [ 0, %b3 ], [ %45, %b13 ]
  %13 = icmp ult i16 %12, %4
  br i1 %13, label %b6, label %b8

b6:
  %14 = getelementptr i8, ptr %1, i16 %12
  %15 = mul i32 %11, 10
  %16 = load i8, ptr %14
  %17 = icmp ult i8 %16, 48
  %18 = sext i1 %17 to i8
  br i1 %17, label %22, label %19

19:
  %20 = icmp ugt i8 %16, 57
  %21 = sext i1 %20 to i8
  br label %22

22:
  %23 = phi i8 [ %18, %b6 ], [ %21, %19 ]
  %24 = icmp ne i8 %23, 0
  br i1 %24, label %25, label %26

25:
  store i8 1, ptr addrspace(1) %7
  store i8 1, ptr addrspace(1) %9
  store i16 0, ptr addrspace(1) %10
  br label %30

26:
  %27 = zext i8 %16 to i16
  %28 = add i16 %27, -48
  %29 = trunc i16 %28 to i8
  store i8 0, ptr addrspace(1) %7
  store i8 %29, ptr addrspace(1) %9
  br label %30

30:
  %31 = load i8, ptr %2, !tbaa !2
  %32 = icmp eq i8 %31, 1
  br i1 %32, label %b9, label %b10

b8:
  %33 = trunc i32 %11 to i16
  store i8 0, ptr addrspace(1) %0
  %34 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %33, ptr addrspace(1) %34
  call addrspace(1) void @N$BDRP(ptr %1)
  ret void

b9:
  %35 = load i16, ptr %8, !tbaa !2
  %36 = getelementptr inbounds i8, ptr %2, i16 4
  %37 = load i16, ptr %36, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %38 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %35, ptr addrspace(1) %38
  %39 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %37, ptr addrspace(1) %39
  call addrspace(1) void @N$BDRP(ptr %1)
  ret void

b10:
  %40 = load i8, ptr %8, !tbaa !2
  %41 = zext i8 %40 to i32
  %42 = add i32 %15, %41
  %43 = icmp ugt i32 %42, 65535
  br i1 %43, label %b11, label %b13

b11:
  store i8 1, ptr addrspace(1) %0
  %44 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 2, ptr addrspace(1) %44
  call addrspace(1) void @N$BDRP(ptr %1)
  ret void

b13:
  %45 = add i16 %12, 1
  br label %b5
}

define internal i32 @first_even(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) willreturn {
b1:
  %1 = alloca [4 x i8]
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 4, i1 false)
  %2 = load i16, ptr addrspace(1) %0
  %3 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %4 = load ptr addrspace(1), ptr addrspace(1) %3
  br label %b2

b2:
  %5 = phi i16 [ 0, %b1 ], [ %18, %b8 ]
  %6 = icmp ult i16 %5, %2
  br i1 %6, label %b3, label %b5

b3:
  %7 = shl i16 %5, 1
  %8 = getelementptr i8, ptr addrspace(1) %4, i16 %7
  %9 = load i16, ptr addrspace(1) %8
  %10 = srem i16 %9, 2
  %11 = icmp eq i16 %10, 0
  br i1 %11, label %b6, label %b8

b5:
  store i8 1, ptr %1, !tbaa !2
  %12 = addrspacecast ptr %1 to ptr addrspace(1)
  %13 = load i32, ptr addrspace(1) %12, !tbaa !2
  ret i32 %13

b6:
  %14 = load i16, ptr addrspace(1) %8
  store i8 0, ptr %1, !tbaa !2
  %15 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 %14, ptr %15, !tbaa !2
  %16 = addrspacecast ptr %1 to ptr addrspace(1)
  %17 = load i32, ptr addrspace(1) %16, !tbaa !2
  ret i32 %17

b8:
  %18 = add i16 %5, 1
  br label %b2
}

define internal void @report(ptr %0) addrspace(1) {
b1:
  %1 = alloca [6 x i8]
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 6, i1 false)
  %2 = addrspacecast ptr %1 to ptr addrspace(1)
  call addrspace(1) void @parse(ptr addrspace(1) %2, ptr %0)
  %3 = load i8, ptr %1, !tbaa !2
  %4 = icmp eq i8 %3, 0
  br i1 %4, label %b4, label %b3

b2:
  call addrspace(1) void @N$BDRP(ptr null)
  ret void

b3:
  %5 = icmp eq i8 %3, 1
  br i1 %5, label %b6, label %b5

b4:
  %6 = getelementptr inbounds i8, ptr %1, i16 2
  %7 = load i16, ptr %6, !tbaa !2
  call addrspace(1) void @N$PU2(i16 %7)
  call addrspace(1) void @N$PN()
  br label %b2

b5:
  %8 = load i8, ptr %1, !tbaa !2
  %9 = icmp eq i8 %8, 1
  br i1 %9, label %b9, label %b8

b6:
  %10 = getelementptr inbounds i8, ptr %1, i16 2
  %11 = load i8, ptr %10, !tbaa !2
  %12 = icmp eq i8 %11, 0
  br i1 %12, label %b7, label %b5

b7:
  %13 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %13)
  call addrspace(1) void @N$PN()
  br label %b2

b8:
  %14 = getelementptr i8, ptr @$str3, i16 6
  call addrspace(1) void @N$PS(ptr %14)
  call addrspace(1) void @N$PN()
  br label %b2

b9:
  %15 = getelementptr inbounds i8, ptr %1, i16 2
  %16 = load i8, ptr %15, !tbaa !2
  %17 = icmp eq i8 %16, 1
  br i1 %17, label %b10, label %b8

b10:
  %18 = getelementptr i8, ptr @$str2, i16 6
  call addrspace(1) void @N$PS(ptr %18)
  call addrspace(1) void @N$PN()
  br label %b2
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca [4 x i8]
  %1 = alloca [6 x i8]
  %2 = alloca [6 x i8]
  %3 = alloca [6 x i8]
  %4 = alloca [6 x i8]
  %5 = alloca [4 x i8]
  %6 = alloca [8 x i8]
  call void @llvm.memset.p0.i16(ptr %5, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr %6, i8 0, i16 8, i1 false)
  %7 = getelementptr i8, ptr @$str4, i16 6
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 6, i1 false)
  %8 = addrspacecast ptr %4 to ptr addrspace(1)
  call addrspace(1) void @parse(ptr addrspace(1) %8, ptr %7)
  %9 = load i8, ptr %4
  %10 = icmp eq i8 %9, 0
  br i1 %10, label %18, label %16

11:
  call addrspace(1) void @N$BDRP(ptr null)
  %12 = getelementptr i8, ptr @$str5, i16 6
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 6, i1 false)
  %13 = addrspacecast ptr %3 to ptr addrspace(1)
  call addrspace(1) void @parse(ptr addrspace(1) %13, ptr %12)
  %14 = load i8, ptr %3
  %15 = icmp eq i8 %14, 0
  br i1 %15, label %45, label %43

16:
  %17 = icmp eq i8 %9, 1
  br i1 %17, label %24, label %21

18:
  %19 = getelementptr inbounds i8, ptr %4, i16 2
  %20 = load i16, ptr %19
  call addrspace(1) void @N$PU2(i16 %20)
  call addrspace(1) void @N$PN()
  br label %11

21:
  %22 = load i8, ptr %4
  %23 = icmp eq i8 %22, 1
  br i1 %23, label %32, label %30

24:
  %25 = getelementptr inbounds i8, ptr %4, i16 2
  %26 = load i8, ptr %25
  %27 = icmp eq i8 %26, 0
  br i1 %27, label %28, label %21

28:
  %29 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %29)
  call addrspace(1) void @N$PN()
  br label %11

30:
  %31 = getelementptr i8, ptr @$str3, i16 6
  call addrspace(1) void @N$PS(ptr %31)
  call addrspace(1) void @N$PN()
  br label %11

32:
  %33 = getelementptr inbounds i8, ptr %4, i16 2
  %34 = load i8, ptr %33
  %35 = icmp eq i8 %34, 1
  br i1 %35, label %36, label %30

36:
  %37 = getelementptr i8, ptr @$str2, i16 6
  call addrspace(1) void @N$PS(ptr %37)
  call addrspace(1) void @N$PN()
  br label %11

38:
  call addrspace(1) void @N$BDRP(ptr null)
  %39 = getelementptr i8, ptr @$str6, i16 6
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 6, i1 false)
  %40 = addrspacecast ptr %2 to ptr addrspace(1)
  call addrspace(1) void @parse(ptr addrspace(1) %40, ptr %39)
  %41 = load i8, ptr %2
  %42 = icmp eq i8 %41, 0
  br i1 %42, label %72, label %70

43:
  %44 = icmp eq i8 %14, 1
  br i1 %44, label %51, label %48

45:
  %46 = getelementptr inbounds i8, ptr %3, i16 2
  %47 = load i16, ptr %46
  call addrspace(1) void @N$PU2(i16 %47)
  call addrspace(1) void @N$PN()
  br label %38

48:
  %49 = load i8, ptr %3
  %50 = icmp eq i8 %49, 1
  br i1 %50, label %59, label %57

51:
  %52 = getelementptr inbounds i8, ptr %3, i16 2
  %53 = load i8, ptr %52
  %54 = icmp eq i8 %53, 0
  br i1 %54, label %55, label %48

55:
  %56 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %56)
  call addrspace(1) void @N$PN()
  br label %38

57:
  %58 = getelementptr i8, ptr @$str3, i16 6
  call addrspace(1) void @N$PS(ptr %58)
  call addrspace(1) void @N$PN()
  br label %38

59:
  %60 = getelementptr inbounds i8, ptr %3, i16 2
  %61 = load i8, ptr %60
  %62 = icmp eq i8 %61, 1
  br i1 %62, label %63, label %57

63:
  %64 = getelementptr i8, ptr @$str2, i16 6
  call addrspace(1) void @N$PS(ptr %64)
  call addrspace(1) void @N$PN()
  br label %38

65:
  call addrspace(1) void @N$BDRP(ptr null)
  %66 = getelementptr i8, ptr @$str7, i16 6
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 6, i1 false)
  %67 = addrspacecast ptr %1 to ptr addrspace(1)
  call addrspace(1) void @parse(ptr addrspace(1) %67, ptr %66)
  %68 = load i8, ptr %1
  %69 = icmp eq i8 %68, 0
  br i1 %69, label %100, label %98

70:
  %71 = icmp eq i8 %41, 1
  br i1 %71, label %78, label %75

72:
  %73 = getelementptr inbounds i8, ptr %2, i16 2
  %74 = load i16, ptr %73
  call addrspace(1) void @N$PU2(i16 %74)
  call addrspace(1) void @N$PN()
  br label %65

75:
  %76 = load i8, ptr %2
  %77 = icmp eq i8 %76, 1
  br i1 %77, label %86, label %84

78:
  %79 = getelementptr inbounds i8, ptr %2, i16 2
  %80 = load i8, ptr %79
  %81 = icmp eq i8 %80, 0
  br i1 %81, label %82, label %75

82:
  %83 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %83)
  call addrspace(1) void @N$PN()
  br label %65

84:
  %85 = getelementptr i8, ptr @$str3, i16 6
  call addrspace(1) void @N$PS(ptr %85)
  call addrspace(1) void @N$PN()
  br label %65

86:
  %87 = getelementptr inbounds i8, ptr %2, i16 2
  %88 = load i8, ptr %87
  %89 = icmp eq i8 %88, 1
  br i1 %89, label %90, label %84

90:
  %91 = getelementptr i8, ptr @$str2, i16 6
  call addrspace(1) void @N$PS(ptr %91)
  call addrspace(1) void @N$PN()
  br label %65

92:
  call addrspace(1) void @N$BDRP(ptr null)
  %93 = getelementptr inbounds i16, ptr %6, i16 0
  store i16 3, ptr %93, !tbaa !2
  %94 = getelementptr inbounds i16, ptr %6, i16 1
  store i16 7, ptr %94, !tbaa !2
  %95 = getelementptr inbounds i16, ptr %6, i16 2
  store i16 8, ptr %95, !tbaa !2
  %96 = getelementptr inbounds i16, ptr %6, i16 3
  store i16 9, ptr %96, !tbaa !2
  %97 = addrspacecast ptr %6 to ptr addrspace(1)
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 4, i1 false)
  br label %120

98:
  %99 = icmp eq i8 %68, 1
  br i1 %99, label %106, label %103

100:
  %101 = getelementptr inbounds i8, ptr %1, i16 2
  %102 = load i16, ptr %101
  call addrspace(1) void @N$PU2(i16 %102)
  call addrspace(1) void @N$PN()
  br label %92

103:
  %104 = load i8, ptr %1
  %105 = icmp eq i8 %104, 1
  br i1 %105, label %114, label %112

106:
  %107 = getelementptr inbounds i8, ptr %1, i16 2
  %108 = load i8, ptr %107
  %109 = icmp eq i8 %108, 0
  br i1 %109, label %110, label %103

110:
  %111 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %111)
  call addrspace(1) void @N$PN()
  br label %92

112:
  %113 = getelementptr i8, ptr @$str3, i16 6
  call addrspace(1) void @N$PS(ptr %113)
  call addrspace(1) void @N$PN()
  br label %92

114:
  %115 = getelementptr inbounds i8, ptr %1, i16 2
  %116 = load i8, ptr %115
  %117 = icmp eq i8 %116, 1
  br i1 %117, label %118, label %112

118:
  %119 = getelementptr i8, ptr @$str2, i16 6
  call addrspace(1) void @N$PS(ptr %119)
  call addrspace(1) void @N$PN()
  br label %92

120:
  %121 = phi i16 [ 0, %92 ], [ %138, %137 ]
  %122 = icmp ult i16 %121, 4
  br i1 %122, label %123, label %129

123:
  %124 = shl i16 %121, 1
  %125 = getelementptr i8, ptr addrspace(1) %97, i16 %124
  %126 = load i16, ptr addrspace(1) %125
  %127 = srem i16 %126, 2
  %128 = icmp eq i16 %127, 0
  br i1 %128, label %132, label %137

129:
  store i8 1, ptr %0
  %130 = addrspacecast ptr %0 to ptr addrspace(1)
  %131 = load i32, ptr addrspace(1) %130
  br label %139

132:
  %133 = load i16, ptr addrspace(1) %125
  store i8 0, ptr %0
  %134 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 %133, ptr %134
  %135 = addrspacecast ptr %0 to ptr addrspace(1)
  %136 = load i32, ptr addrspace(1) %135
  br label %139

137:
  %138 = add i16 %121, 1
  br label %120

139:
  %140 = phi i32 [ %131, %129 ], [ %136, %132 ]
  %141 = addrspacecast ptr %5 to ptr addrspace(1)
  store i32 %140, ptr addrspace(1) %141, !tbaa !2
  %142 = load i8, ptr %5, !tbaa !2
  %143 = icmp eq i8 %142, 0
  br i1 %143, label %b4, label %b3

b2:
  ret i16 0

b3:
  %144 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %144)
  call addrspace(1) void @N$PN()
  br label %b2

b4:
  %145 = getelementptr inbounds i8, ptr %5, i16 2
  %146 = load i16, ptr %145, !tbaa !2
  %147 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @N$PS(ptr %147)
  call addrspace(1) void @N$PI2(i16 %146)
  call addrspace(1) void @N$PN()
  br label %b2
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$BDRP(ptr) addrspace(1)

declare void @N$PU2(i16) addrspace(1)

declare void @N$PN() addrspace(1)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PI2(i16) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
