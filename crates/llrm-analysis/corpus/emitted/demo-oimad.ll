target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$data = internal global [36 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"BENCHHI&" = internal global [4 x i8] zeroinitializer
@"BENCHLO&" = internal global [4 x i8] zeroinitializer
@"BENCHFRAME&" = internal global [4 x i8] zeroinitializer
@"DSPBASE%" = internal global [2 x i8] zeroinitializer
@BUFFER$ = internal global [32767 x i8] zeroinitializer
@BUFFER$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @BUFFER$ to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr getelementptr (i8, ptr @BUFFER$, i16 -32767), [6 x i8] c"\FF\7F\01\00\01\00" }>
@"BUFOFS&" = internal global [4 x i8] zeroinitializer
@"SND%" = internal global [2 x i8] zeroinitializer
@"TEXT%" = internal global [9062 x i8] zeroinitializer
@TEXT$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"TEXT%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"TEXT%", [6 x i8] c"\02\00\B3\11\00\00" }>
@"MASK%" = internal global [9062 x i8] zeroinitializer
@MASK$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"MASK%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"MASK%", [6 x i8] c"\02\00\B3\11\00\00" }>
@b$seg = global [2 x i8] zeroinitializer
@$string7 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\08\00", ptr getelementptr (i8, ptr @$string7, i16 4), [8 x i8] c"text.bsv" }>
@$string8 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\08\00", ptr getelementptr (i8, ptr @$string8, i16 4), [8 x i8] c"mask.bsv" }>
@$string9 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\08\00", ptr getelementptr (i8, ptr @$string9, i16 4), [8 x i8] c"dem1.raw" }>
@"OLDTIMER#" = internal global [8 x i8] zeroinitializer
@"OLDTIMER2#" = internal global [8 x i8] zeroinitializer
@"ADDX%" = internal global [2 x i8] zeroinitializer
@"X2%" = internal global [2 x i8] zeroinitializer
@"X%" = internal global [2 x i8] zeroinitializer
@"Y%" = internal global [2 x i8] zeroinitializer
@$string10 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string10, i16 4), [20 x i8] c"press Escape to exit" }>
@$float11 = internal constant [4 x i8] zeroinitializer
@$string12 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\08\00", ptr getelementptr (i8, ptr @$string12, i16 4), [8 x i8] c"-NOSOUND" }>
@$string13 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string13, i16 4), [8 x i8] c"pal.dat\00" }>
@$string14 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string14, i16 4), [2 x i8] c"V\00" }>
@$string15 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string15, i16 4), [4 x i8] c".BIN" }>
@$string16 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string16, i16 4), [2 x i8] c"P\00" }>
@$string17 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string17, i16 4), [4 x i8] c".BIN" }>
@$string18 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string18, i16 4), [2 x i8] c"T\00" }>
@$string19 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string19, i16 4), [4 x i8] c".BIN" }>

define internal cc1000 void @__main() addrspace(1) {
b1:
  %0 = sub i16 1, 1
  %1 = getelementptr inbounds [32767 x i8], ptr @BUFFER$, i16 %0
  %2 = addrspacecast ptr %1 to ptr addrspace(1)
  %3 = addrspacecast ptr addrspace(1) %2 to ptr addrspace(2)
  %4 = ptrtoint ptr addrspace(2) %3 to i16
  %5 = sext i16 %4 to i32
  %6 = mul i32 %5, 16
  %7 = sub i16 1, 1
  %8 = getelementptr inbounds [32767 x i8], ptr @BUFFER$, i16 %7
  %9 = addrspacecast ptr %8 to ptr addrspace(1)
  %10 = ptrtoint ptr addrspace(1) %9 to i16
  %11 = sext i16 %10 to i32
  %12 = add i32 %6, %11
  store i32 %12, ptr @"BUFOFS&", !tbaa !2
  %13 = sub i16 0, 0
  %14 = getelementptr inbounds i16, ptr @"TEXT%", i16 %13
  %15 = addrspacecast ptr %14 to ptr addrspace(1)
  %16 = addrspacecast ptr addrspace(1) %15 to ptr addrspace(2)
  %17 = ptrtoint ptr addrspace(2) %16 to i16
  store i16 %17, ptr @b$seg, !tbaa !2
  %18 = sub i16 0, 0
  %19 = getelementptr inbounds i16, ptr @"TEXT%", i16 %18
  %20 = addrspacecast ptr %19 to ptr addrspace(1)
  %21 = ptrtoint ptr addrspace(1) %20 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$BLOD(ptr @$string7, i16 %21, i16 1)
  %22 = sub i16 0, 0
  %23 = getelementptr inbounds i16, ptr @"MASK%", i16 %22
  %24 = addrspacecast ptr %23 to ptr addrspace(1)
  %25 = addrspacecast ptr addrspace(1) %24 to ptr addrspace(2)
  %26 = ptrtoint ptr addrspace(2) %25 to i16
  store i16 %26, ptr @b$seg, !tbaa !2
  %27 = sub i16 0, 0
  %28 = getelementptr inbounds i16, ptr @"MASK%", i16 %27
  %29 = addrspacecast ptr %28 to ptr addrspace(1)
  %30 = ptrtoint ptr addrspace(1) %29 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$BLOD(ptr @$string8, i16 %30, i16 1)
  call cc1000 addrspace(1) void @llrm.qb.B$DSG0()
  store i16 544, ptr @"DSPBASE%", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$OPEN(ptr @$string9, i16 1, i16 -1, i16 32)
  call cc1000 addrspace(1) void @INIT()
  store i16 0, ptr @$data, !tbaa !2
  call cc1000 addrspace(1) void @BENCHMARK(ptr @$data)
  %31 = call cc1000 addrspace(1) ptr @llrm.qb.B$TIMR()
  %32 = load float, ptr %31
  %33 = fpext float %32 to double
  store double %33, ptr @"OLDTIMER#", !tbaa !2
  %34 = call cc1000 addrspace(1) ptr @llrm.qb.B$TIMR()
  %35 = load float, ptr %34
  %36 = fpext float %35 to double
  store double %36, ptr @"OLDTIMER2#", !tbaa !2
  store i16 1, ptr @"ADDX%", !tbaa !2
  br label %b3

b2:
  %37 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  %38 = icmp sge i32 %37, 3000
  %39 = sext i1 %38 to i16
  %40 = icmp ne i16 %39, 0
  br i1 %40, label %b4, label %b3

b3:
  %41 = load i16, ptr @"X2%", !tbaa !2
  %42 = load i16, ptr @"ADDX%", !tbaa !2
  %43 = add i16 %41, %42
  store i16 %43, ptr @"X2%", !tbaa !2
  %44 = load i16, ptr @"X2%", !tbaa !2
  %45 = icmp sgt i16 %44, 150
  %46 = sext i1 %45 to i16
  %47 = icmp ne i16 %46, 0
  br i1 %47, label %b5, label %b6

b4:
  %48 = getelementptr i8, ptr @$data, i16 32
  store i16 1, ptr %48, !tbaa !2
  %49 = getelementptr i8, ptr @$data, i16 32
  call cc1000 addrspace(1) void @BENCHMARK(ptr %49)
  call cc1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable

b5:
  %50 = sub i16 0, 1
  store i16 %50, ptr @"ADDX%", !tbaa !2
  br label %b7

b6:
  br label %b7

b7:
  %51 = load i16, ptr @"X2%", !tbaa !2
  %52 = icmp slt i16 %51, 20
  %53 = sext i1 %52 to i16
  %54 = icmp ne i16 %53, 0
  br i1 %54, label %b8, label %b9

b8:
  store i16 1, ptr @"ADDX%", !tbaa !2
  br label %b10

b9:
  br label %b10

b10:
  %55 = load i16, ptr @"X2%", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$N1I2(i16 %55, i16 100)
  %56 = getelementptr i8, ptr @$data, i16 2
  store i16 16, ptr %56, !tbaa !2
  %57 = getelementptr i8, ptr @$data, i16 2
  %58 = load i16, ptr %57, !tbaa !2
  %59 = sitofp i16 %58 to float
  %60 = getelementptr i8, ptr @$data, i16 4
  store float %59, ptr %60, !tbaa !2
  %61 = call cc1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %62 = load float, ptr %61
  %63 = getelementptr i8, ptr @$data, i16 8
  store i16 64, ptr %63, !tbaa !2
  %64 = getelementptr i8, ptr @$data, i16 8
  %65 = load i16, ptr %64, !tbaa !2
  %66 = sitofp i16 %65 to float
  %67 = fmul float %62, %66
  %68 = call float @llvm.rint.f32(float %67)
  %69 = fcmp olt float %67, %68
  %70 = sext i1 %69 to i16
  %71 = getelementptr i8, ptr @$data, i16 10
  store i16 %70, ptr %71, !tbaa !2
  %72 = getelementptr i8, ptr @$data, i16 10
  %73 = load i16, ptr %72, !tbaa !2
  %74 = sitofp i16 %73 to float
  %75 = fadd float %68, %74
  %76 = getelementptr i8, ptr @$data, i16 12
  store i16 200, ptr %76, !tbaa !2
  %77 = getelementptr i8, ptr @$data, i16 12
  %78 = load i16, ptr %77, !tbaa !2
  %79 = sitofp i16 %78 to float
  %80 = fadd float %75, %79
  %81 = call i16 @llvm.lrint.i16.f32(float %80)
  %82 = getelementptr i8, ptr @$data, i16 4
  %83 = load float, ptr %82, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$CIRC(float %83, i16 %81)
  %84 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %84, ptr @"BENCHFRAME&", !tbaa !2
  %85 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  %86 = srem i32 %85, 8
  %87 = icmp eq i32 %86, 0
  %88 = sext i1 %87 to i16
  %89 = icmp ne i16 %88, 0
  br i1 %89, label %b11, label %b12

b11:
  %90 = load i16, ptr @"X%", !tbaa !2
  %91 = add i16 15, %90
  %92 = load i16, ptr @"Y%", !tbaa !2
  %93 = add i16 15, %92
  call cc1000 addrspace(1) void @llrm.qb.B$N1I2(i16 %91, i16 %93)
  %94 = sub i16 0, 0
  %95 = getelementptr inbounds i16, ptr @"MASK%", i16 %94
  %96 = addrspacecast ptr %95 to ptr addrspace(1)
  call cc1000 addrspace(1) void @llrm.qb.B$GPUT(ptr addrspace(1) %96, ptr @MASK$descriptor, i16 1)
  %97 = call cc1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %98 = load float, ptr %97
  %99 = getelementptr i8, ptr @$data, i16 14
  store i16 10, ptr %99, !tbaa !2
  %100 = getelementptr i8, ptr @$data, i16 14
  %101 = load i16, ptr %100, !tbaa !2
  %102 = sitofp i16 %101 to float
  %103 = fmul float %98, %102
  %104 = call float @llvm.rint.f32(float %103)
  %105 = fcmp olt float %103, %104
  %106 = sext i1 %105 to i16
  %107 = getelementptr i8, ptr @$data, i16 16
  store i16 %106, ptr %107, !tbaa !2
  %108 = getelementptr i8, ptr @$data, i16 16
  %109 = load i16, ptr %108, !tbaa !2
  %110 = sitofp i16 %109 to float
  %111 = fadd float %104, %110
  %112 = call i16 @llvm.lrint.i16.f32(float %111)
  store i16 %112, ptr @"X%", !tbaa !2
  %113 = call cc1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %114 = load float, ptr %113
  %115 = getelementptr i8, ptr @$data, i16 18
  store i16 10, ptr %115, !tbaa !2
  %116 = getelementptr i8, ptr @$data, i16 18
  %117 = load i16, ptr %116, !tbaa !2
  %118 = sitofp i16 %117 to float
  %119 = fmul float %114, %118
  %120 = call float @llvm.rint.f32(float %119)
  %121 = fcmp olt float %119, %120
  %122 = sext i1 %121 to i16
  %123 = getelementptr i8, ptr @$data, i16 20
  store i16 %122, ptr %123, !tbaa !2
  %124 = getelementptr i8, ptr @$data, i16 20
  %125 = load i16, ptr %124, !tbaa !2
  %126 = sitofp i16 %125 to float
  %127 = fadd float %120, %126
  %128 = call i16 @llvm.lrint.i16.f32(float %127)
  store i16 %128, ptr @"Y%", !tbaa !2
  %129 = load i16, ptr @"X%", !tbaa !2
  %130 = add i16 15, %129
  %131 = load i16, ptr @"Y%", !tbaa !2
  %132 = add i16 15, %131
  call cc1000 addrspace(1) void @llrm.qb.B$N1I2(i16 %130, i16 %132)
  %133 = sub i16 0, 0
  %134 = getelementptr inbounds i16, ptr @"MASK%", i16 %133
  %135 = addrspacecast ptr %134 to ptr addrspace(1)
  call cc1000 addrspace(1) void @llrm.qb.B$GPUT(ptr addrspace(1) %135, ptr @MASK$descriptor, i16 1)
  %136 = load i16, ptr @"X%", !tbaa !2
  %137 = add i16 15, %136
  %138 = load i16, ptr @"Y%", !tbaa !2
  %139 = add i16 15, %138
  call cc1000 addrspace(1) void @llrm.qb.B$N1I2(i16 %137, i16 %139)
  %140 = sub i16 0, 0
  %141 = getelementptr inbounds i16, ptr @"TEXT%", i16 %140
  %142 = addrspacecast ptr %141 to ptr addrspace(1)
  call cc1000 addrspace(1) void @llrm.qb.B$GPUT(ptr addrspace(1) %142, ptr @TEXT$descriptor, i16 0)
  %143 = load i16, ptr @"X%", !tbaa !2
  %144 = add i16 80, %143
  %145 = load i16, ptr @"Y%", !tbaa !2
  %146 = add i16 150, %145
  call cc1000 addrspace(1) void @llrm.qb.B$N1I2(i16 %144, i16 %146)
  %147 = sub i16 1500, 0
  %148 = getelementptr inbounds i16, ptr @"MASK%", i16 %147
  %149 = addrspacecast ptr %148 to ptr addrspace(1)
  call cc1000 addrspace(1) void @llrm.qb.B$GPUT(ptr addrspace(1) %149, ptr @MASK$descriptor, i16 1)
  %150 = call cc1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %151 = load float, ptr %150
  %152 = getelementptr i8, ptr @$data, i16 22
  store i16 10, ptr %152, !tbaa !2
  %153 = getelementptr i8, ptr @$data, i16 22
  %154 = load i16, ptr %153, !tbaa !2
  %155 = sitofp i16 %154 to float
  %156 = fmul float %151, %155
  %157 = call float @llvm.rint.f32(float %156)
  %158 = fcmp olt float %156, %157
  %159 = sext i1 %158 to i16
  %160 = getelementptr i8, ptr @$data, i16 24
  store i16 %159, ptr %160, !tbaa !2
  %161 = getelementptr i8, ptr @$data, i16 24
  %162 = load i16, ptr %161, !tbaa !2
  %163 = sitofp i16 %162 to float
  %164 = fadd float %157, %163
  %165 = call i16 @llvm.lrint.i16.f32(float %164)
  store i16 %165, ptr @"X%", !tbaa !2
  %166 = call cc1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %167 = load float, ptr %166
  %168 = getelementptr i8, ptr @$data, i16 26
  store i16 10, ptr %168, !tbaa !2
  %169 = getelementptr i8, ptr @$data, i16 26
  %170 = load i16, ptr %169, !tbaa !2
  %171 = sitofp i16 %170 to float
  %172 = fmul float %167, %171
  %173 = call float @llvm.rint.f32(float %172)
  %174 = fcmp olt float %172, %173
  %175 = sext i1 %174 to i16
  %176 = getelementptr i8, ptr @$data, i16 28
  store i16 %175, ptr %176, !tbaa !2
  %177 = getelementptr i8, ptr @$data, i16 28
  %178 = load i16, ptr %177, !tbaa !2
  %179 = sitofp i16 %178 to float
  %180 = fadd float %173, %179
  %181 = call i16 @llvm.lrint.i16.f32(float %180)
  store i16 %181, ptr @"Y%", !tbaa !2
  %182 = load i16, ptr @"X%", !tbaa !2
  %183 = add i16 80, %182
  %184 = load i16, ptr @"Y%", !tbaa !2
  %185 = add i16 150, %184
  call cc1000 addrspace(1) void @llrm.qb.B$N1I2(i16 %183, i16 %185)
  %186 = sub i16 1500, 0
  %187 = getelementptr inbounds i16, ptr @"MASK%", i16 %186
  %188 = addrspacecast ptr %187 to ptr addrspace(1)
  call cc1000 addrspace(1) void @llrm.qb.B$GPUT(ptr addrspace(1) %188, ptr @MASK$descriptor, i16 1)
  %189 = load i16, ptr @"X%", !tbaa !2
  %190 = add i16 80, %189
  %191 = load i16, ptr @"Y%", !tbaa !2
  %192 = add i16 150, %191
  call cc1000 addrspace(1) void @llrm.qb.B$N1I2(i16 %190, i16 %192)
  %193 = sub i16 1500, 0
  %194 = getelementptr inbounds i16, ptr @"TEXT%", i16 %193
  %195 = addrspacecast ptr %194 to ptr addrspace(1)
  call cc1000 addrspace(1) void @llrm.qb.B$GPUT(ptr addrspace(1) %195, ptr @TEXT$descriptor, i16 0)
  %196 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %196, ptr @"BENCHFRAME&", !tbaa !2
  %197 = call cc1000 addrspace(1) ptr @llrm.qb.B$TIMR()
  %198 = load float, ptr %197
  %199 = fpext float %198 to double
  store double %199, ptr @"OLDTIMER#", !tbaa !2
  br label %b13

b12:
  br label %b13

b13:
  %200 = load i16, ptr @"SND%", !tbaa !2
  %201 = icmp ne i16 %200, 0
  br i1 %201, label %b14, label %b15

b14:
  %202 = getelementptr i8, ptr @$data, i16 30
  store i16 32767, ptr %202, !tbaa !2
  %203 = getelementptr i8, ptr @$data, i16 30
  %204 = call cc1000 addrspace(1) i16 @"DMADONE%"(ptr %203)
  %205 = icmp ne i16 %204, 0
  br i1 %205, label %b17, label %b18

b15:
  br label %b16

b16:
  %206 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  %207 = icmp sgt i32 %206, 1500
  %208 = sext i1 %207 to i16
  %209 = icmp ne i16 %208, 0
  br i1 %209, label %b20, label %b21

b17:
  call cc1000 addrspace(1) void @PLAYMUSIC()
  br label %b19

b18:
  br label %b19

b19:
  br label %b16

b20:
  call cc1000 addrspace(1) void @llrm.qb.B$COLR(i16 1, i16 150, i16 2)
  call cc1000 addrspace(1) void @llrm.qb.B$LOCT(i16 1, i16 8, i16 1, i16 7, i16 4)
  call cc1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string10)
  br label %b22

b21:
  br label %b22

b22:
  %210 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  %211 = add i32 %210, 1
  store i32 %211, ptr @"BENCHFRAME&", !tbaa !2
  br label %b2
}

define cc1000 i16 @DMADONE(ptr %0) addrspace(1) {
b1:
  %1 = alloca i16
  %2 = alloca i32
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  store i16 0, ptr %1
  store i32 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %4
  store i16 0, ptr %5
  %6 = call i8 @llrm.ia16.in.i8(i16 3)
  %7 = zext i8 %6 to i16
  store i16 %7, ptr %4, !tbaa !2
  %8 = call i8 @llrm.ia16.in.i8(i16 3)
  %9 = zext i8 %8 to i16
  store i16 %9, ptr %3, !tbaa !2
  %10 = load i16, ptr %3, !tbaa !2
  %11 = sext i16 %10 to i32
  %12 = mul i32 %11, 256
  %13 = load i16, ptr %4, !tbaa !2
  %14 = sext i16 %13 to i32
  %15 = add i32 %12, %14
  store i32 %15, ptr %2, !tbaa !2
  %16 = load i32, ptr %2, !tbaa !2
  %17 = load i16, ptr %0
  %18 = sub i16 %17, 1
  %19 = sext i16 %18 to i32
  %20 = icmp sgt i32 %16, %19
  %21 = sext i1 %20 to i16
  %22 = icmp ne i16 %21, 0
  br i1 %22, label %b2, label %b3

b2:
  %23 = load i16, ptr @"DSPBASE%", !tbaa !2
  %24 = add i16 %23, 14
  %25 = call i8 @llrm.ia16.in.i8(i16 %24)
  %26 = zext i8 %25 to i16
  store i16 %26, ptr %1, !tbaa !2
  store i16 1, ptr %5, !tbaa !2
  br label %b4

b3:
  br label %b4

b4:
  br label %b5

b5:
  %27 = load i16, ptr %5, !tbaa !2
  ret i16 %27
}

define cc1000 void @INIT() addrspace(1) {
b1:
  %0 = alloca i16
  %1 = alloca i32
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i32
  %5 = alloca i16
  %6 = alloca i32
  %7 = alloca i16
  %8 = alloca float
  %9 = alloca i16
  %10 = alloca float
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca float
  %14 = alloca i16
  %15 = alloca i16
  %16 = alloca i16
  %17 = alloca i16
  %18 = alloca float
  %19 = alloca i16
  %20 = alloca float
  %21 = alloca i16
  %22 = alloca float
  %23 = alloca i16
  %24 = alloca i16
  %25 = alloca float
  store i16 0, ptr %0
  store i32 0, ptr %1
  store i16 0, ptr %2
  store i16 0, ptr %3
  store i32 0, ptr %4
  store i16 0, ptr %5
  store i32 0, ptr %6
  store i16 0, ptr %7
  store float 0.000000e+00, ptr %8
  store i16 0, ptr %9
  store float 0.000000e+00, ptr %10
  store i16 0, ptr %11
  store i16 0, ptr %12
  store float 0.000000e+00, ptr %13
  store i16 0, ptr %14
  store i16 0, ptr %15
  store i16 0, ptr %16
  store i16 0, ptr %17
  store float 0.000000e+00, ptr %18
  store i16 0, ptr %19
  store float 0.000000e+00, ptr %20
  store i16 0, ptr %21
  store float 0.000000e+00, ptr %22
  store i16 0, ptr %23
  store i16 0, ptr %24
  store float 0.000000e+00, ptr %25
  %26 = load i16, ptr @"DSPBASE%", !tbaa !2
  %27 = add i16 %26, 6
  call void @llrm.ia16.out.i8(i16 %27, i8 1)
  store i16 1, ptr %24, !tbaa !2
  %28 = load i16, ptr %24, !tbaa !2
  %29 = sitofp i16 %28 to float
  store float %29, ptr %25, !tbaa !2
  store i16 4, ptr %23, !tbaa !2
  %30 = load i16, ptr %23, !tbaa !2
  %31 = sitofp i16 %30 to float
  store float %31, ptr %22, !tbaa !2
  store i16 1, ptr %21, !tbaa !2
  %32 = load i16, ptr %21, !tbaa !2
  %33 = sitofp i16 %32 to float
  store float %33, ptr %20, !tbaa !2
  br label %b2

b2:
  %34 = load float, ptr %20, !tbaa !2
  %35 = load float, ptr @$float11, !tbaa !2
  %36 = fcmp oge float %34, %35
  %37 = sext i1 %36 to i16
  %38 = icmp ne i16 %37, 0
  br i1 %38, label %b3, label %b4

b3:
  %39 = load float, ptr %25, !tbaa !2
  %40 = load float, ptr %22, !tbaa !2
  %41 = fcmp ole float %39, %40
  %42 = sext i1 %41 to i16
  %43 = icmp ne i16 %42, 0
  br i1 %43, label %b5, label %b6

b4:
  %44 = load float, ptr %25, !tbaa !2
  %45 = load float, ptr %22, !tbaa !2
  %46 = fcmp oge float %44, %45
  %47 = sext i1 %46 to i16
  %48 = icmp ne i16 %47, 0
  br i1 %48, label %b5, label %b6

b5:
  %49 = load i16, ptr @"DSPBASE%", !tbaa !2
  %50 = add i16 %49, 6
  %51 = call i8 @llrm.ia16.in.i8(i16 %50)
  %52 = zext i8 %51 to i16
  store i16 %52, ptr %19, !tbaa !2
  %53 = load float, ptr %25, !tbaa !2
  %54 = load float, ptr %20, !tbaa !2
  %55 = fadd float %53, %54
  store float %55, ptr %25, !tbaa !2
  br label %b2

b6:
  %56 = load i16, ptr @"DSPBASE%", !tbaa !2
  %57 = add i16 %56, 6
  call void @llrm.ia16.out.i8(i16 %57, i8 0)
  %58 = sub i16 0, 1
  store i16 %58, ptr %17, !tbaa !2
  %59 = load i16, ptr %17, !tbaa !2
  %60 = sitofp i16 %59 to float
  store float %60, ptr %18, !tbaa !2
  %61 = call cc1000 addrspace(1) ptr @llrm.qb.B$FCMD()
  %62 = call cc1000 addrspace(1) i16 @llrm.qb.B$INS2(ptr %61, ptr @$string12)
  %63 = icmp ne i16 %62, 0
  br i1 %63, label %b7, label %b8

b7:
  store i16 0, ptr %16, !tbaa !2
  %64 = load i16, ptr %16, !tbaa !2
  %65 = sitofp i16 %64 to float
  store float %65, ptr %18, !tbaa !2
  br label %b9

b8:
  br label %b9

b9:
  %66 = load i16, ptr @"DSPBASE%", !tbaa !2
  %67 = add i16 %66, 14
  %68 = call i8 @llrm.ia16.in.i8(i16 %67)
  %69 = zext i8 %68 to i16
  %70 = and i16 %69, 128
  %71 = icmp eq i16 %70, 128
  %72 = sext i1 %71 to i16
  %73 = load i16, ptr @"DSPBASE%", !tbaa !2
  %74 = add i16 %73, 10
  %75 = call i8 @llrm.ia16.in.i8(i16 %74)
  %76 = zext i8 %75 to i16
  %77 = icmp eq i16 %76, 170
  %78 = sext i1 %77 to i16
  %79 = and i16 %72, %78
  %80 = icmp ne i16 %79, 0
  br i1 %80, label %b10, label %b11

b10:
  store i16 0, ptr %15, !tbaa !2
  %81 = load i16, ptr %15, !tbaa !2
  %82 = sitofp i16 %81 to float
  store float %82, ptr %18, !tbaa !2
  br label %b12

b11:
  br label %b12

b12:
  store i16 209, ptr %14, !tbaa !2
  call cc1000 addrspace(1) void @SENDDSP(ptr %14)
  %83 = load float, ptr %18, !tbaa !2
  %84 = load float, ptr @$float11, !tbaa !2
  %85 = fcmp une float %83, %84
  %86 = sext i1 %85 to i16
  %87 = icmp ne i16 %86, 0
  br i1 %87, label %b13, label %b14

b13:
  call cc1000 addrspace(1) void @PLAYMUSIC()
  br label %b15

b14:
  br label %b15

b15:
  call cc1000 addrspace(1) void @llrm.qb.B$CSCN(i16 1, i16 13, i16 2)
  store i16 -24576, ptr @b$seg, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$BLOD(ptr @$string13, i16 -1536, i16 1)
  store i16 0, ptr %12, !tbaa !2
  %88 = load i16, ptr %12, !tbaa !2
  %89 = sitofp i16 %88 to float
  store float %89, ptr %13, !tbaa !2
  store i16 255, ptr %11, !tbaa !2
  %90 = load i16, ptr %11, !tbaa !2
  %91 = sitofp i16 %90 to float
  store float %91, ptr %10, !tbaa !2
  store i16 1, ptr %9, !tbaa !2
  %92 = load i16, ptr %9, !tbaa !2
  %93 = sitofp i16 %92 to float
  store float %93, ptr %8, !tbaa !2
  br label %b16

b16:
  %94 = load float, ptr %8, !tbaa !2
  %95 = load float, ptr @$float11, !tbaa !2
  %96 = fcmp oge float %94, %95
  %97 = sext i1 %96 to i16
  %98 = icmp ne i16 %97, 0
  br i1 %98, label %b17, label %b18

b17:
  %99 = load float, ptr %13, !tbaa !2
  %100 = load float, ptr %10, !tbaa !2
  %101 = fcmp ole float %99, %100
  %102 = sext i1 %101 to i16
  %103 = icmp ne i16 %102, 0
  br i1 %103, label %b19, label %b20

b18:
  %104 = load float, ptr %13, !tbaa !2
  %105 = load float, ptr %10, !tbaa !2
  %106 = fcmp oge float %104, %105
  %107 = sext i1 %106 to i16
  %108 = icmp ne i16 %107, 0
  br i1 %108, label %b19, label %b20

b19:
  %109 = load float, ptr %13, !tbaa !2
  %110 = call i16 @llvm.lrint.i16.f32(float %109)
  %111 = trunc i16 %110 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %111)
  %112 = load float, ptr %13, !tbaa !2
  store i16 3, ptr %7, !tbaa !2
  %113 = load i16, ptr %7, !tbaa !2
  %114 = sitofp i16 %113 to float
  %115 = fmul float %112, %114
  store i32 64000, ptr %6, !tbaa !2
  %116 = load i32, ptr %6, !tbaa !2
  %117 = sitofp i32 %116 to float
  %118 = fadd float %117, %115
  %119 = call i32 @llvm.lrint.i32.f32(float %118)
  %120 = trunc i32 %119 to i16
  %121 = load i16, ptr @b$seg, !tbaa !2
  %122 = inttoptr i16 %121 to ptr addrspace(2)
  %123 = addrspacecast ptr addrspace(2) %122 to ptr addrspace(1)
  %124 = getelementptr i8, ptr addrspace(1) %123, i16 %120
  %125 = load i8, ptr addrspace(1) %124
  %126 = zext i8 %125 to i16
  %127 = trunc i16 %126 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %127)
  %128 = load float, ptr %13, !tbaa !2
  store i16 3, ptr %5, !tbaa !2
  %129 = load i16, ptr %5, !tbaa !2
  %130 = sitofp i16 %129 to float
  %131 = fmul float %128, %130
  store i32 64000, ptr %4, !tbaa !2
  %132 = load i32, ptr %4, !tbaa !2
  %133 = sitofp i32 %132 to float
  %134 = fadd float %133, %131
  store i16 1, ptr %3, !tbaa !2
  %135 = load i16, ptr %3, !tbaa !2
  %136 = sitofp i16 %135 to float
  %137 = fadd float %134, %136
  %138 = call i32 @llvm.lrint.i32.f32(float %137)
  %139 = trunc i32 %138 to i16
  %140 = load i16, ptr @b$seg, !tbaa !2
  %141 = inttoptr i16 %140 to ptr addrspace(2)
  %142 = addrspacecast ptr addrspace(2) %141 to ptr addrspace(1)
  %143 = getelementptr i8, ptr addrspace(1) %142, i16 %139
  %144 = load i8, ptr addrspace(1) %143
  %145 = zext i8 %144 to i16
  %146 = trunc i16 %145 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %146)
  %147 = load float, ptr %13, !tbaa !2
  store i16 3, ptr %2, !tbaa !2
  %148 = load i16, ptr %2, !tbaa !2
  %149 = sitofp i16 %148 to float
  %150 = fmul float %147, %149
  store i32 64000, ptr %1, !tbaa !2
  %151 = load i32, ptr %1, !tbaa !2
  %152 = sitofp i32 %151 to float
  %153 = fadd float %152, %150
  store i16 2, ptr %0, !tbaa !2
  %154 = load i16, ptr %0, !tbaa !2
  %155 = sitofp i16 %154 to float
  %156 = fadd float %153, %155
  %157 = call i32 @llvm.lrint.i32.f32(float %156)
  %158 = trunc i32 %157 to i16
  %159 = load i16, ptr @b$seg, !tbaa !2
  %160 = inttoptr i16 %159 to ptr addrspace(2)
  %161 = addrspacecast ptr addrspace(2) %160 to ptr addrspace(1)
  %162 = getelementptr i8, ptr addrspace(1) %161, i16 %158
  %163 = load i8, ptr addrspace(1) %162
  %164 = zext i8 %163 to i16
  %165 = trunc i16 %164 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %165)
  %166 = load float, ptr %13, !tbaa !2
  %167 = load float, ptr %8, !tbaa !2
  %168 = fadd float %166, %167
  store float %168, ptr %13, !tbaa !2
  br label %b16

b20:
  call cc1000 addrspace(1) void @llrm.qb.B$DSG0()
  ret void
}

define cc1000 void @PLAYMUSIC() addrspace(1) {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  store i16 0, ptr %0
  store i16 0, ptr %1
  store i16 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %4
  store i16 0, ptr %5
  store i16 0, ptr %6
  %7 = sub i16 1, 1
  %8 = getelementptr inbounds [32767 x i8], ptr @BUFFER$, i16 %7
  %9 = addrspacecast ptr %8 to ptr addrspace(1)
  call cc1000 addrspace(1) void @llrm.qb.B$GET3(i16 1, ptr addrspace(1) %9, i16 32767)
  call void @llrm.ia16.out.i8(i16 10, i8 5)
  call void @llrm.ia16.out.i8(i16 12, i8 2)
  call void @llrm.ia16.out.i8(i16 11, i8 73)
  %10 = load i16, ptr %6, !tbaa !2
  %11 = trunc i16 %10 to i8
  call void @llrm.ia16.out.i8(i16 2, i8 %11)
  %12 = load i16, ptr %5, !tbaa !2
  %13 = trunc i16 %12 to i8
  call void @llrm.ia16.out.i8(i16 2, i8 %13)
  %14 = load i32, ptr @"BUFOFS&", !tbaa !2
  %15 = and i32 %14, 255
  %16 = trunc i32 %15 to i16
  store i16 %16, ptr %6, !tbaa !2
  %17 = load i32, ptr @"BUFOFS&", !tbaa !2
  %18 = load i16, ptr %6, !tbaa !2
  %19 = sext i16 %18 to i32
  %20 = sub i32 %17, %19
  %21 = sdiv i32 %20, 255
  %22 = trunc i32 %21 to i16
  store i16 %22, ptr %5, !tbaa !2
  %23 = load i16, ptr %6, !tbaa !2
  %24 = trunc i16 %23 to i8
  call void @llrm.ia16.out.i8(i16 3, i8 %24)
  %25 = load i16, ptr %5, !tbaa !2
  %26 = trunc i16 %25 to i8
  call void @llrm.ia16.out.i8(i16 3, i8 %26)
  %27 = load i32, ptr @"BUFOFS&", !tbaa !2
  %28 = sdiv i32 %27, 65536
  %29 = trunc i32 %28 to i8
  call void @llrm.ia16.out.i8(i16 131, i8 %29)
  call void @llrm.ia16.out.i8(i16 10, i8 1)
  store i16 64, ptr %4, !tbaa !2
  call cc1000 addrspace(1) void @SENDDSP(ptr %4)
  %30 = sub i16 0, 250
  store i16 %30, ptr %3, !tbaa !2
  call cc1000 addrspace(1) void @SENDDSP(ptr %3)
  store i16 20, ptr %2, !tbaa !2
  call cc1000 addrspace(1) void @SENDDSP(ptr %2)
  %31 = load i16, ptr %6, !tbaa !2
  store i16 %31, ptr %1, !tbaa !2
  call cc1000 addrspace(1) void @SENDDSP(ptr %1)
  %32 = load i16, ptr %5, !tbaa !2
  store i16 %32, ptr %0, !tbaa !2
  call cc1000 addrspace(1) void @SENDDSP(ptr %0)
  ret void
}

define cc1000 void @SENDDSP(ptr %0) addrspace(1) {
b1:
  br label %b2

b2:
  %1 = load i16, ptr @"DSPBASE%", !tbaa !2
  %2 = add i16 %1, 12
  %3 = call i8 @llrm.ia16.in.i8(i16 %2)
  %4 = zext i8 %3 to i16
  %5 = and i16 %4, 128
  %6 = icmp ne i16 %5, 0
  br i1 %6, label %b3, label %b4

b3:
  br label %b2

b4:
  %7 = load i16, ptr @"DSPBASE%", !tbaa !2
  %8 = add i16 %7, 12
  %9 = load i16, ptr %0
  %10 = trunc i16 %9 to i8
  call void @llrm.ia16.out.i8(i16 %8, i8 %10)
  ret void
}

define cc1000 void @BENCHMARK(ptr %0) addrspace(1) {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i32
  %4 = alloca i32
  %5 = alloca i16
  %6 = alloca i32
  %7 = alloca i32
  %8 = alloca i32
  %9 = alloca i32
  %10 = alloca i32
  %11 = alloca [18 x i8]
  %12 = alloca [18 x i8]
  store i16 0, ptr %1
  store i16 0, ptr %2
  store i32 0, ptr %3
  store i32 0, ptr %4
  store i16 0, ptr %5
  store i32 0, ptr %6
  store i32 0, ptr %7
  store i32 0, ptr %8
  store i32 0, ptr %9
  store i32 0, ptr %10
  call void @llvm.memset.p0.i16(ptr %11, i8 0, i16 18, i1 false)
  call void @llvm.memset.p0.i16(ptr %12, i8 0, i16 18, i1 false)
  call cc1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 767, i16 2, i16 257, ptr %12)
  call cc1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 5, i16 4, i16 257, ptr %11)
  call cc1000 addrspace(1) void @TSCSNAP(ptr %10, ptr %9)
  store i32 1, ptr %7, !tbaa !2
  store i32 0, ptr %6, !tbaa !2
  store i16 -24576, ptr @b$seg, !tbaa !2
  store i32 0, ptr %8, !tbaa !2
  store i32 63999, ptr %4, !tbaa !2
  store i32 1, ptr %3, !tbaa !2
  br label %b2

b2:
  %13 = load i32, ptr %3, !tbaa !2
  %14 = icmp sge i32 %13, 0
  %15 = sext i1 %14 to i16
  %16 = icmp ne i16 %15, 0
  br i1 %16, label %b3, label %b4

b3:
  %17 = load i32, ptr %8, !tbaa !2
  %18 = load i32, ptr %4, !tbaa !2
  %19 = icmp sle i32 %17, %18
  %20 = sext i1 %19 to i16
  %21 = icmp ne i16 %20, 0
  br i1 %21, label %b5, label %b6

b4:
  %22 = load i32, ptr %8, !tbaa !2
  %23 = load i32, ptr %4, !tbaa !2
  %24 = icmp sge i32 %22, %23
  %25 = sext i1 %24 to i16
  %26 = icmp ne i16 %25, 0
  br i1 %26, label %b5, label %b6

b5:
  %27 = load i32, ptr %7, !tbaa !2
  %28 = load i32, ptr %8, !tbaa !2
  %29 = trunc i32 %28 to i16
  %30 = load i16, ptr @b$seg, !tbaa !2
  %31 = inttoptr i16 %30 to ptr addrspace(2)
  %32 = addrspacecast ptr addrspace(2) %31 to ptr addrspace(1)
  %33 = getelementptr i8, ptr addrspace(1) %32, i16 %29
  %34 = load i8, ptr addrspace(1) %33
  %35 = zext i8 %34 to i16
  %36 = sext i16 %35 to i32
  %37 = add i32 %27, %36
  %38 = srem i32 %37, 65521
  store i32 %38, ptr %7, !tbaa !2
  %39 = load i32, ptr %6, !tbaa !2
  %40 = load i32, ptr %7, !tbaa !2
  %41 = add i32 %39, %40
  %42 = srem i32 %41, 65521
  store i32 %42, ptr %6, !tbaa !2
  %43 = load i32, ptr %8, !tbaa !2
  %44 = load i32, ptr %3, !tbaa !2
  %45 = add i32 %43, %44
  store i32 %45, ptr %8, !tbaa !2
  br label %b2

b6:
  %46 = load i16, ptr %0
  %47 = call cc1000 addrspace(1) ptr @llrm.qb.B$STI2(i16 %46)
  %48 = call cc1000 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %47)
  %49 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string14, ptr %48)
  %50 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %49, ptr @$string15)
  call cc1000 addrspace(1) void @llrm.qb.B$BSAV(ptr %50, i16 0, i16 -1536)
  call cc1000 addrspace(1) void @llrm.qb.B$DSG0()
  call void @llrm.ia16.out.i8(i16 967, i8 0)
  store i16 0, ptr %5, !tbaa !2
  store i16 767, ptr %2, !tbaa !2
  store i16 1, ptr %1, !tbaa !2
  br label %b7

b7:
  %51 = load i16, ptr %1, !tbaa !2
  %52 = icmp sge i16 %51, 0
  %53 = sext i1 %52 to i16
  %54 = icmp ne i16 %53, 0
  br i1 %54, label %b8, label %b9

b8:
  %55 = load i16, ptr %5, !tbaa !2
  %56 = load i16, ptr %2, !tbaa !2
  %57 = icmp sle i16 %55, %56
  %58 = sext i1 %57 to i16
  %59 = icmp ne i16 %58, 0
  br i1 %59, label %b10, label %b11

b9:
  %60 = load i16, ptr %5, !tbaa !2
  %61 = load i16, ptr %2, !tbaa !2
  %62 = icmp sge i16 %60, %61
  %63 = sext i1 %62 to i16
  %64 = icmp ne i16 %63, 0
  br i1 %64, label %b10, label %b11

b10:
  %65 = load i16, ptr %5, !tbaa !2
  %66 = mul i16 %65, 2
  %67 = getelementptr i8, ptr %12, i16 2
  %68 = load i16, ptr %67, !tbaa !2
  %69 = add i16 0, %66
  %70 = inttoptr i16 %68 to ptr addrspace(2)
  %71 = addrspacecast ptr addrspace(2) %70 to ptr addrspace(1)
  %72 = getelementptr i8, ptr addrspace(1) %71, i16 %69
  %73 = call i8 @llrm.ia16.in.i8(i16 969)
  %74 = zext i8 %73 to i16
  store i16 %74, ptr addrspace(1) %72, !tbaa !4
  %75 = load i16, ptr %5, !tbaa !2
  %76 = load i16, ptr %1, !tbaa !2
  %77 = add i16 %75, %76
  store i16 %77, ptr %5, !tbaa !2
  br label %b7

b11:
  %78 = mul i16 0, 2
  %79 = getelementptr i8, ptr %12, i16 2
  %80 = load i16, ptr %79, !tbaa !2
  %81 = add i16 0, %78
  %82 = inttoptr i16 %80 to ptr addrspace(2)
  %83 = addrspacecast ptr addrspace(2) %82 to ptr addrspace(1)
  %84 = getelementptr i8, ptr addrspace(1) %83, i16 %81
  %85 = addrspacecast ptr addrspace(1) %84 to ptr addrspace(2)
  %86 = ptrtoint ptr addrspace(2) %85 to i16
  store i16 %86, ptr @b$seg, !tbaa !2
  %87 = load i16, ptr %0
  %88 = call cc1000 addrspace(1) ptr @llrm.qb.B$STI2(i16 %87)
  %89 = call cc1000 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %88)
  %90 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string16, ptr %89)
  %91 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %90, ptr @$string17)
  %92 = mul i16 0, 2
  %93 = getelementptr i8, ptr %12, i16 2
  %94 = load i16, ptr %93, !tbaa !2
  %95 = add i16 0, %92
  %96 = inttoptr i16 %94 to ptr addrspace(2)
  %97 = addrspacecast ptr addrspace(2) %96 to ptr addrspace(1)
  %98 = getelementptr i8, ptr addrspace(1) %97, i16 %95
  %99 = ptrtoint ptr addrspace(1) %98 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$BSAV(ptr %91, i16 %99, i16 1536)
  %100 = mul i16 0, 4
  %101 = getelementptr i8, ptr %11, i16 2
  %102 = load i16, ptr %101, !tbaa !2
  %103 = add i16 0, %100
  %104 = inttoptr i16 %102 to ptr addrspace(2)
  %105 = addrspacecast ptr addrspace(2) %104 to ptr addrspace(1)
  %106 = getelementptr i8, ptr addrspace(1) %105, i16 %103
  %107 = load i32, ptr @"BENCHHI&", !tbaa !2
  store i32 %107, ptr addrspace(1) %106, !tbaa !4
  %108 = mul i16 1, 4
  %109 = getelementptr i8, ptr %11, i16 2
  %110 = load i16, ptr %109, !tbaa !2
  %111 = add i16 0, %108
  %112 = inttoptr i16 %110 to ptr addrspace(2)
  %113 = addrspacecast ptr addrspace(2) %112 to ptr addrspace(1)
  %114 = getelementptr i8, ptr addrspace(1) %113, i16 %111
  %115 = load i32, ptr @"BENCHLO&", !tbaa !2
  store i32 %115, ptr addrspace(1) %114, !tbaa !4
  %116 = mul i16 2, 4
  %117 = getelementptr i8, ptr %11, i16 2
  %118 = load i16, ptr %117, !tbaa !2
  %119 = add i16 0, %116
  %120 = inttoptr i16 %118 to ptr addrspace(2)
  %121 = addrspacecast ptr addrspace(2) %120 to ptr addrspace(1)
  %122 = getelementptr i8, ptr addrspace(1) %121, i16 %119
  %123 = load i32, ptr %10, !tbaa !2
  store i32 %123, ptr addrspace(1) %122, !tbaa !4
  %124 = mul i16 3, 4
  %125 = getelementptr i8, ptr %11, i16 2
  %126 = load i16, ptr %125, !tbaa !2
  %127 = add i16 0, %124
  %128 = inttoptr i16 %126 to ptr addrspace(2)
  %129 = addrspacecast ptr addrspace(2) %128 to ptr addrspace(1)
  %130 = getelementptr i8, ptr addrspace(1) %129, i16 %127
  %131 = load i32, ptr %9, !tbaa !2
  store i32 %131, ptr addrspace(1) %130, !tbaa !4
  %132 = mul i16 4, 4
  %133 = getelementptr i8, ptr %11, i16 2
  %134 = load i16, ptr %133, !tbaa !2
  %135 = add i16 0, %132
  %136 = inttoptr i16 %134 to ptr addrspace(2)
  %137 = addrspacecast ptr addrspace(2) %136 to ptr addrspace(1)
  %138 = getelementptr i8, ptr addrspace(1) %137, i16 %135
  %139 = load i32, ptr %6, !tbaa !2
  store i32 %139, ptr addrspace(1) %138, !tbaa !4
  %140 = mul i16 5, 4
  %141 = getelementptr i8, ptr %11, i16 2
  %142 = load i16, ptr %141, !tbaa !2
  %143 = add i16 0, %140
  %144 = inttoptr i16 %142 to ptr addrspace(2)
  %145 = addrspacecast ptr addrspace(2) %144 to ptr addrspace(1)
  %146 = getelementptr i8, ptr addrspace(1) %145, i16 %143
  %147 = load i32, ptr %7, !tbaa !2
  store i32 %147, ptr addrspace(1) %146, !tbaa !4
  %148 = mul i16 0, 4
  %149 = getelementptr i8, ptr %11, i16 2
  %150 = load i16, ptr %149, !tbaa !2
  %151 = add i16 0, %148
  %152 = inttoptr i16 %150 to ptr addrspace(2)
  %153 = addrspacecast ptr addrspace(2) %152 to ptr addrspace(1)
  %154 = getelementptr i8, ptr addrspace(1) %153, i16 %151
  %155 = addrspacecast ptr addrspace(1) %154 to ptr addrspace(2)
  %156 = ptrtoint ptr addrspace(2) %155 to i16
  store i16 %156, ptr @b$seg, !tbaa !2
  %157 = load i16, ptr %0
  %158 = call cc1000 addrspace(1) ptr @llrm.qb.B$STI2(i16 %157)
  %159 = call cc1000 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %158)
  %160 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string18, ptr %159)
  %161 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %160, ptr @$string19)
  %162 = mul i16 0, 4
  %163 = getelementptr i8, ptr %11, i16 2
  %164 = load i16, ptr %163, !tbaa !2
  %165 = add i16 0, %162
  %166 = inttoptr i16 %164 to ptr addrspace(2)
  %167 = addrspacecast ptr addrspace(2) %166 to ptr addrspace(1)
  %168 = getelementptr i8, ptr addrspace(1) %167, i16 %165
  %169 = ptrtoint ptr addrspace(1) %168 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$BSAV(ptr %161, i16 %169, i16 24)
  call cc1000 addrspace(1) void @llrm.qb.B$DSG0()
  call cc1000 addrspace(1) void @TSCSNAP(ptr @"BENCHHI&", ptr @"BENCHLO&")
  call cc1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %11)
  call cc1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %12)
  ret void
}

declare cc1000 void @llrm.qb.B$BLOD(ptr, i16, i16) addrspace(1)

declare cc1000 void @llrm.qb.B$DSG0() addrspace(1)

declare cc1000 void @llrm.qb.B$OPEN(ptr, i16, i16, i16) addrspace(1)

declare cc1000 ptr @llrm.qb.B$TIMR() addrspace(1)

declare cc1000 void @llrm.qb.B$CEND() addrspace(1)

declare cc1000 void @llrm.qb.B$N1I2(i16, i16) addrspace(1)

declare cc1000 ptr @llrm.qb.B$RND0() addrspace(1)

declare float @llvm.rint.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i16 @llvm.lrint.i16.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc1000 void @llrm.qb.B$CIRC(float, i16) addrspace(1)

declare cc1000 void @llrm.qb.B$GPUT(ptr addrspace(1), ptr, i16) addrspace(1)

declare cc1000 i16 @"DMADONE%"(ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$COLR(i16, i16, i16) addrspace(1)

declare cc1000 void @llrm.qb.B$LOCT(i16, i16, i16, i16, i16) addrspace(1)

declare cc1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare i8 @llrm.ia16.in.i8(i16) nocallback nofree nounwind willreturn memory(read, inaccessiblemem: readwrite)

declare void @llrm.ia16.out.i8(i16, i8) nocallback nofree nounwind willreturn memory(read, inaccessiblemem: readwrite)

declare cc1000 ptr @llrm.qb.B$FCMD() addrspace(1)

declare cc1000 i16 @llrm.qb.B$INS2(ptr, ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$CSCN(i16, i16, i16) addrspace(1)

declare i32 @llvm.lrint.i32.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc1000 void @llrm.qb.B$GET3(i16, ptr addrspace(1), i16) addrspace(1)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare cc1000 void @llrm.qb.B$DDIM(i16, i16, i16, i16, ptr) addrspace(1)

declare cc1000 void @TSCSNAP(ptr, ptr) addrspace(1)

declare cc1000 ptr @llrm.qb.B$STI2(i16) addrspace(1)

declare cc1000 ptr @llrm.qb.B$LTRM(ptr) addrspace(1)

declare cc1000 ptr @llrm.qb.B$SCAT(ptr, ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$BSAV(ptr, i16, i16) addrspace(1)

declare cc1000 void @llrm.qb.B$ERAS(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
