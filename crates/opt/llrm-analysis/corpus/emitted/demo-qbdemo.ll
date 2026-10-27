target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [24 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"BENCHHI&" = internal global [4 x i8] zeroinitializer
@"BENCHLO&" = internal global [4 x i8] zeroinitializer
@"BENCHFRAME&" = internal global [4 x i8] zeroinitializer
@"TOTALFRAMECOUNT%" = internal global [2 x i8] zeroinitializer
@"FRACTAL1%" = internal global [18 x i8] zeroinitializer
@"FRACTAL2%" = internal global [18 x i8] zeroinitializer
@"BOBSPRITE%" = internal global [22 x i8] zeroinitializer
@"X%" = internal global [2 x i8] zeroinitializer
@$arguments$17 = internal global [36 x i8] zeroinitializer
@"TI1!" = internal global [4 x i8] zeroinitializer
@"Y%" = internal global [2 x i8] zeroinitializer
@b$seg = external global [2 x i8]
@$float5 = internal constant [8 x i8] c"^A\DA\22%2\E4\BF"
@$float6 = internal constant [8 x i8] c"\8E`s\FBw}\E2\BF"
@$float7 = internal constant [4 x i8] c"\CD\CC\CC?"
@$string8 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string8, i16 4) }>
@$string9 = internal constant <{ [2 x i8], ptr, [10 x i8] }> <{ [2 x i8] c"\0A\00", ptr getelementptr (i8, ptr @$string9, i16 4), [10 x i8] c"canada.bsv" }>
@$string10 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string10, i16 4) }>
@$float11 = internal constant [4 x i8] c"\C3\F5H@"
@$float12 = internal constant [4 x i8] c"\00\00\00O"
@$string13 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string13, i16 4) }>
@$string14 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string14, i16 4) }>
@$float15 = internal constant [8 x i8] c"\18-DT\FB!\09@"
@$float16 = internal constant [4 x i8] c"\C0\CF\B8:"
@$float17 = internal constant [8 x i8] c"\00\00\00\00\00\00\E0A"
@$float18 = internal constant [4 x i8] c"\B0\03g<"
@$float19 = internal constant [4 x i8] c"\89\D2^<"
@$string20 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string20, i16 4), [2 x i8] c"V\00" }>
@$string21 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string21, i16 4), [4 x i8] c".BIN" }>
@$string22 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string22, i16 4), [2 x i8] c"P\00" }>
@$string23 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string23, i16 4), [4 x i8] c".BIN" }>
@$string24 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string24, i16 4), [2 x i8] c"T\00" }>
@$string25 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string25, i16 4), [4 x i8] c".BIN" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  call cc 1000 addrspace(1) void @llrm.qb.B$CSCN(i16 1, i16 13, i16 2)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 32000, i16 2, i16 257, ptr @"FRACTAL1%")
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 32000, i16 2, i16 257, ptr @"FRACTAL2%")
  store i16 0, ptr @"X%", !tbaa !2
  store i16 63, ptr @$data, !tbaa !2
  %0 = getelementptr i8, ptr @$data, i16 2
  store i16 1, ptr %0, !tbaa !2
  br label %b2

b2:
  %1 = getelementptr i8, ptr @$data, i16 2
  %2 = load i16, ptr %1, !tbaa !2
  %3 = icmp sge i16 %2, 0
  br i1 %3, label %b3, label %b4

b3:
  %4 = load i16, ptr @"X%", !tbaa !2
  %5 = load i16, ptr @$data, !tbaa !2
  %6 = icmp sle i16 %4, %5
  br i1 %6, label %b5, label %b6

b4:
  %7 = load i16, ptr @"X%", !tbaa !2
  %8 = load i16, ptr @$data, !tbaa !2
  %9 = icmp sge i16 %7, %8
  br i1 %9, label %b5, label %b6

b5:
  %10 = load i16, ptr @"X%", !tbaa !2
  %11 = trunc i16 %10 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %11)
  %12 = load i16, ptr @"X%", !tbaa !2
  %13 = trunc i16 %12 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %13)
  %14 = load i16, ptr @"X%", !tbaa !2
  %15 = trunc i16 %14 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %15)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  %16 = load i16, ptr @"X%", !tbaa !2
  %17 = getelementptr i8, ptr @$data, i16 2
  %18 = load i16, ptr %17, !tbaa !2
  %19 = add nsw i16 %16, %18
  store i16 %19, ptr @"X%", !tbaa !2
  br label %b2

b6:
  store i16 64, ptr @"X%", !tbaa !2
  %20 = getelementptr i8, ptr @$data, i16 4
  store i16 127, ptr %20, !tbaa !2
  %21 = getelementptr i8, ptr @$data, i16 6
  store i16 1, ptr %21, !tbaa !2
  br label %b7

b7:
  %22 = getelementptr i8, ptr @$data, i16 6
  %23 = load i16, ptr %22, !tbaa !2
  %24 = icmp sge i16 %23, 0
  br i1 %24, label %b8, label %b9

b8:
  %25 = load i16, ptr @"X%", !tbaa !2
  %26 = getelementptr i8, ptr @$data, i16 4
  %27 = load i16, ptr %26, !tbaa !2
  %28 = icmp sle i16 %25, %27
  br i1 %28, label %b10, label %b11

b9:
  %29 = load i16, ptr @"X%", !tbaa !2
  %30 = getelementptr i8, ptr @$data, i16 4
  %31 = load i16, ptr %30, !tbaa !2
  %32 = icmp sge i16 %29, %31
  br i1 %32, label %b10, label %b11

b10:
  %33 = load i16, ptr @"X%", !tbaa !2
  %34 = trunc i16 %33 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %34)
  call void @llrm.ia16.out.i8(i16 969, i8 63)
  call void @llrm.ia16.out.i8(i16 969, i8 63)
  %35 = load i16, ptr @"X%", !tbaa !2
  %36 = trunc i16 %35 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %36)
  %37 = load i16, ptr @"X%", !tbaa !2
  %38 = getelementptr i8, ptr @$data, i16 6
  %39 = load i16, ptr %38, !tbaa !2
  %40 = add nsw i16 %37, %39
  store i16 %40, ptr @"X%", !tbaa !2
  br label %b7

b11:
  store i16 128, ptr @"X%", !tbaa !2
  %41 = getelementptr i8, ptr @$data, i16 8
  store i16 191, ptr %41, !tbaa !2
  %42 = getelementptr i8, ptr @$data, i16 10
  store i16 1, ptr %42, !tbaa !2
  br label %b12

b12:
  %43 = getelementptr i8, ptr @$data, i16 10
  %44 = load i16, ptr %43, !tbaa !2
  %45 = icmp sge i16 %44, 0
  br i1 %45, label %b13, label %b14

b13:
  %46 = load i16, ptr @"X%", !tbaa !2
  %47 = getelementptr i8, ptr @$data, i16 8
  %48 = load i16, ptr %47, !tbaa !2
  %49 = icmp sle i16 %46, %48
  br i1 %49, label %b15, label %b16

b14:
  %50 = load i16, ptr @"X%", !tbaa !2
  %51 = getelementptr i8, ptr @$data, i16 8
  %52 = load i16, ptr %51, !tbaa !2
  %53 = icmp sge i16 %50, %52
  br i1 %53, label %b15, label %b16

b15:
  %54 = load i16, ptr @"X%", !tbaa !2
  %55 = trunc i16 %54 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %55)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  %56 = load i16, ptr @"X%", !tbaa !2
  %57 = trunc i16 %56 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %57)
  %58 = load i16, ptr @"X%", !tbaa !2
  %59 = getelementptr i8, ptr @$data, i16 10
  %60 = load i16, ptr %59, !tbaa !2
  %61 = add nsw i16 %58, %60
  store i16 %61, ptr @"X%", !tbaa !2
  br label %b12

b16:
  store i16 191, ptr @"X%", !tbaa !2
  %62 = getelementptr i8, ptr @$data, i16 12
  store i16 255, ptr %62, !tbaa !2
  %63 = getelementptr i8, ptr @$data, i16 14
  store i16 1, ptr %63, !tbaa !2
  br label %b17

b17:
  %64 = getelementptr i8, ptr @$data, i16 14
  %65 = load i16, ptr %64, !tbaa !2
  %66 = icmp sge i16 %65, 0
  br i1 %66, label %b18, label %b19

b18:
  %67 = load i16, ptr @"X%", !tbaa !2
  %68 = getelementptr i8, ptr @$data, i16 12
  %69 = load i16, ptr %68, !tbaa !2
  %70 = icmp sle i16 %67, %69
  br i1 %70, label %b20, label %b21

b19:
  %71 = load i16, ptr @"X%", !tbaa !2
  %72 = getelementptr i8, ptr @$data, i16 12
  %73 = load i16, ptr %72, !tbaa !2
  %74 = icmp sge i16 %71, %73
  br i1 %74, label %b20, label %b21

b20:
  %75 = load i16, ptr @"X%", !tbaa !2
  %76 = trunc i16 %75 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %76)
  %77 = load i16, ptr @"X%", !tbaa !2
  %78 = trunc i16 %77 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %78)
  %79 = load i16, ptr @"X%", !tbaa !2
  %80 = trunc i16 %79 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %80)
  call void @llrm.ia16.out.i8(i16 969, i8 63)
  %81 = load i16, ptr @"X%", !tbaa !2
  %82 = getelementptr i8, ptr @$data, i16 14
  %83 = load i16, ptr %82, !tbaa !2
  %84 = add nsw i16 %81, %83
  store i16 %84, ptr @"X%", !tbaa !2
  br label %b17

b21:
  call void @llrm.ia16.out.i8(i16 968, i8 -1)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  call void @llrm.ia16.out.i8(i16 968, i8 127)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  store i16 0, ptr @$arguments$17, !tbaa !2
  call cc 1000 addrspace(1) void @BENCHMARK(ptr @$arguments$17)
  %85 = call cc 1000 addrspace(1) ptr @llrm.qb.B$TIMR()
  %86 = load float, ptr %85
  store float %86, ptr @"TI1!", !tbaa !2
  %87 = getelementptr i8, ptr @$arguments$17, i16 2
  store i16 150, ptr %87, !tbaa !2
  %88 = getelementptr i8, ptr @$arguments$17, i16 2
  call cc 1000 addrspace(1) void @FRACTALEFFECT(ptr %88)
  %89 = getelementptr i8, ptr @$arguments$17, i16 4
  store i16 1, ptr %89, !tbaa !2
  %90 = getelementptr i8, ptr @$arguments$17, i16 4
  call cc 1000 addrspace(1) void @BENCHMARK(ptr %90)
  %91 = call cc 1000 addrspace(1) ptr @llrm.qb.B$TIMR()
  %92 = load float, ptr %91
  %93 = load float, ptr @"TI1!", !tbaa !2
  %94 = fsub float %92, %93
  %95 = fpround float %94 to float
  store float %95, ptr @"TI1!", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr @"FRACTAL1%")
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr @"FRACTAL2%")
  call cc 1000 addrspace(1) void @llrm.qb.B$SCLS(i16 -1)
  %96 = getelementptr i8, ptr @$arguments$17, i16 6
  store i16 16, ptr %96, !tbaa !2
  %97 = getelementptr i8, ptr @$arguments$17, i16 6
  call cc 1000 addrspace(1) void @UNWHITEFADE(ptr %97)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 32, i16 0, i16 32, i16 2, i16 258, ptr @"BOBSPRITE%")
  store i16 0, ptr @"X%", !tbaa !2
  %98 = getelementptr i8, ptr @$data, i16 16
  store i16 32, ptr %98, !tbaa !2
  %99 = getelementptr i8, ptr @$data, i16 18
  store i16 1, ptr %99, !tbaa !2
  br label %b22

b22:
  %100 = getelementptr i8, ptr @$data, i16 18
  %101 = load i16, ptr %100, !tbaa !2
  %102 = icmp sge i16 %101, 0
  br i1 %102, label %b23, label %b24

b23:
  %103 = load i16, ptr @"X%", !tbaa !2
  %104 = getelementptr i8, ptr @$data, i16 16
  %105 = load i16, ptr %104, !tbaa !2
  %106 = icmp sle i16 %103, %105
  br i1 %106, label %b25, label %b26

b24:
  %107 = load i16, ptr @"X%", !tbaa !2
  %108 = getelementptr i8, ptr @$data, i16 16
  %109 = load i16, ptr %108, !tbaa !2
  %110 = icmp sge i16 %107, %109
  br i1 %110, label %b25, label %b26

b25:
  store i16 0, ptr @"Y%", !tbaa !2
  %111 = getelementptr i8, ptr @$data, i16 20
  store i16 32, ptr %111, !tbaa !2
  %112 = getelementptr i8, ptr @$data, i16 22
  store i16 1, ptr %112, !tbaa !2
  br label %b27

b26:
  %113 = getelementptr i8, ptr @$arguments$17, i16 22
  store i16 1024, ptr %113, !tbaa !2
  %114 = getelementptr i8, ptr @$arguments$17, i16 22
  call cc 1000 addrspace(1) void @SHADEBOBEFFECT(ptr %114)
  %115 = getelementptr i8, ptr @$arguments$17, i16 24
  store i16 2, ptr %115, !tbaa !2
  %116 = getelementptr i8, ptr @$arguments$17, i16 24
  call cc 1000 addrspace(1) void @BENCHMARK(ptr %116)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr @"BOBSPRITE%")
  call cc 1000 addrspace(1) void @llrm.qb.B$SCLS(i16 -1)
  %117 = getelementptr i8, ptr @$arguments$17, i16 26
  store i16 16, ptr %117, !tbaa !2
  %118 = getelementptr i8, ptr @$arguments$17, i16 26
  call cc 1000 addrspace(1) void @UNWHITEFADE(ptr %118)
  %119 = getelementptr i8, ptr @$arguments$17, i16 28
  store i16 160, ptr %119, !tbaa !2
  %120 = getelementptr i8, ptr @$arguments$17, i16 28
  call cc 1000 addrspace(1) void @PLASMA(ptr %120)
  %121 = getelementptr i8, ptr @$arguments$17, i16 30
  store i16 3, ptr %121, !tbaa !2
  %122 = getelementptr i8, ptr @$arguments$17, i16 30
  call cc 1000 addrspace(1) void @BENCHMARK(ptr %122)
  call cc 1000 addrspace(1) void @llrm.qb.B$SCLS(i16 -1)
  %123 = getelementptr i8, ptr @$arguments$17, i16 32
  store i16 96, ptr %123, !tbaa !2
  %124 = getelementptr i8, ptr @$arguments$17, i16 32
  call cc 1000 addrspace(1) void @OHCANADA(ptr %124)
  %125 = getelementptr i8, ptr @$arguments$17, i16 34
  store i16 4, ptr %125, !tbaa !2
  %126 = getelementptr i8, ptr @$arguments$17, i16 34
  call cc 1000 addrspace(1) void @BENCHMARK(ptr %126)
  call cc 1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable

b27:
  %127 = getelementptr i8, ptr @$data, i16 22
  %128 = load i16, ptr %127, !tbaa !2
  %129 = icmp sge i16 %128, 0
  br i1 %129, label %b28, label %b29

b28:
  %130 = load i16, ptr @"Y%", !tbaa !2
  %131 = getelementptr i8, ptr @$data, i16 20
  %132 = load i16, ptr %131, !tbaa !2
  %133 = icmp sle i16 %130, %132
  br i1 %133, label %b30, label %b31

b29:
  %134 = load i16, ptr @"Y%", !tbaa !2
  %135 = getelementptr i8, ptr @$data, i16 20
  %136 = load i16, ptr %135, !tbaa !2
  %137 = icmp sge i16 %134, %136
  br i1 %137, label %b30, label %b31

b30:
  %138 = load i16, ptr @"X%", !tbaa !2
  %139 = load i16, ptr @"Y%", !tbaa !2
  %140 = mul i16 %139, 33
  %141 = add i16 %140, %138
  %142 = mul i16 %141, 2
  %143 = getelementptr i8, ptr @"BOBSPRITE%", i16 2
  %144 = load i16, ptr %143, !tbaa !2
  %145 = add i16 0, %142
  %146 = inttoptr i16 %144 to ptr addrspace(2)
  %147 = addrspacecast ptr addrspace(2) %146 to ptr addrspace(1)
  %148 = getelementptr i8, ptr addrspace(1) %147, i16 %145
  %149 = load i16, ptr @"X%", !tbaa !2
  %150 = sub i16 16, %149
  %151 = getelementptr i8, ptr @$arguments$17, i16 8
  store i16 %150, ptr %151, !tbaa !2
  %152 = getelementptr i8, ptr @$arguments$17, i16 8
  %153 = load i16, ptr %152, !tbaa !2
  %154 = sitofp i16 %153 to float
  %155 = fpround float %154 to float
  %156 = getelementptr i8, ptr @$arguments$17, i16 10
  store float %155, ptr %156, !tbaa !2
  %157 = getelementptr i8, ptr @$arguments$17, i16 10
  %158 = load float, ptr %157, !tbaa !2
  %159 = getelementptr i8, ptr @$arguments$17, i16 10
  %160 = load float, ptr %159, !tbaa !2
  %161 = fmul float %158, %160
  %162 = load i16, ptr @"Y%", !tbaa !2
  %163 = sub i16 16, %162
  %164 = getelementptr i8, ptr @$arguments$17, i16 14
  store i16 %163, ptr %164, !tbaa !2
  %165 = getelementptr i8, ptr @$arguments$17, i16 14
  %166 = load i16, ptr %165, !tbaa !2
  %167 = sitofp i16 %166 to float
  %168 = fpround float %167 to float
  %169 = getelementptr i8, ptr @$arguments$17, i16 16
  store float %168, ptr %169, !tbaa !2
  %170 = getelementptr i8, ptr @$arguments$17, i16 16
  %171 = load float, ptr %170, !tbaa !2
  %172 = getelementptr i8, ptr @$arguments$17, i16 16
  %173 = load float, ptr %172, !tbaa !2
  %174 = fmul float %171, %173
  %175 = fadd float %161, %174
  %176 = call float @llvm.sqrt.f32(float %175)
  %177 = getelementptr i8, ptr @$arguments$17, i16 20
  store i16 16, ptr %177, !tbaa !2
  %178 = getelementptr i8, ptr @$arguments$17, i16 20
  %179 = load i16, ptr %178, !tbaa !2
  %180 = sitofp i16 %179 to float
  %181 = fsub float %180, %176
  %182 = call i16 @llvm.lrint.i16.f32(float %181)
  store i16 %182, ptr addrspace(1) %148, !tbaa !6
  %183 = load i16, ptr @"X%", !tbaa !2
  %184 = load i16, ptr @"Y%", !tbaa !2
  %185 = mul i16 %184, 33
  %186 = add i16 %185, %183
  %187 = mul i16 %186, 2
  %188 = getelementptr i8, ptr @"BOBSPRITE%", i16 2
  %189 = load i16, ptr %188, !tbaa !2
  %190 = add i16 0, %187
  %191 = inttoptr i16 %189 to ptr addrspace(2)
  %192 = addrspacecast ptr addrspace(2) %191 to ptr addrspace(1)
  %193 = getelementptr i8, ptr addrspace(1) %192, i16 %190
  %194 = load i16, ptr addrspace(1) %193, !tbaa !6
  %195 = icmp slt i16 %194, 0
  br i1 %195, label %b32, label %b33

b31:
  %196 = load i16, ptr @"X%", !tbaa !2
  %197 = getelementptr i8, ptr @$data, i16 18
  %198 = load i16, ptr %197, !tbaa !2
  %199 = add nsw i16 %196, %198
  store i16 %199, ptr @"X%", !tbaa !2
  br label %b22

b32:
  %200 = load i16, ptr @"X%", !tbaa !2
  %201 = load i16, ptr @"Y%", !tbaa !2
  %202 = mul i16 %201, 33
  %203 = add i16 %202, %200
  %204 = mul i16 %203, 2
  %205 = getelementptr i8, ptr @"BOBSPRITE%", i16 2
  %206 = load i16, ptr %205, !tbaa !2
  %207 = add i16 0, %204
  %208 = inttoptr i16 %206 to ptr addrspace(2)
  %209 = addrspacecast ptr addrspace(2) %208 to ptr addrspace(1)
  %210 = getelementptr i8, ptr addrspace(1) %209, i16 %207
  store i16 0, ptr addrspace(1) %210, !tbaa !6
  br label %b34

b33:
  br label %b34

b34:
  %211 = load i16, ptr @"Y%", !tbaa !2
  %212 = getelementptr i8, ptr @$data, i16 22
  %213 = load i16, ptr %212, !tbaa !2
  %214 = add nsw i16 %211, %213
  store i16 %214, ptr @"Y%", !tbaa !2
  br label %b27
}

define cc 1000 void @DRAWBOB(ptr dereferenceable(2) %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  store i16 0, ptr %6, !tbaa !2
  store i16 31, ptr %5, !tbaa !2
  store i16 1, ptr %4, !tbaa !2
  br label %b2

b2:
  %7 = load i16, ptr %4, !tbaa !2
  %8 = icmp sge i16 %7, 0
  br i1 %8, label %b3, label %b4

b3:
  %9 = load i16, ptr %6, !tbaa !2
  %10 = load i16, ptr %5, !tbaa !2
  %11 = icmp sle i16 %9, %10
  br i1 %11, label %b5, label %b6

b4:
  %12 = load i16, ptr %6, !tbaa !2
  %13 = load i16, ptr %5, !tbaa !2
  %14 = icmp sge i16 %12, %13
  br i1 %14, label %b5, label %b6

b5:
  %15 = load i16, ptr %0
  %16 = add i16 %15, 288
  store i16 %16, ptr %0
  store i16 0, ptr %3, !tbaa !2
  store i16 31, ptr %2, !tbaa !2
  store i16 1, ptr %1, !tbaa !2
  br label %b7

b6:
  ret void

b7:
  %17 = load i16, ptr %1, !tbaa !2
  %18 = icmp sge i16 %17, 0
  br i1 %18, label %b8, label %b9

b8:
  %19 = load i16, ptr %3, !tbaa !2
  %20 = load i16, ptr %2, !tbaa !2
  %21 = icmp sle i16 %19, %20
  br i1 %21, label %b10, label %b11

b9:
  %22 = load i16, ptr %3, !tbaa !2
  %23 = load i16, ptr %2, !tbaa !2
  %24 = icmp sge i16 %22, %23
  br i1 %24, label %b10, label %b11

b10:
  %25 = load i16, ptr %0
  %26 = load i16, ptr %0
  %27 = load i16, ptr @b$seg, !tbaa !2
  %28 = inttoptr i16 %27 to ptr addrspace(2)
  %29 = addrspacecast ptr addrspace(2) %28 to ptr addrspace(1)
  %30 = getelementptr i8, ptr addrspace(1) %29, i16 %26
  %31 = load volatile i8, ptr addrspace(1) %30
  %32 = zext i8 %31 to i16
  %33 = load i16, ptr %3, !tbaa !2
  %34 = load i16, ptr %6, !tbaa !2
  %35 = mul i16 %34, 33
  %36 = add i16 %35, %33
  %37 = mul i16 %36, 2
  %38 = getelementptr i8, ptr @"BOBSPRITE%", i16 2
  %39 = load i16, ptr %38, !tbaa !2
  %40 = add i16 0, %37
  %41 = inttoptr i16 %39 to ptr addrspace(2)
  %42 = addrspacecast ptr addrspace(2) %41 to ptr addrspace(1)
  %43 = getelementptr i8, ptr addrspace(1) %42, i16 %40
  %44 = load i16, ptr addrspace(1) %43, !tbaa !6
  %45 = add i16 %32, %44
  %46 = trunc i16 %45 to i8
  %47 = load i16, ptr @b$seg, !tbaa !2
  %48 = inttoptr i16 %47 to ptr addrspace(2)
  %49 = addrspacecast ptr addrspace(2) %48 to ptr addrspace(1)
  %50 = getelementptr i8, ptr addrspace(1) %49, i16 %25
  store volatile i8 %46, ptr addrspace(1) %50
  %51 = load i16, ptr %0
  %52 = add i16 %51, 1
  store i16 %52, ptr %0
  %53 = load i16, ptr %3, !tbaa !2
  %54 = load i16, ptr %1, !tbaa !2
  %55 = add nsw i16 %53, %54
  store i16 %55, ptr %3, !tbaa !2
  br label %b7

b11:
  %56 = load i16, ptr %6, !tbaa !2
  %57 = load i16, ptr %4, !tbaa !2
  %58 = add nsw i16 %56, %57
  store i16 %58, ptr %6, !tbaa !2
  br label %b2
}

define cc 1000 void @FRACLINE(ptr dereferenceable(2) %0, ptr dereferenceable(8) %1, ptr dereferenceable(8) %2, ptr dereferenceable(8) %3, ptr dereferenceable(8) %4, ptr dereferenceable(8) %5) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca double
  %10 = alloca i16
  %11 = alloca double
  %12 = alloca i16
  %13 = alloca double
  %14 = alloca i16
  %15 = alloca i16
  %16 = alloca i16
  %17 = alloca double
  %18 = alloca i16
  %19 = alloca i16
  %20 = alloca double
  %21 = alloca i16
  %22 = alloca double
  %23 = alloca i16
  %24 = alloca double
  %25 = load double, ptr %4
  %26 = load double, ptr %3
  %27 = fsub double %25, %26
  store i16 160, ptr %23, !tbaa !2
  %28 = load i16, ptr %23, !tbaa !2
  %29 = sitofp i16 %28 to double
  %30 = fdiv double %27, %29
  %31 = fpround double %30 to double
  store double %31, ptr %24, !tbaa !2
  %32 = load double, ptr %2
  %33 = load double, ptr %1
  %34 = fsub double %32, %33
  store i16 160, ptr %21, !tbaa !2
  %35 = load i16, ptr %21, !tbaa !2
  %36 = sitofp i16 %35 to double
  %37 = fdiv double %34, %36
  %38 = fpround double %37 to double
  store double %38, ptr %22, !tbaa !2
  %39 = load double, ptr %1
  store double %39, ptr %20, !tbaa !2
  store i16 1, ptr %19, !tbaa !2
  %40 = load i16, ptr %0
  %41 = mul i16 %40, 320
  store i16 %41, ptr %18, !tbaa !2
  %42 = load double, ptr %3
  %43 = load double, ptr %24, !tbaa !2
  store i16 2, ptr %16, !tbaa !2
  %44 = load i16, ptr %16, !tbaa !2
  %45 = sitofp i16 %44 to double
  %46 = fdiv double %43, %45
  %47 = fadd double %42, %46
  %48 = fpround double %47 to double
  store double %48, ptr %17, !tbaa !2
  br label %b3

b3:
  %49 = load i16, ptr %18, !tbaa !2
  %50 = load i16, ptr %19, !tbaa !2
  %51 = add i16 %49, %50
  %52 = sub i16 %51, 1
  %53 = load i16, ptr @b$seg, !tbaa !2
  %54 = inttoptr i16 %53 to ptr addrspace(2)
  %55 = addrspacecast ptr addrspace(2) %54 to ptr addrspace(1)
  %56 = getelementptr i8, ptr addrspace(1) %55, i16 %52
  %57 = load volatile i8, ptr addrspace(1) %56
  %58 = zext i8 %57 to i16
  store i16 %58, ptr %15, !tbaa !2
  %59 = load i16, ptr %15, !tbaa !2
  %60 = load i16, ptr %18, !tbaa !2
  %61 = load i16, ptr %19, !tbaa !2
  %62 = add i16 %60, %61
  %63 = add i16 %62, 1
  %64 = load i16, ptr @b$seg, !tbaa !2
  %65 = inttoptr i16 %64 to ptr addrspace(2)
  %66 = addrspacecast ptr addrspace(2) %65 to ptr addrspace(1)
  %67 = getelementptr i8, ptr addrspace(1) %66, i16 %63
  %68 = load volatile i8, ptr addrspace(1) %67
  %69 = zext i8 %68 to i16
  %70 = icmp eq i16 %59, %69
  %71 = sext i1 %70 to i16
  %72 = load i16, ptr %15, !tbaa !2
  %73 = icmp sgt i16 %72, 0
  %74 = sext i1 %73 to i16
  %75 = and i16 %71, %74
  %76 = icmp ne i16 %75, 0
  br i1 %76, label %b5, label %b6

b4:
  store i16 160, ptr %19, !tbaa !2
  %77 = load double, ptr %3
  %78 = load double, ptr %4
  %79 = fadd double %77, %78
  store i16 2, ptr %8, !tbaa !2
  %80 = load i16, ptr %8, !tbaa !2
  %81 = sitofp i16 %80 to double
  %82 = fdiv double %79, %81
  %83 = fpround double %82 to double
  store double %83, ptr %17, !tbaa !2
  store i16 0, ptr %7, !tbaa !2
  %84 = load i16, ptr %7, !tbaa !2
  %85 = sitofp i16 %84 to double
  %86 = fpround double %85 to double
  store double %86, ptr %13, !tbaa !2
  store i16 0, ptr %6, !tbaa !2
  %87 = load i16, ptr %6, !tbaa !2
  %88 = sitofp i16 %87 to double
  %89 = fpround double %88 to double
  store double %89, ptr %11, !tbaa !2
  store i16 0, ptr %14, !tbaa !2
  br label %b16

b5:
  %90 = load i16, ptr %15, !tbaa !2
  store i16 %90, ptr %14, !tbaa !2
  br label %b7

b6:
  store i16 0, ptr %12, !tbaa !2
  %91 = load i16, ptr %12, !tbaa !2
  %92 = sitofp i16 %91 to double
  %93 = fpround double %92 to double
  store double %93, ptr %13, !tbaa !2
  store i16 0, ptr %10, !tbaa !2
  %94 = load i16, ptr %10, !tbaa !2
  %95 = sitofp i16 %94 to double
  %96 = fpround double %95 to double
  store double %96, ptr %11, !tbaa !2
  store i16 0, ptr %14, !tbaa !2
  br label %b9

b7:
  %97 = load i16, ptr %19, !tbaa !2
  %98 = load i16, ptr %18, !tbaa !2
  %99 = add i16 %97, %98
  %100 = load i16, ptr %14, !tbaa !2
  %101 = trunc i16 %100 to i8
  %102 = load i16, ptr @b$seg, !tbaa !2
  %103 = inttoptr i16 %102 to ptr addrspace(2)
  %104 = addrspacecast ptr addrspace(2) %103 to ptr addrspace(1)
  %105 = getelementptr i8, ptr addrspace(1) %104, i16 %99
  store volatile i8 %101, ptr addrspace(1) %105
  %106 = load i16, ptr %19, !tbaa !2
  %107 = add i16 %106, 2
  store i16 %107, ptr %19, !tbaa !2
  %108 = load i16, ptr %19, !tbaa !2
  %109 = icmp sge i16 %108, 320
  br i1 %109, label %b11, label %b12

b8:
  %110 = load double, ptr %13, !tbaa !2
  %111 = load double, ptr %13, !tbaa !2
  %112 = fmul double %110, %111
  %113 = load double, ptr %11, !tbaa !2
  %114 = load double, ptr %11, !tbaa !2
  %115 = fmul double %113, %114
  %116 = fadd double %112, %115
  %117 = load double, ptr %5
  %118 = fcmp oge double %116, %117
  %119 = sext i1 %118 to i16
  %120 = load i16, ptr %14, !tbaa !2
  %121 = icmp eq i16 %120, 255
  %122 = sext i1 %121 to i16
  %123 = or i16 %119, %122
  %124 = icmp ne i16 %123, 0
  br i1 %124, label %b10, label %b9

b9:
  %125 = load double, ptr %13, !tbaa !2
  %126 = load double, ptr %13, !tbaa !2
  %127 = fmul double %125, %126
  %128 = load double, ptr %11, !tbaa !2
  %129 = load double, ptr %11, !tbaa !2
  %130 = fmul double %128, %129
  %131 = fsub double %127, %130
  %132 = fpround double %131 to double
  store double %132, ptr %9, !tbaa !2
  %133 = load double, ptr %13, !tbaa !2
  %134 = load double, ptr %11, !tbaa !2
  %135 = fmul double %133, %134
  %136 = fpround double %135 to double
  store double %136, ptr %11, !tbaa !2
  %137 = load double, ptr %11, !tbaa !2
  %138 = load double, ptr %11, !tbaa !2
  %139 = fadd double %137, %138
  %140 = load double, ptr %20, !tbaa !2
  %141 = fadd double %139, %140
  %142 = fpround double %141 to double
  store double %142, ptr %11, !tbaa !2
  %143 = load double, ptr %9, !tbaa !2
  %144 = load double, ptr %17, !tbaa !2
  %145 = fadd double %143, %144
  %146 = fpround double %145 to double
  store double %146, ptr %13, !tbaa !2
  %147 = load i16, ptr %14, !tbaa !2
  %148 = add i16 %147, 1
  store i16 %148, ptr %14, !tbaa !2
  br label %b8

b10:
  br label %b7

b11:
  br label %b4

b12:
  br label %b13

b13:
  %149 = load double, ptr %17, !tbaa !2
  %150 = load double, ptr %24, !tbaa !2
  %151 = fadd double %149, %150
  %152 = fpround double %151 to double
  store double %152, ptr %17, !tbaa !2
  %153 = load double, ptr %20, !tbaa !2
  %154 = load double, ptr %22, !tbaa !2
  %155 = fadd double %153, %154
  %156 = fpround double %155 to double
  store double %156, ptr %20, !tbaa !2
  br label %b3

b15:
  %157 = load double, ptr %13, !tbaa !2
  %158 = load double, ptr %13, !tbaa !2
  %159 = fmul double %157, %158
  %160 = load double, ptr %11, !tbaa !2
  %161 = load double, ptr %11, !tbaa !2
  %162 = fmul double %160, %161
  %163 = fadd double %159, %162
  %164 = load double, ptr %5
  %165 = fcmp oge double %163, %164
  %166 = sext i1 %165 to i16
  %167 = load i16, ptr %14, !tbaa !2
  %168 = icmp eq i16 %167, 255
  %169 = sext i1 %168 to i16
  %170 = or i16 %166, %169
  %171 = icmp ne i16 %170, 0
  br i1 %171, label %b17, label %b16

b16:
  %172 = load double, ptr %13, !tbaa !2
  %173 = load double, ptr %13, !tbaa !2
  %174 = fmul double %172, %173
  %175 = load double, ptr %11, !tbaa !2
  %176 = load double, ptr %11, !tbaa !2
  %177 = fmul double %175, %176
  %178 = fsub double %174, %177
  %179 = fpround double %178 to double
  store double %179, ptr %9, !tbaa !2
  %180 = load double, ptr %13, !tbaa !2
  %181 = load double, ptr %11, !tbaa !2
  %182 = fmul double %180, %181
  %183 = fpround double %182 to double
  store double %183, ptr %11, !tbaa !2
  %184 = load double, ptr %11, !tbaa !2
  %185 = load double, ptr %11, !tbaa !2
  %186 = fadd double %184, %185
  %187 = load double, ptr %20, !tbaa !2
  %188 = fadd double %186, %187
  %189 = fpround double %188 to double
  store double %189, ptr %11, !tbaa !2
  %190 = load double, ptr %9, !tbaa !2
  %191 = load double, ptr %17, !tbaa !2
  %192 = fadd double %190, %191
  %193 = fpround double %192 to double
  store double %193, ptr %13, !tbaa !2
  %194 = load i16, ptr %14, !tbaa !2
  %195 = add i16 %194, 1
  store i16 %195, ptr %14, !tbaa !2
  br label %b15

b17:
  %196 = load i16, ptr %19, !tbaa !2
  %197 = load i16, ptr %18, !tbaa !2
  %198 = add i16 %196, %197
  %199 = load i16, ptr %14, !tbaa !2
  %200 = trunc i16 %199 to i8
  %201 = load i16, ptr @b$seg, !tbaa !2
  %202 = inttoptr i16 %201 to ptr addrspace(2)
  %203 = addrspacecast ptr addrspace(2) %202 to ptr addrspace(1)
  %204 = getelementptr i8, ptr addrspace(1) %203, i16 %198
  store volatile i8 %200, ptr addrspace(1) %204
  ret void
}

define cc 1000 void @FRACLINE2(ptr dereferenceable(2) %0, ptr dereferenceable(8) %1, ptr dereferenceable(8) %2, ptr dereferenceable(8) %3, ptr dereferenceable(8) %4, ptr dereferenceable(8) %5) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %6 = alloca double
  %7 = alloca i16
  %8 = alloca double
  %9 = alloca i16
  %10 = alloca double
  %11 = alloca i16
  %12 = alloca double
  %13 = alloca i16
  %14 = alloca i16
  %15 = alloca double
  %16 = alloca i16
  %17 = alloca double
  %18 = alloca i16
  %19 = alloca double
  %20 = alloca [18 x i8]
  store i16 0, ptr %20, !tbaa !2
  %21 = getelementptr inbounds i8, ptr %20, i16 2
  store i16 0, ptr %21, !tbaa !2
  %22 = getelementptr inbounds i8, ptr %20, i16 4
  store i16 0, ptr %22, !tbaa !2
  %23 = getelementptr inbounds i8, ptr %20, i16 6
  store i16 0, ptr %23, !tbaa !2
  %24 = getelementptr inbounds i8, ptr %20, i16 8
  store i16 0, ptr %24, !tbaa !2
  %25 = getelementptr inbounds i8, ptr %20, i16 10
  store i16 0, ptr %25, !tbaa !2
  %26 = getelementptr inbounds i8, ptr %20, i16 12
  store i16 0, ptr %26, !tbaa !2
  %27 = getelementptr inbounds i8, ptr %20, i16 14
  store i16 0, ptr %27, !tbaa !2
  %28 = getelementptr inbounds i8, ptr %20, i16 16
  store i16 0, ptr %28, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 320, i16 2, i16 257, ptr %20)
  %29 = load double, ptr %4
  %30 = load double, ptr %3
  %31 = fsub double %29, %30
  store i16 320, ptr %18, !tbaa !2
  %32 = load i16, ptr %18, !tbaa !2
  %33 = sitofp i16 %32 to double
  %34 = fdiv double %31, %33
  %35 = fpround double %34 to double
  store double %35, ptr %19, !tbaa !2
  %36 = load double, ptr %2
  %37 = load double, ptr %1
  %38 = fsub double %36, %37
  store i16 320, ptr %16, !tbaa !2
  %39 = load i16, ptr %16, !tbaa !2
  %40 = sitofp i16 %39 to double
  %41 = fdiv double %38, %40
  %42 = fpround double %41 to double
  store double %42, ptr %17, !tbaa !2
  %43 = load double, ptr %1
  store double %43, ptr %15, !tbaa !2
  store i16 0, ptr %14, !tbaa !2
  %44 = load i16, ptr %0
  %45 = mul i16 %44, 320
  store i16 %45, ptr %13, !tbaa !2
  %46 = load double, ptr %3
  store double %46, ptr %12, !tbaa !2
  br label %b3

b3:
  %47 = load i16, ptr %14, !tbaa !2
  %48 = load i16, ptr %13, !tbaa !2
  %49 = add i16 %47, %48
  %50 = sub i16 %49, 320
  %51 = load i16, ptr @b$seg, !tbaa !2
  %52 = inttoptr i16 %51 to ptr addrspace(2)
  %53 = addrspacecast ptr addrspace(2) %52 to ptr addrspace(1)
  %54 = getelementptr i8, ptr addrspace(1) %53, i16 %50
  %55 = load volatile i8, ptr addrspace(1) %54
  %56 = zext i8 %55 to i16
  %57 = load i16, ptr %14, !tbaa !2
  %58 = load i16, ptr %13, !tbaa !2
  %59 = add i16 %57, %58
  %60 = add i16 %59, 320
  %61 = load i16, ptr @b$seg, !tbaa !2
  %62 = inttoptr i16 %61 to ptr addrspace(2)
  %63 = addrspacecast ptr addrspace(2) %62 to ptr addrspace(1)
  %64 = getelementptr i8, ptr addrspace(1) %63, i16 %60
  %65 = load volatile i8, ptr addrspace(1) %64
  %66 = zext i8 %65 to i16
  %67 = icmp eq i16 %56, %66
  br i1 %67, label %b5, label %b6

b5:
  %68 = load i16, ptr %14, !tbaa !2
  %69 = load i16, ptr %13, !tbaa !2
  %70 = add i16 %68, %69
  %71 = sub i16 %70, 320
  %72 = load i16, ptr @b$seg, !tbaa !2
  %73 = inttoptr i16 %72 to ptr addrspace(2)
  %74 = addrspacecast ptr addrspace(2) %73 to ptr addrspace(1)
  %75 = getelementptr i8, ptr addrspace(1) %74, i16 %71
  %76 = load volatile i8, ptr addrspace(1) %75
  %77 = zext i8 %76 to i16
  store i16 %77, ptr %11, !tbaa !2
  br label %b7

b6:
  store i16 0, ptr %9, !tbaa !2
  %78 = load i16, ptr %9, !tbaa !2
  %79 = sitofp i16 %78 to double
  %80 = fpround double %79 to double
  store double %80, ptr %10, !tbaa !2
  store i16 0, ptr %7, !tbaa !2
  %81 = load i16, ptr %7, !tbaa !2
  %82 = sitofp i16 %81 to double
  %83 = fpround double %82 to double
  store double %83, ptr %8, !tbaa !2
  store i16 0, ptr %11, !tbaa !2
  br label %b9

b7:
  %84 = load i16, ptr %14, !tbaa !2
  %85 = load i16, ptr %13, !tbaa !2
  %86 = add i16 %84, %85
  %87 = load i16, ptr %11, !tbaa !2
  %88 = trunc i16 %87 to i8
  %89 = load i16, ptr @b$seg, !tbaa !2
  %90 = inttoptr i16 %89 to ptr addrspace(2)
  %91 = addrspacecast ptr addrspace(2) %90 to ptr addrspace(1)
  %92 = getelementptr i8, ptr addrspace(1) %91, i16 %86
  store volatile i8 %88, ptr addrspace(1) %92
  %93 = load i16, ptr %14, !tbaa !2
  %94 = add i16 %93, 1
  store i16 %94, ptr %14, !tbaa !2
  %95 = load i16, ptr %14, !tbaa !2
  %96 = icmp sge i16 %95, 320
  br i1 %96, label %b11, label %b12

b8:
  %97 = load double, ptr %10, !tbaa !2
  %98 = load double, ptr %10, !tbaa !2
  %99 = fmul double %97, %98
  %100 = load double, ptr %8, !tbaa !2
  %101 = load double, ptr %8, !tbaa !2
  %102 = fmul double %100, %101
  %103 = fadd double %99, %102
  %104 = load double, ptr %5
  %105 = fcmp oge double %103, %104
  %106 = sext i1 %105 to i16
  %107 = load i16, ptr %11, !tbaa !2
  %108 = icmp eq i16 %107, 255
  %109 = sext i1 %108 to i16
  %110 = or i16 %106, %109
  %111 = icmp ne i16 %110, 0
  br i1 %111, label %b10, label %b9

b9:
  %112 = load double, ptr %10, !tbaa !2
  %113 = load double, ptr %10, !tbaa !2
  %114 = fmul double %112, %113
  %115 = load double, ptr %8, !tbaa !2
  %116 = load double, ptr %8, !tbaa !2
  %117 = fmul double %115, %116
  %118 = fsub double %114, %117
  %119 = fpround double %118 to double
  store double %119, ptr %6, !tbaa !2
  %120 = load double, ptr %10, !tbaa !2
  %121 = load double, ptr %8, !tbaa !2
  %122 = fmul double %120, %121
  %123 = fpround double %122 to double
  store double %123, ptr %8, !tbaa !2
  %124 = load double, ptr %8, !tbaa !2
  %125 = load double, ptr %8, !tbaa !2
  %126 = fadd double %124, %125
  %127 = load double, ptr %15, !tbaa !2
  %128 = fadd double %126, %127
  %129 = fpround double %128 to double
  store double %129, ptr %8, !tbaa !2
  %130 = load double, ptr %6, !tbaa !2
  %131 = load double, ptr %12, !tbaa !2
  %132 = fadd double %130, %131
  %133 = fpround double %132 to double
  store double %133, ptr %10, !tbaa !2
  %134 = load i16, ptr %11, !tbaa !2
  %135 = add i16 %134, 1
  store i16 %135, ptr %11, !tbaa !2
  br label %b8

b10:
  br label %b7

b11:
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %20)
  ret void

b12:
  br label %b13

b13:
  %136 = load double, ptr %12, !tbaa !2
  %137 = load double, ptr %19, !tbaa !2
  %138 = fadd double %136, %137
  %139 = fpround double %138 to double
  store double %139, ptr %12, !tbaa !2
  %140 = load double, ptr %15, !tbaa !2
  %141 = load double, ptr %17, !tbaa !2
  %142 = fadd double %140, %141
  %143 = fpround double %142 to double
  store double %143, ptr %15, !tbaa !2
  br label %b3
}

define cc 1000 void @FRACTALEFFECT(ptr dereferenceable(2) %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca i16
  %14 = alloca i16
  %15 = alloca i16
  %16 = alloca i16
  %17 = alloca i16
  %18 = alloca i16
  %19 = alloca i16
  %20 = alloca i16
  %21 = alloca i16
  %22 = alloca i16
  %23 = alloca i16
  %24 = alloca i16
  %25 = alloca i16
  %26 = alloca i16
  %27 = alloca i16
  %28 = alloca double
  %29 = alloca i16
  %30 = alloca double
  %31 = alloca i16
  %32 = alloca double
  %33 = alloca i16
  %34 = alloca double
  %35 = alloca i16
  %36 = alloca double
  %37 = alloca i16
  %38 = alloca double
  %39 = alloca i16
  %40 = alloca double
  %41 = alloca i16
  %42 = alloca i16
  %43 = alloca i16
  %44 = alloca i16
  %45 = alloca i16
  %46 = alloca i16
  %47 = alloca i16
  %48 = alloca i16
  %49 = alloca double
  %50 = alloca i16
  %51 = alloca [22 x i8]
  store i16 0, ptr %51, !tbaa !2
  %52 = getelementptr inbounds i8, ptr %51, i16 2
  store i16 0, ptr %52, !tbaa !2
  %53 = getelementptr inbounds i8, ptr %51, i16 4
  store i16 0, ptr %53, !tbaa !2
  %54 = getelementptr inbounds i8, ptr %51, i16 6
  store i16 0, ptr %54, !tbaa !2
  %55 = getelementptr inbounds i8, ptr %51, i16 8
  store i16 0, ptr %55, !tbaa !2
  %56 = getelementptr inbounds i8, ptr %51, i16 10
  store i16 0, ptr %56, !tbaa !2
  %57 = getelementptr inbounds i8, ptr %51, i16 12
  store i16 0, ptr %57, !tbaa !2
  %58 = getelementptr inbounds i8, ptr %51, i16 14
  store i16 0, ptr %58, !tbaa !2
  %59 = getelementptr inbounds i8, ptr %51, i16 16
  store i16 0, ptr %59, !tbaa !2
  %60 = getelementptr inbounds i8, ptr %51, i16 18
  store i16 0, ptr %60, !tbaa !2
  %61 = getelementptr inbounds i8, ptr %51, i16 20
  store i16 0, ptr %61, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 160, i16 0, i16 100, i16 2, i16 258, ptr %51)
  store i16 -100, ptr %50, !tbaa !2
  store i16 1, ptr %48, !tbaa !2
  %62 = load i16, ptr %48, !tbaa !2
  %63 = sitofp i16 %62 to double
  %64 = fpround double %63 to double
  store double %64, ptr %49, !tbaa !2
  store i16 1, ptr %47, !tbaa !2
  %65 = load i16, ptr %0
  store i16 %65, ptr %46, !tbaa !2
  store i16 1, ptr %45, !tbaa !2
  br label %b2

b2:
  %66 = load i16, ptr %45, !tbaa !2
  %67 = icmp sge i16 %66, 0
  br i1 %67, label %b3, label %b4

b3:
  %68 = load i16, ptr %47, !tbaa !2
  %69 = load i16, ptr %46, !tbaa !2
  %70 = icmp sle i16 %68, %69
  br i1 %70, label %b5, label %b6

b4:
  %71 = load i16, ptr %47, !tbaa !2
  %72 = load i16, ptr %46, !tbaa !2
  %73 = icmp sge i16 %71, %72
  br i1 %73, label %b5, label %b6

b5:
  %74 = load i16, ptr %50, !tbaa !2
  store i16 %74, ptr %44, !tbaa !2
  %75 = load i16, ptr %50, !tbaa !2
  %76 = add i16 %75, 1
  store i16 %76, ptr %43, !tbaa !2
  %77 = load i16, ptr %50, !tbaa !2
  %78 = add i16 %77, 2
  store i16 %78, ptr %42, !tbaa !2
  %79 = load i16, ptr %50, !tbaa !2
  %80 = add i16 %79, 3
  store i16 %80, ptr %41, !tbaa !2
  %81 = load i16, ptr %44, !tbaa !2
  %82 = load double, ptr %49, !tbaa !2
  store i16 %81, ptr %39, !tbaa !2
  %83 = load i16, ptr %39, !tbaa !2
  %84 = sitofp i16 %83 to double
  %85 = fdiv double %84, %82
  %86 = load double, ptr @$float5, !tbaa !2
  %87 = fadd double %85, %86
  %88 = fpround double %87 to double
  store double %88, ptr %40, !tbaa !2
  %89 = load i16, ptr %43, !tbaa !2
  %90 = load double, ptr %49, !tbaa !2
  store i16 %89, ptr %37, !tbaa !2
  %91 = load i16, ptr %37, !tbaa !2
  %92 = sitofp i16 %91 to double
  %93 = fdiv double %92, %90
  %94 = load double, ptr @$float5, !tbaa !2
  %95 = fadd double %93, %94
  %96 = fpround double %95 to double
  store double %96, ptr %38, !tbaa !2
  %97 = load i16, ptr %42, !tbaa !2
  %98 = load double, ptr %49, !tbaa !2
  store i16 %97, ptr %35, !tbaa !2
  %99 = load i16, ptr %35, !tbaa !2
  %100 = sitofp i16 %99 to double
  %101 = fdiv double %100, %98
  %102 = load double, ptr @$float5, !tbaa !2
  %103 = fadd double %101, %102
  %104 = fpround double %103 to double
  store double %104, ptr %36, !tbaa !2
  %105 = load i16, ptr %41, !tbaa !2
  %106 = load double, ptr %49, !tbaa !2
  store i16 %105, ptr %33, !tbaa !2
  %107 = load i16, ptr %33, !tbaa !2
  %108 = sitofp i16 %107 to double
  %109 = fdiv double %108, %106
  %110 = load double, ptr @$float5, !tbaa !2
  %111 = fadd double %109, %110
  %112 = fpround double %111 to double
  store double %112, ptr %34, !tbaa !2
  %113 = load double, ptr @$float6, !tbaa !2
  %114 = load double, ptr %49, !tbaa !2
  store i16 160, ptr %31, !tbaa !2
  %115 = load i16, ptr %31, !tbaa !2
  %116 = sitofp i16 %115 to double
  %117 = fdiv double %116, %114
  %118 = fsub double %113, %117
  %119 = fpround double %118 to double
  store double %119, ptr %32, !tbaa !2
  %120 = load double, ptr @$float6, !tbaa !2
  %121 = load double, ptr %49, !tbaa !2
  store i16 160, ptr %29, !tbaa !2
  %122 = load i16, ptr %29, !tbaa !2
  %123 = sitofp i16 %122 to double
  %124 = fdiv double %123, %121
  %125 = fadd double %120, %124
  %126 = fpround double %125 to double
  store double %126, ptr %30, !tbaa !2
  %127 = getelementptr i8, ptr @"FRACTAL1%", i16 2
  %128 = load i16, ptr %127, !tbaa !2
  %129 = inttoptr i16 %128 to ptr addrspace(2)
  %130 = addrspacecast ptr addrspace(2) %129 to ptr addrspace(1)
  %131 = getelementptr i8, ptr addrspace(1) %130, i16 0
  %132 = addrspacecast ptr addrspace(1) %131 to ptr addrspace(2)
  %133 = ptrtoint ptr addrspace(2) %132 to i16
  store i16 %133, ptr @b$seg, !tbaa !2
  store i16 4, ptr %27, !tbaa !2
  %134 = load i16, ptr %27, !tbaa !2
  %135 = sitofp i16 %134 to double
  %136 = fpround double %135 to double
  store double %136, ptr %28, !tbaa !2
  %137 = load i16, ptr %44, !tbaa !2
  %138 = add i16 %137, 100
  store i16 %138, ptr %26, !tbaa !2
  call cc 1000 addrspace(1) void @FRACLINE(ptr %26, ptr %40, ptr %40, ptr %32, ptr %30, ptr %28)
  %139 = load i16, ptr %42, !tbaa !2
  %140 = add i16 %139, 100
  store i16 %140, ptr %25, !tbaa !2
  call cc 1000 addrspace(1) void @FRACLINE(ptr %25, ptr %36, ptr %36, ptr %32, ptr %30, ptr %28)
  %141 = load i16, ptr %43, !tbaa !2
  %142 = add i16 %141, 100
  store i16 %142, ptr %24, !tbaa !2
  call cc 1000 addrspace(1) void @FRACLINE2(ptr %24, ptr %38, ptr %38, ptr %32, ptr %30, ptr %28)
  %143 = load i16, ptr %41, !tbaa !2
  %144 = add i16 %143, 100
  store i16 %144, ptr %23, !tbaa !2
  call cc 1000 addrspace(1) void @FRACLINE2(ptr %23, ptr %34, ptr %34, ptr %32, ptr %30, ptr %28)
  %145 = load i16, ptr %50, !tbaa !2
  %146 = add i16 %145, 4
  store i16 %146, ptr %50, !tbaa !2
  %147 = load i16, ptr %50, !tbaa !2
  %148 = icmp sge i16 %147, 100
  br i1 %148, label %b7, label %b8

b6:
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %51)
  ret void

b7:
  store i16 -100, ptr %50, !tbaa !2
  %149 = load double, ptr %49, !tbaa !2
  store i16 2, ptr %22, !tbaa !2
  %150 = load i16, ptr %22, !tbaa !2
  %151 = sitofp i16 %150 to double
  %152 = fmul double %149, %151
  %153 = fpround double %152 to double
  store double %153, ptr %49, !tbaa !2
  store i16 16080, ptr %21, !tbaa !2
  store i16 0, ptr %20, !tbaa !2
  store i16 99, ptr %19, !tbaa !2
  store i16 1, ptr %18, !tbaa !2
  br label %b10

b8:
  br label %b9

b9:
  %154 = getelementptr i8, ptr @"FRACTAL2%", i16 2
  %155 = load i16, ptr %154, !tbaa !2
  %156 = inttoptr i16 %155 to ptr addrspace(2)
  %157 = addrspacecast ptr addrspace(2) %156 to ptr addrspace(1)
  %158 = getelementptr i8, ptr addrspace(1) %157, i16 0
  %159 = addrspacecast ptr addrspace(1) %158 to ptr addrspace(2)
  %160 = ptrtoint ptr addrspace(2) %159 to i16
  store i16 %160, ptr @b$seg, !tbaa !2
  %161 = load i16, ptr %47, !tbaa !2
  %162 = sext i16 %161 to i32
  %163 = srem i32 %162, 50
  %164 = trunc i32 %163 to i16
  store i16 %164, ptr %8, !tbaa !2
  %165 = load i16, ptr %8, !tbaa !2
  %166 = load float, ptr @$float7, !tbaa !2
  store i16 %165, ptr %6, !tbaa !2
  %167 = load i16, ptr %6, !tbaa !2
  %168 = sitofp i16 %167 to float
  %169 = fmul float %168, %166
  %170 = call i16 @llvm.lrint.i16.f32(float %169)
  store i16 %170, ptr %7, !tbaa !2
  %171 = load i16, ptr %8, !tbaa !2
  store i16 %171, ptr %5, !tbaa !2
  %172 = load i16, ptr %8, !tbaa !2
  %173 = load float, ptr @$float7, !tbaa !2
  store i16 %172, ptr %3, !tbaa !2
  %174 = load i16, ptr %3, !tbaa !2
  %175 = sitofp i16 %174 to float
  %176 = fmul float %175, %173
  store i16 320, ptr %2, !tbaa !2
  %177 = load i16, ptr %2, !tbaa !2
  %178 = sitofp i16 %177 to float
  %179 = fsub float %178, %176
  %180 = call i16 @llvm.lrint.i16.f32(float %179)
  store i16 %180, ptr %4, !tbaa !2
  %181 = load i16, ptr %8, !tbaa !2
  %182 = sub i16 200, %181
  store i16 %182, ptr %1, !tbaa !2
  call cc 1000 addrspace(1) void @RENDER(ptr %7, ptr %5, ptr %4, ptr %1)
  %183 = call cc 1000 addrspace(1) ptr @llrm.qb.B$INKY()
  %184 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %183, ptr @$string8)
  %185 = icmp sgt i16 %184, 0
  br i1 %185, label %b35, label %b36

b10:
  %186 = load i16, ptr %18, !tbaa !2
  %187 = icmp sge i16 %186, 0
  br i1 %187, label %b11, label %b12

b11:
  %188 = load i16, ptr %20, !tbaa !2
  %189 = load i16, ptr %19, !tbaa !2
  %190 = icmp sle i16 %188, %189
  br i1 %190, label %b13, label %b14

b12:
  %191 = load i16, ptr %20, !tbaa !2
  %192 = load i16, ptr %19, !tbaa !2
  %193 = icmp sge i16 %191, %192
  br i1 %193, label %b13, label %b14

b13:
  store i16 0, ptr %17, !tbaa !2
  store i16 159, ptr %16, !tbaa !2
  store i16 1, ptr %15, !tbaa !2
  br label %b15

b14:
  store i16 0, ptr %21, !tbaa !2
  store i16 0, ptr %17, !tbaa !2
  store i16 32000, ptr %14, !tbaa !2
  store i16 1, ptr %13, !tbaa !2
  br label %b20

b15:
  %194 = load i16, ptr %15, !tbaa !2
  %195 = icmp sge i16 %194, 0
  br i1 %195, label %b16, label %b17

b16:
  %196 = load i16, ptr %17, !tbaa !2
  %197 = load i16, ptr %16, !tbaa !2
  %198 = icmp sle i16 %196, %197
  br i1 %198, label %b18, label %b19

b17:
  %199 = load i16, ptr %17, !tbaa !2
  %200 = load i16, ptr %16, !tbaa !2
  %201 = icmp sge i16 %199, %200
  br i1 %201, label %b18, label %b19

b18:
  %202 = load i16, ptr %17, !tbaa !2
  %203 = load i16, ptr %20, !tbaa !2
  %204 = mul i16 %203, 161
  %205 = add i16 %204, %202
  %206 = mul i16 %205, 2
  %207 = getelementptr i8, ptr %51, i16 2
  %208 = load i16, ptr %207, !tbaa !2
  %209 = add i16 0, %206
  %210 = inttoptr i16 %208 to ptr addrspace(2)
  %211 = addrspacecast ptr addrspace(2) %210 to ptr addrspace(1)
  %212 = getelementptr i8, ptr addrspace(1) %211, i16 %209
  %213 = load i16, ptr %21, !tbaa !2
  %214 = load i16, ptr @b$seg, !tbaa !2
  %215 = inttoptr i16 %214 to ptr addrspace(2)
  %216 = addrspacecast ptr addrspace(2) %215 to ptr addrspace(1)
  %217 = getelementptr i8, ptr addrspace(1) %216, i16 %213
  %218 = load volatile i8, ptr addrspace(1) %217
  %219 = zext i8 %218 to i16
  store i16 %219, ptr addrspace(1) %212, !tbaa !8
  %220 = load i16, ptr %21, !tbaa !2
  %221 = add i16 %220, 1
  store i16 %221, ptr %21, !tbaa !2
  %222 = load i16, ptr %17, !tbaa !2
  %223 = load i16, ptr %15, !tbaa !2
  %224 = add nsw i16 %222, %223
  store i16 %224, ptr %17, !tbaa !2
  br label %b15

b19:
  %225 = load i16, ptr %21, !tbaa !2
  %226 = add i16 %225, 160
  store i16 %226, ptr %21, !tbaa !2
  %227 = load i16, ptr %20, !tbaa !2
  %228 = load i16, ptr %18, !tbaa !2
  %229 = add nsw i16 %227, %228
  store i16 %229, ptr %20, !tbaa !2
  br label %b10

b20:
  %230 = load i16, ptr %13, !tbaa !2
  %231 = icmp sge i16 %230, 0
  br i1 %231, label %b21, label %b22

b21:
  %232 = load i16, ptr %17, !tbaa !2
  %233 = load i16, ptr %14, !tbaa !2
  %234 = icmp sle i16 %232, %233
  br i1 %234, label %b23, label %b24

b22:
  %235 = load i16, ptr %17, !tbaa !2
  %236 = load i16, ptr %14, !tbaa !2
  %237 = icmp sge i16 %235, %236
  br i1 %237, label %b23, label %b24

b23:
  %238 = load i16, ptr %17, !tbaa !2
  %239 = mul i16 %238, 2
  %240 = getelementptr i8, ptr @"FRACTAL2%", i16 2
  %241 = load i16, ptr %240, !tbaa !2
  %242 = add i16 0, %239
  %243 = inttoptr i16 %241 to ptr addrspace(2)
  %244 = addrspacecast ptr addrspace(2) %243 to ptr addrspace(1)
  %245 = getelementptr i8, ptr addrspace(1) %244, i16 %242
  %246 = load i16, ptr %17, !tbaa !2
  %247 = mul i16 %246, 2
  %248 = getelementptr i8, ptr @"FRACTAL1%", i16 2
  %249 = load i16, ptr %248, !tbaa !2
  %250 = add i16 0, %247
  %251 = inttoptr i16 %249 to ptr addrspace(2)
  %252 = addrspacecast ptr addrspace(2) %251 to ptr addrspace(1)
  %253 = getelementptr i8, ptr addrspace(1) %252, i16 %250
  %254 = load i16, ptr addrspace(1) %253, !tbaa !10
  store i16 %254, ptr addrspace(1) %245, !tbaa !12
  %255 = load i16, ptr %17, !tbaa !2
  %256 = mul i16 %255, 2
  %257 = getelementptr i8, ptr @"FRACTAL1%", i16 2
  %258 = load i16, ptr %257, !tbaa !2
  %259 = add i16 0, %256
  %260 = inttoptr i16 %258 to ptr addrspace(2)
  %261 = addrspacecast ptr addrspace(2) %260 to ptr addrspace(1)
  %262 = getelementptr i8, ptr addrspace(1) %261, i16 %259
  store i16 0, ptr addrspace(1) %262, !tbaa !10
  %263 = load i16, ptr %17, !tbaa !2
  %264 = load i16, ptr %13, !tbaa !2
  %265 = add nsw i16 %263, %264
  store i16 %265, ptr %17, !tbaa !2
  br label %b20

b24:
  store i16 0, ptr %20, !tbaa !2
  store i16 99, ptr %12, !tbaa !2
  store i16 1, ptr %11, !tbaa !2
  br label %b25

b25:
  %266 = load i16, ptr %11, !tbaa !2
  %267 = icmp sge i16 %266, 0
  br i1 %267, label %b26, label %b27

b26:
  %268 = load i16, ptr %20, !tbaa !2
  %269 = load i16, ptr %12, !tbaa !2
  %270 = icmp sle i16 %268, %269
  br i1 %270, label %b28, label %b29

b27:
  %271 = load i16, ptr %20, !tbaa !2
  %272 = load i16, ptr %12, !tbaa !2
  %273 = icmp sge i16 %271, %272
  br i1 %273, label %b28, label %b29

b28:
  store i16 0, ptr %17, !tbaa !2
  store i16 159, ptr %10, !tbaa !2
  store i16 1, ptr %9, !tbaa !2
  br label %b30

b29:
  br label %b9

b30:
  %274 = load i16, ptr %9, !tbaa !2
  %275 = icmp sge i16 %274, 0
  br i1 %275, label %b31, label %b32

b31:
  %276 = load i16, ptr %17, !tbaa !2
  %277 = load i16, ptr %10, !tbaa !2
  %278 = icmp sle i16 %276, %277
  br i1 %278, label %b33, label %b34

b32:
  %279 = load i16, ptr %17, !tbaa !2
  %280 = load i16, ptr %10, !tbaa !2
  %281 = icmp sge i16 %279, %280
  br i1 %281, label %b33, label %b34

b33:
  %282 = load i16, ptr %17, !tbaa !2
  %283 = mul i16 %282, 2
  %284 = load i16, ptr %20, !tbaa !2
  %285 = mul i16 %284, 640
  %286 = add i16 %283, %285
  %287 = load i16, ptr %17, !tbaa !2
  %288 = load i16, ptr %20, !tbaa !2
  %289 = mul i16 %288, 161
  %290 = add i16 %289, %287
  %291 = mul i16 %290, 2
  %292 = getelementptr i8, ptr %51, i16 2
  %293 = load i16, ptr %292, !tbaa !2
  %294 = add i16 0, %291
  %295 = inttoptr i16 %293 to ptr addrspace(2)
  %296 = addrspacecast ptr addrspace(2) %295 to ptr addrspace(1)
  %297 = getelementptr i8, ptr addrspace(1) %296, i16 %294
  %298 = load i16, ptr addrspace(1) %297, !tbaa !8
  %299 = trunc i16 %298 to i8
  %300 = load i16, ptr @b$seg, !tbaa !2
  %301 = inttoptr i16 %300 to ptr addrspace(2)
  %302 = addrspacecast ptr addrspace(2) %301 to ptr addrspace(1)
  %303 = getelementptr i8, ptr addrspace(1) %302, i16 %286
  store volatile i8 %299, ptr addrspace(1) %303
  %304 = load i16, ptr %17, !tbaa !2
  %305 = load i16, ptr %9, !tbaa !2
  %306 = add nsw i16 %304, %305
  store i16 %306, ptr %17, !tbaa !2
  br label %b30

b34:
  %307 = load i16, ptr %20, !tbaa !2
  %308 = load i16, ptr %11, !tbaa !2
  %309 = add nsw i16 %307, %308
  store i16 %309, ptr %20, !tbaa !2
  br label %b25

b35:
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %51)
  ret void

b36:
  br label %b37

b37:
  %310 = load i16, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %311 = add i16 %310, 1
  store i16 %311, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %312 = load i16, ptr %47, !tbaa !2
  %313 = load i16, ptr %45, !tbaa !2
  %314 = add nsw i16 %312, %313
  store i16 %314, ptr %47, !tbaa !2
  br label %b2
}

define cc 1000 void @OHCANADA(ptr dereferenceable(2) %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca i16
  store i16 0, ptr %9, !tbaa !2
  store i16 255, ptr %8, !tbaa !2
  store i16 1, ptr %7, !tbaa !2
  br label %b2

b2:
  %10 = load i16, ptr %7, !tbaa !2
  %11 = icmp sge i16 %10, 0
  br i1 %11, label %b3, label %b4

b3:
  %12 = load i16, ptr %9, !tbaa !2
  %13 = load i16, ptr %8, !tbaa !2
  %14 = icmp sle i16 %12, %13
  br i1 %14, label %b5, label %b6

b4:
  %15 = load i16, ptr %9, !tbaa !2
  %16 = load i16, ptr %8, !tbaa !2
  %17 = icmp sge i16 %15, %16
  br i1 %17, label %b5, label %b6

b5:
  %18 = load i16, ptr %9, !tbaa !2
  %19 = trunc i16 %18 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %19)
  call void @llrm.ia16.out.i8(i16 969, i8 63)
  call void @llrm.ia16.out.i8(i16 969, i8 63)
  call void @llrm.ia16.out.i8(i16 969, i8 63)
  %20 = load i16, ptr %9, !tbaa !2
  %21 = load i16, ptr %7, !tbaa !2
  %22 = add nsw i16 %20, %21
  store i16 %22, ptr %9, !tbaa !2
  br label %b2

b6:
  store i16 -24576, ptr @b$seg, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$BLOD(ptr @$string9, i16 0, i16 1)
  store i16 0, ptr %9, !tbaa !2
  store i16 255, ptr %6, !tbaa !2
  store i16 1, ptr %5, !tbaa !2
  br label %b7

b7:
  %23 = load i16, ptr %5, !tbaa !2
  %24 = icmp sge i16 %23, 0
  br i1 %24, label %b8, label %b9

b8:
  %25 = load i16, ptr %9, !tbaa !2
  %26 = load i16, ptr %6, !tbaa !2
  %27 = icmp sle i16 %25, %26
  br i1 %27, label %b10, label %b11

b9:
  %28 = load i16, ptr %9, !tbaa !2
  %29 = load i16, ptr %6, !tbaa !2
  %30 = icmp sge i16 %28, %29
  br i1 %30, label %b10, label %b11

b10:
  %31 = load i16, ptr %9, !tbaa !2
  %32 = trunc i16 %31 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %32)
  call void @llrm.ia16.out.i8(i16 969, i8 63)
  %33 = load i16, ptr %9, !tbaa !2
  %34 = sext i16 %33 to i32
  %35 = sdiv i32 %34, 4
  %36 = trunc i32 %35 to i16
  %37 = trunc i16 %36 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %37)
  %38 = load i16, ptr %9, !tbaa !2
  %39 = sext i16 %38 to i32
  %40 = sdiv i32 %39, 4
  %41 = trunc i32 %40 to i16
  %42 = trunc i16 %41 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %42)
  %43 = load i16, ptr %9, !tbaa !2
  %44 = load i16, ptr %5, !tbaa !2
  %45 = add nsw i16 %43, %44
  store i16 %45, ptr %9, !tbaa !2
  br label %b7

b11:
  store i16 256, ptr %4, !tbaa !2
  call cc 1000 addrspace(1) void @UNWHITEFADE(ptr %4)
  %46 = load i16, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %47 = add i16 %46, 16
  store i16 %47, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  store i16 1, ptr %3, !tbaa !2
  %48 = load i16, ptr %0
  store i16 %48, ptr %2, !tbaa !2
  store i16 1, ptr %1, !tbaa !2
  br label %b12

b12:
  %49 = load i16, ptr %1, !tbaa !2
  %50 = icmp sge i16 %49, 0
  br i1 %50, label %b13, label %b14

b13:
  %51 = load i16, ptr %3, !tbaa !2
  %52 = load i16, ptr %2, !tbaa !2
  %53 = icmp sle i16 %51, %52
  br i1 %53, label %b15, label %b16

b14:
  %54 = load i16, ptr %3, !tbaa !2
  %55 = load i16, ptr %2, !tbaa !2
  %56 = icmp sge i16 %54, %55
  br i1 %56, label %b15, label %b16

b15:
  %57 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %57, ptr @"BENCHFRAME&", !tbaa !2
  %58 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %58, ptr @"BENCHFRAME&", !tbaa !2
  %59 = load i16, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %60 = add i16 %59, 1
  store i16 %60, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %61 = call cc 1000 addrspace(1) ptr @llrm.qb.B$INKY()
  %62 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %61, ptr @$string10)
  %63 = icmp sgt i16 %62, 0
  br i1 %63, label %b17, label %b18

b16:
  ret void

b17:
  ret void

b18:
  br label %b19

b19:
  %64 = load i16, ptr %3, !tbaa !2
  %65 = load i16, ptr %1, !tbaa !2
  %66 = add nsw i16 %64, %65
  store i16 %66, ptr %3, !tbaa !2
  br label %b12
}

define cc 1000 void @PLASMA(ptr dereferenceable(2) %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca i16
  %14 = alloca i16
  %15 = alloca i16
  %16 = alloca i16
  %17 = alloca i16
  %18 = alloca i16
  %19 = alloca i16
  %20 = alloca i16
  %21 = alloca i16
  %22 = alloca i16
  %23 = alloca i16
  %24 = alloca i16
  %25 = alloca i16
  %26 = alloca i16
  %27 = alloca [22 x i8]
  %28 = alloca [18 x i8]
  %29 = alloca [18 x i8]
  %30 = alloca [18 x i8]
  store i16 0, ptr %30, !tbaa !2
  %31 = getelementptr inbounds i8, ptr %30, i16 2
  store i16 0, ptr %31, !tbaa !2
  %32 = getelementptr inbounds i8, ptr %30, i16 4
  store i16 0, ptr %32, !tbaa !2
  %33 = getelementptr inbounds i8, ptr %30, i16 6
  store i16 0, ptr %33, !tbaa !2
  %34 = getelementptr inbounds i8, ptr %30, i16 8
  store i16 0, ptr %34, !tbaa !2
  %35 = getelementptr inbounds i8, ptr %30, i16 10
  store i16 0, ptr %35, !tbaa !2
  %36 = getelementptr inbounds i8, ptr %30, i16 12
  store i16 0, ptr %36, !tbaa !2
  %37 = getelementptr inbounds i8, ptr %30, i16 14
  store i16 0, ptr %37, !tbaa !2
  %38 = getelementptr inbounds i8, ptr %30, i16 16
  store i16 0, ptr %38, !tbaa !2
  store i16 0, ptr %29, !tbaa !2
  %39 = getelementptr inbounds i8, ptr %29, i16 2
  store i16 0, ptr %39, !tbaa !2
  %40 = getelementptr inbounds i8, ptr %29, i16 4
  store i16 0, ptr %40, !tbaa !2
  %41 = getelementptr inbounds i8, ptr %29, i16 6
  store i16 0, ptr %41, !tbaa !2
  %42 = getelementptr inbounds i8, ptr %29, i16 8
  store i16 0, ptr %42, !tbaa !2
  %43 = getelementptr inbounds i8, ptr %29, i16 10
  store i16 0, ptr %43, !tbaa !2
  %44 = getelementptr inbounds i8, ptr %29, i16 12
  store i16 0, ptr %44, !tbaa !2
  %45 = getelementptr inbounds i8, ptr %29, i16 14
  store i16 0, ptr %45, !tbaa !2
  %46 = getelementptr inbounds i8, ptr %29, i16 16
  store i16 0, ptr %46, !tbaa !2
  store i16 0, ptr %28, !tbaa !2
  %47 = getelementptr inbounds i8, ptr %28, i16 2
  store i16 0, ptr %47, !tbaa !2
  %48 = getelementptr inbounds i8, ptr %28, i16 4
  store i16 0, ptr %48, !tbaa !2
  %49 = getelementptr inbounds i8, ptr %28, i16 6
  store i16 0, ptr %49, !tbaa !2
  %50 = getelementptr inbounds i8, ptr %28, i16 8
  store i16 0, ptr %50, !tbaa !2
  %51 = getelementptr inbounds i8, ptr %28, i16 10
  store i16 0, ptr %51, !tbaa !2
  %52 = getelementptr inbounds i8, ptr %28, i16 12
  store i16 0, ptr %52, !tbaa !2
  %53 = getelementptr inbounds i8, ptr %28, i16 14
  store i16 0, ptr %53, !tbaa !2
  %54 = getelementptr inbounds i8, ptr %28, i16 16
  store i16 0, ptr %54, !tbaa !2
  store i16 0, ptr %27, !tbaa !2
  %55 = getelementptr inbounds i8, ptr %27, i16 2
  store i16 0, ptr %55, !tbaa !2
  %56 = getelementptr inbounds i8, ptr %27, i16 4
  store i16 0, ptr %56, !tbaa !2
  %57 = getelementptr inbounds i8, ptr %27, i16 6
  store i16 0, ptr %57, !tbaa !2
  %58 = getelementptr inbounds i8, ptr %27, i16 8
  store i16 0, ptr %58, !tbaa !2
  %59 = getelementptr inbounds i8, ptr %27, i16 10
  store i16 0, ptr %59, !tbaa !2
  %60 = getelementptr inbounds i8, ptr %27, i16 12
  store i16 0, ptr %60, !tbaa !2
  %61 = getelementptr inbounds i8, ptr %27, i16 14
  store i16 0, ptr %61, !tbaa !2
  %62 = getelementptr inbounds i8, ptr %27, i16 16
  store i16 0, ptr %62, !tbaa !2
  %63 = getelementptr inbounds i8, ptr %27, i16 18
  store i16 0, ptr %63, !tbaa !2
  %64 = getelementptr inbounds i8, ptr %27, i16 20
  store i16 0, ptr %64, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 320, i16 2, i16 257, ptr %30)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 320, i16 2, i16 257, ptr %29)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 512, i16 2, i16 257, ptr %28)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 128, i16 0, i16 128, i16 2, i16 258, ptr %27)
  store i16 -24576, ptr @b$seg, !tbaa !2
  store i16 0, ptr %26, !tbaa !2
  store i16 512, ptr %25, !tbaa !2
  store i16 1, ptr %24, !tbaa !2
  br label %b2

b2:
  %65 = load i16, ptr %24, !tbaa !2
  %66 = icmp sge i16 %65, 0
  br i1 %66, label %b3, label %b4

b3:
  %67 = load i16, ptr %26, !tbaa !2
  %68 = load i16, ptr %25, !tbaa !2
  %69 = icmp sle i16 %67, %68
  br i1 %69, label %b5, label %b6

b4:
  %70 = load i16, ptr %26, !tbaa !2
  %71 = load i16, ptr %25, !tbaa !2
  %72 = icmp sge i16 %70, %71
  br i1 %72, label %b5, label %b6

b5:
  %73 = load i16, ptr %26, !tbaa !2
  %74 = mul i16 %73, 2
  %75 = getelementptr i8, ptr %28, i16 2
  %76 = load i16, ptr %75, !tbaa !2
  %77 = add i16 0, %74
  %78 = inttoptr i16 %76 to ptr addrspace(2)
  %79 = addrspacecast ptr addrspace(2) %78 to ptr addrspace(1)
  %80 = getelementptr i8, ptr addrspace(1) %79, i16 %77
  %81 = load i16, ptr %26, !tbaa !2
  %82 = load float, ptr @$float11, !tbaa !2
  store i16 %81, ptr %23, !tbaa !2
  %83 = load i16, ptr %23, !tbaa !2
  %84 = sitofp i16 %83 to float
  %85 = fmul float %84, %82
  store i16 256, ptr %22, !tbaa !2
  %86 = load i16, ptr %22, !tbaa !2
  %87 = sitofp i16 %86 to float
  %88 = fdiv float %85, %87
  %89 = call float @llvm.fabs.f32(float %88)
  %90 = load float, ptr @$float12, !tbaa !2
  %91 = fcmp ogt float %89, %90
  br i1 %91, label %b7, label %b8

b6:
  store i16 1, ptr %19, !tbaa !2
  %92 = load i16, ptr %0
  store i16 %92, ptr %18, !tbaa !2
  store i16 1, ptr %17, !tbaa !2
  br label %b9

b7:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b8:
  %93 = call float @llvm.sin.f32(float %88)
  store i16 32, ptr %21, !tbaa !2
  %94 = load i16, ptr %21, !tbaa !2
  %95 = sitofp i16 %94 to float
  %96 = fmul float %93, %95
  store i16 32, ptr %20, !tbaa !2
  %97 = load i16, ptr %20, !tbaa !2
  %98 = sitofp i16 %97 to float
  %99 = fadd float %96, %98
  %100 = call i16 @llvm.lrint.i16.f32(float %99)
  store i16 %100, ptr addrspace(1) %80, !tbaa !14
  %101 = load i16, ptr %26, !tbaa !2
  %102 = load i16, ptr %24, !tbaa !2
  %103 = add nsw i16 %101, %102
  store i16 %103, ptr %26, !tbaa !2
  br label %b2

b9:
  %104 = load i16, ptr %17, !tbaa !2
  %105 = icmp sge i16 %104, 0
  br i1 %105, label %b10, label %b11

b10:
  %106 = load i16, ptr %19, !tbaa !2
  %107 = load i16, ptr %18, !tbaa !2
  %108 = icmp sle i16 %106, %107
  br i1 %108, label %b12, label %b13

b11:
  %109 = load i16, ptr %19, !tbaa !2
  %110 = load i16, ptr %18, !tbaa !2
  %111 = icmp sge i16 %109, %110
  br i1 %111, label %b12, label %b13

b12:
  store i16 0, ptr %26, !tbaa !2
  store i16 320, ptr %16, !tbaa !2
  store i16 1, ptr %15, !tbaa !2
  br label %b14

b13:
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %27)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %28)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %30)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %29)
  ret void

b14:
  %112 = load i16, ptr %15, !tbaa !2
  %113 = icmp sge i16 %112, 0
  br i1 %113, label %b15, label %b16

b15:
  %114 = load i16, ptr %26, !tbaa !2
  %115 = load i16, ptr %16, !tbaa !2
  %116 = icmp sle i16 %114, %115
  br i1 %116, label %b17, label %b18

b16:
  %117 = load i16, ptr %26, !tbaa !2
  %118 = load i16, ptr %16, !tbaa !2
  %119 = icmp sge i16 %117, %118
  br i1 %119, label %b17, label %b18

b17:
  %120 = load i16, ptr %26, !tbaa !2
  %121 = mul i16 %120, 2
  %122 = getelementptr i8, ptr %30, i16 2
  %123 = load i16, ptr %122, !tbaa !2
  %124 = add i16 0, %121
  %125 = inttoptr i16 %123 to ptr addrspace(2)
  %126 = addrspacecast ptr addrspace(2) %125 to ptr addrspace(1)
  %127 = getelementptr i8, ptr addrspace(1) %126, i16 %124
  %128 = load i16, ptr %26, !tbaa !2
  %129 = load i16, ptr %19, !tbaa !2
  %130 = add i16 %128, %129
  %131 = and i16 %130, 511
  %132 = mul i16 %131, 2
  %133 = getelementptr i8, ptr %28, i16 2
  %134 = load i16, ptr %133, !tbaa !2
  %135 = add i16 0, %132
  %136 = inttoptr i16 %134 to ptr addrspace(2)
  %137 = addrspacecast ptr addrspace(2) %136 to ptr addrspace(1)
  %138 = getelementptr i8, ptr addrspace(1) %137, i16 %135
  %139 = load i16, ptr addrspace(1) %138, !tbaa !14
  %140 = load i16, ptr %26, !tbaa !2
  %141 = mul i16 3, %140
  %142 = load i16, ptr %19, !tbaa !2
  %143 = mul i16 7, %142
  %144 = add i16 %141, %143
  %145 = add i16 %144, 3
  %146 = and i16 %145, 511
  %147 = mul i16 %146, 2
  %148 = getelementptr i8, ptr %28, i16 2
  %149 = load i16, ptr %148, !tbaa !2
  %150 = add i16 0, %147
  %151 = inttoptr i16 %149 to ptr addrspace(2)
  %152 = addrspacecast ptr addrspace(2) %151 to ptr addrspace(1)
  %153 = getelementptr i8, ptr addrspace(1) %152, i16 %150
  %154 = load i16, ptr addrspace(1) %153, !tbaa !14
  %155 = add i16 %139, %154
  store i16 %155, ptr addrspace(1) %127, !tbaa !16
  %156 = load i16, ptr %26, !tbaa !2
  %157 = load i16, ptr %15, !tbaa !2
  %158 = add nsw i16 %156, %157
  store i16 %158, ptr %26, !tbaa !2
  br label %b14

b18:
  store i16 0, ptr %14, !tbaa !2
  store i16 0, ptr %13, !tbaa !2
  store i16 128, ptr %12, !tbaa !2
  store i16 1, ptr %11, !tbaa !2
  br label %b19

b19:
  %159 = load i16, ptr %11, !tbaa !2
  %160 = icmp sge i16 %159, 0
  br i1 %160, label %b20, label %b21

b20:
  %161 = load i16, ptr %13, !tbaa !2
  %162 = load i16, ptr %12, !tbaa !2
  %163 = icmp sle i16 %161, %162
  br i1 %163, label %b22, label %b23

b21:
  %164 = load i16, ptr %13, !tbaa !2
  %165 = load i16, ptr %12, !tbaa !2
  %166 = icmp sge i16 %164, %165
  br i1 %166, label %b22, label %b23

b22:
  %167 = load i16, ptr %13, !tbaa !2
  %168 = mul i16 %167, 7
  %169 = load i16, ptr %19, !tbaa !2
  %170 = mul i16 %169, 5
  %171 = add i16 %168, %170
  %172 = and i16 %171, 511
  %173 = mul i16 %172, 2
  %174 = getelementptr i8, ptr %28, i16 2
  %175 = load i16, ptr %174, !tbaa !2
  %176 = add i16 0, %173
  %177 = inttoptr i16 %175 to ptr addrspace(2)
  %178 = addrspacecast ptr addrspace(2) %177 to ptr addrspace(1)
  %179 = getelementptr i8, ptr addrspace(1) %178, i16 %176
  %180 = load i16, ptr addrspace(1) %179, !tbaa !14
  %181 = load i16, ptr %13, !tbaa !2
  %182 = mul i16 %181, 14
  %183 = load i16, ptr %19, !tbaa !2
  %184 = mul i16 %183, 11
  %185 = add i16 %182, %184
  %186 = add i16 %185, 1943
  %187 = and i16 %186, 511
  %188 = mul i16 %187, 2
  %189 = getelementptr i8, ptr %28, i16 2
  %190 = load i16, ptr %189, !tbaa !2
  %191 = add i16 0, %188
  %192 = inttoptr i16 %190 to ptr addrspace(2)
  %193 = addrspacecast ptr addrspace(2) %192 to ptr addrspace(1)
  %194 = getelementptr i8, ptr addrspace(1) %193, i16 %191
  %195 = load i16, ptr addrspace(1) %194, !tbaa !14
  %196 = add i16 %180, %195
  store i16 %196, ptr %10, !tbaa !2
  store i16 0, ptr %26, !tbaa !2
  store i16 128, ptr %9, !tbaa !2
  store i16 1, ptr %8, !tbaa !2
  br label %b24

b23:
  store i16 0, ptr %26, !tbaa !2
  store i16 320, ptr %7, !tbaa !2
  store i16 1, ptr %6, !tbaa !2
  br label %b29

b24:
  %197 = load i16, ptr %8, !tbaa !2
  %198 = icmp sge i16 %197, 0
  br i1 %198, label %b25, label %b26

b25:
  %199 = load i16, ptr %26, !tbaa !2
  %200 = load i16, ptr %9, !tbaa !2
  %201 = icmp sle i16 %199, %200
  br i1 %201, label %b27, label %b28

b26:
  %202 = load i16, ptr %26, !tbaa !2
  %203 = load i16, ptr %9, !tbaa !2
  %204 = icmp sge i16 %202, %203
  br i1 %204, label %b27, label %b28

b27:
  %205 = load i16, ptr %26, !tbaa !2
  %206 = load i16, ptr %13, !tbaa !2
  %207 = mul i16 %206, 129
  %208 = add i16 %207, %205
  %209 = mul i16 %208, 2
  %210 = getelementptr i8, ptr %27, i16 2
  %211 = load i16, ptr %210, !tbaa !2
  %212 = add i16 0, %209
  %213 = inttoptr i16 %211 to ptr addrspace(2)
  %214 = addrspacecast ptr addrspace(2) %213 to ptr addrspace(1)
  %215 = getelementptr i8, ptr addrspace(1) %214, i16 %212
  %216 = load i16, ptr %26, !tbaa !2
  %217 = mul i16 %216, 2
  %218 = getelementptr i8, ptr %30, i16 2
  %219 = load i16, ptr %218, !tbaa !2
  %220 = add i16 0, %217
  %221 = inttoptr i16 %219 to ptr addrspace(2)
  %222 = addrspacecast ptr addrspace(2) %221 to ptr addrspace(1)
  %223 = getelementptr i8, ptr addrspace(1) %222, i16 %220
  %224 = load i16, ptr addrspace(1) %223, !tbaa !16
  %225 = load i16, ptr %10, !tbaa !2
  %226 = add i16 %224, %225
  store i16 %226, ptr addrspace(1) %215, !tbaa !18
  %227 = load i16, ptr %14, !tbaa !2
  %228 = add i16 %227, 1
  store i16 %228, ptr %14, !tbaa !2
  %229 = load i16, ptr %26, !tbaa !2
  %230 = load i16, ptr %8, !tbaa !2
  %231 = add nsw i16 %229, %230
  store i16 %231, ptr %26, !tbaa !2
  br label %b24

b28:
  %232 = load i16, ptr %13, !tbaa !2
  %233 = load i16, ptr %11, !tbaa !2
  %234 = add nsw i16 %232, %233
  store i16 %234, ptr %13, !tbaa !2
  br label %b19

b29:
  %235 = load i16, ptr %6, !tbaa !2
  %236 = icmp sge i16 %235, 0
  br i1 %236, label %b30, label %b31

b30:
  %237 = load i16, ptr %26, !tbaa !2
  %238 = load i16, ptr %7, !tbaa !2
  %239 = icmp sle i16 %237, %238
  br i1 %239, label %b32, label %b33

b31:
  %240 = load i16, ptr %26, !tbaa !2
  %241 = load i16, ptr %7, !tbaa !2
  %242 = icmp sge i16 %240, %241
  br i1 %242, label %b32, label %b33

b32:
  %243 = load i16, ptr %26, !tbaa !2
  %244 = mul i16 %243, 2
  %245 = getelementptr i8, ptr %30, i16 2
  %246 = load i16, ptr %245, !tbaa !2
  %247 = add i16 0, %244
  %248 = inttoptr i16 %246 to ptr addrspace(2)
  %249 = addrspacecast ptr addrspace(2) %248 to ptr addrspace(1)
  %250 = getelementptr i8, ptr addrspace(1) %249, i16 %247
  %251 = load i16, ptr %26, !tbaa !2
  %252 = mul i16 %251, 11
  %253 = load i16, ptr %19, !tbaa !2
  %254 = mul i16 %253, 7
  %255 = add i16 %252, %254
  %256 = and i16 %255, 511
  %257 = mul i16 %256, 2
  %258 = getelementptr i8, ptr %28, i16 2
  %259 = load i16, ptr %258, !tbaa !2
  %260 = add i16 0, %257
  %261 = inttoptr i16 %259 to ptr addrspace(2)
  %262 = addrspacecast ptr addrspace(2) %261 to ptr addrspace(1)
  %263 = getelementptr i8, ptr addrspace(1) %262, i16 %260
  %264 = load i16, ptr addrspace(1) %263, !tbaa !14
  %265 = load i16, ptr %26, !tbaa !2
  %266 = mul i16 3, %265
  %267 = load i16, ptr %19, !tbaa !2
  %268 = mul i16 7, %267
  %269 = add i16 %266, %268
  %270 = add i16 %269, 3
  %271 = and i16 %270, 511
  %272 = mul i16 %271, 2
  %273 = getelementptr i8, ptr %28, i16 2
  %274 = load i16, ptr %273, !tbaa !2
  %275 = add i16 0, %272
  %276 = inttoptr i16 %274 to ptr addrspace(2)
  %277 = addrspacecast ptr addrspace(2) %276 to ptr addrspace(1)
  %278 = getelementptr i8, ptr addrspace(1) %277, i16 %275
  %279 = load i16, ptr addrspace(1) %278, !tbaa !14
  %280 = add i16 %264, %279
  store i16 %280, ptr addrspace(1) %250, !tbaa !16
  %281 = load i16, ptr %26, !tbaa !2
  %282 = mul i16 %281, 2
  %283 = getelementptr i8, ptr %29, i16 2
  %284 = load i16, ptr %283, !tbaa !2
  %285 = add i16 0, %282
  %286 = inttoptr i16 %284 to ptr addrspace(2)
  %287 = addrspacecast ptr addrspace(2) %286 to ptr addrspace(1)
  %288 = getelementptr i8, ptr addrspace(1) %287, i16 %285
  %289 = load i16, ptr %26, !tbaa !2
  %290 = mul i16 %289, 4
  %291 = load i16, ptr %19, !tbaa !2
  %292 = mul i16 %291, 5
  %293 = add i16 %290, %292
  %294 = and i16 %293, 511
  %295 = mul i16 %294, 2
  %296 = getelementptr i8, ptr %28, i16 2
  %297 = load i16, ptr %296, !tbaa !2
  %298 = add i16 0, %295
  %299 = inttoptr i16 %297 to ptr addrspace(2)
  %300 = addrspacecast ptr addrspace(2) %299 to ptr addrspace(1)
  %301 = getelementptr i8, ptr addrspace(1) %300, i16 %298
  %302 = load i16, ptr addrspace(1) %301, !tbaa !14
  %303 = load i16, ptr %26, !tbaa !2
  %304 = mul i16 9, %303
  %305 = load i16, ptr %19, !tbaa !2
  %306 = mul i16 2, %305
  %307 = add i16 %304, %306
  %308 = add i16 %307, 371
  %309 = and i16 %308, 511
  %310 = mul i16 %309, 2
  %311 = getelementptr i8, ptr %28, i16 2
  %312 = load i16, ptr %311, !tbaa !2
  %313 = add i16 0, %310
  %314 = inttoptr i16 %312 to ptr addrspace(2)
  %315 = addrspacecast ptr addrspace(2) %314 to ptr addrspace(1)
  %316 = getelementptr i8, ptr addrspace(1) %315, i16 %313
  %317 = load i16, ptr addrspace(1) %316, !tbaa !14
  %318 = add i16 %302, %317
  store i16 %318, ptr addrspace(1) %288, !tbaa !20
  %319 = load i16, ptr %26, !tbaa !2
  %320 = load i16, ptr %6, !tbaa !2
  %321 = add nsw i16 %319, %320
  store i16 %321, ptr %26, !tbaa !2
  br label %b29

b33:
  store i16 0, ptr %14, !tbaa !2
  store i16 0, ptr %13, !tbaa !2
  store i16 199, ptr %5, !tbaa !2
  store i16 1, ptr %4, !tbaa !2
  br label %b34

b34:
  %322 = load i16, ptr %4, !tbaa !2
  %323 = icmp sge i16 %322, 0
  br i1 %323, label %b35, label %b36

b35:
  %324 = load i16, ptr %13, !tbaa !2
  %325 = load i16, ptr %5, !tbaa !2
  %326 = icmp sle i16 %324, %325
  br i1 %326, label %b37, label %b38

b36:
  %327 = load i16, ptr %13, !tbaa !2
  %328 = load i16, ptr %5, !tbaa !2
  %329 = icmp sge i16 %327, %328
  br i1 %329, label %b37, label %b38

b37:
  %330 = load i16, ptr %13, !tbaa !2
  %331 = mul i16 %330, 11
  %332 = load i16, ptr %19, !tbaa !2
  %333 = mul i16 %332, 6
  %334 = add i16 %331, %333
  %335 = and i16 %334, 511
  %336 = mul i16 %335, 2
  %337 = getelementptr i8, ptr %28, i16 2
  %338 = load i16, ptr %337, !tbaa !2
  %339 = add i16 0, %336
  %340 = inttoptr i16 %338 to ptr addrspace(2)
  %341 = addrspacecast ptr addrspace(2) %340 to ptr addrspace(1)
  %342 = getelementptr i8, ptr addrspace(1) %341, i16 %339
  %343 = load i16, ptr addrspace(1) %342, !tbaa !14
  %344 = load i16, ptr %13, !tbaa !2
  %345 = mul i16 %344, 14
  %346 = load i16, ptr %19, !tbaa !2
  %347 = mul i16 %346, 11
  %348 = add i16 %345, %347
  %349 = add i16 %348, 1943
  %350 = and i16 %349, 511
  %351 = mul i16 %350, 2
  %352 = getelementptr i8, ptr %28, i16 2
  %353 = load i16, ptr %352, !tbaa !2
  %354 = add i16 0, %351
  %355 = inttoptr i16 %353 to ptr addrspace(2)
  %356 = addrspacecast ptr addrspace(2) %355 to ptr addrspace(1)
  %357 = getelementptr i8, ptr addrspace(1) %356, i16 %354
  %358 = load i16, ptr addrspace(1) %357, !tbaa !14
  %359 = add i16 %343, %358
  store i16 %359, ptr %10, !tbaa !2
  %360 = load i16, ptr %13, !tbaa !2
  %361 = mul i16 %360, 9
  %362 = load i16, ptr %19, !tbaa !2
  %363 = mul i16 %362, 4
  %364 = add i16 %361, %363
  %365 = and i16 %364, 511
  %366 = mul i16 %365, 2
  %367 = getelementptr i8, ptr %28, i16 2
  %368 = load i16, ptr %367, !tbaa !2
  %369 = add i16 0, %366
  %370 = inttoptr i16 %368 to ptr addrspace(2)
  %371 = addrspacecast ptr addrspace(2) %370 to ptr addrspace(1)
  %372 = getelementptr i8, ptr addrspace(1) %371, i16 %369
  %373 = load i16, ptr addrspace(1) %372, !tbaa !14
  %374 = load i16, ptr %13, !tbaa !2
  %375 = mul i16 %374, 17
  %376 = load i16, ptr %19, !tbaa !2
  %377 = mul i16 %376, 23
  %378 = add i16 %375, %377
  %379 = add i16 %378, 1943
  %380 = and i16 %379, 511
  %381 = mul i16 %380, 2
  %382 = getelementptr i8, ptr %28, i16 2
  %383 = load i16, ptr %382, !tbaa !2
  %384 = add i16 0, %381
  %385 = inttoptr i16 %383 to ptr addrspace(2)
  %386 = addrspacecast ptr addrspace(2) %385 to ptr addrspace(1)
  %387 = getelementptr i8, ptr addrspace(1) %386, i16 %384
  %388 = load i16, ptr addrspace(1) %387, !tbaa !14
  %389 = add i16 %373, %388
  store i16 %389, ptr %3, !tbaa !2
  store i16 0, ptr %26, !tbaa !2
  store i16 319, ptr %2, !tbaa !2
  store i16 1, ptr %1, !tbaa !2
  br label %b39

b38:
  call cc 1000 addrspace(1) void @UPDPALPLASMA(ptr %19)
  %390 = load i16, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %391 = add i16 %390, 1
  store i16 %391, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %392 = call cc 1000 addrspace(1) ptr @llrm.qb.B$INKY()
  %393 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %392, ptr @$string13)
  %394 = icmp sgt i16 %393, 0
  br i1 %394, label %b44, label %b45

b39:
  %395 = load i16, ptr %1, !tbaa !2
  %396 = icmp sge i16 %395, 0
  br i1 %396, label %b40, label %b41

b40:
  %397 = load i16, ptr %26, !tbaa !2
  %398 = load i16, ptr %2, !tbaa !2
  %399 = icmp sle i16 %397, %398
  br i1 %399, label %b42, label %b43

b41:
  %400 = load i16, ptr %26, !tbaa !2
  %401 = load i16, ptr %2, !tbaa !2
  %402 = icmp sge i16 %400, %401
  br i1 %402, label %b42, label %b43

b42:
  %403 = load i16, ptr %14, !tbaa !2
  %404 = load i16, ptr %26, !tbaa !2
  %405 = mul i16 %404, 2
  %406 = getelementptr i8, ptr %30, i16 2
  %407 = load i16, ptr %406, !tbaa !2
  %408 = add i16 0, %405
  %409 = inttoptr i16 %407 to ptr addrspace(2)
  %410 = addrspacecast ptr addrspace(2) %409 to ptr addrspace(1)
  %411 = getelementptr i8, ptr addrspace(1) %410, i16 %408
  %412 = load i16, ptr addrspace(1) %411, !tbaa !16
  %413 = load i16, ptr %10, !tbaa !2
  %414 = add i16 %412, %413
  %415 = and i16 %414, 127
  %416 = load i16, ptr %26, !tbaa !2
  %417 = mul i16 %416, 2
  %418 = getelementptr i8, ptr %29, i16 2
  %419 = load i16, ptr %418, !tbaa !2
  %420 = add i16 0, %417
  %421 = inttoptr i16 %419 to ptr addrspace(2)
  %422 = addrspacecast ptr addrspace(2) %421 to ptr addrspace(1)
  %423 = getelementptr i8, ptr addrspace(1) %422, i16 %420
  %424 = load i16, ptr addrspace(1) %423, !tbaa !20
  %425 = load i16, ptr %3, !tbaa !2
  %426 = add i16 %424, %425
  %427 = and i16 %426, 127
  %428 = mul i16 %427, 129
  %429 = add i16 %428, %415
  %430 = mul i16 %429, 2
  %431 = getelementptr i8, ptr %27, i16 2
  %432 = load i16, ptr %431, !tbaa !2
  %433 = add i16 0, %430
  %434 = inttoptr i16 %432 to ptr addrspace(2)
  %435 = addrspacecast ptr addrspace(2) %434 to ptr addrspace(1)
  %436 = getelementptr i8, ptr addrspace(1) %435, i16 %433
  %437 = load i16, ptr addrspace(1) %436, !tbaa !18
  %438 = trunc i16 %437 to i8
  %439 = load i16, ptr @b$seg, !tbaa !2
  %440 = inttoptr i16 %439 to ptr addrspace(2)
  %441 = addrspacecast ptr addrspace(2) %440 to ptr addrspace(1)
  %442 = getelementptr i8, ptr addrspace(1) %441, i16 %403
  store volatile i8 %438, ptr addrspace(1) %442, !tbaa !4
  %443 = load i16, ptr %14, !tbaa !2
  %444 = add i16 %443, 1
  store i16 %444, ptr %14, !tbaa !2
  %445 = load i16, ptr %26, !tbaa !2
  %446 = load i16, ptr %1, !tbaa !2
  %447 = add nsw i16 %445, %446
  store i16 %447, ptr %26, !tbaa !2
  br label %b39

b43:
  %448 = load i16, ptr %13, !tbaa !2
  %449 = load i16, ptr %4, !tbaa !2
  %450 = add nsw i16 %448, %449
  store i16 %450, ptr %13, !tbaa !2
  br label %b34

b44:
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %27)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %28)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %30)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %29)
  ret void

b45:
  br label %b46

b46:
  %451 = load i16, ptr %19, !tbaa !2
  %452 = load i16, ptr %17, !tbaa !2
  %453 = add nsw i16 %451, %452
  store i16 %453, ptr %19, !tbaa !2
  br label %b9
}

define cc 1000 void @RENDER(ptr dereferenceable(2) %0, ptr dereferenceable(2) %1, ptr dereferenceable(2) %2, ptr dereferenceable(2) %3) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca i16
  %14 = alloca i16
  %15 = alloca i16
  %16 = alloca i16
  %17 = alloca i16
  %18 = alloca i16
  %19 = alloca i16
  %20 = alloca i16
  %21 = alloca i16
  %22 = alloca i16
  %23 = alloca i16
  %24 = alloca [22 x i8]
  store i16 0, ptr %24, !tbaa !2
  %25 = getelementptr inbounds i8, ptr %24, i16 2
  store i16 0, ptr %25, !tbaa !2
  %26 = getelementptr inbounds i8, ptr %24, i16 4
  store i16 0, ptr %26, !tbaa !2
  %27 = getelementptr inbounds i8, ptr %24, i16 6
  store i16 0, ptr %27, !tbaa !2
  %28 = getelementptr inbounds i8, ptr %24, i16 8
  store i16 0, ptr %28, !tbaa !2
  %29 = getelementptr inbounds i8, ptr %24, i16 10
  store i16 0, ptr %29, !tbaa !2
  %30 = getelementptr inbounds i8, ptr %24, i16 12
  store i16 0, ptr %30, !tbaa !2
  %31 = getelementptr inbounds i8, ptr %24, i16 14
  store i16 0, ptr %31, !tbaa !2
  %32 = getelementptr inbounds i8, ptr %24, i16 16
  store i16 0, ptr %32, !tbaa !2
  %33 = getelementptr inbounds i8, ptr %24, i16 18
  store i16 0, ptr %33, !tbaa !2
  %34 = getelementptr inbounds i8, ptr %24, i16 20
  store i16 0, ptr %34, !tbaa !2
  store i16 0, ptr %9, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 160, i16 0, i16 100, i16 2, i16 258, ptr %24)
  %35 = load i16, ptr %3
  %36 = load i16, ptr %1
  %37 = sub i16 %35, %36
  store i16 %37, ptr %23, !tbaa !2
  %38 = load i16, ptr %2
  %39 = load i16, ptr %0
  %40 = sub i16 %38, %39
  store i16 %40, ptr %22, !tbaa !2
  %41 = load i16, ptr %23, !tbaa !2
  %42 = sext i16 %41 to i32
  %43 = sdiv i32 %42, 100
  %44 = trunc i32 %43 to i16
  store i16 %44, ptr %21, !tbaa !2
  %45 = load i16, ptr %22, !tbaa !2
  %46 = sext i16 %45 to i32
  %47 = sdiv i32 %46, 160
  %48 = trunc i32 %47 to i16
  store i16 %48, ptr %20, !tbaa !2
  %49 = load i16, ptr %22, !tbaa !2
  %50 = sext i16 %49 to i32
  %51 = srem i32 %50, 160
  %52 = trunc i32 %51 to i16
  store i16 %52, ptr %22, !tbaa !2
  %53 = load i16, ptr %23, !tbaa !2
  %54 = sext i16 %53 to i32
  %55 = srem i32 %54, 100
  %56 = trunc i32 %55 to i16
  store i16 %56, ptr %23, !tbaa !2
  %57 = load i16, ptr %1
  store i16 %57, ptr %19, !tbaa !2
  store i16 0, ptr %18, !tbaa !2
  store i16 99, ptr %17, !tbaa !2
  store i16 1, ptr %16, !tbaa !2
  br label %b2

b2:
  %58 = load i16, ptr %16, !tbaa !2
  %59 = icmp sge i16 %58, 0
  br i1 %59, label %b3, label %b4

b3:
  %60 = load i16, ptr %18, !tbaa !2
  %61 = load i16, ptr %17, !tbaa !2
  %62 = icmp sle i16 %60, %61
  br i1 %62, label %b5, label %b6

b4:
  %63 = load i16, ptr %18, !tbaa !2
  %64 = load i16, ptr %17, !tbaa !2
  %65 = icmp sge i16 %63, %64
  br i1 %65, label %b5, label %b6

b5:
  %66 = load i16, ptr %0
  store i16 %66, ptr %15, !tbaa !2
  store i16 0, ptr %14, !tbaa !2
  %67 = load i16, ptr %19, !tbaa !2
  %68 = mul i16 %67, 320
  store i16 %68, ptr %13, !tbaa !2
  store i16 0, ptr %12, !tbaa !2
  store i16 159, ptr %11, !tbaa !2
  store i16 1, ptr %10, !tbaa !2
  br label %b7

b6:
  store i16 -24576, ptr @b$seg, !tbaa !2
  store i16 16080, ptr %8, !tbaa !2
  store i16 0, ptr %19, !tbaa !2
  store i16 99, ptr %7, !tbaa !2
  store i16 1, ptr %6, !tbaa !2
  br label %b18

b7:
  %69 = load i16, ptr %10, !tbaa !2
  %70 = icmp sge i16 %69, 0
  br i1 %70, label %b8, label %b9

b8:
  %71 = load i16, ptr %12, !tbaa !2
  %72 = load i16, ptr %11, !tbaa !2
  %73 = icmp sle i16 %71, %72
  br i1 %73, label %b10, label %b11

b9:
  %74 = load i16, ptr %12, !tbaa !2
  %75 = load i16, ptr %11, !tbaa !2
  %76 = icmp sge i16 %74, %75
  br i1 %76, label %b10, label %b11

b10:
  %77 = load i16, ptr %12, !tbaa !2
  %78 = load i16, ptr %18, !tbaa !2
  %79 = mul i16 %78, 161
  %80 = add i16 %79, %77
  %81 = mul i16 %80, 2
  %82 = getelementptr i8, ptr %24, i16 2
  %83 = load i16, ptr %82, !tbaa !2
  %84 = add i16 0, %81
  %85 = inttoptr i16 %83 to ptr addrspace(2)
  %86 = addrspacecast ptr addrspace(2) %85 to ptr addrspace(1)
  %87 = getelementptr i8, ptr addrspace(1) %86, i16 %84
  %88 = load i16, ptr %15, !tbaa !2
  %89 = load i16, ptr %13, !tbaa !2
  %90 = add i16 %88, %89
  %91 = load i16, ptr @b$seg, !tbaa !2
  %92 = inttoptr i16 %91 to ptr addrspace(2)
  %93 = addrspacecast ptr addrspace(2) %92 to ptr addrspace(1)
  %94 = getelementptr i8, ptr addrspace(1) %93, i16 %90
  %95 = load volatile i8, ptr addrspace(1) %94
  %96 = zext i8 %95 to i16
  store i16 %96, ptr addrspace(1) %87, !tbaa !22
  %97 = load i16, ptr %14, !tbaa !2
  %98 = load i16, ptr %22, !tbaa !2
  %99 = add i16 %97, %98
  store i16 %99, ptr %14, !tbaa !2
  %100 = load i16, ptr %14, !tbaa !2
  %101 = icmp sgt i16 %100, 160
  br i1 %101, label %b12, label %b13

b11:
  %102 = load i16, ptr %9, !tbaa !2
  %103 = load i16, ptr %23, !tbaa !2
  %104 = add i16 %102, %103
  store i16 %104, ptr %9, !tbaa !2
  %105 = load i16, ptr %9, !tbaa !2
  %106 = icmp sgt i16 %105, 100
  br i1 %106, label %b15, label %b16

b12:
  %107 = load i16, ptr %14, !tbaa !2
  %108 = sub i16 %107, 160
  store i16 %108, ptr %14, !tbaa !2
  %109 = load i16, ptr %15, !tbaa !2
  %110 = add i16 %109, 1
  store i16 %110, ptr %15, !tbaa !2
  br label %b14

b13:
  br label %b14

b14:
  %111 = load i16, ptr %15, !tbaa !2
  %112 = load i16, ptr %20, !tbaa !2
  %113 = add i16 %111, %112
  store i16 %113, ptr %15, !tbaa !2
  %114 = load i16, ptr %12, !tbaa !2
  %115 = load i16, ptr %10, !tbaa !2
  %116 = add nsw i16 %114, %115
  store i16 %116, ptr %12, !tbaa !2
  br label %b7

b15:
  %117 = load i16, ptr %9, !tbaa !2
  %118 = sub i16 %117, 100
  store i16 %118, ptr %9, !tbaa !2
  %119 = load i16, ptr %19, !tbaa !2
  %120 = add i16 %119, 1
  store i16 %120, ptr %19, !tbaa !2
  br label %b17

b16:
  br label %b17

b17:
  %121 = load i16, ptr %19, !tbaa !2
  %122 = load i16, ptr %21, !tbaa !2
  %123 = add i16 %121, %122
  store i16 %123, ptr %19, !tbaa !2
  %124 = load i16, ptr %18, !tbaa !2
  %125 = load i16, ptr %16, !tbaa !2
  %126 = add nsw i16 %124, %125
  store i16 %126, ptr %18, !tbaa !2
  br label %b2

b18:
  %127 = load i16, ptr %6, !tbaa !2
  %128 = icmp sge i16 %127, 0
  br i1 %128, label %b19, label %b20

b19:
  %129 = load i16, ptr %19, !tbaa !2
  %130 = load i16, ptr %7, !tbaa !2
  %131 = icmp sle i16 %129, %130
  br i1 %131, label %b21, label %b22

b20:
  %132 = load i16, ptr %19, !tbaa !2
  %133 = load i16, ptr %7, !tbaa !2
  %134 = icmp sge i16 %132, %133
  br i1 %134, label %b21, label %b22

b21:
  store i16 0, ptr %15, !tbaa !2
  store i16 159, ptr %5, !tbaa !2
  store i16 1, ptr %4, !tbaa !2
  br label %b23

b22:
  %135 = getelementptr i8, ptr @"FRACTAL2%", i16 2
  %136 = load i16, ptr %135, !tbaa !2
  %137 = inttoptr i16 %136 to ptr addrspace(2)
  %138 = addrspacecast ptr addrspace(2) %137 to ptr addrspace(1)
  %139 = getelementptr i8, ptr addrspace(1) %138, i16 0
  %140 = addrspacecast ptr addrspace(1) %139 to ptr addrspace(2)
  %141 = ptrtoint ptr addrspace(2) %140 to i16
  store i16 %141, ptr @b$seg, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %24)
  ret void

b23:
  %142 = load i16, ptr %4, !tbaa !2
  %143 = icmp sge i16 %142, 0
  br i1 %143, label %b24, label %b25

b24:
  %144 = load i16, ptr %15, !tbaa !2
  %145 = load i16, ptr %5, !tbaa !2
  %146 = icmp sle i16 %144, %145
  br i1 %146, label %b26, label %b27

b25:
  %147 = load i16, ptr %15, !tbaa !2
  %148 = load i16, ptr %5, !tbaa !2
  %149 = icmp sge i16 %147, %148
  br i1 %149, label %b26, label %b27

b26:
  %150 = load i16, ptr %8, !tbaa !2
  %151 = load i16, ptr %15, !tbaa !2
  %152 = add i16 %150, %151
  %153 = load i16, ptr %15, !tbaa !2
  %154 = load i16, ptr %19, !tbaa !2
  %155 = mul i16 %154, 161
  %156 = add i16 %155, %153
  %157 = mul i16 %156, 2
  %158 = getelementptr i8, ptr %24, i16 2
  %159 = load i16, ptr %158, !tbaa !2
  %160 = add i16 0, %157
  %161 = inttoptr i16 %159 to ptr addrspace(2)
  %162 = addrspacecast ptr addrspace(2) %161 to ptr addrspace(1)
  %163 = getelementptr i8, ptr addrspace(1) %162, i16 %160
  %164 = load i16, ptr addrspace(1) %163, !tbaa !22
  %165 = trunc i16 %164 to i8
  %166 = load i16, ptr @b$seg, !tbaa !2
  %167 = inttoptr i16 %166 to ptr addrspace(2)
  %168 = addrspacecast ptr addrspace(2) %167 to ptr addrspace(1)
  %169 = getelementptr i8, ptr addrspace(1) %168, i16 %152
  store volatile i8 %165, ptr addrspace(1) %169, !tbaa !4
  %170 = load i16, ptr %15, !tbaa !2
  %171 = load i16, ptr %4, !tbaa !2
  %172 = add nsw i16 %170, %171
  store i16 %172, ptr %15, !tbaa !2
  br label %b23

b27:
  %173 = load i16, ptr %8, !tbaa !2
  %174 = add i16 %173, 320
  store i16 %174, ptr %8, !tbaa !2
  %175 = load i16, ptr %19, !tbaa !2
  %176 = load i16, ptr %6, !tbaa !2
  %177 = add nsw i16 %175, %176
  store i16 %177, ptr %19, !tbaa !2
  br label %b18
}

define cc 1000 void @SHADEBOBEFFECT(ptr dereferenceable(2) %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca i16
  %14 = alloca i16
  %15 = alloca i16
  %16 = alloca i16
  %17 = alloca i16
  %18 = alloca i16
  %19 = alloca i16
  %20 = alloca i16
  %21 = alloca i16
  %22 = alloca i16
  %23 = alloca i16
  %24 = alloca i16
  %25 = alloca i16
  %26 = alloca i16
  %27 = alloca i16
  %28 = alloca i16
  %29 = alloca i16
  %30 = alloca i16
  %31 = alloca [18 x i8]
  store i16 0, ptr %31, !tbaa !2
  %32 = getelementptr inbounds i8, ptr %31, i16 2
  store i16 0, ptr %32, !tbaa !2
  %33 = getelementptr inbounds i8, ptr %31, i16 4
  store i16 0, ptr %33, !tbaa !2
  %34 = getelementptr inbounds i8, ptr %31, i16 6
  store i16 0, ptr %34, !tbaa !2
  %35 = getelementptr inbounds i8, ptr %31, i16 8
  store i16 0, ptr %35, !tbaa !2
  %36 = getelementptr inbounds i8, ptr %31, i16 10
  store i16 0, ptr %36, !tbaa !2
  %37 = getelementptr inbounds i8, ptr %31, i16 12
  store i16 0, ptr %37, !tbaa !2
  %38 = getelementptr inbounds i8, ptr %31, i16 14
  store i16 0, ptr %38, !tbaa !2
  %39 = getelementptr inbounds i8, ptr %31, i16 16
  store i16 0, ptr %39, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 4096, i16 2, i16 257, ptr %31)
  store i16 0, ptr %30, !tbaa !2
  store i16 -24576, ptr @b$seg, !tbaa !2
  store i16 1, ptr %29, !tbaa !2
  %40 = load i16, ptr %0
  store i16 %40, ptr %28, !tbaa !2
  store i16 1, ptr %27, !tbaa !2
  br label %b2

b2:
  %41 = load i16, ptr %27, !tbaa !2
  %42 = icmp sge i16 %41, 0
  br i1 %42, label %b3, label %b4

b3:
  %43 = load i16, ptr %29, !tbaa !2
  %44 = load i16, ptr %28, !tbaa !2
  %45 = icmp sle i16 %43, %44
  br i1 %45, label %b5, label %b6

b4:
  %46 = load i16, ptr %29, !tbaa !2
  %47 = load i16, ptr %28, !tbaa !2
  %48 = icmp sge i16 %46, %47
  br i1 %48, label %b5, label %b6

b5:
  %49 = load i16, ptr %30, !tbaa !2
  %50 = mul i16 %49, 2
  %51 = getelementptr i8, ptr %31, i16 2
  %52 = load i16, ptr %51, !tbaa !2
  %53 = add i16 0, %50
  %54 = inttoptr i16 %52 to ptr addrspace(2)
  %55 = addrspacecast ptr addrspace(2) %54 to ptr addrspace(1)
  %56 = getelementptr i8, ptr addrspace(1) %55, i16 %53
  %57 = load i16, ptr addrspace(1) %56, !tbaa !24
  store i16 %57, ptr %26, !tbaa !2
  call cc 1000 addrspace(1) void @UNDRAWBOB(ptr %26)
  %58 = load i16, ptr %29, !tbaa !2
  store i16 %58, ptr %24, !tbaa !2
  %59 = load i16, ptr %24, !tbaa !2
  %60 = sitofp i16 %59 to float
  store i16 71, ptr %23, !tbaa !2
  %61 = load i16, ptr %23, !tbaa !2
  %62 = sitofp i16 %61 to float
  %63 = fdiv float %60, %62
  %64 = call float @llvm.fabs.f32(float %63)
  %65 = load float, ptr @$float12, !tbaa !2
  %66 = fcmp ogt float %64, %65
  br i1 %66, label %b7, label %b8

b6:
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %31)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %31)
  ret void

b7:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b8:
  %67 = call float @llvm.sin.f32(float %63)
  %68 = load i16, ptr %29, !tbaa !2
  store i16 %68, ptr %22, !tbaa !2
  %69 = load i16, ptr %22, !tbaa !2
  %70 = sitofp i16 %69 to float
  store i16 47, ptr %21, !tbaa !2
  %71 = load i16, ptr %21, !tbaa !2
  %72 = sitofp i16 %71 to float
  %73 = fdiv float %70, %72
  store i16 2, ptr %20, !tbaa !2
  %74 = load i16, ptr %20, !tbaa !2
  %75 = sitofp i16 %74 to float
  %76 = fadd float %73, %75
  %77 = call float @llvm.fabs.f32(float %76)
  %78 = load float, ptr @$float12, !tbaa !2
  %79 = fcmp ogt float %77, %78
  br i1 %79, label %b9, label %b10

b9:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b10:
  %80 = call float @llvm.cos.f32(float %76)
  %81 = fadd float %67, %80
  %82 = load i16, ptr %29, !tbaa !2
  store i16 %82, ptr %19, !tbaa !2
  %83 = load i16, ptr %19, !tbaa !2
  %84 = sitofp i16 %83 to float
  store i16 91, ptr %18, !tbaa !2
  %85 = load i16, ptr %18, !tbaa !2
  %86 = sitofp i16 %85 to float
  %87 = fdiv float %84, %86
  store i16 7, ptr %17, !tbaa !2
  %88 = load i16, ptr %17, !tbaa !2
  %89 = sitofp i16 %88 to float
  %90 = fadd float %87, %89
  %91 = call float @llvm.fabs.f32(float %90)
  %92 = load float, ptr @$float12, !tbaa !2
  %93 = fcmp ogt float %91, %92
  br i1 %93, label %b11, label %b12

b11:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b12:
  %94 = call float @llvm.cos.f32(float %90)
  %95 = fadd float %81, %94
  store i16 48, ptr %16, !tbaa !2
  %96 = load i16, ptr %16, !tbaa !2
  %97 = sitofp i16 %96 to float
  %98 = fmul float %95, %97
  store i16 160, ptr %15, !tbaa !2
  %99 = load i16, ptr %15, !tbaa !2
  %100 = sitofp i16 %99 to float
  %101 = fadd float %98, %100
  %102 = call i16 @llvm.lrint.i16.f32(float %101)
  store i16 %102, ptr %25, !tbaa !2
  %103 = load i16, ptr %29, !tbaa !2
  store i16 %103, ptr %13, !tbaa !2
  %104 = load i16, ptr %13, !tbaa !2
  %105 = sitofp i16 %104 to float
  store i16 49, ptr %12, !tbaa !2
  %106 = load i16, ptr %12, !tbaa !2
  %107 = sitofp i16 %106 to float
  %108 = fdiv float %105, %107
  store i16 3, ptr %11, !tbaa !2
  %109 = load i16, ptr %11, !tbaa !2
  %110 = sitofp i16 %109 to float
  %111 = fadd float %108, %110
  %112 = call float @llvm.fabs.f32(float %111)
  %113 = load float, ptr @$float12, !tbaa !2
  %114 = fcmp ogt float %112, %113
  br i1 %114, label %b13, label %b14

b13:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b14:
  %115 = call float @llvm.cos.f32(float %111)
  %116 = load i16, ptr %29, !tbaa !2
  store i16 %116, ptr %10, !tbaa !2
  %117 = load i16, ptr %10, !tbaa !2
  %118 = sitofp i16 %117 to float
  store i16 41, ptr %9, !tbaa !2
  %119 = load i16, ptr %9, !tbaa !2
  %120 = sitofp i16 %119 to float
  %121 = fdiv float %118, %120
  store i16 2, ptr %8, !tbaa !2
  %122 = load i16, ptr %8, !tbaa !2
  %123 = sitofp i16 %122 to float
  %124 = fadd float %121, %123
  %125 = call float @llvm.fabs.f32(float %124)
  %126 = load float, ptr @$float12, !tbaa !2
  %127 = fcmp ogt float %125, %126
  br i1 %127, label %b15, label %b16

b15:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b16:
  %128 = call float @llvm.sin.f32(float %124)
  %129 = fadd float %115, %128
  %130 = load i16, ptr %29, !tbaa !2
  store i16 %130, ptr %7, !tbaa !2
  %131 = load i16, ptr %7, !tbaa !2
  %132 = sitofp i16 %131 to float
  store i16 97, ptr %6, !tbaa !2
  %133 = load i16, ptr %6, !tbaa !2
  %134 = sitofp i16 %133 to float
  %135 = fdiv float %132, %134
  store i16 7, ptr %5, !tbaa !2
  %136 = load i16, ptr %5, !tbaa !2
  %137 = sitofp i16 %136 to float
  %138 = fadd float %135, %137
  %139 = call float @llvm.fabs.f32(float %138)
  %140 = load float, ptr @$float12, !tbaa !2
  %141 = fcmp ogt float %139, %140
  br i1 %141, label %b17, label %b18

b17:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b18:
  %142 = call float @llvm.sin.f32(float %138)
  %143 = fadd float %129, %142
  store i16 28, ptr %4, !tbaa !2
  %144 = load i16, ptr %4, !tbaa !2
  %145 = sitofp i16 %144 to float
  %146 = fmul float %143, %145
  store i16 100, ptr %3, !tbaa !2
  %147 = load i16, ptr %3, !tbaa !2
  %148 = sitofp i16 %147 to float
  %149 = fadd float %146, %148
  %150 = call i16 @llvm.lrint.i16.f32(float %149)
  store i16 %150, ptr %14, !tbaa !2
  %151 = load i16, ptr %30, !tbaa !2
  %152 = mul i16 %151, 2
  %153 = getelementptr i8, ptr %31, i16 2
  %154 = load i16, ptr %153, !tbaa !2
  %155 = add i16 0, %152
  %156 = inttoptr i16 %154 to ptr addrspace(2)
  %157 = addrspacecast ptr addrspace(2) %156 to ptr addrspace(1)
  %158 = getelementptr i8, ptr addrspace(1) %157, i16 %155
  %159 = load i16, ptr %25, !tbaa !2
  %160 = load i16, ptr %14, !tbaa !2
  %161 = mul i16 %160, 320
  %162 = add i16 %159, %161
  store i16 %162, ptr addrspace(1) %158, !tbaa !24
  %163 = load i16, ptr %30, !tbaa !2
  %164 = mul i16 %163, 2
  %165 = getelementptr i8, ptr %31, i16 2
  %166 = load i16, ptr %165, !tbaa !2
  %167 = add i16 0, %164
  %168 = inttoptr i16 %166 to ptr addrspace(2)
  %169 = addrspacecast ptr addrspace(2) %168 to ptr addrspace(1)
  %170 = getelementptr i8, ptr addrspace(1) %169, i16 %167
  %171 = load i16, ptr addrspace(1) %170, !tbaa !24
  store i16 %171, ptr %2, !tbaa !2
  call cc 1000 addrspace(1) void @DRAWBOB(ptr %2)
  %172 = load i16, ptr %29, !tbaa !2
  %173 = sext i16 %172 to i32
  %174 = sdiv i32 %173, 2
  %175 = trunc i32 %174 to i16
  %176 = add i16 %175, 1
  store i16 %176, ptr %1, !tbaa !2
  %177 = load i16, ptr %30, !tbaa !2
  %178 = add i16 %177, 1
  store i16 %178, ptr %30, !tbaa !2
  %179 = load i16, ptr %30, !tbaa !2
  %180 = load i16, ptr %1, !tbaa !2
  %181 = sext i16 %179 to i32
  %182 = sext i16 %180 to i32
  %183 = icmp eq i32 %182, 0
  br i1 %183, label %b19, label %b20

b19:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11) cold
  unreachable

b20:
  %184 = add i32 %182, 1
  %185 = add i32 %181, 32768
  %186 = or i32 %184, %185
  %187 = icmp eq i32 %186, 0
  br i1 %187, label %b21, label %b22

b21:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11) cold
  unreachable

b22:
  %188 = srem i32 %181, %182
  %189 = trunc i32 %188 to i16
  store i16 %189, ptr %30, !tbaa !2
  %190 = load i16, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %191 = add i16 %190, 1
  store i16 %191, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %192 = call cc 1000 addrspace(1) ptr @llrm.qb.B$INKY()
  %193 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %192, ptr @$string14)
  %194 = icmp sgt i16 %193, 0
  br i1 %194, label %b23, label %b24

b23:
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %31)
  ret void

b24:
  br label %b25

b25:
  %195 = load i16, ptr %29, !tbaa !2
  %196 = load i16, ptr %27, !tbaa !2
  %197 = add nsw i16 %195, %196
  store i16 %197, ptr %29, !tbaa !2
  br label %b2
}

define cc 1000 void @UNDRAWBOB(ptr dereferenceable(2) %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  store i16 0, ptr %6, !tbaa !2
  store i16 31, ptr %5, !tbaa !2
  store i16 1, ptr %4, !tbaa !2
  br label %b2

b2:
  %7 = load i16, ptr %4, !tbaa !2
  %8 = icmp sge i16 %7, 0
  br i1 %8, label %b3, label %b4

b3:
  %9 = load i16, ptr %6, !tbaa !2
  %10 = load i16, ptr %5, !tbaa !2
  %11 = icmp sle i16 %9, %10
  br i1 %11, label %b5, label %b6

b4:
  %12 = load i16, ptr %6, !tbaa !2
  %13 = load i16, ptr %5, !tbaa !2
  %14 = icmp sge i16 %12, %13
  br i1 %14, label %b5, label %b6

b5:
  %15 = load i16, ptr %0
  %16 = add i16 %15, 288
  store i16 %16, ptr %0
  store i16 0, ptr %3, !tbaa !2
  store i16 31, ptr %2, !tbaa !2
  store i16 1, ptr %1, !tbaa !2
  br label %b7

b6:
  ret void

b7:
  %17 = load i16, ptr %1, !tbaa !2
  %18 = icmp sge i16 %17, 0
  br i1 %18, label %b8, label %b9

b8:
  %19 = load i16, ptr %3, !tbaa !2
  %20 = load i16, ptr %2, !tbaa !2
  %21 = icmp sle i16 %19, %20
  br i1 %21, label %b10, label %b11

b9:
  %22 = load i16, ptr %3, !tbaa !2
  %23 = load i16, ptr %2, !tbaa !2
  %24 = icmp sge i16 %22, %23
  br i1 %24, label %b10, label %b11

b10:
  %25 = load i16, ptr %0
  %26 = load i16, ptr %0
  %27 = load i16, ptr @b$seg, !tbaa !2
  %28 = inttoptr i16 %27 to ptr addrspace(2)
  %29 = addrspacecast ptr addrspace(2) %28 to ptr addrspace(1)
  %30 = getelementptr i8, ptr addrspace(1) %29, i16 %26
  %31 = load volatile i8, ptr addrspace(1) %30
  %32 = zext i8 %31 to i16
  %33 = load i16, ptr %3, !tbaa !2
  %34 = load i16, ptr %6, !tbaa !2
  %35 = mul i16 %34, 33
  %36 = add i16 %35, %33
  %37 = mul i16 %36, 2
  %38 = getelementptr i8, ptr @"BOBSPRITE%", i16 2
  %39 = load i16, ptr %38, !tbaa !2
  %40 = add i16 0, %37
  %41 = inttoptr i16 %39 to ptr addrspace(2)
  %42 = addrspacecast ptr addrspace(2) %41 to ptr addrspace(1)
  %43 = getelementptr i8, ptr addrspace(1) %42, i16 %40
  %44 = load i16, ptr addrspace(1) %43, !tbaa !6
  %45 = sub i16 %32, %44
  %46 = trunc i16 %45 to i8
  %47 = load i16, ptr @b$seg, !tbaa !2
  %48 = inttoptr i16 %47 to ptr addrspace(2)
  %49 = addrspacecast ptr addrspace(2) %48 to ptr addrspace(1)
  %50 = getelementptr i8, ptr addrspace(1) %49, i16 %25
  store volatile i8 %46, ptr addrspace(1) %50
  %51 = load i16, ptr %0
  %52 = add i16 %51, 1
  store i16 %52, ptr %0
  %53 = load i16, ptr %3, !tbaa !2
  %54 = load i16, ptr %1, !tbaa !2
  %55 = add nsw i16 %53, %54
  store i16 %55, ptr %3, !tbaa !2
  br label %b7

b11:
  %56 = load i16, ptr %6, !tbaa !2
  %57 = load i16, ptr %4, !tbaa !2
  %58 = add nsw i16 %56, %57
  store i16 %58, ptr %6, !tbaa !2
  br label %b2
}

define cc 1000 void @UNWHITEFADE(ptr dereferenceable(2) %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca i16
  %14 = alloca float
  %15 = alloca i16
  %16 = alloca i16
  %17 = alloca i16
  %18 = alloca i16
  %19 = alloca i16
  %20 = alloca i16
  %21 = alloca [22 x i8]
  store i16 0, ptr %21, !tbaa !2
  %22 = getelementptr inbounds i8, ptr %21, i16 2
  store i16 0, ptr %22, !tbaa !2
  %23 = getelementptr inbounds i8, ptr %21, i16 4
  store i16 0, ptr %23, !tbaa !2
  %24 = getelementptr inbounds i8, ptr %21, i16 6
  store i16 0, ptr %24, !tbaa !2
  %25 = getelementptr inbounds i8, ptr %21, i16 8
  store i16 0, ptr %25, !tbaa !2
  %26 = getelementptr inbounds i8, ptr %21, i16 10
  store i16 0, ptr %26, !tbaa !2
  %27 = getelementptr inbounds i8, ptr %21, i16 12
  store i16 0, ptr %27, !tbaa !2
  %28 = getelementptr inbounds i8, ptr %21, i16 14
  store i16 0, ptr %28, !tbaa !2
  %29 = getelementptr inbounds i8, ptr %21, i16 16
  store i16 0, ptr %29, !tbaa !2
  %30 = getelementptr inbounds i8, ptr %21, i16 18
  store i16 0, ptr %30, !tbaa !2
  %31 = getelementptr inbounds i8, ptr %21, i16 20
  store i16 0, ptr %31, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 256, i16 0, i16 3, i16 2, i16 258, ptr %21)
  store i16 0, ptr %20, !tbaa !2
  store i16 255, ptr %19, !tbaa !2
  store i16 1, ptr %18, !tbaa !2
  br label %b2

b2:
  %32 = load i16, ptr %18, !tbaa !2
  %33 = icmp sge i16 %32, 0
  br i1 %33, label %b3, label %b4

b3:
  %34 = load i16, ptr %20, !tbaa !2
  %35 = load i16, ptr %19, !tbaa !2
  %36 = icmp sle i16 %34, %35
  br i1 %36, label %b5, label %b6

b4:
  %37 = load i16, ptr %20, !tbaa !2
  %38 = load i16, ptr %19, !tbaa !2
  %39 = icmp sge i16 %37, %38
  br i1 %39, label %b5, label %b6

b5:
  %40 = load i16, ptr %20, !tbaa !2
  %41 = trunc i16 %40 to i8
  call void @llrm.ia16.out.i8(i16 967, i8 %41)
  %42 = load i16, ptr %20, !tbaa !2
  %43 = add i16 0, %42
  %44 = mul i16 %43, 2
  %45 = getelementptr i8, ptr %21, i16 2
  %46 = load i16, ptr %45, !tbaa !2
  %47 = add i16 0, %44
  %48 = inttoptr i16 %46 to ptr addrspace(2)
  %49 = addrspacecast ptr addrspace(2) %48 to ptr addrspace(1)
  %50 = getelementptr i8, ptr addrspace(1) %49, i16 %47
  %51 = call i8 @llrm.ia16.in.i8(i16 969)
  %52 = zext i8 %51 to i16
  store i16 %52, ptr addrspace(1) %50, !tbaa !26
  %53 = load i16, ptr %20, !tbaa !2
  %54 = add i16 257, %53
  %55 = mul i16 %54, 2
  %56 = getelementptr i8, ptr %21, i16 2
  %57 = load i16, ptr %56, !tbaa !2
  %58 = add i16 0, %55
  %59 = inttoptr i16 %57 to ptr addrspace(2)
  %60 = addrspacecast ptr addrspace(2) %59 to ptr addrspace(1)
  %61 = getelementptr i8, ptr addrspace(1) %60, i16 %58
  %62 = call i8 @llrm.ia16.in.i8(i16 969)
  %63 = zext i8 %62 to i16
  store i16 %63, ptr addrspace(1) %61, !tbaa !26
  %64 = load i16, ptr %20, !tbaa !2
  %65 = add i16 514, %64
  %66 = mul i16 %65, 2
  %67 = getelementptr i8, ptr %21, i16 2
  %68 = load i16, ptr %67, !tbaa !2
  %69 = add i16 0, %66
  %70 = inttoptr i16 %68 to ptr addrspace(2)
  %71 = addrspacecast ptr addrspace(2) %70 to ptr addrspace(1)
  %72 = getelementptr i8, ptr addrspace(1) %71, i16 %69
  %73 = call i8 @llrm.ia16.in.i8(i16 969)
  %74 = zext i8 %73 to i16
  store i16 %74, ptr addrspace(1) %72, !tbaa !26
  %75 = load i16, ptr %20, !tbaa !2
  %76 = load i16, ptr %18, !tbaa !2
  %77 = add nsw i16 %75, %76
  store i16 %77, ptr %20, !tbaa !2
  br label %b2

b6:
  store i16 0, ptr %17, !tbaa !2
  %78 = load i16, ptr %0
  store i16 %78, ptr %16, !tbaa !2
  store i16 1, ptr %15, !tbaa !2
  br label %b7

b7:
  %79 = load i16, ptr %15, !tbaa !2
  %80 = icmp sge i16 %79, 0
  br i1 %80, label %b8, label %b9

b8:
  %81 = load i16, ptr %17, !tbaa !2
  %82 = load i16, ptr %16, !tbaa !2
  %83 = icmp sle i16 %81, %82
  br i1 %83, label %b10, label %b11

b9:
  %84 = load i16, ptr %17, !tbaa !2
  %85 = load i16, ptr %16, !tbaa !2
  %86 = icmp sge i16 %84, %85
  br i1 %86, label %b10, label %b11

b10:
  %87 = load i16, ptr %17, !tbaa !2
  %88 = load i16, ptr %0
  store i16 %87, ptr %13, !tbaa !2
  %89 = load i16, ptr %13, !tbaa !2
  %90 = sitofp i16 %89 to float
  store i16 %88, ptr %12, !tbaa !2
  %91 = load i16, ptr %12, !tbaa !2
  %92 = sitofp i16 %91 to float
  %93 = fdiv float %90, %92
  %94 = fpround float %93 to float
  store float %94, ptr %14, !tbaa !2
  %95 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %95, ptr @"BENCHFRAME&", !tbaa !2
  %96 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %96, ptr @"BENCHFRAME&", !tbaa !2
  store i16 0, ptr %20, !tbaa !2
  store i16 255, ptr %11, !tbaa !2
  store i16 1, ptr %10, !tbaa !2
  br label %b12

b11:
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %21)
  ret void

b12:
  %97 = load i16, ptr %10, !tbaa !2
  %98 = icmp sge i16 %97, 0
  br i1 %98, label %b13, label %b14

b13:
  %99 = load i16, ptr %20, !tbaa !2
  %100 = load i16, ptr %11, !tbaa !2
  %101 = icmp sle i16 %99, %100
  br i1 %101, label %b15, label %b16

b14:
  %102 = load i16, ptr %20, !tbaa !2
  %103 = load i16, ptr %11, !tbaa !2
  %104 = icmp sge i16 %102, %103
  br i1 %104, label %b15, label %b16

b15:
  %105 = load i16, ptr %20, !tbaa !2
  %106 = trunc i16 %105 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %106)
  %107 = load i16, ptr %20, !tbaa !2
  %108 = add i16 0, %107
  %109 = mul i16 %108, 2
  %110 = getelementptr i8, ptr %21, i16 2
  %111 = load i16, ptr %110, !tbaa !2
  %112 = add i16 0, %109
  %113 = inttoptr i16 %111 to ptr addrspace(2)
  %114 = addrspacecast ptr addrspace(2) %113 to ptr addrspace(1)
  %115 = getelementptr i8, ptr addrspace(1) %114, i16 %112
  %116 = load i16, ptr addrspace(1) %115, !tbaa !26
  %117 = load float, ptr %14, !tbaa !2
  store i16 %116, ptr %9, !tbaa !2
  %118 = load i16, ptr %9, !tbaa !2
  %119 = sitofp i16 %118 to float
  %120 = fmul float %119, %117
  %121 = load float, ptr %14, !tbaa !2
  store i16 1, ptr %8, !tbaa !2
  %122 = load i16, ptr %8, !tbaa !2
  %123 = sitofp i16 %122 to float
  %124 = fsub float %123, %121
  store i16 63, ptr %7, !tbaa !2
  %125 = load i16, ptr %7, !tbaa !2
  %126 = sitofp i16 %125 to float
  %127 = fmul float %126, %124
  %128 = fadd float %120, %127
  %129 = call i16 @llvm.lrint.i16.f32(float %128)
  %130 = trunc i16 %129 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %130)
  %131 = load i16, ptr %20, !tbaa !2
  %132 = add i16 257, %131
  %133 = mul i16 %132, 2
  %134 = getelementptr i8, ptr %21, i16 2
  %135 = load i16, ptr %134, !tbaa !2
  %136 = add i16 0, %133
  %137 = inttoptr i16 %135 to ptr addrspace(2)
  %138 = addrspacecast ptr addrspace(2) %137 to ptr addrspace(1)
  %139 = getelementptr i8, ptr addrspace(1) %138, i16 %136
  %140 = load i16, ptr addrspace(1) %139, !tbaa !26
  %141 = load float, ptr %14, !tbaa !2
  store i16 %140, ptr %6, !tbaa !2
  %142 = load i16, ptr %6, !tbaa !2
  %143 = sitofp i16 %142 to float
  %144 = fmul float %143, %141
  %145 = load float, ptr %14, !tbaa !2
  store i16 1, ptr %5, !tbaa !2
  %146 = load i16, ptr %5, !tbaa !2
  %147 = sitofp i16 %146 to float
  %148 = fsub float %147, %145
  store i16 63, ptr %4, !tbaa !2
  %149 = load i16, ptr %4, !tbaa !2
  %150 = sitofp i16 %149 to float
  %151 = fmul float %150, %148
  %152 = fadd float %144, %151
  %153 = call i16 @llvm.lrint.i16.f32(float %152)
  %154 = trunc i16 %153 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %154)
  %155 = load i16, ptr %20, !tbaa !2
  %156 = add i16 514, %155
  %157 = mul i16 %156, 2
  %158 = getelementptr i8, ptr %21, i16 2
  %159 = load i16, ptr %158, !tbaa !2
  %160 = add i16 0, %157
  %161 = inttoptr i16 %159 to ptr addrspace(2)
  %162 = addrspacecast ptr addrspace(2) %161 to ptr addrspace(1)
  %163 = getelementptr i8, ptr addrspace(1) %162, i16 %160
  %164 = load i16, ptr addrspace(1) %163, !tbaa !26
  %165 = load float, ptr %14, !tbaa !2
  store i16 %164, ptr %3, !tbaa !2
  %166 = load i16, ptr %3, !tbaa !2
  %167 = sitofp i16 %166 to float
  %168 = fmul float %167, %165
  %169 = load float, ptr %14, !tbaa !2
  store i16 1, ptr %2, !tbaa !2
  %170 = load i16, ptr %2, !tbaa !2
  %171 = sitofp i16 %170 to float
  %172 = fsub float %171, %169
  store i16 63, ptr %1, !tbaa !2
  %173 = load i16, ptr %1, !tbaa !2
  %174 = sitofp i16 %173 to float
  %175 = fmul float %174, %172
  %176 = fadd float %168, %175
  %177 = call i16 @llvm.lrint.i16.f32(float %176)
  %178 = trunc i16 %177 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %178)
  %179 = load i16, ptr %20, !tbaa !2
  %180 = load i16, ptr %10, !tbaa !2
  %181 = add nsw i16 %179, %180
  store i16 %181, ptr %20, !tbaa !2
  br label %b12

b16:
  %182 = load i16, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %183 = add i16 %182, 1
  store i16 %183, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %184 = load i16, ptr %17, !tbaa !2
  %185 = load i16, ptr %15, !tbaa !2
  %186 = add nsw i16 %184, %185
  store i16 %186, ptr %17, !tbaa !2
  br label %b7
}

define cc 1000 void @UPDPALPLASMA(ptr dereferenceable(2) %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca i16
  %14 = alloca i16
  %15 = alloca i16
  %16 = alloca i16
  %17 = alloca i16
  %18 = alloca i16
  store i16 0, ptr %18, !tbaa !2
  store i16 255, ptr %17, !tbaa !2
  store i16 1, ptr %16, !tbaa !2
  br label %b2

b2:
  %19 = load i16, ptr %16, !tbaa !2
  %20 = icmp sge i16 %19, 0
  br i1 %20, label %b3, label %b4

b3:
  %21 = load i16, ptr %18, !tbaa !2
  %22 = load i16, ptr %17, !tbaa !2
  %23 = icmp sle i16 %21, %22
  br i1 %23, label %b5, label %b6

b4:
  %24 = load i16, ptr %18, !tbaa !2
  %25 = load i16, ptr %17, !tbaa !2
  %26 = icmp sge i16 %24, %25
  br i1 %26, label %b5, label %b6

b5:
  %27 = load i16, ptr %18, !tbaa !2
  %28 = trunc i16 %27 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %28)
  %29 = load i16, ptr %18, !tbaa !2
  %30 = load double, ptr @$float15, !tbaa !2
  store i16 %29, ptr %15, !tbaa !2
  %31 = load i16, ptr %15, !tbaa !2
  %32 = sitofp i16 %31 to double
  %33 = fmul double %32, %30
  store i16 128, ptr %14, !tbaa !2
  %34 = load i16, ptr %14, !tbaa !2
  %35 = sitofp i16 %34 to double
  %36 = fdiv double %33, %35
  %37 = load i16, ptr %0
  %38 = load float, ptr @$float16, !tbaa !2
  store i16 %37, ptr %13, !tbaa !2
  %39 = load i16, ptr %13, !tbaa !2
  %40 = sitofp i16 %39 to float
  %41 = fmul float %40, %38
  %42 = fpext float %41 to double
  %43 = fadd double %36, %42
  %44 = call double @llvm.fabs.f64(double %43)
  %45 = load double, ptr @$float17, !tbaa !2
  %46 = fcmp ogt double %44, %45
  br i1 %46, label %b7, label %b8

b6:
  ret void

b7:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b8:
  %47 = call double @llvm.cos.f64(double %43)
  store i16 31, ptr %12, !tbaa !2
  %48 = load i16, ptr %12, !tbaa !2
  %49 = sitofp i16 %48 to double
  %50 = fmul double %49, %47
  store i16 32, ptr %11, !tbaa !2
  %51 = load i16, ptr %11, !tbaa !2
  %52 = sitofp i16 %51 to double
  %53 = fsub double %52, %50
  %54 = call i16 @llvm.lrint.i16.f64(double %53)
  %55 = trunc i16 %54 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %55)
  %56 = load i16, ptr %18, !tbaa !2
  %57 = load double, ptr @$float15, !tbaa !2
  store i16 %56, ptr %10, !tbaa !2
  %58 = load i16, ptr %10, !tbaa !2
  %59 = sitofp i16 %58 to double
  %60 = fmul double %59, %57
  store i16 128, ptr %9, !tbaa !2
  %61 = load i16, ptr %9, !tbaa !2
  %62 = sitofp i16 %61 to double
  %63 = fdiv double %60, %62
  %64 = load i16, ptr %0
  %65 = load float, ptr @$float18, !tbaa !2
  store i16 %64, ptr %8, !tbaa !2
  %66 = load i16, ptr %8, !tbaa !2
  %67 = sitofp i16 %66 to float
  %68 = fmul float %67, %65
  %69 = fpext float %68 to double
  %70 = fadd double %63, %69
  %71 = call double @llvm.fabs.f64(double %70)
  %72 = load double, ptr @$float17, !tbaa !2
  %73 = fcmp ogt double %71, %72
  br i1 %73, label %b9, label %b10

b9:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b10:
  %74 = call double @llvm.cos.f64(double %70)
  store i16 31, ptr %7, !tbaa !2
  %75 = load i16, ptr %7, !tbaa !2
  %76 = sitofp i16 %75 to double
  %77 = fmul double %76, %74
  store i16 32, ptr %6, !tbaa !2
  %78 = load i16, ptr %6, !tbaa !2
  %79 = sitofp i16 %78 to double
  %80 = fsub double %79, %77
  %81 = call i16 @llvm.lrint.i16.f64(double %80)
  %82 = trunc i16 %81 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %82)
  %83 = load i16, ptr %18, !tbaa !2
  %84 = load double, ptr @$float15, !tbaa !2
  store i16 %83, ptr %5, !tbaa !2
  %85 = load i16, ptr %5, !tbaa !2
  %86 = sitofp i16 %85 to double
  %87 = fmul double %86, %84
  store i16 64, ptr %4, !tbaa !2
  %88 = load i16, ptr %4, !tbaa !2
  %89 = sitofp i16 %88 to double
  %90 = fdiv double %87, %89
  %91 = load i16, ptr %0
  %92 = load float, ptr @$float19, !tbaa !2
  store i16 %91, ptr %3, !tbaa !2
  %93 = load i16, ptr %3, !tbaa !2
  %94 = sitofp i16 %93 to float
  %95 = fmul float %94, %92
  %96 = fpext float %95 to double
  %97 = fadd double %90, %96
  %98 = call double @llvm.fabs.f64(double %97)
  %99 = load double, ptr @$float17, !tbaa !2
  %100 = fcmp ogt double %98, %99
  br i1 %100, label %b11, label %b12

b11:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b12:
  %101 = call double @llvm.cos.f64(double %97)
  store i16 31, ptr %2, !tbaa !2
  %102 = load i16, ptr %2, !tbaa !2
  %103 = sitofp i16 %102 to double
  %104 = fmul double %103, %101
  store i16 32, ptr %1, !tbaa !2
  %105 = load i16, ptr %1, !tbaa !2
  %106 = sitofp i16 %105 to double
  %107 = fsub double %106, %104
  %108 = call i16 @llvm.lrint.i16.f64(double %107)
  %109 = trunc i16 %108 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %109)
  %110 = load i16, ptr %18, !tbaa !2
  %111 = load i16, ptr %16, !tbaa !2
  %112 = add nsw i16 %110, %111
  store i16 %112, ptr %18, !tbaa !2
  br label %b2
}

define cc 1000 void @WHITEFADE(ptr dereferenceable(2) %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca i16
  %14 = alloca float
  %15 = alloca i16
  %16 = alloca i16
  %17 = alloca i16
  %18 = alloca i16
  %19 = alloca i16
  %20 = alloca i16
  %21 = alloca [22 x i8]
  store i16 0, ptr %21, !tbaa !2
  %22 = getelementptr inbounds i8, ptr %21, i16 2
  store i16 0, ptr %22, !tbaa !2
  %23 = getelementptr inbounds i8, ptr %21, i16 4
  store i16 0, ptr %23, !tbaa !2
  %24 = getelementptr inbounds i8, ptr %21, i16 6
  store i16 0, ptr %24, !tbaa !2
  %25 = getelementptr inbounds i8, ptr %21, i16 8
  store i16 0, ptr %25, !tbaa !2
  %26 = getelementptr inbounds i8, ptr %21, i16 10
  store i16 0, ptr %26, !tbaa !2
  %27 = getelementptr inbounds i8, ptr %21, i16 12
  store i16 0, ptr %27, !tbaa !2
  %28 = getelementptr inbounds i8, ptr %21, i16 14
  store i16 0, ptr %28, !tbaa !2
  %29 = getelementptr inbounds i8, ptr %21, i16 16
  store i16 0, ptr %29, !tbaa !2
  %30 = getelementptr inbounds i8, ptr %21, i16 18
  store i16 0, ptr %30, !tbaa !2
  %31 = getelementptr inbounds i8, ptr %21, i16 20
  store i16 0, ptr %31, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 256, i16 0, i16 3, i16 2, i16 258, ptr %21)
  store i16 0, ptr %20, !tbaa !2
  store i16 255, ptr %19, !tbaa !2
  store i16 1, ptr %18, !tbaa !2
  br label %b2

b2:
  %32 = load i16, ptr %18, !tbaa !2
  %33 = icmp sge i16 %32, 0
  br i1 %33, label %b3, label %b4

b3:
  %34 = load i16, ptr %20, !tbaa !2
  %35 = load i16, ptr %19, !tbaa !2
  %36 = icmp sle i16 %34, %35
  br i1 %36, label %b5, label %b6

b4:
  %37 = load i16, ptr %20, !tbaa !2
  %38 = load i16, ptr %19, !tbaa !2
  %39 = icmp sge i16 %37, %38
  br i1 %39, label %b5, label %b6

b5:
  %40 = load i16, ptr %20, !tbaa !2
  %41 = trunc i16 %40 to i8
  call void @llrm.ia16.out.i8(i16 967, i8 %41)
  %42 = load i16, ptr %20, !tbaa !2
  %43 = add i16 0, %42
  %44 = mul i16 %43, 2
  %45 = getelementptr i8, ptr %21, i16 2
  %46 = load i16, ptr %45, !tbaa !2
  %47 = add i16 0, %44
  %48 = inttoptr i16 %46 to ptr addrspace(2)
  %49 = addrspacecast ptr addrspace(2) %48 to ptr addrspace(1)
  %50 = getelementptr i8, ptr addrspace(1) %49, i16 %47
  %51 = call i8 @llrm.ia16.in.i8(i16 969)
  %52 = zext i8 %51 to i16
  store i16 %52, ptr addrspace(1) %50, !tbaa !28
  %53 = load i16, ptr %20, !tbaa !2
  %54 = add i16 257, %53
  %55 = mul i16 %54, 2
  %56 = getelementptr i8, ptr %21, i16 2
  %57 = load i16, ptr %56, !tbaa !2
  %58 = add i16 0, %55
  %59 = inttoptr i16 %57 to ptr addrspace(2)
  %60 = addrspacecast ptr addrspace(2) %59 to ptr addrspace(1)
  %61 = getelementptr i8, ptr addrspace(1) %60, i16 %58
  %62 = call i8 @llrm.ia16.in.i8(i16 969)
  %63 = zext i8 %62 to i16
  store i16 %63, ptr addrspace(1) %61, !tbaa !28
  %64 = load i16, ptr %20, !tbaa !2
  %65 = add i16 514, %64
  %66 = mul i16 %65, 2
  %67 = getelementptr i8, ptr %21, i16 2
  %68 = load i16, ptr %67, !tbaa !2
  %69 = add i16 0, %66
  %70 = inttoptr i16 %68 to ptr addrspace(2)
  %71 = addrspacecast ptr addrspace(2) %70 to ptr addrspace(1)
  %72 = getelementptr i8, ptr addrspace(1) %71, i16 %69
  %73 = call i8 @llrm.ia16.in.i8(i16 969)
  %74 = zext i8 %73 to i16
  store i16 %74, ptr addrspace(1) %72, !tbaa !28
  %75 = load i16, ptr %20, !tbaa !2
  %76 = load i16, ptr %18, !tbaa !2
  %77 = add nsw i16 %75, %76
  store i16 %77, ptr %20, !tbaa !2
  br label %b2

b6:
  %78 = load i16, ptr %0
  store i16 %78, ptr %17, !tbaa !2
  store i16 0, ptr %16, !tbaa !2
  store i16 -1, ptr %15, !tbaa !2
  br label %b7

b7:
  %79 = load i16, ptr %15, !tbaa !2
  %80 = icmp sge i16 %79, 0
  br i1 %80, label %b8, label %b9

b8:
  %81 = load i16, ptr %17, !tbaa !2
  %82 = load i16, ptr %16, !tbaa !2
  %83 = icmp sle i16 %81, %82
  br i1 %83, label %b10, label %b11

b9:
  %84 = load i16, ptr %17, !tbaa !2
  %85 = load i16, ptr %16, !tbaa !2
  %86 = icmp sge i16 %84, %85
  br i1 %86, label %b10, label %b11

b10:
  %87 = load i16, ptr %17, !tbaa !2
  %88 = load i16, ptr %0
  store i16 %87, ptr %13, !tbaa !2
  %89 = load i16, ptr %13, !tbaa !2
  %90 = sitofp i16 %89 to float
  store i16 %88, ptr %12, !tbaa !2
  %91 = load i16, ptr %12, !tbaa !2
  %92 = sitofp i16 %91 to float
  %93 = fdiv float %90, %92
  %94 = fpround float %93 to float
  store float %94, ptr %14, !tbaa !2
  %95 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %95, ptr @"BENCHFRAME&", !tbaa !2
  %96 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %96, ptr @"BENCHFRAME&", !tbaa !2
  store i16 0, ptr %20, !tbaa !2
  store i16 255, ptr %11, !tbaa !2
  store i16 1, ptr %10, !tbaa !2
  br label %b12

b11:
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %21)
  ret void

b12:
  %97 = load i16, ptr %10, !tbaa !2
  %98 = icmp sge i16 %97, 0
  br i1 %98, label %b13, label %b14

b13:
  %99 = load i16, ptr %20, !tbaa !2
  %100 = load i16, ptr %11, !tbaa !2
  %101 = icmp sle i16 %99, %100
  br i1 %101, label %b15, label %b16

b14:
  %102 = load i16, ptr %20, !tbaa !2
  %103 = load i16, ptr %11, !tbaa !2
  %104 = icmp sge i16 %102, %103
  br i1 %104, label %b15, label %b16

b15:
  %105 = load i16, ptr %20, !tbaa !2
  %106 = trunc i16 %105 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %106)
  %107 = load i16, ptr %20, !tbaa !2
  %108 = add i16 0, %107
  %109 = mul i16 %108, 2
  %110 = getelementptr i8, ptr %21, i16 2
  %111 = load i16, ptr %110, !tbaa !2
  %112 = add i16 0, %109
  %113 = inttoptr i16 %111 to ptr addrspace(2)
  %114 = addrspacecast ptr addrspace(2) %113 to ptr addrspace(1)
  %115 = getelementptr i8, ptr addrspace(1) %114, i16 %112
  %116 = load i16, ptr addrspace(1) %115, !tbaa !28
  %117 = load float, ptr %14, !tbaa !2
  store i16 %116, ptr %9, !tbaa !2
  %118 = load i16, ptr %9, !tbaa !2
  %119 = sitofp i16 %118 to float
  %120 = fmul float %119, %117
  %121 = load float, ptr %14, !tbaa !2
  store i16 1, ptr %8, !tbaa !2
  %122 = load i16, ptr %8, !tbaa !2
  %123 = sitofp i16 %122 to float
  %124 = fsub float %123, %121
  store i16 63, ptr %7, !tbaa !2
  %125 = load i16, ptr %7, !tbaa !2
  %126 = sitofp i16 %125 to float
  %127 = fmul float %126, %124
  %128 = fadd float %120, %127
  %129 = call i16 @llvm.lrint.i16.f32(float %128)
  %130 = trunc i16 %129 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %130)
  %131 = load i16, ptr %20, !tbaa !2
  %132 = add i16 257, %131
  %133 = mul i16 %132, 2
  %134 = getelementptr i8, ptr %21, i16 2
  %135 = load i16, ptr %134, !tbaa !2
  %136 = add i16 0, %133
  %137 = inttoptr i16 %135 to ptr addrspace(2)
  %138 = addrspacecast ptr addrspace(2) %137 to ptr addrspace(1)
  %139 = getelementptr i8, ptr addrspace(1) %138, i16 %136
  %140 = load i16, ptr addrspace(1) %139, !tbaa !28
  %141 = load float, ptr %14, !tbaa !2
  store i16 %140, ptr %6, !tbaa !2
  %142 = load i16, ptr %6, !tbaa !2
  %143 = sitofp i16 %142 to float
  %144 = fmul float %143, %141
  %145 = load float, ptr %14, !tbaa !2
  store i16 1, ptr %5, !tbaa !2
  %146 = load i16, ptr %5, !tbaa !2
  %147 = sitofp i16 %146 to float
  %148 = fsub float %147, %145
  store i16 63, ptr %4, !tbaa !2
  %149 = load i16, ptr %4, !tbaa !2
  %150 = sitofp i16 %149 to float
  %151 = fmul float %150, %148
  %152 = fadd float %144, %151
  %153 = call i16 @llvm.lrint.i16.f32(float %152)
  %154 = trunc i16 %153 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %154)
  %155 = load i16, ptr %20, !tbaa !2
  %156 = add i16 514, %155
  %157 = mul i16 %156, 2
  %158 = getelementptr i8, ptr %21, i16 2
  %159 = load i16, ptr %158, !tbaa !2
  %160 = add i16 0, %157
  %161 = inttoptr i16 %159 to ptr addrspace(2)
  %162 = addrspacecast ptr addrspace(2) %161 to ptr addrspace(1)
  %163 = getelementptr i8, ptr addrspace(1) %162, i16 %160
  %164 = load i16, ptr addrspace(1) %163, !tbaa !28
  %165 = load float, ptr %14, !tbaa !2
  store i16 %164, ptr %3, !tbaa !2
  %166 = load i16, ptr %3, !tbaa !2
  %167 = sitofp i16 %166 to float
  %168 = fmul float %167, %165
  %169 = load float, ptr %14, !tbaa !2
  store i16 1, ptr %2, !tbaa !2
  %170 = load i16, ptr %2, !tbaa !2
  %171 = sitofp i16 %170 to float
  %172 = fsub float %171, %169
  store i16 63, ptr %1, !tbaa !2
  %173 = load i16, ptr %1, !tbaa !2
  %174 = sitofp i16 %173 to float
  %175 = fmul float %174, %172
  %176 = fadd float %168, %175
  %177 = call i16 @llvm.lrint.i16.f32(float %176)
  %178 = trunc i16 %177 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %178)
  %179 = load i16, ptr %20, !tbaa !2
  %180 = load i16, ptr %10, !tbaa !2
  %181 = add nsw i16 %179, %180
  store i16 %181, ptr %20, !tbaa !2
  br label %b12

b16:
  %182 = load i16, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %183 = add i16 %182, 1
  store i16 %183, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %184 = load i16, ptr %17, !tbaa !2
  %185 = load i16, ptr %15, !tbaa !2
  %186 = add nsw i16 %184, %185
  store i16 %186, ptr %17, !tbaa !2
  br label %b7
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
  %59 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string20, ptr %58)
  %60 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %59, ptr @$string21)
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
  store i16 %78, ptr addrspace(1) %76, !tbaa !30
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
  %92 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string22, ptr %91)
  %93 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %92, ptr @$string23)
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
  store i32 %105, ptr addrspace(1) %104, !tbaa !32
  %106 = getelementptr i8, ptr %11, i16 2
  %107 = load i16, ptr %106, !tbaa !2
  %108 = inttoptr i16 %107 to ptr addrspace(2)
  %109 = addrspacecast ptr addrspace(2) %108 to ptr addrspace(1)
  %110 = getelementptr i8, ptr addrspace(1) %109, i16 4
  %111 = load i32, ptr @"BENCHLO&", !tbaa !2
  store i32 %111, ptr addrspace(1) %110, !tbaa !32
  %112 = getelementptr i8, ptr %11, i16 2
  %113 = load i16, ptr %112, !tbaa !2
  %114 = inttoptr i16 %113 to ptr addrspace(2)
  %115 = addrspacecast ptr addrspace(2) %114 to ptr addrspace(1)
  %116 = getelementptr i8, ptr addrspace(1) %115, i16 8
  %117 = load i32, ptr %10, !tbaa !2
  store i32 %117, ptr addrspace(1) %116, !tbaa !32
  %118 = getelementptr i8, ptr %11, i16 2
  %119 = load i16, ptr %118, !tbaa !2
  %120 = inttoptr i16 %119 to ptr addrspace(2)
  %121 = addrspacecast ptr addrspace(2) %120 to ptr addrspace(1)
  %122 = getelementptr i8, ptr addrspace(1) %121, i16 12
  %123 = load i32, ptr %9, !tbaa !2
  store i32 %123, ptr addrspace(1) %122, !tbaa !32
  %124 = getelementptr i8, ptr %11, i16 2
  %125 = load i16, ptr %124, !tbaa !2
  %126 = inttoptr i16 %125 to ptr addrspace(2)
  %127 = addrspacecast ptr addrspace(2) %126 to ptr addrspace(1)
  %128 = getelementptr i8, ptr addrspace(1) %127, i16 16
  %129 = load i32, ptr %6, !tbaa !2
  store i32 %129, ptr addrspace(1) %128, !tbaa !32
  %130 = getelementptr i8, ptr %11, i16 2
  %131 = load i16, ptr %130, !tbaa !2
  %132 = inttoptr i16 %131 to ptr addrspace(2)
  %133 = addrspacecast ptr addrspace(2) %132 to ptr addrspace(1)
  %134 = getelementptr i8, ptr addrspace(1) %133, i16 20
  %135 = load i32, ptr %7, !tbaa !2
  store i32 %135, ptr addrspace(1) %134, !tbaa !32
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
  %146 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string24, ptr %145)
  %147 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %146, ptr @$string25)
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

declare cc 1000 void @llrm.qb.B$CSCN(i16, i16, i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$DDIM(i16, i16, i16, i16, ptr) addrspace(1)

declare void @llrm.ia16.out.i8(i16, i8) nocallback nofree nounwind willreturn memory(readwrite)

declare cc 1000 ptr @llrm.qb.B$TIMR() addrspace(1)

declare cc 1000 void @llrm.qb.B$ERAS(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$SCLS(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$CEND() addrspace(1)

declare float @llvm.sqrt.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i16 @llvm.lrint.i16.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1000 ptr @llrm.qb.B$INKY() addrspace(1)

declare cc 1000 i16 @llrm.qb.B$SCMP(ptr, ptr) addrspace(1) threeway

declare cc 1000 void @llrm.qb.B$BLOD(ptr, i16, i16) addrspace(1)

declare float @llvm.fabs.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1)

declare float @llvm.sin.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare float @llvm.cos.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i8 @llrm.ia16.in.i8(i16) nocallback nofree nounwind willreturn memory(readwrite)

declare double @llvm.fabs.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare double @llvm.cos.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i16 @llvm.lrint.i16.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1000 void @TSCSNAP(ptr, ptr) addrspace(1)

declare cc 1000 ptr @llrm.qb.B$STI2(i16) addrspace(1)

declare cc 1000 ptr @llrm.qb.B$LTRM(ptr) addrspace(1)

declare cc 1000 ptr @llrm.qb.B$SCAT(ptr, ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$BSAV(ptr, i16, i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$DSG0() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{!"allocation.0", !3, i64 0}
!6 = !{!5, !5, i64 0}
!7 = !{!"allocation.1", !3, i64 0}
!8 = !{!7, !7, i64 0}
!9 = !{!"allocation.2", !3, i64 0}
!10 = !{!9, !9, i64 0}
!11 = !{!"allocation.3", !3, i64 0}
!12 = !{!11, !11, i64 0}
!13 = !{!"allocation.4", !3, i64 0}
!14 = !{!13, !13, i64 0}
!15 = !{!"allocation.5", !3, i64 0}
!16 = !{!15, !15, i64 0}
!17 = !{!"allocation.6", !3, i64 0}
!18 = !{!17, !17, i64 0}
!19 = !{!"allocation.7", !3, i64 0}
!20 = !{!19, !19, i64 0}
!21 = !{!"allocation.8", !3, i64 0}
!22 = !{!21, !21, i64 0}
!23 = !{!"allocation.9", !3, i64 0}
!24 = !{!23, !23, i64 0}
!25 = !{!"allocation.10", !3, i64 0}
!26 = !{!25, !25, i64 0}
!27 = !{!"allocation.11", !3, i64 0}
!28 = !{!27, !27, i64 0}
!29 = !{!"allocation.12", !3, i64 0}
!30 = !{!29, !29, i64 0}
!31 = !{!"allocation.13", !3, i64 0}
!32 = !{!31, !31, i64 0}
