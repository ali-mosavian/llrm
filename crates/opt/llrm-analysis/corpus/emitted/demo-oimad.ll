target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"BENCHHI&" = internal global [4 x i8] zeroinitializer
@"BENCHLO&" = internal global [4 x i8] zeroinitializer
@"BENCHFRAME&" = internal global [4 x i8] zeroinitializer
@"DSPBASE%" = internal global [2 x i8] zeroinitializer
@BUFFER$ = internal global [32767 x i8] zeroinitializer
@BUFFER$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @BUFFER$ to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr getelementptr (i8, ptr @BUFFER$, i16 -32767), [6 x i8] c"\FF\7F\01\00\01\00" }>
@"BUFOFS&" = internal global [4 x i8] zeroinitializer
@"SND%" = internal global [2 x i8] zeroinitializer
@"TEXT%" = internal global [9062 x i8] zeroinitializer, align 2
@TEXT$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"TEXT%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"TEXT%", [6 x i8] c"\02\00\B3\11\00\00" }>
@"MASK%" = internal global [9062 x i8] zeroinitializer, align 2
@MASK$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"MASK%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"MASK%", [6 x i8] c"\02\00\B3\11\00\00" }>
@b$seg = external global [2 x i8]
@$string7 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\08\00", ptr getelementptr (i8, ptr @$string7, i16 4), [8 x i8] c"text.bsv" }>
@$string8 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\08\00", ptr getelementptr (i8, ptr @$string8, i16 4), [8 x i8] c"mask.bsv" }>
@$string9 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\08\00", ptr getelementptr (i8, ptr @$string9, i16 4), [8 x i8] c"dem1.raw" }>
@$arguments$20 = internal global [36 x i8] zeroinitializer
@"OLDTIMER#" = internal global [8 x i8] zeroinitializer
@"OLDTIMER2#" = internal global [8 x i8] zeroinitializer
@"ADDX%" = internal global [2 x i8] zeroinitializer
@"X2%" = internal global [2 x i8] zeroinitializer
@"X%" = internal global [2 x i8] zeroinitializer
@"Y%" = internal global [2 x i8] zeroinitializer
@$string11 = internal constant <{ [2 x i8], ptr, [20 x i8] }> <{ [2 x i8] c"\14\00", ptr getelementptr (i8, ptr @$string11, i16 4), [20 x i8] c"press Escape to exit" }>
@$float12 = internal constant [4 x i8] zeroinitializer
@$string13 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\08\00", ptr getelementptr (i8, ptr @$string13, i16 4), [8 x i8] c"-NOSOUND" }>
@$string14 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string14, i16 4), [8 x i8] c"pal.dat\00" }>
@$string15 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string15, i16 4), [2 x i8] c"V\00" }>
@$string16 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string16, i16 4), [4 x i8] c".BIN" }>
@$string17 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string17, i16 4), [2 x i8] c"P\00" }>
@$string18 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string18, i16 4), [4 x i8] c".BIN" }>
@$string19 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string19, i16 4), [2 x i8] c"T\00" }>
@$string20 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string20, i16 4), [4 x i8] c".BIN" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = getelementptr inbounds [32767 x i8], ptr @BUFFER$, i16 0
  %1 = addrspacecast ptr %0 to ptr addrspace(1)
  %2 = addrspacecast ptr addrspace(1) %1 to ptr addrspace(2)
  %3 = ptrtoint ptr addrspace(2) %2 to i16
  %4 = sext i16 %3 to i32
  %5 = mul i32 %4, 16
  %6 = getelementptr inbounds [32767 x i8], ptr @BUFFER$, i16 0
  %7 = addrspacecast ptr %6 to ptr addrspace(1)
  %8 = ptrtoint ptr addrspace(1) %7 to i16
  %9 = sext i16 %8 to i32
  %10 = add i32 %5, %9
  store i32 %10, ptr @"BUFOFS&", !tbaa !2
  %11 = getelementptr inbounds i16, ptr @"TEXT%", i16 0
  %12 = addrspacecast ptr %11 to ptr addrspace(1)
  %13 = addrspacecast ptr addrspace(1) %12 to ptr addrspace(2)
  %14 = ptrtoint ptr addrspace(2) %13 to i16
  store i16 %14, ptr @b$seg, !tbaa !2
  %15 = getelementptr inbounds i16, ptr @"TEXT%", i16 0
  %16 = addrspacecast ptr %15 to ptr addrspace(1)
  %17 = ptrtoint ptr addrspace(1) %16 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$BLOD(ptr @$string7, i16 %17, i16 1)
  %18 = getelementptr inbounds i16, ptr @"MASK%", i16 0
  %19 = addrspacecast ptr %18 to ptr addrspace(1)
  %20 = addrspacecast ptr addrspace(1) %19 to ptr addrspace(2)
  %21 = ptrtoint ptr addrspace(2) %20 to i16
  store i16 %21, ptr @b$seg, !tbaa !2
  %22 = getelementptr inbounds i16, ptr @"MASK%", i16 0
  %23 = addrspacecast ptr %22 to ptr addrspace(1)
  %24 = ptrtoint ptr addrspace(1) %23 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$BLOD(ptr @$string8, i16 %24, i16 1)
  call cc 1000 addrspace(1) void @llrm.qb.B$DSG0()
  store i16 544, ptr @"DSPBASE%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$OPEN(ptr @$string9, i16 1, i16 -1, i16 32)
  call cc 1000 addrspace(1) void @INIT()
  store i16 0, ptr @$arguments$20, !tbaa !2
  call cc 1000 addrspace(1) void @BENCHMARK(ptr @$arguments$20)
  %25 = call cc 1000 addrspace(1) ptr @llrm.qb.B$TIMR()
  %26 = load float, ptr %25
  %27 = fpext float %26 to double
  %28 = fpround double %27 to double
  store double %28, ptr @"OLDTIMER#", !tbaa !2
  %29 = call cc 1000 addrspace(1) ptr @llrm.qb.B$TIMR()
  %30 = load float, ptr %29
  %31 = fpext float %30 to double
  %32 = fpround double %31 to double
  store double %32, ptr @"OLDTIMER2#", !tbaa !2
  store i16 1, ptr @"ADDX%", !tbaa !2
  br label %b3

b2:
  %33 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  %34 = icmp sge i32 %33, 3000
  br i1 %34, label %b4, label %b3

b3:
  %35 = load i16, ptr @"X2%", !tbaa !2
  %36 = load i16, ptr @"ADDX%", !tbaa !2
  %37 = add i16 %35, %36
  store i16 %37, ptr @"X2%", !tbaa !2
  %38 = load i16, ptr @"X2%", !tbaa !2
  %39 = icmp sgt i16 %38, 150
  br i1 %39, label %b5, label %b6

b4:
  %40 = getelementptr i8, ptr @$arguments$20, i16 32
  store i16 1, ptr %40, !tbaa !2
  %41 = getelementptr i8, ptr @$arguments$20, i16 32
  call cc 1000 addrspace(1) void @BENCHMARK(ptr %41)
  call cc 1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable

b5:
  store i16 -1, ptr @"ADDX%", !tbaa !2
  br label %b7

b6:
  br label %b7

b7:
  %42 = load i16, ptr @"X2%", !tbaa !2
  %43 = icmp slt i16 %42, 20
  br i1 %43, label %b8, label %b9

b8:
  store i16 1, ptr @"ADDX%", !tbaa !2
  br label %b10

b9:
  br label %b10

b10:
  %44 = load i16, ptr @"X2%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 %44, i16 100)
  %45 = getelementptr i8, ptr @$arguments$20, i16 2
  store i16 16, ptr %45, !tbaa !2
  %46 = getelementptr i8, ptr @$arguments$20, i16 2
  %47 = load i16, ptr %46, !tbaa !2
  %48 = sitofp i16 %47 to float
  %49 = fpround float %48 to float
  %50 = getelementptr i8, ptr @$arguments$20, i16 4
  store float %49, ptr %50, !tbaa !2
  %51 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %52 = load float, ptr %51
  %53 = getelementptr i8, ptr @$arguments$20, i16 8
  store i16 64, ptr %53, !tbaa !2
  %54 = getelementptr i8, ptr @$arguments$20, i16 8
  %55 = load i16, ptr %54, !tbaa !2
  %56 = sitofp i16 %55 to float
  %57 = fmul float %52, %56
  %58 = call float @llvm.rint.f32(float %57)
  %59 = fcmp olt float %57, %58
  %60 = sext i1 %59 to i16
  %61 = getelementptr i8, ptr @$arguments$20, i16 10
  store i16 %60, ptr %61, !tbaa !2
  %62 = getelementptr i8, ptr @$arguments$20, i16 10
  %63 = load i16, ptr %62, !tbaa !2
  %64 = sitofp i16 %63 to float
  %65 = fadd float %58, %64
  %66 = getelementptr i8, ptr @$arguments$20, i16 12
  store i16 200, ptr %66, !tbaa !2
  %67 = getelementptr i8, ptr @$arguments$20, i16 12
  %68 = load i16, ptr %67, !tbaa !2
  %69 = sitofp i16 %68 to float
  %70 = fadd float %65, %69
  %71 = call i16 @llvm.lrint.i16.f32(float %70)
  %72 = getelementptr i8, ptr @$arguments$20, i16 4
  %73 = load float, ptr %72, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$CIRC(float %73, i16 %71)
  %74 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %74, ptr @"BENCHFRAME&", !tbaa !2
  %75 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  %76 = srem i32 %75, 8
  %77 = icmp eq i32 %76, 0
  br i1 %77, label %b11, label %b12

b11:
  %78 = load i16, ptr @"X%", !tbaa !2
  %79 = add i16 15, %78
  %80 = load i16, ptr @"Y%", !tbaa !2
  %81 = add i16 15, %80
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 %79, i16 %81)
  %82 = getelementptr inbounds i16, ptr @"MASK%", i16 0
  %83 = addrspacecast ptr %82 to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$GPUT(ptr addrspace(1) %83, ptr @MASK$descriptor, i16 1)
  %84 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %85 = load float, ptr %84
  %86 = getelementptr i8, ptr @$arguments$20, i16 14
  store i16 10, ptr %86, !tbaa !2
  %87 = getelementptr i8, ptr @$arguments$20, i16 14
  %88 = load i16, ptr %87, !tbaa !2
  %89 = sitofp i16 %88 to float
  %90 = fmul float %85, %89
  %91 = call float @llvm.rint.f32(float %90)
  %92 = fcmp olt float %90, %91
  %93 = sext i1 %92 to i16
  %94 = getelementptr i8, ptr @$arguments$20, i16 16
  store i16 %93, ptr %94, !tbaa !2
  %95 = getelementptr i8, ptr @$arguments$20, i16 16
  %96 = load i16, ptr %95, !tbaa !2
  %97 = sitofp i16 %96 to float
  %98 = fadd float %91, %97
  %99 = call i16 @llvm.lrint.i16.f32(float %98)
  store i16 %99, ptr @"X%", !tbaa !2
  %100 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %101 = load float, ptr %100
  %102 = getelementptr i8, ptr @$arguments$20, i16 18
  store i16 10, ptr %102, !tbaa !2
  %103 = getelementptr i8, ptr @$arguments$20, i16 18
  %104 = load i16, ptr %103, !tbaa !2
  %105 = sitofp i16 %104 to float
  %106 = fmul float %101, %105
  %107 = call float @llvm.rint.f32(float %106)
  %108 = fcmp olt float %106, %107
  %109 = sext i1 %108 to i16
  %110 = getelementptr i8, ptr @$arguments$20, i16 20
  store i16 %109, ptr %110, !tbaa !2
  %111 = getelementptr i8, ptr @$arguments$20, i16 20
  %112 = load i16, ptr %111, !tbaa !2
  %113 = sitofp i16 %112 to float
  %114 = fadd float %107, %113
  %115 = call i16 @llvm.lrint.i16.f32(float %114)
  store i16 %115, ptr @"Y%", !tbaa !2
  %116 = load i16, ptr @"X%", !tbaa !2
  %117 = add i16 15, %116
  %118 = load i16, ptr @"Y%", !tbaa !2
  %119 = add i16 15, %118
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 %117, i16 %119)
  %120 = getelementptr inbounds i16, ptr @"MASK%", i16 0
  %121 = addrspacecast ptr %120 to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$GPUT(ptr addrspace(1) %121, ptr @MASK$descriptor, i16 1)
  %122 = load i16, ptr @"X%", !tbaa !2
  %123 = add i16 15, %122
  %124 = load i16, ptr @"Y%", !tbaa !2
  %125 = add i16 15, %124
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 %123, i16 %125)
  %126 = getelementptr inbounds i16, ptr @"TEXT%", i16 0
  %127 = addrspacecast ptr %126 to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$GPUT(ptr addrspace(1) %127, ptr @TEXT$descriptor, i16 0)
  %128 = load i16, ptr @"X%", !tbaa !2
  %129 = add i16 80, %128
  %130 = load i16, ptr @"Y%", !tbaa !2
  %131 = add i16 150, %130
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 %129, i16 %131)
  %132 = getelementptr inbounds i16, ptr @"MASK%", i16 1500
  %133 = addrspacecast ptr %132 to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$GPUT(ptr addrspace(1) %133, ptr @MASK$descriptor, i16 1)
  %134 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %135 = load float, ptr %134
  %136 = getelementptr i8, ptr @$arguments$20, i16 22
  store i16 10, ptr %136, !tbaa !2
  %137 = getelementptr i8, ptr @$arguments$20, i16 22
  %138 = load i16, ptr %137, !tbaa !2
  %139 = sitofp i16 %138 to float
  %140 = fmul float %135, %139
  %141 = call float @llvm.rint.f32(float %140)
  %142 = fcmp olt float %140, %141
  %143 = sext i1 %142 to i16
  %144 = getelementptr i8, ptr @$arguments$20, i16 24
  store i16 %143, ptr %144, !tbaa !2
  %145 = getelementptr i8, ptr @$arguments$20, i16 24
  %146 = load i16, ptr %145, !tbaa !2
  %147 = sitofp i16 %146 to float
  %148 = fadd float %141, %147
  %149 = call i16 @llvm.lrint.i16.f32(float %148)
  store i16 %149, ptr @"X%", !tbaa !2
  %150 = call cc 1000 addrspace(1) ptr @llrm.qb.B$RND0()
  %151 = load float, ptr %150
  %152 = getelementptr i8, ptr @$arguments$20, i16 26
  store i16 10, ptr %152, !tbaa !2
  %153 = getelementptr i8, ptr @$arguments$20, i16 26
  %154 = load i16, ptr %153, !tbaa !2
  %155 = sitofp i16 %154 to float
  %156 = fmul float %151, %155
  %157 = call float @llvm.rint.f32(float %156)
  %158 = fcmp olt float %156, %157
  %159 = sext i1 %158 to i16
  %160 = getelementptr i8, ptr @$arguments$20, i16 28
  store i16 %159, ptr %160, !tbaa !2
  %161 = getelementptr i8, ptr @$arguments$20, i16 28
  %162 = load i16, ptr %161, !tbaa !2
  %163 = sitofp i16 %162 to float
  %164 = fadd float %157, %163
  %165 = call i16 @llvm.lrint.i16.f32(float %164)
  store i16 %165, ptr @"Y%", !tbaa !2
  %166 = load i16, ptr @"X%", !tbaa !2
  %167 = add i16 80, %166
  %168 = load i16, ptr @"Y%", !tbaa !2
  %169 = add i16 150, %168
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 %167, i16 %169)
  %170 = getelementptr inbounds i16, ptr @"MASK%", i16 1500
  %171 = addrspacecast ptr %170 to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$GPUT(ptr addrspace(1) %171, ptr @MASK$descriptor, i16 1)
  %172 = load i16, ptr @"X%", !tbaa !2
  %173 = add i16 80, %172
  %174 = load i16, ptr @"Y%", !tbaa !2
  %175 = add i16 150, %174
  call cc 1000 addrspace(1) void @llrm.qb.B$N1I2(i16 %173, i16 %175)
  %176 = getelementptr inbounds i16, ptr @"TEXT%", i16 1500
  %177 = addrspacecast ptr %176 to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$GPUT(ptr addrspace(1) %177, ptr @TEXT$descriptor, i16 0)
  %178 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %178, ptr @"BENCHFRAME&", !tbaa !2
  %179 = call cc 1000 addrspace(1) ptr @llrm.qb.B$TIMR()
  %180 = load float, ptr %179
  %181 = fpext float %180 to double
  %182 = fpround double %181 to double
  store double %182, ptr @"OLDTIMER#", !tbaa !2
  br label %b13

b12:
  br label %b13

b13:
  %183 = load i16, ptr @"SND%", !tbaa !2
  %184 = icmp ne i16 %183, 0
  br i1 %184, label %b14, label %b15

b14:
  %185 = getelementptr i8, ptr @$arguments$20, i16 30
  store i16 32767, ptr %185, !tbaa !2
  %186 = getelementptr i8, ptr @$arguments$20, i16 30
  %187 = call cc 1000 addrspace(1) i16 @"DMADONE%"(ptr %186)
  %188 = icmp ne i16 %187, 0
  br i1 %188, label %b17, label %b18

b15:
  br label %b16

b16:
  %189 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  %190 = icmp sgt i32 %189, 1500
  br i1 %190, label %b20, label %b21

b17:
  call cc 1000 addrspace(1) void @PLAYMUSIC()
  br label %b19

b18:
  br label %b19

b19:
  br label %b16

b20:
  call cc 1000 addrspace(1) void @llrm.qb.B$COLR(i16 1, i16 150, i16 2)
  call cc 1000 addrspace(1) void @llrm.qb.B$LOCT(i16 1, i16 8, i16 1, i16 7, i16 4)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string11)
  br label %b22

b21:
  br label %b22

b22:
  %191 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  %192 = add i32 %191, 1
  store i32 %192, ptr @"BENCHFRAME&", !tbaa !2
  br label %b2
}

define cc 1000 i16 @DMADONE(ptr dereferenceable(2) %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i32
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  store i16 0, ptr %5, !tbaa !2
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
  br i1 %20, label %b2, label %b3

b2:
  %21 = load i16, ptr @"DSPBASE%", !tbaa !2
  %22 = add i16 %21, 14
  %23 = call i8 @llrm.ia16.in.i8(i16 %22)
  %24 = zext i8 %23 to i16
  store i16 %24, ptr %1, !tbaa !2
  store i16 1, ptr %5, !tbaa !2
  br label %b4

b3:
  br label %b4

b4:
  br label %b5

b5:
  %25 = load i16, ptr %5, !tbaa !2
  ret i16 %25
}

define cc 1000 void @INIT() addrspace(1) "llrm-fp-eval"="x87-extended" {
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
  %26 = load i16, ptr @"DSPBASE%", !tbaa !2
  %27 = add i16 %26, 6
  call void @llrm.ia16.out.i8(i16 %27, i8 1)
  store i16 1, ptr %24, !tbaa !2
  %28 = load i16, ptr %24, !tbaa !2
  %29 = sitofp i16 %28 to float
  %30 = fpround float %29 to float
  store float %30, ptr %25, !tbaa !2
  store i16 4, ptr %23, !tbaa !2
  %31 = load i16, ptr %23, !tbaa !2
  %32 = sitofp i16 %31 to float
  %33 = fpround float %32 to float
  store float %33, ptr %22, !tbaa !2
  store i16 1, ptr %21, !tbaa !2
  %34 = load i16, ptr %21, !tbaa !2
  %35 = sitofp i16 %34 to float
  %36 = fpround float %35 to float
  store float %36, ptr %20, !tbaa !2
  br label %b2

b2:
  %37 = load float, ptr %20, !tbaa !2
  %38 = load float, ptr @$float12, !tbaa !2
  %39 = fcmp oge float %37, %38
  br i1 %39, label %b3, label %b4

b3:
  %40 = load float, ptr %25, !tbaa !2
  %41 = load float, ptr %22, !tbaa !2
  %42 = fcmp ole float %40, %41
  br i1 %42, label %b5, label %b6

b4:
  %43 = load float, ptr %25, !tbaa !2
  %44 = load float, ptr %22, !tbaa !2
  %45 = fcmp oge float %43, %44
  br i1 %45, label %b5, label %b6

b5:
  %46 = load i16, ptr @"DSPBASE%", !tbaa !2
  %47 = add i16 %46, 6
  %48 = call i8 @llrm.ia16.in.i8(i16 %47)
  %49 = zext i8 %48 to i16
  store i16 %49, ptr %19, !tbaa !2
  %50 = load float, ptr %25, !tbaa !2
  %51 = load float, ptr %20, !tbaa !2
  %52 = fadd float %50, %51
  %53 = fpround float %52 to float
  store float %53, ptr %25, !tbaa !2
  br label %b2

b6:
  %54 = load i16, ptr @"DSPBASE%", !tbaa !2
  %55 = add i16 %54, 6
  call void @llrm.ia16.out.i8(i16 %55, i8 0)
  store i16 -1, ptr %17, !tbaa !2
  %56 = load i16, ptr %17, !tbaa !2
  %57 = sitofp i16 %56 to float
  %58 = fpround float %57 to float
  store float %58, ptr %18, !tbaa !2
  %59 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FCMD()
  %60 = call cc 1000 addrspace(1) i16 @llrm.qb.B$INS2(ptr %59, ptr @$string13)
  %61 = icmp ne i16 %60, 0
  br i1 %61, label %b7, label %b8

b7:
  store i16 0, ptr %16, !tbaa !2
  %62 = load i16, ptr %16, !tbaa !2
  %63 = sitofp i16 %62 to float
  %64 = fpround float %63 to float
  store float %64, ptr %18, !tbaa !2
  br label %b9

b8:
  br label %b9

b9:
  %65 = load i16, ptr @"DSPBASE%", !tbaa !2
  %66 = add i16 %65, 14
  %67 = call i8 @llrm.ia16.in.i8(i16 %66)
  %68 = zext i8 %67 to i16
  %69 = and i16 %68, 128
  %70 = icmp eq i16 %69, 128
  %71 = sext i1 %70 to i16
  %72 = load i16, ptr @"DSPBASE%", !tbaa !2
  %73 = add i16 %72, 10
  %74 = call i8 @llrm.ia16.in.i8(i16 %73)
  %75 = zext i8 %74 to i16
  %76 = icmp eq i16 %75, 170
  %77 = sext i1 %76 to i16
  %78 = and i16 %71, %77
  %79 = icmp ne i16 %78, 0
  br i1 %79, label %b10, label %b11

b10:
  store i16 0, ptr %15, !tbaa !2
  %80 = load i16, ptr %15, !tbaa !2
  %81 = sitofp i16 %80 to float
  %82 = fpround float %81 to float
  store float %82, ptr %18, !tbaa !2
  br label %b12

b11:
  br label %b12

b12:
  store i16 209, ptr %14, !tbaa !2
  call cc 1000 addrspace(1) void @SENDDSP(ptr %14)
  %83 = load float, ptr %18, !tbaa !2
  %84 = load float, ptr @$float12, !tbaa !2
  %85 = fcmp une float %83, %84
  br i1 %85, label %b13, label %b14

b13:
  call cc 1000 addrspace(1) void @PLAYMUSIC()
  br label %b15

b14:
  br label %b15

b15:
  call cc 1000 addrspace(1) void @llrm.qb.B$CSCN(i16 1, i16 13, i16 2)
  store i16 -24576, ptr @b$seg, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$BLOD(ptr @$string14, i16 -1536, i16 1)
  store i16 0, ptr %12, !tbaa !2
  %86 = load i16, ptr %12, !tbaa !2
  %87 = sitofp i16 %86 to float
  %88 = fpround float %87 to float
  store float %88, ptr %13, !tbaa !2
  store i16 255, ptr %11, !tbaa !2
  %89 = load i16, ptr %11, !tbaa !2
  %90 = sitofp i16 %89 to float
  %91 = fpround float %90 to float
  store float %91, ptr %10, !tbaa !2
  store i16 1, ptr %9, !tbaa !2
  %92 = load i16, ptr %9, !tbaa !2
  %93 = sitofp i16 %92 to float
  %94 = fpround float %93 to float
  store float %94, ptr %8, !tbaa !2
  br label %b16

b16:
  %95 = load float, ptr %8, !tbaa !2
  %96 = load float, ptr @$float12, !tbaa !2
  %97 = fcmp oge float %95, %96
  br i1 %97, label %b17, label %b18

b17:
  %98 = load float, ptr %13, !tbaa !2
  %99 = load float, ptr %10, !tbaa !2
  %100 = fcmp ole float %98, %99
  br i1 %100, label %b19, label %b20

b18:
  %101 = load float, ptr %13, !tbaa !2
  %102 = load float, ptr %10, !tbaa !2
  %103 = fcmp oge float %101, %102
  br i1 %103, label %b19, label %b20

b19:
  %104 = load float, ptr %13, !tbaa !2
  %105 = call i16 @llvm.lrint.i16.f32(float %104)
  %106 = trunc i16 %105 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %106)
  %107 = load float, ptr %13, !tbaa !2
  store i16 3, ptr %7, !tbaa !2
  %108 = load i16, ptr %7, !tbaa !2
  %109 = sitofp i16 %108 to float
  %110 = fmul float %107, %109
  store i32 64000, ptr %6, !tbaa !2
  %111 = load i32, ptr %6, !tbaa !2
  %112 = sitofp i32 %111 to float
  %113 = fadd float %112, %110
  %114 = call i32 @llvm.lrint.i32.f32(float %113)
  %115 = trunc i32 %114 to i16
  %116 = load i16, ptr @b$seg, !tbaa !2
  %117 = inttoptr i16 %116 to ptr addrspace(2)
  %118 = addrspacecast ptr addrspace(2) %117 to ptr addrspace(1)
  %119 = getelementptr i8, ptr addrspace(1) %118, i16 %115
  %120 = load volatile i8, ptr addrspace(1) %119
  %121 = zext i8 %120 to i16
  %122 = trunc i16 %121 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %122)
  %123 = load float, ptr %13, !tbaa !2
  store i16 3, ptr %5, !tbaa !2
  %124 = load i16, ptr %5, !tbaa !2
  %125 = sitofp i16 %124 to float
  %126 = fmul float %123, %125
  store i32 64000, ptr %4, !tbaa !2
  %127 = load i32, ptr %4, !tbaa !2
  %128 = sitofp i32 %127 to float
  %129 = fadd float %128, %126
  store i16 1, ptr %3, !tbaa !2
  %130 = load i16, ptr %3, !tbaa !2
  %131 = sitofp i16 %130 to float
  %132 = fadd float %129, %131
  %133 = call i32 @llvm.lrint.i32.f32(float %132)
  %134 = trunc i32 %133 to i16
  %135 = load i16, ptr @b$seg, !tbaa !2
  %136 = inttoptr i16 %135 to ptr addrspace(2)
  %137 = addrspacecast ptr addrspace(2) %136 to ptr addrspace(1)
  %138 = getelementptr i8, ptr addrspace(1) %137, i16 %134
  %139 = load volatile i8, ptr addrspace(1) %138
  %140 = zext i8 %139 to i16
  %141 = trunc i16 %140 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %141)
  %142 = load float, ptr %13, !tbaa !2
  store i16 3, ptr %2, !tbaa !2
  %143 = load i16, ptr %2, !tbaa !2
  %144 = sitofp i16 %143 to float
  %145 = fmul float %142, %144
  store i32 64000, ptr %1, !tbaa !2
  %146 = load i32, ptr %1, !tbaa !2
  %147 = sitofp i32 %146 to float
  %148 = fadd float %147, %145
  store i16 2, ptr %0, !tbaa !2
  %149 = load i16, ptr %0, !tbaa !2
  %150 = sitofp i16 %149 to float
  %151 = fadd float %148, %150
  %152 = call i32 @llvm.lrint.i32.f32(float %151)
  %153 = trunc i32 %152 to i16
  %154 = load i16, ptr @b$seg, !tbaa !2
  %155 = inttoptr i16 %154 to ptr addrspace(2)
  %156 = addrspacecast ptr addrspace(2) %155 to ptr addrspace(1)
  %157 = getelementptr i8, ptr addrspace(1) %156, i16 %153
  %158 = load volatile i8, ptr addrspace(1) %157
  %159 = zext i8 %158 to i16
  %160 = trunc i16 %159 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %160)
  %161 = load float, ptr %13, !tbaa !2
  %162 = load float, ptr %8, !tbaa !2
  %163 = fadd float %161, %162
  %164 = fpround float %163 to float
  store float %164, ptr %13, !tbaa !2
  br label %b16

b20:
  call cc 1000 addrspace(1) void @llrm.qb.B$DSG0()
  ret void
}

define cc 1000 void @PLAYMUSIC() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  store i16 0, ptr %6, !tbaa !2
  store i16 0, ptr %5, !tbaa !2
  %7 = getelementptr inbounds [32767 x i8], ptr @BUFFER$, i16 0
  %8 = addrspacecast ptr %7 to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$GET3(i16 1, ptr addrspace(1) %8, i16 32767)
  call void @llrm.ia16.out.i8(i16 10, i8 5)
  call void @llrm.ia16.out.i8(i16 12, i8 2)
  call void @llrm.ia16.out.i8(i16 11, i8 73)
  %9 = load i16, ptr %6, !tbaa !2
  %10 = trunc i16 %9 to i8
  call void @llrm.ia16.out.i8(i16 2, i8 %10)
  %11 = load i16, ptr %5, !tbaa !2
  %12 = trunc i16 %11 to i8
  call void @llrm.ia16.out.i8(i16 2, i8 %12)
  %13 = load i32, ptr @"BUFOFS&", !tbaa !2
  %14 = and i32 %13, 255
  %15 = trunc i32 %14 to i16
  store i16 %15, ptr %6, !tbaa !2
  %16 = load i32, ptr @"BUFOFS&", !tbaa !2
  %17 = load i16, ptr %6, !tbaa !2
  %18 = sext i16 %17 to i32
  %19 = sub i32 %16, %18
  %20 = sdiv i32 %19, 255
  %21 = trunc i32 %20 to i16
  store i16 %21, ptr %5, !tbaa !2
  %22 = load i16, ptr %6, !tbaa !2
  %23 = trunc i16 %22 to i8
  call void @llrm.ia16.out.i8(i16 3, i8 %23)
  %24 = load i16, ptr %5, !tbaa !2
  %25 = trunc i16 %24 to i8
  call void @llrm.ia16.out.i8(i16 3, i8 %25)
  %26 = load i32, ptr @"BUFOFS&", !tbaa !2
  %27 = sdiv i32 %26, 65536
  %28 = trunc i32 %27 to i8
  call void @llrm.ia16.out.i8(i16 131, i8 %28)
  call void @llrm.ia16.out.i8(i16 10, i8 1)
  store i16 64, ptr %4, !tbaa !2
  call cc 1000 addrspace(1) void @SENDDSP(ptr %4)
  store i16 -250, ptr %3, !tbaa !2
  call cc 1000 addrspace(1) void @SENDDSP(ptr %3)
  store i16 20, ptr %2, !tbaa !2
  call cc 1000 addrspace(1) void @SENDDSP(ptr %2)
  %29 = load i16, ptr %6, !tbaa !2
  store i16 %29, ptr %1, !tbaa !2
  call cc 1000 addrspace(1) void @SENDDSP(ptr %1)
  %30 = load i16, ptr %5, !tbaa !2
  store i16 %30, ptr %0, !tbaa !2
  call cc 1000 addrspace(1) void @SENDDSP(ptr %0)
  ret void
}

define cc 1000 void @SENDDSP(ptr dereferenceable(2) %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
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

define cc 1000 void @BENCHMARK(ptr dereferenceable(2) %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
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
  store i16 0, ptr %12, !tbaa !2
  %13 = getelementptr inbounds i8, ptr %12, i16 2
  store i16 0, ptr %13, !tbaa !2
  %14 = getelementptr inbounds i8, ptr %12, i16 4
  store i16 0, ptr %14, !tbaa !2
  %15 = getelementptr inbounds i8, ptr %12, i16 6
  store i16 0, ptr %15, !tbaa !2
  %16 = getelementptr inbounds i8, ptr %12, i16 8
  store i16 0, ptr %16, !tbaa !2
  %17 = getelementptr inbounds i8, ptr %12, i16 10
  store i16 0, ptr %17, !tbaa !2
  %18 = getelementptr inbounds i8, ptr %12, i16 12
  store i16 0, ptr %18, !tbaa !2
  %19 = getelementptr inbounds i8, ptr %12, i16 14
  store i16 0, ptr %19, !tbaa !2
  %20 = getelementptr inbounds i8, ptr %12, i16 16
  store i16 0, ptr %20, !tbaa !2
  store i16 0, ptr %11, !tbaa !2
  %21 = getelementptr inbounds i8, ptr %11, i16 2
  store i16 0, ptr %21, !tbaa !2
  %22 = getelementptr inbounds i8, ptr %11, i16 4
  store i16 0, ptr %22, !tbaa !2
  %23 = getelementptr inbounds i8, ptr %11, i16 6
  store i16 0, ptr %23, !tbaa !2
  %24 = getelementptr inbounds i8, ptr %11, i16 8
  store i16 0, ptr %24, !tbaa !2
  %25 = getelementptr inbounds i8, ptr %11, i16 10
  store i16 0, ptr %25, !tbaa !2
  %26 = getelementptr inbounds i8, ptr %11, i16 12
  store i16 0, ptr %26, !tbaa !2
  %27 = getelementptr inbounds i8, ptr %11, i16 14
  store i16 0, ptr %27, !tbaa !2
  %28 = getelementptr inbounds i8, ptr %11, i16 16
  store i16 0, ptr %28, !tbaa !2
  store i32 0, ptr %10, !tbaa !2
  store i32 0, ptr %9, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 767, i16 2, i16 257, ptr %12)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 5, i16 4, i16 257, ptr %11)
  call cc 1000 addrspace(1) void @TSCSNAP(ptr %10, ptr %9)
  store i32 1, ptr %7, !tbaa !2
  store i32 0, ptr %6, !tbaa !2
  store i16 -24576, ptr @b$seg, !tbaa !2
  store i32 0, ptr %8, !tbaa !2
  store i32 63999, ptr %4, !tbaa !2
  store i32 1, ptr %3, !tbaa !2
  br label %b2

b2:
  %29 = load i32, ptr %3, !tbaa !2
  %30 = icmp sge i32 %29, 0
  br i1 %30, label %b3, label %b4

b3:
  %31 = load i32, ptr %8, !tbaa !2
  %32 = load i32, ptr %4, !tbaa !2
  %33 = icmp sle i32 %31, %32
  br i1 %33, label %b5, label %b6

b4:
  %34 = load i32, ptr %8, !tbaa !2
  %35 = load i32, ptr %4, !tbaa !2
  %36 = icmp sge i32 %34, %35
  br i1 %36, label %b5, label %b6

b5:
  %37 = load i32, ptr %7, !tbaa !2
  %38 = load i32, ptr %8, !tbaa !2
  %39 = trunc i32 %38 to i16
  %40 = load i16, ptr @b$seg, !tbaa !2
  %41 = inttoptr i16 %40 to ptr addrspace(2)
  %42 = addrspacecast ptr addrspace(2) %41 to ptr addrspace(1)
  %43 = getelementptr i8, ptr addrspace(1) %42, i16 %39
  %44 = load volatile i8, ptr addrspace(1) %43
  %45 = zext i8 %44 to i16
  %46 = sext i16 %45 to i32
  %47 = add i32 %37, %46
  %48 = srem i32 %47, 65521
  store i32 %48, ptr %7, !tbaa !2
  %49 = load i32, ptr %6, !tbaa !2
  %50 = load i32, ptr %7, !tbaa !2
  %51 = add i32 %49, %50
  %52 = srem i32 %51, 65521
  store i32 %52, ptr %6, !tbaa !2
  %53 = load i32, ptr %8, !tbaa !2
  %54 = load i32, ptr %3, !tbaa !2
  %55 = add nsw i32 %53, %54
  store i32 %55, ptr %8, !tbaa !2
  br label %b2

b6:
  %56 = load i16, ptr %0
  %57 = call cc 1000 addrspace(1) ptr @llrm.qb.B$STI2(i16 %56)
  %58 = call cc 1000 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %57)
  %59 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string15, ptr %58)
  %60 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %59, ptr @$string16)
  call cc 1000 addrspace(1) void @llrm.qb.B$BSAV(ptr %60, i16 0, i16 -1536)
  call cc 1000 addrspace(1) void @llrm.qb.B$DSG0()
  call void @llrm.ia16.out.i8(i16 967, i8 0)
  store i16 0, ptr %5, !tbaa !2
  store i16 767, ptr %2, !tbaa !2
  store i16 1, ptr %1, !tbaa !2
  br label %b7

b7:
  %61 = load i16, ptr %1, !tbaa !2
  %62 = icmp sge i16 %61, 0
  br i1 %62, label %b8, label %b9

b8:
  %63 = load i16, ptr %5, !tbaa !2
  %64 = load i16, ptr %2, !tbaa !2
  %65 = icmp sle i16 %63, %64
  br i1 %65, label %b10, label %b11

b9:
  %66 = load i16, ptr %5, !tbaa !2
  %67 = load i16, ptr %2, !tbaa !2
  %68 = icmp sge i16 %66, %67
  br i1 %68, label %b10, label %b11

b10:
  %69 = load i16, ptr %5, !tbaa !2
  %70 = mul i16 %69, 2
  %71 = getelementptr i8, ptr %12, i16 2
  %72 = load i16, ptr %71, !tbaa !2
  %73 = add i16 0, %70
  %74 = inttoptr i16 %72 to ptr addrspace(2)
  %75 = addrspacecast ptr addrspace(2) %74 to ptr addrspace(1)
  %76 = getelementptr i8, ptr addrspace(1) %75, i16 %73
  %77 = call i8 @llrm.ia16.in.i8(i16 969)
  %78 = zext i8 %77 to i16
  store i16 %78, ptr addrspace(1) %76, !tbaa !6
  %79 = load i16, ptr %5, !tbaa !2
  %80 = load i16, ptr %1, !tbaa !2
  %81 = add nsw i16 %79, %80
  store i16 %81, ptr %5, !tbaa !2
  br label %b7

b11:
  %82 = getelementptr i8, ptr %12, i16 2
  %83 = load i16, ptr %82, !tbaa !2
  %84 = inttoptr i16 %83 to ptr addrspace(2)
  %85 = addrspacecast ptr addrspace(2) %84 to ptr addrspace(1)
  %86 = getelementptr i8, ptr addrspace(1) %85, i16 0
  %87 = addrspacecast ptr addrspace(1) %86 to ptr addrspace(2)
  %88 = ptrtoint ptr addrspace(2) %87 to i16
  store i16 %88, ptr @b$seg, !tbaa !2
  %89 = load i16, ptr %0
  %90 = call cc 1000 addrspace(1) ptr @llrm.qb.B$STI2(i16 %89)
  %91 = call cc 1000 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %90)
  %92 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string17, ptr %91)
  %93 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %92, ptr @$string18)
  %94 = getelementptr i8, ptr %12, i16 2
  %95 = load i16, ptr %94, !tbaa !2
  %96 = inttoptr i16 %95 to ptr addrspace(2)
  %97 = addrspacecast ptr addrspace(2) %96 to ptr addrspace(1)
  %98 = getelementptr i8, ptr addrspace(1) %97, i16 0
  %99 = ptrtoint ptr addrspace(1) %98 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$BSAV(ptr %93, i16 %99, i16 1536)
  %100 = getelementptr i8, ptr %11, i16 2
  %101 = load i16, ptr %100, !tbaa !2
  %102 = inttoptr i16 %101 to ptr addrspace(2)
  %103 = addrspacecast ptr addrspace(2) %102 to ptr addrspace(1)
  %104 = getelementptr i8, ptr addrspace(1) %103, i16 0
  %105 = load i32, ptr @"BENCHHI&", !tbaa !2
  store i32 %105, ptr addrspace(1) %104, !tbaa !8
  %106 = getelementptr i8, ptr %11, i16 2
  %107 = load i16, ptr %106, !tbaa !2
  %108 = inttoptr i16 %107 to ptr addrspace(2)
  %109 = addrspacecast ptr addrspace(2) %108 to ptr addrspace(1)
  %110 = getelementptr i8, ptr addrspace(1) %109, i16 4
  %111 = load i32, ptr @"BENCHLO&", !tbaa !2
  store i32 %111, ptr addrspace(1) %110, !tbaa !8
  %112 = getelementptr i8, ptr %11, i16 2
  %113 = load i16, ptr %112, !tbaa !2
  %114 = inttoptr i16 %113 to ptr addrspace(2)
  %115 = addrspacecast ptr addrspace(2) %114 to ptr addrspace(1)
  %116 = getelementptr i8, ptr addrspace(1) %115, i16 8
  %117 = load i32, ptr %10, !tbaa !2
  store i32 %117, ptr addrspace(1) %116, !tbaa !8
  %118 = getelementptr i8, ptr %11, i16 2
  %119 = load i16, ptr %118, !tbaa !2
  %120 = inttoptr i16 %119 to ptr addrspace(2)
  %121 = addrspacecast ptr addrspace(2) %120 to ptr addrspace(1)
  %122 = getelementptr i8, ptr addrspace(1) %121, i16 12
  %123 = load i32, ptr %9, !tbaa !2
  store i32 %123, ptr addrspace(1) %122, !tbaa !8
  %124 = getelementptr i8, ptr %11, i16 2
  %125 = load i16, ptr %124, !tbaa !2
  %126 = inttoptr i16 %125 to ptr addrspace(2)
  %127 = addrspacecast ptr addrspace(2) %126 to ptr addrspace(1)
  %128 = getelementptr i8, ptr addrspace(1) %127, i16 16
  %129 = load i32, ptr %6, !tbaa !2
  store i32 %129, ptr addrspace(1) %128, !tbaa !8
  %130 = getelementptr i8, ptr %11, i16 2
  %131 = load i16, ptr %130, !tbaa !2
  %132 = inttoptr i16 %131 to ptr addrspace(2)
  %133 = addrspacecast ptr addrspace(2) %132 to ptr addrspace(1)
  %134 = getelementptr i8, ptr addrspace(1) %133, i16 20
  %135 = load i32, ptr %7, !tbaa !2
  store i32 %135, ptr addrspace(1) %134, !tbaa !8
  %136 = getelementptr i8, ptr %11, i16 2
  %137 = load i16, ptr %136, !tbaa !2
  %138 = inttoptr i16 %137 to ptr addrspace(2)
  %139 = addrspacecast ptr addrspace(2) %138 to ptr addrspace(1)
  %140 = getelementptr i8, ptr addrspace(1) %139, i16 0
  %141 = addrspacecast ptr addrspace(1) %140 to ptr addrspace(2)
  %142 = ptrtoint ptr addrspace(2) %141 to i16
  store i16 %142, ptr @b$seg, !tbaa !2
  %143 = load i16, ptr %0
  %144 = call cc 1000 addrspace(1) ptr @llrm.qb.B$STI2(i16 %143)
  %145 = call cc 1000 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %144)
  %146 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string19, ptr %145)
  %147 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %146, ptr @$string20)
  %148 = getelementptr i8, ptr %11, i16 2
  %149 = load i16, ptr %148, !tbaa !2
  %150 = inttoptr i16 %149 to ptr addrspace(2)
  %151 = addrspacecast ptr addrspace(2) %150 to ptr addrspace(1)
  %152 = getelementptr i8, ptr addrspace(1) %151, i16 0
  %153 = ptrtoint ptr addrspace(1) %152 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$BSAV(ptr %147, i16 %153, i16 24)
  call cc 1000 addrspace(1) void @llrm.qb.B$DSG0()
  call cc 1000 addrspace(1) void @TSCSNAP(ptr @"BENCHHI&", ptr @"BENCHLO&")
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %11)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %12)
  ret void
}

declare cc 1000 void @llrm.qb.B$BLOD(ptr, i16, i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$DSG0() addrspace(1)

declare cc 1000 void @llrm.qb.B$OPEN(ptr, i16, i16, i16) addrspace(1)

declare cc 1000 ptr @llrm.qb.B$TIMR() addrspace(1)

declare cc 1000 void @llrm.qb.B$CEND() addrspace(1)

declare cc 1000 void @llrm.qb.B$N1I2(i16, i16) addrspace(1)

declare cc 1000 ptr @llrm.qb.B$RND0() addrspace(1)

declare float @llvm.rint.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i16 @llvm.lrint.i16.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1000 void @llrm.qb.B$CIRC(float, i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$GPUT(ptr addrspace(1), ptr, i16) addrspace(1)

declare cc 1000 i16 @"DMADONE%"(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$COLR(i16, i16, i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$LOCT(i16, i16, i16, i16, i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare i8 @llrm.ia16.in.i8(i16) nocallback nofree nounwind willreturn memory(readwrite)

declare void @llrm.ia16.out.i8(i16, i8) nocallback nofree nounwind willreturn memory(readwrite)

declare cc 1000 ptr @llrm.qb.B$FCMD() addrspace(1)

declare cc 1000 i16 @llrm.qb.B$INS2(ptr, ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$CSCN(i16, i16, i16) addrspace(1)

declare i32 @llvm.lrint.i32.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1000 void @llrm.qb.B$GET3(i16, ptr addrspace(1), i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$DDIM(i16, i16, i16, i16, ptr) addrspace(1)

declare cc 1000 void @TSCSNAP(ptr, ptr) addrspace(1)

declare cc 1000 ptr @llrm.qb.B$STI2(i16) addrspace(1)

declare cc 1000 ptr @llrm.qb.B$LTRM(ptr) addrspace(1)

declare cc 1000 ptr @llrm.qb.B$SCAT(ptr, ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$BSAV(ptr, i16, i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$ERAS(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{!"allocation.0", !3, i64 0}
!6 = !{!5, !5, i64 0}
!7 = !{!"allocation.1", !3, i64 0}
!8 = !{!7, !7, i64 0}
