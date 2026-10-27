target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str2 = internal constant [10 x i8] c"\08\00\03\00\03\00end\00"
@$str3 = internal constant [9 x i8] c"\08\00\02\00\02\00: \00"
@$str4 = internal constant [15 x i8] c"\08\00\08\00\08\00 entries\00"
@$str5 = internal constant [9 x i8] c"\08\00\02\00\02\00  \00"
@$str6 = internal constant [10 x i8] c"\08\00\03\00\03\00 = \00"
@$str7 = internal constant [14 x i8] c"\08\00\07\00\07\00: line \00"
@$str8 = internal constant [18 x i8] c"\08\00\0B\00\0B\00 has no '='\00"
@$str9 = internal constant [23 x i8] c"\08\00\10\00\10\00 is not a number\00"
@$str10 = internal constant [21 x i8] c"\08\00\0E\00\0E\00 repeats a key\00"
@$str11 = internal constant [12 x i8] c"\08\00\05\00\05\00video\00"
@$str12 = internal constant [35 x i8] c"\08\00\1C\00\1C\00width=320\0Aheight=200\0Adepth=8\00"
@$str13 = internal constant [12 x i8] c"\08\00\05\00\05\00early\00"
@$str14 = internal constant [22 x i8] c"\08\00\0F\00\0F\00a=1\0Ab=2\0Aend\0Ac=3\00"
@$str15 = internal constant [15 x i8] c"\08\00\08\00\08\00nonumber\00"
@$str16 = internal constant [18 x i8] c"\08\00\0B\00\0B\00a=1\0Ab=2\0Ac=x\00"
@$str17 = internal constant [15 x i8] c"\08\00\08\00\08\00noequals\00"
@$str18 = internal constant [12 x i8] c"\08\00\05\00\05\00a=1\0Ab\00"
@$str19 = internal constant [13 x i8] c"\08\00\06\00\06\00repeat\00"
@$str20 = internal constant [18 x i8] c"\08\00\0B\00\0B\00a=1\0Ab=2\0Aa=3\00"

define internal cc 1005 void @report(ptr addrspace(5) %0, ptr addrspace(5) nocapture readonly %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
  %3 = addrspacecast ptr addrspace(5) %0 to ptr addrspace(1)
  %4 = alloca [8 x i8]
  %5 = alloca [8 x i8]
  %6 = alloca [8 x i8]
  %7 = alloca [8 x i8]
  %8 = alloca [8 x i8]
  call void @llvm.lifetime.start.p0(i64 8, ptr nonnull %8)
  call void @llvm.lifetime.start.p0(i64 8, ptr nonnull %7)
  call void @llvm.lifetime.start.p0(i64 8, ptr nonnull %6)
  %9 = tail call cc 1005 addrspace(1) ptr @N$VCPY(ptr addrspace(1) %3)
  %10 = load i16, ptr addrspace(5) %1
  %11 = add i16 %10, 1
  %12 = icmp eq i16 %11, 0
  br i1 %12, label %33, label %13

13:
  %14 = getelementptr inbounds nuw i8, ptr addrspace(5) %1, i16 4
  %15 = load ptr addrspace(1), ptr addrspace(5) %14
  %16 = addrspacecast ptr %8 to ptr addrspace(5)
  %17 = addrspacecast ptr %8 to ptr addrspace(1)
  %18 = addrspacecast ptr %7 to ptr addrspace(1)
  %19 = addrspacecast ptr %5 to ptr addrspace(1)
  %20 = addrspacecast ptr %6 to ptr addrspace(1)
  %21 = addrspacecast ptr %4 to ptr addrspace(1)
  br label %22

22:
  %23 = phi i16 [ 0, %13 ], [ %32, %28 ]
  %24 = phi i16 [ 1, %13 ], [ %31, %28 ]
  %25 = phi i16 [ 0, %13 ], [ %29, %28 ]
  %26 = phi ptr [ getelementptr inbounds nuw (i8, ptr @$str1, i16 6), %13 ], [ %30, %28 ]
  %27 = icmp ult i16 %25, %10
  br i1 %27, label %37, label %43

28:
  %29 = phi i16 [ %42, %41 ], [ %137, %128 ]
  %30 = phi ptr [ %133, %128 ], [ %26, %41 ]
  %31 = phi i16 [ %24, %41 ], [ %138, %128 ]
  %32 = phi i16 [ %23, %41 ], [ %137, %128 ]
  %exitcond5 = icmp eq i16 %29, %11
  br i1 %exitcond5, label %33, label %22

33:
  %34 = phi ptr [ getelementptr inbounds nuw (i8, ptr @$str1, i16 6), %2 ], [ %26, %45 ], [ %30, %28 ]
  %35 = ptrtoint ptr %9 to i16
  %36 = ptrtoint ptr %34 to i16
  br label %152

37:
  %38 = getelementptr inbounds i8, ptr addrspace(1) %15, i16 %25
  %39 = load i8, ptr addrspace(1) %38
  %40 = icmp eq i8 %39, 10
  br i1 %40, label %43, label %41

41:
  %42 = add nuw i16 %25, 1
  br label %28

43:
  %44 = icmp ult i16 %25, %23
  br i1 %44, label %53, label %45

45:
  %46 = getelementptr inbounds i8, ptr addrspace(1) %15, i16 %23
  %47 = sub nuw i16 %25, %23
  store i16 %47, ptr %8, !tbaa !2
  %48 = getelementptr i8, ptr %8, i16 2
  store i16 %47, ptr %48, !tbaa !2
  %49 = getelementptr i8, ptr %8, i16 4
  store ptr addrspace(1) %46, ptr %49, !tbaa !2
  store i32 196611, ptr %7
  %50 = getelementptr i8, ptr %7, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6) to ptr addrspace(1)), ptr %50, !tbaa !2
  %51 = call cc 1005 addrspace(1) i8 @N$VCMP(ptr addrspace(1) %17, ptr addrspace(1) %18)
  %52 = icmp eq i8 %51, 0
  br i1 %52, label %33, label %54

53:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

54:
  call void @llvm.lifetime.start.p0(i64 8, ptr nonnull %5)
  %55 = load i16, ptr addrspace(5) %16
  %56 = icmp eq i16 %55, 0
  br i1 %56, label %85, label %57

57:
  %58 = getelementptr i8, ptr addrspace(5) %16, i16 4
  %59 = load ptr addrspace(1), ptr addrspace(5) %58
  br label %66

60:
  %61 = add i16 %67, 1
  %62 = add i16 %55, -1
  %63 = icmp eq i16 %62, %67
  br i1 %63, label %85, label %64

64:
  br label %72

65:
  %exitcond1 = icmp eq i16 %55, %71
  br i1 %exitcond1, label %85, label %66

66:
  %67 = phi i16 [ 0, %57 ], [ %71, %65 ]
  %68 = getelementptr i8, ptr addrspace(1) %59, i16 %67
  %69 = load i8, ptr addrspace(1) %68
  %70 = icmp eq i8 %69, 61
  %71 = add nuw i16 %67, 1
  br i1 %70, label %60, label %65

72:
  %73 = phi i16 [ %61, %64 ], [ %84, %80 ]
  %74 = phi i16 [ %83, %80 ], [ 0, %64 ]
  %75 = getelementptr i8, ptr addrspace(1) %59, i16 %73
  %76 = load i8, ptr addrspace(1) %75
  %77 = zext nneg i8 %76 to i16
  %78 = icmp ult i16 %77, 48
  %79 = icmp ugt i16 %77, 57
  %or.cond2 = or i1 %78, %79
  br i1 %or.cond2, label %85, label %80

80:
  %81 = mul i16 %74, 10
  %82 = add i16 %81, -48
  %83 = add i16 %82, %77
  %84 = add i16 %73, 1
  %exitcond2 = icmp eq i16 %55, %84
  br i1 %exitcond2, label %87, label %72

85:
  %.ph = phi i16 [ 0, %65 ], [ 1, %72 ], [ 0, %54 ], [ 1, %60 ]
  call void @llvm.lifetime.end.p0(i64 8, ptr nonnull %5)
  %86 = icmp eq ptr %26, null
  br i1 %86, label %152, label %116

87:
  store i16 %67, ptr %5, !tbaa !2
  %88 = getelementptr i8, ptr %5, i16 2
  store i16 %67, ptr %88, !tbaa !2
  %89 = getelementptr i8, ptr %5, i16 4
  store ptr addrspace(1) %59, ptr %89, !tbaa !2
  %90 = call cc 1005 addrspace(1) ptr @N$VCPY(ptr addrspace(1) %19)
  %91 = ptrtoint ptr %90 to i16
  call void @llvm.lifetime.end.p0(i64 8, ptr nonnull %5)
  %92 = inttoptr i16 %91 to ptr
  %93 = getelementptr i8, ptr %92, i16 -4
  %94 = load i16, ptr %93
  %95 = addrspacecast ptr %92 to ptr addrspace(1)
  store i16 %94, ptr %6, !tbaa !2
  %96 = getelementptr i8, ptr %6, i16 2
  store i16 %94, ptr %96, !tbaa !2
  %97 = getelementptr i8, ptr %6, i16 4
  store ptr addrspace(1) %95, ptr %97, !tbaa !2
  call void @llvm.lifetime.start.p0(i64 8, ptr nonnull %4)
  %98 = getelementptr i8, ptr %26, i16 -4
  %99 = load i16, ptr %98
  %100 = icmp eq i16 %99, 0
  br i1 %100, label %128, label %101

101:
  br label %105

102:
  %103 = getelementptr i8, ptr %107, i16 4
  %104 = add i16 %106, -1
  %exitcond = icmp eq i16 %104, 0
  br i1 %exitcond, label %128, label %105

105:
  %106 = phi i16 [ %99, %101 ], [ %104, %102 ]
  %107 = phi ptr [ %26, %101 ], [ %103, %102 ]
  %108 = load ptr, ptr %107
  %109 = getelementptr i8, ptr %108, i16 -4
  %110 = load i16, ptr %109
  %111 = addrspacecast ptr %108 to ptr addrspace(1)
  store i16 %110, ptr %4, !tbaa !2
  %112 = getelementptr i8, ptr %4, i16 2
  store i16 %110, ptr %112, !tbaa !2
  %113 = getelementptr i8, ptr %4, i16 4
  store ptr addrspace(1) %111, ptr %113, !tbaa !2
  %114 = call cc 1005 addrspace(1) i8 @N$VCMP(ptr addrspace(1) %21, ptr addrspace(1) %20)
  %115 = icmp eq i8 %114, 0
  br i1 %115, label %139, label %102

116:
  %117 = getelementptr i8, ptr %26, i16 -4
  %118 = load i16, ptr %117
  %119 = icmp eq i16 %118, 0
  br i1 %119, label %152, label %120

120:
  br label %121

121:
  %122 = phi i16 [ 0, %120 ], [ %127, %121 ]
  %123 = phi i16 [ %118, %120 ], [ %126, %121 ]
  %124 = getelementptr i8, ptr %26, i16 %122
  %125 = load ptr, ptr %124
  call cc 1005 addrspace(1) void @N$BDRP(ptr %125)
  %126 = add i16 %123, -1
  %127 = add i16 %122, 4
  %exitcond3 = icmp eq i16 %126, 0
  br i1 %exitcond3, label %152, label %121

128:
  %129 = ptrtoint ptr %90 to i16
  %130 = inttoptr i16 %129 to ptr
  call void @llvm.lifetime.end.p0(i64 8, ptr nonnull %4)
  %131 = getelementptr i8, ptr %26, i16 -4
  %132 = load i16, ptr %131
  %133 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr nonnull %26, i16 1, i16 4)
  %134 = shl i16 %132, 2
  %135 = getelementptr inbounds i8, ptr %133, i16 %134
  store ptr %130, ptr %135
  %136 = getelementptr i8, ptr %135, i16 2
  store i16 %83, ptr %136
  %137 = add nuw i16 %25, 1
  %138 = add i16 %24, 1
  call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  br label %28

139:
  %140 = ptrtoint ptr %90 to i16
  %141 = inttoptr i16 %140 to ptr
  call void @llvm.lifetime.end.p0(i64 8, ptr nonnull %4)
  call cc 1005 addrspace(1) void @N$BDRP(ptr %141)
  %142 = getelementptr i8, ptr %26, i16 -4
  %143 = load i16, ptr %142
  %144 = icmp eq i16 %143, 0
  br i1 %144, label %152, label %145

145:
  br label %146

146:
  %147 = phi ptr [ %26, %145 ], [ %151, %146 ]
  %148 = phi i16 [ %143, %145 ], [ %150, %146 ]
  %149 = load ptr, ptr %147
  call cc 1005 addrspace(1) void @N$BDRP(ptr %149)
  %150 = add i16 %148, -1
  %151 = getelementptr i8, ptr %147, i16 4
  %exitcond4 = icmp eq i16 %150, 0
  br i1 %exitcond4, label %152, label %146

152:
  %153 = phi i16 [ %36, %33 ], [ %24, %85 ], [ %24, %116 ], [ %24, %139 ], [ %24, %121 ], [ %24, %146 ]
  %154 = phi i16 [ %35, %33 ], [ %.ph, %85 ], [ %.ph, %116 ], [ 2, %139 ], [ %.ph, %121 ], [ 2, %146 ]
  %155 = phi i1 [ true, %33 ], [ false, %85 ], [ false, %116 ], [ false, %139 ], [ false, %121 ], [ false, %146 ]
  %sink4 = phi ptr [ null, %33 ], [ null, %85 ], [ %26, %116 ], [ %26, %139 ], [ %26, %121 ], [ %26, %146 ]
  %sink3 = phi ptr [ null, %33 ], [ %9, %85 ], [ %9, %116 ], [ %9, %139 ], [ %9, %121 ], [ %9, %146 ]
  call cc 1005 addrspace(1) void @N$BDRP(ptr %sink4)
  call cc 1005 addrspace(1) void @N$BDRP(ptr %sink3)
  call void @llvm.lifetime.end.p0(i64 8, ptr nonnull %6)
  call void @llvm.lifetime.end.p0(i64 8, ptr nonnull %7)
  call void @llvm.lifetime.end.p0(i64 8, ptr nonnull %8)
  br i1 %155, label %b4, label %b3

b2:
  ret void

b3:
  %extract.trunc = trunc i16 %154 to i8
  call cc 1005 addrspace(1) void @N$PV(ptr addrspace(1) %3)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str7, i16 6))
  call cc 1005 addrspace(1) void @N$PU2(i16 %153)
  %156 = icmp eq i8 %extract.trunc, 0
  br i1 %156, label %b16, label %157

157:
  %158 = icmp eq i8 %extract.trunc, 1
  br i1 %158, label %b19, label %b17

b4:
  %159 = inttoptr i16 %154 to ptr
  %160 = inttoptr i16 %153 to ptr
  call cc 1005 addrspace(1) void @N$PS(ptr %159)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6))
  %161 = getelementptr i8, ptr %160, i16 -4
  %162 = load i16, ptr %161
  call cc 1005 addrspace(1) void @N$PU2(i16 %162)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str4, i16 6))
  call cc 1005 addrspace(1) void @N$PN()
  %163 = load i16, ptr %161
  %164 = icmp eq i16 %163, 0
  br i1 %164, label %b9, label %165

165:
  %166 = inttoptr i16 %153 to ptr
  %167 = getelementptr i8, ptr %166, i16 2
  br label %b6

b6:
  %168 = phi ptr [ %167, %165 ], [ %174, %b6 ]
  %169 = phi i16 [ %163, %165 ], [ %173, %b6 ]
  %170 = getelementptr i8, ptr %168, i16 -2
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str5, i16 6))
  %171 = load ptr, ptr %170
  call cc 1005 addrspace(1) void @N$PS(ptr %171)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str6, i16 6))
  %172 = load i16, ptr %168
  call cc 1005 addrspace(1) void @N$PI2(i16 %172)
  call cc 1005 addrspace(1) void @N$PN()
  %173 = add i16 %169, -1
  %174 = getelementptr i8, ptr %168, i16 4
  %exitcond7 = icmp eq i16 %173, 0
  br i1 %exitcond7, label %b10, label %b6

b9:
  %175 = inttoptr i16 %153 to ptr
  %176 = inttoptr i16 %154 to ptr
  call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %175)
  call cc 1005 addrspace(1) void @N$BDRP(ptr %176)
  br label %b2

b10:
  %177 = inttoptr i16 %153 to ptr
  %178 = getelementptr i8, ptr %177, i16 -4
  %179 = load i16, ptr %178
  %180 = icmp eq i16 %179, 0
  br i1 %180, label %b9, label %181

181:
  %182 = inttoptr i16 %153 to ptr
  br label %b13

b13:
  %183 = phi ptr [ %182, %181 ], [ %187, %b13 ]
  %184 = phi i16 [ %179, %181 ], [ %186, %b13 ]
  %185 = load ptr, ptr %183
  call cc 1005 addrspace(1) void @N$BDRP(ptr %185)
  %186 = add i16 %184, -1
  %187 = getelementptr i8, ptr %183, i16 4
  %exitcond6 = icmp eq i16 %186, 0
  br i1 %exitcond6, label %b9, label %b13

b16:
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str8, i16 6))
  call cc 1005 addrspace(1) void @N$PN()
  br label %b2

b17:
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str10, i16 6))
  call cc 1005 addrspace(1) void @N$PN()
  br label %b2

b19:
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str9, i16 6))
  call cc 1005 addrspace(1) void @N$PN()
  br label %b2
}

define cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca [8 x i8]
  %1 = alloca [8 x i8]
  %2 = alloca [8 x i8]
  %3 = alloca [8 x i8]
  %4 = alloca [8 x i8]
  %5 = alloca [8 x i8]
  %6 = alloca [8 x i8]
  %7 = alloca [8 x i8]
  %8 = alloca [8 x i8]
  %9 = alloca [8 x i8]
  store i32 327685, ptr %9
  %10 = getelementptr inbounds nuw i8, ptr %9, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str11, i16 6) to ptr addrspace(1)), ptr %10, !tbaa !2
  %11 = addrspacecast ptr %9 to ptr addrspace(5)
  store i32 1835036, ptr %8
  %12 = getelementptr inbounds nuw i8, ptr %8, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str12, i16 6) to ptr addrspace(1)), ptr %12, !tbaa !2
  %13 = addrspacecast ptr %8 to ptr addrspace(5)
  call cc 1005 addrspace(1) void @report(ptr addrspace(5) %11, ptr addrspace(5) %13)
  store i32 327685, ptr %7
  %14 = getelementptr inbounds nuw i8, ptr %7, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str13, i16 6) to ptr addrspace(1)), ptr %14, !tbaa !2
  %15 = addrspacecast ptr %7 to ptr addrspace(5)
  store i32 983055, ptr %6
  %16 = getelementptr inbounds nuw i8, ptr %6, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str14, i16 6) to ptr addrspace(1)), ptr %16, !tbaa !2
  %17 = addrspacecast ptr %6 to ptr addrspace(5)
  call cc 1005 addrspace(1) void @report(ptr addrspace(5) %15, ptr addrspace(5) %17)
  store i32 524296, ptr %5
  %18 = getelementptr inbounds nuw i8, ptr %5, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str15, i16 6) to ptr addrspace(1)), ptr %18, !tbaa !2
  %19 = addrspacecast ptr %5 to ptr addrspace(5)
  store i32 720907, ptr %4
  %20 = getelementptr inbounds nuw i8, ptr %4, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str16, i16 6) to ptr addrspace(1)), ptr %20, !tbaa !2
  %21 = addrspacecast ptr %4 to ptr addrspace(5)
  call cc 1005 addrspace(1) void @report(ptr addrspace(5) %19, ptr addrspace(5) %21)
  store i32 524296, ptr %3
  %22 = getelementptr inbounds nuw i8, ptr %3, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str17, i16 6) to ptr addrspace(1)), ptr %22, !tbaa !2
  %23 = addrspacecast ptr %3 to ptr addrspace(5)
  store i32 327685, ptr %2
  %24 = getelementptr inbounds nuw i8, ptr %2, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str18, i16 6) to ptr addrspace(1)), ptr %24, !tbaa !2
  %25 = addrspacecast ptr %2 to ptr addrspace(5)
  call cc 1005 addrspace(1) void @report(ptr addrspace(5) %23, ptr addrspace(5) %25)
  store i32 393222, ptr %1
  %26 = getelementptr inbounds nuw i8, ptr %1, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str19, i16 6) to ptr addrspace(1)), ptr %26, !tbaa !2
  %27 = addrspacecast ptr %1 to ptr addrspace(5)
  store i32 720907, ptr %0
  %28 = getelementptr inbounds nuw i8, ptr %0, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str20, i16 6) to ptr addrspace(1)), ptr %28, !tbaa !2
  %29 = addrspacecast ptr %0 to ptr addrspace(5)
  call cc 1005 addrspace(1) void @report(ptr addrspace(5) %27, ptr addrspace(5) %29)
  ret i16 0
}

declare cc 1005 i8 @N$VCMP(ptr addrspace(1), ptr addrspace(1)) addrspace(1) memory(read)

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare cc 1005 ptr @N$VCPY(ptr addrspace(1)) addrspace(1)

declare void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) noalias nocapture writeonly, ptr noalias nocapture readonly, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

declare cc 1005 ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PU2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PV(ptr addrspace(1)) addrspace(1)

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i8 0, i8 2}
!6 = !{i8 0, i8 2}
!7 = !{i8 0, i8 2}
!8 = !{i8 0, i8 3}
