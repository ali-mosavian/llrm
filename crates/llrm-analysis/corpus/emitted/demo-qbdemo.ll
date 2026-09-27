target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$data = internal global [60 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"BENCHHI&" = internal global [4 x i8] zeroinitializer
@"BENCHLO&" = internal global [4 x i8] zeroinitializer
@"BENCHFRAME&" = internal global [4 x i8] zeroinitializer
@"TOTALFRAMECOUNT%" = internal global [2 x i8] zeroinitializer
@"FRACTAL1%" = internal global [18 x i8] zeroinitializer
@"FRACTAL2%" = internal global [18 x i8] zeroinitializer
@"BOBSPRITE%" = internal global [22 x i8] zeroinitializer
@"X%" = internal global [2 x i8] zeroinitializer
@"TI1!" = internal global [4 x i8] zeroinitializer
@"Y%" = internal global [2 x i8] zeroinitializer
@b$seg = global [2 x i8] zeroinitializer
@$float4 = internal constant [8 x i8] c"^A\DA\22%2\E4\BF"
@$float5 = internal constant [8 x i8] c"\8E`s\FBw}\E2\BF"
@$float6 = internal constant [4 x i8] c"\CD\CC\CC?"
@$string7 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string7, i16 4) }>
@$string8 = internal constant <{ [2 x i8], ptr, [10 x i8] }> <{ [2 x i8] c"\0A\00", ptr getelementptr (i8, ptr @$string8, i16 4), [10 x i8] c"canada.bsv" }>
@$string9 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string9, i16 4) }>
@$float10 = internal constant [4 x i8] c"\C3\F5H@"
@$string11 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string11, i16 4) }>
@$string12 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string12, i16 4) }>
@$float13 = internal constant [8 x i8] c"\18-DT\FB!\09@"
@$float14 = internal constant [4 x i8] c"\C0\CF\B8:"
@$float15 = internal constant [4 x i8] c"\B0\03g<"
@$float16 = internal constant [4 x i8] c"\89\D2^<"
@$string17 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string17, i16 4), [2 x i8] c"V\00" }>
@$string18 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string18, i16 4), [4 x i8] c".BIN" }>
@$string19 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string19, i16 4), [2 x i8] c"P\00" }>
@$string20 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string20, i16 4), [4 x i8] c".BIN" }>
@$string21 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string21, i16 4), [2 x i8] c"T\00" }>
@$string22 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string22, i16 4), [4 x i8] c".BIN" }>

define internal cc1000 void @__main() addrspace(1) {
b1:
  call cc1000 addrspace(1) void @llrm.qb.B$CSCN(i16 1, i16 13, i16 2)
  call cc1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 32000, i16 2, i16 257, ptr @"FRACTAL1%")
  call cc1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 32000, i16 2, i16 257, ptr @"FRACTAL2%")
  store i16 0, ptr @"X%", !tbaa !2
  store i16 63, ptr @$data, !tbaa !2
  %0 = getelementptr i8, ptr @$data, i16 2
  store i16 1, ptr %0, !tbaa !2
  br label %b2

b2:
  %1 = getelementptr i8, ptr @$data, i16 2
  %2 = load i16, ptr %1, !tbaa !2
  %3 = icmp sge i16 %2, 0
  %4 = sext i1 %3 to i16
  %5 = icmp ne i16 %4, 0
  br i1 %5, label %b3, label %b4

b3:
  %6 = load i16, ptr @"X%", !tbaa !2
  %7 = load i16, ptr @$data, !tbaa !2
  %8 = icmp sle i16 %6, %7
  %9 = sext i1 %8 to i16
  %10 = icmp ne i16 %9, 0
  br i1 %10, label %b5, label %b6

b4:
  %11 = load i16, ptr @"X%", !tbaa !2
  %12 = load i16, ptr @$data, !tbaa !2
  %13 = icmp sge i16 %11, %12
  %14 = sext i1 %13 to i16
  %15 = icmp ne i16 %14, 0
  br i1 %15, label %b5, label %b6

b5:
  %16 = load i16, ptr @"X%", !tbaa !2
  %17 = trunc i16 %16 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %17)
  %18 = load i16, ptr @"X%", !tbaa !2
  %19 = trunc i16 %18 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %19)
  %20 = load i16, ptr @"X%", !tbaa !2
  %21 = trunc i16 %20 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %21)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  %22 = load i16, ptr @"X%", !tbaa !2
  %23 = getelementptr i8, ptr @$data, i16 2
  %24 = load i16, ptr %23, !tbaa !2
  %25 = add i16 %22, %24
  store i16 %25, ptr @"X%", !tbaa !2
  br label %b2

b6:
  store i16 64, ptr @"X%", !tbaa !2
  %26 = getelementptr i8, ptr @$data, i16 4
  store i16 127, ptr %26, !tbaa !2
  %27 = getelementptr i8, ptr @$data, i16 6
  store i16 1, ptr %27, !tbaa !2
  br label %b7

b7:
  %28 = getelementptr i8, ptr @$data, i16 6
  %29 = load i16, ptr %28, !tbaa !2
  %30 = icmp sge i16 %29, 0
  %31 = sext i1 %30 to i16
  %32 = icmp ne i16 %31, 0
  br i1 %32, label %b8, label %b9

b8:
  %33 = load i16, ptr @"X%", !tbaa !2
  %34 = getelementptr i8, ptr @$data, i16 4
  %35 = load i16, ptr %34, !tbaa !2
  %36 = icmp sle i16 %33, %35
  %37 = sext i1 %36 to i16
  %38 = icmp ne i16 %37, 0
  br i1 %38, label %b10, label %b11

b9:
  %39 = load i16, ptr @"X%", !tbaa !2
  %40 = getelementptr i8, ptr @$data, i16 4
  %41 = load i16, ptr %40, !tbaa !2
  %42 = icmp sge i16 %39, %41
  %43 = sext i1 %42 to i16
  %44 = icmp ne i16 %43, 0
  br i1 %44, label %b10, label %b11

b10:
  %45 = load i16, ptr @"X%", !tbaa !2
  %46 = trunc i16 %45 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %46)
  call void @llrm.ia16.out.i8(i16 969, i8 63)
  call void @llrm.ia16.out.i8(i16 969, i8 63)
  %47 = load i16, ptr @"X%", !tbaa !2
  %48 = trunc i16 %47 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %48)
  %49 = load i16, ptr @"X%", !tbaa !2
  %50 = getelementptr i8, ptr @$data, i16 6
  %51 = load i16, ptr %50, !tbaa !2
  %52 = add i16 %49, %51
  store i16 %52, ptr @"X%", !tbaa !2
  br label %b7

b11:
  store i16 128, ptr @"X%", !tbaa !2
  %53 = getelementptr i8, ptr @$data, i16 8
  store i16 191, ptr %53, !tbaa !2
  %54 = getelementptr i8, ptr @$data, i16 10
  store i16 1, ptr %54, !tbaa !2
  br label %b12

b12:
  %55 = getelementptr i8, ptr @$data, i16 10
  %56 = load i16, ptr %55, !tbaa !2
  %57 = icmp sge i16 %56, 0
  %58 = sext i1 %57 to i16
  %59 = icmp ne i16 %58, 0
  br i1 %59, label %b13, label %b14

b13:
  %60 = load i16, ptr @"X%", !tbaa !2
  %61 = getelementptr i8, ptr @$data, i16 8
  %62 = load i16, ptr %61, !tbaa !2
  %63 = icmp sle i16 %60, %62
  %64 = sext i1 %63 to i16
  %65 = icmp ne i16 %64, 0
  br i1 %65, label %b15, label %b16

b14:
  %66 = load i16, ptr @"X%", !tbaa !2
  %67 = getelementptr i8, ptr @$data, i16 8
  %68 = load i16, ptr %67, !tbaa !2
  %69 = icmp sge i16 %66, %68
  %70 = sext i1 %69 to i16
  %71 = icmp ne i16 %70, 0
  br i1 %71, label %b15, label %b16

b15:
  %72 = load i16, ptr @"X%", !tbaa !2
  %73 = trunc i16 %72 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %73)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  call void @llrm.ia16.out.i8(i16 969, i8 0)
  %74 = load i16, ptr @"X%", !tbaa !2
  %75 = trunc i16 %74 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %75)
  %76 = load i16, ptr @"X%", !tbaa !2
  %77 = getelementptr i8, ptr @$data, i16 10
  %78 = load i16, ptr %77, !tbaa !2
  %79 = add i16 %76, %78
  store i16 %79, ptr @"X%", !tbaa !2
  br label %b12

b16:
  store i16 191, ptr @"X%", !tbaa !2
  %80 = getelementptr i8, ptr @$data, i16 12
  store i16 255, ptr %80, !tbaa !2
  %81 = getelementptr i8, ptr @$data, i16 14
  store i16 1, ptr %81, !tbaa !2
  br label %b17

b17:
  %82 = getelementptr i8, ptr @$data, i16 14
  %83 = load i16, ptr %82, !tbaa !2
  %84 = icmp sge i16 %83, 0
  %85 = sext i1 %84 to i16
  %86 = icmp ne i16 %85, 0
  br i1 %86, label %b18, label %b19

b18:
  %87 = load i16, ptr @"X%", !tbaa !2
  %88 = getelementptr i8, ptr @$data, i16 12
  %89 = load i16, ptr %88, !tbaa !2
  %90 = icmp sle i16 %87, %89
  %91 = sext i1 %90 to i16
  %92 = icmp ne i16 %91, 0
  br i1 %92, label %b20, label %b21

b19:
  %93 = load i16, ptr @"X%", !tbaa !2
  %94 = getelementptr i8, ptr @$data, i16 12
  %95 = load i16, ptr %94, !tbaa !2
  %96 = icmp sge i16 %93, %95
  %97 = sext i1 %96 to i16
  %98 = icmp ne i16 %97, 0
  br i1 %98, label %b20, label %b21

b20:
  %99 = load i16, ptr @"X%", !tbaa !2
  %100 = trunc i16 %99 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %100)
  %101 = load i16, ptr @"X%", !tbaa !2
  %102 = trunc i16 %101 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %102)
  %103 = load i16, ptr @"X%", !tbaa !2
  %104 = trunc i16 %103 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %104)
  call void @llrm.ia16.out.i8(i16 969, i8 63)
  %105 = load i16, ptr @"X%", !tbaa !2
  %106 = getelementptr i8, ptr @$data, i16 14
  %107 = load i16, ptr %106, !tbaa !2
  %108 = add i16 %105, %107
  store i16 %108, ptr @"X%", !tbaa !2
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
  %109 = getelementptr i8, ptr @$data, i16 16
  store i16 0, ptr %109, !tbaa !2
  %110 = getelementptr i8, ptr @$data, i16 16
  call cc1000 addrspace(1) void @BENCHMARK(ptr %110)
  %111 = call cc1000 addrspace(1) ptr @llrm.qb.B$TIMR()
  %112 = load float, ptr %111
  store float %112, ptr @"TI1!", !tbaa !2
  %113 = getelementptr i8, ptr @$data, i16 18
  store i16 150, ptr %113, !tbaa !2
  %114 = getelementptr i8, ptr @$data, i16 18
  call cc1000 addrspace(1) void @FRACTALEFFECT(ptr %114)
  %115 = getelementptr i8, ptr @$data, i16 20
  store i16 1, ptr %115, !tbaa !2
  %116 = getelementptr i8, ptr @$data, i16 20
  call cc1000 addrspace(1) void @BENCHMARK(ptr %116)
  %117 = call cc1000 addrspace(1) ptr @llrm.qb.B$TIMR()
  %118 = load float, ptr %117
  %119 = load float, ptr @"TI1!", !tbaa !2
  %120 = fsub float %118, %119
  store float %120, ptr @"TI1!", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$ERAS(ptr @"FRACTAL1%")
  call cc1000 addrspace(1) void @llrm.qb.B$ERAS(ptr @"FRACTAL2%")
  call cc1000 addrspace(1) void @llrm.qb.B$SCLS(i16 -1)
  %121 = getelementptr i8, ptr @$data, i16 22
  store i16 16, ptr %121, !tbaa !2
  %122 = getelementptr i8, ptr @$data, i16 22
  call cc1000 addrspace(1) void @UNWHITEFADE(ptr %122)
  call cc1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 32, i16 0, i16 32, i16 2, i16 258, ptr @"BOBSPRITE%")
  store i16 0, ptr @"X%", !tbaa !2
  %123 = getelementptr i8, ptr @$data, i16 24
  store i16 32, ptr %123, !tbaa !2
  %124 = getelementptr i8, ptr @$data, i16 26
  store i16 1, ptr %124, !tbaa !2
  br label %b22

b22:
  %125 = getelementptr i8, ptr @$data, i16 26
  %126 = load i16, ptr %125, !tbaa !2
  %127 = icmp sge i16 %126, 0
  %128 = sext i1 %127 to i16
  %129 = icmp ne i16 %128, 0
  br i1 %129, label %b23, label %b24

b23:
  %130 = load i16, ptr @"X%", !tbaa !2
  %131 = getelementptr i8, ptr @$data, i16 24
  %132 = load i16, ptr %131, !tbaa !2
  %133 = icmp sle i16 %130, %132
  %134 = sext i1 %133 to i16
  %135 = icmp ne i16 %134, 0
  br i1 %135, label %b25, label %b26

b24:
  %136 = load i16, ptr @"X%", !tbaa !2
  %137 = getelementptr i8, ptr @$data, i16 24
  %138 = load i16, ptr %137, !tbaa !2
  %139 = icmp sge i16 %136, %138
  %140 = sext i1 %139 to i16
  %141 = icmp ne i16 %140, 0
  br i1 %141, label %b25, label %b26

b25:
  store i16 0, ptr @"Y%", !tbaa !2
  %142 = getelementptr i8, ptr @$data, i16 28
  store i16 32, ptr %142, !tbaa !2
  %143 = getelementptr i8, ptr @$data, i16 30
  store i16 1, ptr %143, !tbaa !2
  br label %b27

b26:
  %144 = getelementptr i8, ptr @$data, i16 46
  store i16 1024, ptr %144, !tbaa !2
  %145 = getelementptr i8, ptr @$data, i16 46
  call cc1000 addrspace(1) void @SHADEBOBEFFECT(ptr %145)
  %146 = getelementptr i8, ptr @$data, i16 48
  store i16 2, ptr %146, !tbaa !2
  %147 = getelementptr i8, ptr @$data, i16 48
  call cc1000 addrspace(1) void @BENCHMARK(ptr %147)
  call cc1000 addrspace(1) void @llrm.qb.B$ERAS(ptr @"BOBSPRITE%")
  call cc1000 addrspace(1) void @llrm.qb.B$SCLS(i16 -1)
  %148 = getelementptr i8, ptr @$data, i16 50
  store i16 16, ptr %148, !tbaa !2
  %149 = getelementptr i8, ptr @$data, i16 50
  call cc1000 addrspace(1) void @UNWHITEFADE(ptr %149)
  %150 = getelementptr i8, ptr @$data, i16 52
  store i16 160, ptr %150, !tbaa !2
  %151 = getelementptr i8, ptr @$data, i16 52
  call cc1000 addrspace(1) void @PLASMA(ptr %151)
  %152 = getelementptr i8, ptr @$data, i16 54
  store i16 3, ptr %152, !tbaa !2
  %153 = getelementptr i8, ptr @$data, i16 54
  call cc1000 addrspace(1) void @BENCHMARK(ptr %153)
  call cc1000 addrspace(1) void @llrm.qb.B$SCLS(i16 -1)
  %154 = getelementptr i8, ptr @$data, i16 56
  store i16 96, ptr %154, !tbaa !2
  %155 = getelementptr i8, ptr @$data, i16 56
  call cc1000 addrspace(1) void @OHCANADA(ptr %155)
  %156 = getelementptr i8, ptr @$data, i16 58
  store i16 4, ptr %156, !tbaa !2
  %157 = getelementptr i8, ptr @$data, i16 58
  call cc1000 addrspace(1) void @BENCHMARK(ptr %157)
  call cc1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable

b27:
  %158 = getelementptr i8, ptr @$data, i16 30
  %159 = load i16, ptr %158, !tbaa !2
  %160 = icmp sge i16 %159, 0
  %161 = sext i1 %160 to i16
  %162 = icmp ne i16 %161, 0
  br i1 %162, label %b28, label %b29

b28:
  %163 = load i16, ptr @"Y%", !tbaa !2
  %164 = getelementptr i8, ptr @$data, i16 28
  %165 = load i16, ptr %164, !tbaa !2
  %166 = icmp sle i16 %163, %165
  %167 = sext i1 %166 to i16
  %168 = icmp ne i16 %167, 0
  br i1 %168, label %b30, label %b31

b29:
  %169 = load i16, ptr @"Y%", !tbaa !2
  %170 = getelementptr i8, ptr @$data, i16 28
  %171 = load i16, ptr %170, !tbaa !2
  %172 = icmp sge i16 %169, %171
  %173 = sext i1 %172 to i16
  %174 = icmp ne i16 %173, 0
  br i1 %174, label %b30, label %b31

b30:
  %175 = load i16, ptr @"X%", !tbaa !2
  %176 = load i16, ptr @"Y%", !tbaa !2
  %177 = mul i16 %176, 33
  %178 = add i16 %177, %175
  %179 = mul i16 %178, 2
  %180 = getelementptr i8, ptr @"BOBSPRITE%", i16 2
  %181 = load i16, ptr %180, !tbaa !2
  %182 = add i16 0, %179
  %183 = inttoptr i16 %181 to ptr addrspace(2)
  %184 = addrspacecast ptr addrspace(2) %183 to ptr addrspace(1)
  %185 = getelementptr i8, ptr addrspace(1) %184, i16 %182
  %186 = load i16, ptr @"X%", !tbaa !2
  %187 = sub i16 16, %186
  %188 = getelementptr i8, ptr @$data, i16 32
  store i16 %187, ptr %188, !tbaa !2
  %189 = getelementptr i8, ptr @$data, i16 32
  %190 = load i16, ptr %189, !tbaa !2
  %191 = sitofp i16 %190 to float
  %192 = getelementptr i8, ptr @$data, i16 34
  store float %191, ptr %192, !tbaa !2
  %193 = getelementptr i8, ptr @$data, i16 34
  %194 = load float, ptr %193, !tbaa !2
  %195 = getelementptr i8, ptr @$data, i16 34
  %196 = load float, ptr %195, !tbaa !2
  %197 = fmul float %194, %196
  %198 = load i16, ptr @"Y%", !tbaa !2
  %199 = sub i16 16, %198
  %200 = getelementptr i8, ptr @$data, i16 38
  store i16 %199, ptr %200, !tbaa !2
  %201 = getelementptr i8, ptr @$data, i16 38
  %202 = load i16, ptr %201, !tbaa !2
  %203 = sitofp i16 %202 to float
  %204 = getelementptr i8, ptr @$data, i16 40
  store float %203, ptr %204, !tbaa !2
  %205 = getelementptr i8, ptr @$data, i16 40
  %206 = load float, ptr %205, !tbaa !2
  %207 = getelementptr i8, ptr @$data, i16 40
  %208 = load float, ptr %207, !tbaa !2
  %209 = fmul float %206, %208
  %210 = fadd float %197, %209
  %211 = call float @llvm.sqrt.f32(float %210)
  %212 = getelementptr i8, ptr @$data, i16 44
  store i16 16, ptr %212, !tbaa !2
  %213 = getelementptr i8, ptr @$data, i16 44
  %214 = load i16, ptr %213, !tbaa !2
  %215 = sitofp i16 %214 to float
  %216 = fsub float %215, %211
  %217 = call i16 @llvm.lrint.i16.f32(float %216)
  store i16 %217, ptr addrspace(1) %185, !tbaa !4
  %218 = load i16, ptr @"X%", !tbaa !2
  %219 = load i16, ptr @"Y%", !tbaa !2
  %220 = mul i16 %219, 33
  %221 = add i16 %220, %218
  %222 = mul i16 %221, 2
  %223 = getelementptr i8, ptr @"BOBSPRITE%", i16 2
  %224 = load i16, ptr %223, !tbaa !2
  %225 = add i16 0, %222
  %226 = inttoptr i16 %224 to ptr addrspace(2)
  %227 = addrspacecast ptr addrspace(2) %226 to ptr addrspace(1)
  %228 = getelementptr i8, ptr addrspace(1) %227, i16 %225
  %229 = load i16, ptr addrspace(1) %228, !tbaa !4
  %230 = icmp slt i16 %229, 0
  %231 = sext i1 %230 to i16
  %232 = icmp ne i16 %231, 0
  br i1 %232, label %b32, label %b33

b31:
  %233 = load i16, ptr @"X%", !tbaa !2
  %234 = getelementptr i8, ptr @$data, i16 26
  %235 = load i16, ptr %234, !tbaa !2
  %236 = add i16 %233, %235
  store i16 %236, ptr @"X%", !tbaa !2
  br label %b22

b32:
  %237 = load i16, ptr @"X%", !tbaa !2
  %238 = load i16, ptr @"Y%", !tbaa !2
  %239 = mul i16 %238, 33
  %240 = add i16 %239, %237
  %241 = mul i16 %240, 2
  %242 = getelementptr i8, ptr @"BOBSPRITE%", i16 2
  %243 = load i16, ptr %242, !tbaa !2
  %244 = add i16 0, %241
  %245 = inttoptr i16 %243 to ptr addrspace(2)
  %246 = addrspacecast ptr addrspace(2) %245 to ptr addrspace(1)
  %247 = getelementptr i8, ptr addrspace(1) %246, i16 %244
  store i16 0, ptr addrspace(1) %247, !tbaa !4
  br label %b34

b33:
  br label %b34

b34:
  %248 = load i16, ptr @"Y%", !tbaa !2
  %249 = getelementptr i8, ptr @$data, i16 30
  %250 = load i16, ptr %249, !tbaa !2
  %251 = add i16 %248, %250
  store i16 %251, ptr @"Y%", !tbaa !2
  br label %b27
}

define cc1000 void @DRAWBOB(ptr %0) addrspace(1) {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  store i16 0, ptr %1
  store i16 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %4
  store i16 0, ptr %5
  store i16 0, ptr %6
  store i16 0, ptr %6, !tbaa !2
  store i16 31, ptr %5, !tbaa !2
  store i16 1, ptr %4, !tbaa !2
  br label %b2

b2:
  %7 = load i16, ptr %4, !tbaa !2
  %8 = icmp sge i16 %7, 0
  %9 = sext i1 %8 to i16
  %10 = icmp ne i16 %9, 0
  br i1 %10, label %b3, label %b4

b3:
  %11 = load i16, ptr %6, !tbaa !2
  %12 = load i16, ptr %5, !tbaa !2
  %13 = icmp sle i16 %11, %12
  %14 = sext i1 %13 to i16
  %15 = icmp ne i16 %14, 0
  br i1 %15, label %b5, label %b6

b4:
  %16 = load i16, ptr %6, !tbaa !2
  %17 = load i16, ptr %5, !tbaa !2
  %18 = icmp sge i16 %16, %17
  %19 = sext i1 %18 to i16
  %20 = icmp ne i16 %19, 0
  br i1 %20, label %b5, label %b6

b5:
  %21 = load i16, ptr %0
  %22 = add i16 %21, 288
  store i16 %22, ptr %0
  store i16 0, ptr %3, !tbaa !2
  store i16 31, ptr %2, !tbaa !2
  store i16 1, ptr %1, !tbaa !2
  br label %b7

b6:
  ret void

b7:
  %23 = load i16, ptr %1, !tbaa !2
  %24 = icmp sge i16 %23, 0
  %25 = sext i1 %24 to i16
  %26 = icmp ne i16 %25, 0
  br i1 %26, label %b8, label %b9

b8:
  %27 = load i16, ptr %3, !tbaa !2
  %28 = load i16, ptr %2, !tbaa !2
  %29 = icmp sle i16 %27, %28
  %30 = sext i1 %29 to i16
  %31 = icmp ne i16 %30, 0
  br i1 %31, label %b10, label %b11

b9:
  %32 = load i16, ptr %3, !tbaa !2
  %33 = load i16, ptr %2, !tbaa !2
  %34 = icmp sge i16 %32, %33
  %35 = sext i1 %34 to i16
  %36 = icmp ne i16 %35, 0
  br i1 %36, label %b10, label %b11

b10:
  %37 = load i16, ptr %0
  %38 = load i16, ptr %0
  %39 = load i16, ptr @b$seg, !tbaa !2
  %40 = inttoptr i16 %39 to ptr addrspace(2)
  %41 = addrspacecast ptr addrspace(2) %40 to ptr addrspace(1)
  %42 = getelementptr i8, ptr addrspace(1) %41, i16 %38
  %43 = load i8, ptr addrspace(1) %42
  %44 = zext i8 %43 to i16
  %45 = load i16, ptr %3, !tbaa !2
  %46 = load i16, ptr %6, !tbaa !2
  %47 = mul i16 %46, 33
  %48 = add i16 %47, %45
  %49 = mul i16 %48, 2
  %50 = getelementptr i8, ptr @"BOBSPRITE%", i16 2
  %51 = load i16, ptr %50, !tbaa !2
  %52 = add i16 0, %49
  %53 = inttoptr i16 %51 to ptr addrspace(2)
  %54 = addrspacecast ptr addrspace(2) %53 to ptr addrspace(1)
  %55 = getelementptr i8, ptr addrspace(1) %54, i16 %52
  %56 = load i16, ptr addrspace(1) %55, !tbaa !4
  %57 = add i16 %44, %56
  %58 = trunc i16 %57 to i8
  %59 = load i16, ptr @b$seg, !tbaa !2
  %60 = inttoptr i16 %59 to ptr addrspace(2)
  %61 = addrspacecast ptr addrspace(2) %60 to ptr addrspace(1)
  %62 = getelementptr i8, ptr addrspace(1) %61, i16 %37
  store i8 %58, ptr addrspace(1) %62
  %63 = load i16, ptr %0
  %64 = add i16 %63, 1
  store i16 %64, ptr %0
  %65 = load i16, ptr %3, !tbaa !2
  %66 = load i16, ptr %1, !tbaa !2
  %67 = add i16 %65, %66
  store i16 %67, ptr %3, !tbaa !2
  br label %b7

b11:
  %68 = load i16, ptr %6, !tbaa !2
  %69 = load i16, ptr %4, !tbaa !2
  %70 = add i16 %68, %69
  store i16 %70, ptr %6, !tbaa !2
  br label %b2
}

define cc1000 void @FRACLINE(ptr %0, ptr %1, ptr %2, ptr %3, ptr %4, ptr %5) addrspace(1) {
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
  store i16 0, ptr %6
  store i16 0, ptr %7
  store i16 0, ptr %8
  store double 0.000000e+00, ptr %9
  store i16 0, ptr %10
  store double 0.000000e+00, ptr %11
  store i16 0, ptr %12
  store double 0.000000e+00, ptr %13
  store i16 0, ptr %14
  store i16 0, ptr %15
  store i16 0, ptr %16
  store double 0.000000e+00, ptr %17
  store i16 0, ptr %18
  store i16 0, ptr %19
  store double 0.000000e+00, ptr %20
  store i16 0, ptr %21
  store double 0.000000e+00, ptr %22
  store i16 0, ptr %23
  store double 0.000000e+00, ptr %24
  %25 = load double, ptr %4
  %26 = load double, ptr %3
  %27 = fsub double %25, %26
  store i16 160, ptr %23, !tbaa !2
  %28 = load i16, ptr %23, !tbaa !2
  %29 = sitofp i16 %28 to double
  %30 = fdiv double %27, %29
  store double %30, ptr %24, !tbaa !2
  %31 = load double, ptr %2
  %32 = load double, ptr %1
  %33 = fsub double %31, %32
  store i16 160, ptr %21, !tbaa !2
  %34 = load i16, ptr %21, !tbaa !2
  %35 = sitofp i16 %34 to double
  %36 = fdiv double %33, %35
  store double %36, ptr %22, !tbaa !2
  %37 = load double, ptr %1
  store double %37, ptr %20, !tbaa !2
  store i16 1, ptr %19, !tbaa !2
  %38 = load i16, ptr %0
  %39 = mul i16 %38, 320
  store i16 %39, ptr %18, !tbaa !2
  %40 = load double, ptr %3
  %41 = load double, ptr %24, !tbaa !2
  store i16 2, ptr %16, !tbaa !2
  %42 = load i16, ptr %16, !tbaa !2
  %43 = sitofp i16 %42 to double
  %44 = fdiv double %41, %43
  %45 = fadd double %40, %44
  store double %45, ptr %17, !tbaa !2
  br label %b3

b3:
  %46 = load i16, ptr %18, !tbaa !2
  %47 = load i16, ptr %19, !tbaa !2
  %48 = add i16 %46, %47
  %49 = sub i16 %48, 1
  %50 = load i16, ptr @b$seg, !tbaa !2
  %51 = inttoptr i16 %50 to ptr addrspace(2)
  %52 = addrspacecast ptr addrspace(2) %51 to ptr addrspace(1)
  %53 = getelementptr i8, ptr addrspace(1) %52, i16 %49
  %54 = load i8, ptr addrspace(1) %53
  %55 = zext i8 %54 to i16
  store i16 %55, ptr %15, !tbaa !2
  %56 = load i16, ptr %15, !tbaa !2
  %57 = load i16, ptr %18, !tbaa !2
  %58 = load i16, ptr %19, !tbaa !2
  %59 = add i16 %57, %58
  %60 = add i16 %59, 1
  %61 = load i16, ptr @b$seg, !tbaa !2
  %62 = inttoptr i16 %61 to ptr addrspace(2)
  %63 = addrspacecast ptr addrspace(2) %62 to ptr addrspace(1)
  %64 = getelementptr i8, ptr addrspace(1) %63, i16 %60
  %65 = load i8, ptr addrspace(1) %64
  %66 = zext i8 %65 to i16
  %67 = icmp eq i16 %56, %66
  %68 = sext i1 %67 to i16
  %69 = load i16, ptr %15, !tbaa !2
  %70 = icmp sgt i16 %69, 0
  %71 = sext i1 %70 to i16
  %72 = and i16 %68, %71
  %73 = icmp ne i16 %72, 0
  br i1 %73, label %b5, label %b6

b4:
  store i16 160, ptr %19, !tbaa !2
  %74 = load double, ptr %3
  %75 = load double, ptr %4
  %76 = fadd double %74, %75
  store i16 2, ptr %8, !tbaa !2
  %77 = load i16, ptr %8, !tbaa !2
  %78 = sitofp i16 %77 to double
  %79 = fdiv double %76, %78
  store double %79, ptr %17, !tbaa !2
  store i16 0, ptr %7, !tbaa !2
  %80 = load i16, ptr %7, !tbaa !2
  %81 = sitofp i16 %80 to double
  store double %81, ptr %13, !tbaa !2
  store i16 0, ptr %6, !tbaa !2
  %82 = load i16, ptr %6, !tbaa !2
  %83 = sitofp i16 %82 to double
  store double %83, ptr %11, !tbaa !2
  store i16 0, ptr %14, !tbaa !2
  br label %b16

b5:
  %84 = load i16, ptr %15, !tbaa !2
  store i16 %84, ptr %14, !tbaa !2
  br label %b7

b6:
  store i16 0, ptr %12, !tbaa !2
  %85 = load i16, ptr %12, !tbaa !2
  %86 = sitofp i16 %85 to double
  store double %86, ptr %13, !tbaa !2
  store i16 0, ptr %10, !tbaa !2
  %87 = load i16, ptr %10, !tbaa !2
  %88 = sitofp i16 %87 to double
  store double %88, ptr %11, !tbaa !2
  store i16 0, ptr %14, !tbaa !2
  br label %b9

b7:
  %89 = load i16, ptr %19, !tbaa !2
  %90 = load i16, ptr %18, !tbaa !2
  %91 = add i16 %89, %90
  %92 = load i16, ptr %14, !tbaa !2
  %93 = trunc i16 %92 to i8
  %94 = load i16, ptr @b$seg, !tbaa !2
  %95 = inttoptr i16 %94 to ptr addrspace(2)
  %96 = addrspacecast ptr addrspace(2) %95 to ptr addrspace(1)
  %97 = getelementptr i8, ptr addrspace(1) %96, i16 %91
  store i8 %93, ptr addrspace(1) %97
  %98 = load i16, ptr %19, !tbaa !2
  %99 = add i16 %98, 2
  store i16 %99, ptr %19, !tbaa !2
  %100 = load i16, ptr %19, !tbaa !2
  %101 = icmp sge i16 %100, 320
  %102 = sext i1 %101 to i16
  %103 = icmp ne i16 %102, 0
  br i1 %103, label %b11, label %b12

b8:
  %104 = load double, ptr %13, !tbaa !2
  %105 = load double, ptr %13, !tbaa !2
  %106 = fmul double %104, %105
  %107 = load double, ptr %11, !tbaa !2
  %108 = load double, ptr %11, !tbaa !2
  %109 = fmul double %107, %108
  %110 = fadd double %106, %109
  %111 = load double, ptr %5
  %112 = fcmp oge double %110, %111
  %113 = sext i1 %112 to i16
  %114 = load i16, ptr %14, !tbaa !2
  %115 = icmp eq i16 %114, 255
  %116 = sext i1 %115 to i16
  %117 = or i16 %113, %116
  %118 = icmp ne i16 %117, 0
  br i1 %118, label %b10, label %b9

b9:
  %119 = load double, ptr %13, !tbaa !2
  %120 = load double, ptr %13, !tbaa !2
  %121 = fmul double %119, %120
  %122 = load double, ptr %11, !tbaa !2
  %123 = load double, ptr %11, !tbaa !2
  %124 = fmul double %122, %123
  %125 = fsub double %121, %124
  store double %125, ptr %9, !tbaa !2
  %126 = load double, ptr %13, !tbaa !2
  %127 = load double, ptr %11, !tbaa !2
  %128 = fmul double %126, %127
  store double %128, ptr %11, !tbaa !2
  %129 = load double, ptr %11, !tbaa !2
  %130 = load double, ptr %11, !tbaa !2
  %131 = fadd double %129, %130
  %132 = load double, ptr %20, !tbaa !2
  %133 = fadd double %131, %132
  store double %133, ptr %11, !tbaa !2
  %134 = load double, ptr %9, !tbaa !2
  %135 = load double, ptr %17, !tbaa !2
  %136 = fadd double %134, %135
  store double %136, ptr %13, !tbaa !2
  %137 = load i16, ptr %14, !tbaa !2
  %138 = add i16 %137, 1
  store i16 %138, ptr %14, !tbaa !2
  br label %b8

b10:
  br label %b7

b11:
  br label %b4

b12:
  br label %b13

b13:
  %139 = load double, ptr %17, !tbaa !2
  %140 = load double, ptr %24, !tbaa !2
  %141 = fadd double %139, %140
  store double %141, ptr %17, !tbaa !2
  %142 = load double, ptr %20, !tbaa !2
  %143 = load double, ptr %22, !tbaa !2
  %144 = fadd double %142, %143
  store double %144, ptr %20, !tbaa !2
  br label %b3

b15:
  %145 = load double, ptr %13, !tbaa !2
  %146 = load double, ptr %13, !tbaa !2
  %147 = fmul double %145, %146
  %148 = load double, ptr %11, !tbaa !2
  %149 = load double, ptr %11, !tbaa !2
  %150 = fmul double %148, %149
  %151 = fadd double %147, %150
  %152 = load double, ptr %5
  %153 = fcmp oge double %151, %152
  %154 = sext i1 %153 to i16
  %155 = load i16, ptr %14, !tbaa !2
  %156 = icmp eq i16 %155, 255
  %157 = sext i1 %156 to i16
  %158 = or i16 %154, %157
  %159 = icmp ne i16 %158, 0
  br i1 %159, label %b17, label %b16

b16:
  %160 = load double, ptr %13, !tbaa !2
  %161 = load double, ptr %13, !tbaa !2
  %162 = fmul double %160, %161
  %163 = load double, ptr %11, !tbaa !2
  %164 = load double, ptr %11, !tbaa !2
  %165 = fmul double %163, %164
  %166 = fsub double %162, %165
  store double %166, ptr %9, !tbaa !2
  %167 = load double, ptr %13, !tbaa !2
  %168 = load double, ptr %11, !tbaa !2
  %169 = fmul double %167, %168
  store double %169, ptr %11, !tbaa !2
  %170 = load double, ptr %11, !tbaa !2
  %171 = load double, ptr %11, !tbaa !2
  %172 = fadd double %170, %171
  %173 = load double, ptr %20, !tbaa !2
  %174 = fadd double %172, %173
  store double %174, ptr %11, !tbaa !2
  %175 = load double, ptr %9, !tbaa !2
  %176 = load double, ptr %17, !tbaa !2
  %177 = fadd double %175, %176
  store double %177, ptr %13, !tbaa !2
  %178 = load i16, ptr %14, !tbaa !2
  %179 = add i16 %178, 1
  store i16 %179, ptr %14, !tbaa !2
  br label %b15

b17:
  %180 = load i16, ptr %19, !tbaa !2
  %181 = load i16, ptr %18, !tbaa !2
  %182 = add i16 %180, %181
  %183 = load i16, ptr %14, !tbaa !2
  %184 = trunc i16 %183 to i8
  %185 = load i16, ptr @b$seg, !tbaa !2
  %186 = inttoptr i16 %185 to ptr addrspace(2)
  %187 = addrspacecast ptr addrspace(2) %186 to ptr addrspace(1)
  %188 = getelementptr i8, ptr addrspace(1) %187, i16 %182
  store i8 %184, ptr addrspace(1) %188
  ret void
}

define cc1000 void @FRACLINE2(ptr %0, ptr %1, ptr %2, ptr %3, ptr %4, ptr %5) addrspace(1) {
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
  store double 0.000000e+00, ptr %6
  store i16 0, ptr %7
  store double 0.000000e+00, ptr %8
  store i16 0, ptr %9
  store double 0.000000e+00, ptr %10
  store i16 0, ptr %11
  store double 0.000000e+00, ptr %12
  store i16 0, ptr %13
  store i16 0, ptr %14
  store double 0.000000e+00, ptr %15
  store i16 0, ptr %16
  store double 0.000000e+00, ptr %17
  store i16 0, ptr %18
  store double 0.000000e+00, ptr %19
  call void @llvm.memset.p0.i16(ptr %20, i8 0, i16 18, i1 false)
  call cc1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 320, i16 2, i16 257, ptr %20)
  %21 = load double, ptr %4
  %22 = load double, ptr %3
  %23 = fsub double %21, %22
  store i16 320, ptr %18, !tbaa !2
  %24 = load i16, ptr %18, !tbaa !2
  %25 = sitofp i16 %24 to double
  %26 = fdiv double %23, %25
  store double %26, ptr %19, !tbaa !2
  %27 = load double, ptr %2
  %28 = load double, ptr %1
  %29 = fsub double %27, %28
  store i16 320, ptr %16, !tbaa !2
  %30 = load i16, ptr %16, !tbaa !2
  %31 = sitofp i16 %30 to double
  %32 = fdiv double %29, %31
  store double %32, ptr %17, !tbaa !2
  %33 = load double, ptr %1
  store double %33, ptr %15, !tbaa !2
  store i16 0, ptr %14, !tbaa !2
  %34 = load i16, ptr %0
  %35 = mul i16 %34, 320
  store i16 %35, ptr %13, !tbaa !2
  %36 = load double, ptr %3
  store double %36, ptr %12, !tbaa !2
  br label %b3

b3:
  %37 = load i16, ptr %14, !tbaa !2
  %38 = load i16, ptr %13, !tbaa !2
  %39 = add i16 %37, %38
  %40 = sub i16 %39, 320
  %41 = load i16, ptr @b$seg, !tbaa !2
  %42 = inttoptr i16 %41 to ptr addrspace(2)
  %43 = addrspacecast ptr addrspace(2) %42 to ptr addrspace(1)
  %44 = getelementptr i8, ptr addrspace(1) %43, i16 %40
  %45 = load i8, ptr addrspace(1) %44
  %46 = zext i8 %45 to i16
  %47 = load i16, ptr %14, !tbaa !2
  %48 = load i16, ptr %13, !tbaa !2
  %49 = add i16 %47, %48
  %50 = add i16 %49, 320
  %51 = load i16, ptr @b$seg, !tbaa !2
  %52 = inttoptr i16 %51 to ptr addrspace(2)
  %53 = addrspacecast ptr addrspace(2) %52 to ptr addrspace(1)
  %54 = getelementptr i8, ptr addrspace(1) %53, i16 %50
  %55 = load i8, ptr addrspace(1) %54
  %56 = zext i8 %55 to i16
  %57 = icmp eq i16 %46, %56
  %58 = sext i1 %57 to i16
  %59 = icmp ne i16 %58, 0
  br i1 %59, label %b5, label %b6

b5:
  %60 = load i16, ptr %14, !tbaa !2
  %61 = load i16, ptr %13, !tbaa !2
  %62 = add i16 %60, %61
  %63 = sub i16 %62, 320
  %64 = load i16, ptr @b$seg, !tbaa !2
  %65 = inttoptr i16 %64 to ptr addrspace(2)
  %66 = addrspacecast ptr addrspace(2) %65 to ptr addrspace(1)
  %67 = getelementptr i8, ptr addrspace(1) %66, i16 %63
  %68 = load i8, ptr addrspace(1) %67
  %69 = zext i8 %68 to i16
  store i16 %69, ptr %11, !tbaa !2
  br label %b7

b6:
  store i16 0, ptr %9, !tbaa !2
  %70 = load i16, ptr %9, !tbaa !2
  %71 = sitofp i16 %70 to double
  store double %71, ptr %10, !tbaa !2
  store i16 0, ptr %7, !tbaa !2
  %72 = load i16, ptr %7, !tbaa !2
  %73 = sitofp i16 %72 to double
  store double %73, ptr %8, !tbaa !2
  store i16 0, ptr %11, !tbaa !2
  br label %b9

b7:
  %74 = load i16, ptr %14, !tbaa !2
  %75 = load i16, ptr %13, !tbaa !2
  %76 = add i16 %74, %75
  %77 = load i16, ptr %11, !tbaa !2
  %78 = trunc i16 %77 to i8
  %79 = load i16, ptr @b$seg, !tbaa !2
  %80 = inttoptr i16 %79 to ptr addrspace(2)
  %81 = addrspacecast ptr addrspace(2) %80 to ptr addrspace(1)
  %82 = getelementptr i8, ptr addrspace(1) %81, i16 %76
  store i8 %78, ptr addrspace(1) %82
  %83 = load i16, ptr %14, !tbaa !2
  %84 = add i16 %83, 1
  store i16 %84, ptr %14, !tbaa !2
  %85 = load i16, ptr %14, !tbaa !2
  %86 = icmp sge i16 %85, 320
  %87 = sext i1 %86 to i16
  %88 = icmp ne i16 %87, 0
  br i1 %88, label %b11, label %b12

b8:
  %89 = load double, ptr %10, !tbaa !2
  %90 = load double, ptr %10, !tbaa !2
  %91 = fmul double %89, %90
  %92 = load double, ptr %8, !tbaa !2
  %93 = load double, ptr %8, !tbaa !2
  %94 = fmul double %92, %93
  %95 = fadd double %91, %94
  %96 = load double, ptr %5
  %97 = fcmp oge double %95, %96
  %98 = sext i1 %97 to i16
  %99 = load i16, ptr %11, !tbaa !2
  %100 = icmp eq i16 %99, 255
  %101 = sext i1 %100 to i16
  %102 = or i16 %98, %101
  %103 = icmp ne i16 %102, 0
  br i1 %103, label %b10, label %b9

b9:
  %104 = load double, ptr %10, !tbaa !2
  %105 = load double, ptr %10, !tbaa !2
  %106 = fmul double %104, %105
  %107 = load double, ptr %8, !tbaa !2
  %108 = load double, ptr %8, !tbaa !2
  %109 = fmul double %107, %108
  %110 = fsub double %106, %109
  store double %110, ptr %6, !tbaa !2
  %111 = load double, ptr %10, !tbaa !2
  %112 = load double, ptr %8, !tbaa !2
  %113 = fmul double %111, %112
  store double %113, ptr %8, !tbaa !2
  %114 = load double, ptr %8, !tbaa !2
  %115 = load double, ptr %8, !tbaa !2
  %116 = fadd double %114, %115
  %117 = load double, ptr %15, !tbaa !2
  %118 = fadd double %116, %117
  store double %118, ptr %8, !tbaa !2
  %119 = load double, ptr %6, !tbaa !2
  %120 = load double, ptr %12, !tbaa !2
  %121 = fadd double %119, %120
  store double %121, ptr %10, !tbaa !2
  %122 = load i16, ptr %11, !tbaa !2
  %123 = add i16 %122, 1
  store i16 %123, ptr %11, !tbaa !2
  br label %b8

b10:
  br label %b7

b11:
  call cc1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %20)
  ret void

b12:
  br label %b13

b13:
  %124 = load double, ptr %12, !tbaa !2
  %125 = load double, ptr %19, !tbaa !2
  %126 = fadd double %124, %125
  store double %126, ptr %12, !tbaa !2
  %127 = load double, ptr %15, !tbaa !2
  %128 = load double, ptr %17, !tbaa !2
  %129 = fadd double %127, %128
  store double %129, ptr %15, !tbaa !2
  br label %b3
}

define cc1000 void @FRACTALEFFECT(ptr %0) addrspace(1) {
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
  store i16 0, ptr %1
  store i16 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %4
  store i16 0, ptr %5
  store i16 0, ptr %6
  store i16 0, ptr %7
  store i16 0, ptr %8
  store i16 0, ptr %9
  store i16 0, ptr %10
  store i16 0, ptr %11
  store i16 0, ptr %12
  store i16 0, ptr %13
  store i16 0, ptr %14
  store i16 0, ptr %15
  store i16 0, ptr %16
  store i16 0, ptr %17
  store i16 0, ptr %18
  store i16 0, ptr %19
  store i16 0, ptr %20
  store i16 0, ptr %21
  store i16 0, ptr %22
  store i16 0, ptr %23
  store i16 0, ptr %24
  store i16 0, ptr %25
  store i16 0, ptr %26
  store i16 0, ptr %27
  store double 0.000000e+00, ptr %28
  store i16 0, ptr %29
  store double 0.000000e+00, ptr %30
  store i16 0, ptr %31
  store double 0.000000e+00, ptr %32
  store i16 0, ptr %33
  store double 0.000000e+00, ptr %34
  store i16 0, ptr %35
  store double 0.000000e+00, ptr %36
  store i16 0, ptr %37
  store double 0.000000e+00, ptr %38
  store i16 0, ptr %39
  store double 0.000000e+00, ptr %40
  store i16 0, ptr %41
  store i16 0, ptr %42
  store i16 0, ptr %43
  store i16 0, ptr %44
  store i16 0, ptr %45
  store i16 0, ptr %46
  store i16 0, ptr %47
  store i16 0, ptr %48
  store double 0.000000e+00, ptr %49
  store i16 0, ptr %50
  call void @llvm.memset.p0.i16(ptr %51, i8 0, i16 22, i1 false)
  call cc1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 160, i16 0, i16 100, i16 2, i16 258, ptr %51)
  %52 = sub i16 0, 100
  store i16 %52, ptr %50, !tbaa !2
  store i16 1, ptr %48, !tbaa !2
  %53 = load i16, ptr %48, !tbaa !2
  %54 = sitofp i16 %53 to double
  store double %54, ptr %49, !tbaa !2
  store i16 1, ptr %47, !tbaa !2
  %55 = load i16, ptr %0
  store i16 %55, ptr %46, !tbaa !2
  store i16 1, ptr %45, !tbaa !2
  br label %b2

b2:
  %56 = load i16, ptr %45, !tbaa !2
  %57 = icmp sge i16 %56, 0
  %58 = sext i1 %57 to i16
  %59 = icmp ne i16 %58, 0
  br i1 %59, label %b3, label %b4

b3:
  %60 = load i16, ptr %47, !tbaa !2
  %61 = load i16, ptr %46, !tbaa !2
  %62 = icmp sle i16 %60, %61
  %63 = sext i1 %62 to i16
  %64 = icmp ne i16 %63, 0
  br i1 %64, label %b5, label %b6

b4:
  %65 = load i16, ptr %47, !tbaa !2
  %66 = load i16, ptr %46, !tbaa !2
  %67 = icmp sge i16 %65, %66
  %68 = sext i1 %67 to i16
  %69 = icmp ne i16 %68, 0
  br i1 %69, label %b5, label %b6

b5:
  %70 = load i16, ptr %50, !tbaa !2
  store i16 %70, ptr %44, !tbaa !2
  %71 = load i16, ptr %50, !tbaa !2
  %72 = add i16 %71, 1
  store i16 %72, ptr %43, !tbaa !2
  %73 = load i16, ptr %50, !tbaa !2
  %74 = add i16 %73, 2
  store i16 %74, ptr %42, !tbaa !2
  %75 = load i16, ptr %50, !tbaa !2
  %76 = add i16 %75, 3
  store i16 %76, ptr %41, !tbaa !2
  %77 = load i16, ptr %44, !tbaa !2
  %78 = load double, ptr %49, !tbaa !2
  store i16 %77, ptr %39, !tbaa !2
  %79 = load i16, ptr %39, !tbaa !2
  %80 = sitofp i16 %79 to double
  %81 = fdiv double %80, %78
  %82 = load double, ptr @$float4, !tbaa !2
  %83 = fadd double %81, %82
  store double %83, ptr %40, !tbaa !2
  %84 = load i16, ptr %43, !tbaa !2
  %85 = load double, ptr %49, !tbaa !2
  store i16 %84, ptr %37, !tbaa !2
  %86 = load i16, ptr %37, !tbaa !2
  %87 = sitofp i16 %86 to double
  %88 = fdiv double %87, %85
  %89 = load double, ptr @$float4, !tbaa !2
  %90 = fadd double %88, %89
  store double %90, ptr %38, !tbaa !2
  %91 = load i16, ptr %42, !tbaa !2
  %92 = load double, ptr %49, !tbaa !2
  store i16 %91, ptr %35, !tbaa !2
  %93 = load i16, ptr %35, !tbaa !2
  %94 = sitofp i16 %93 to double
  %95 = fdiv double %94, %92
  %96 = load double, ptr @$float4, !tbaa !2
  %97 = fadd double %95, %96
  store double %97, ptr %36, !tbaa !2
  %98 = load i16, ptr %41, !tbaa !2
  %99 = load double, ptr %49, !tbaa !2
  store i16 %98, ptr %33, !tbaa !2
  %100 = load i16, ptr %33, !tbaa !2
  %101 = sitofp i16 %100 to double
  %102 = fdiv double %101, %99
  %103 = load double, ptr @$float4, !tbaa !2
  %104 = fadd double %102, %103
  store double %104, ptr %34, !tbaa !2
  %105 = load double, ptr @$float5, !tbaa !2
  %106 = load double, ptr %49, !tbaa !2
  store i16 160, ptr %31, !tbaa !2
  %107 = load i16, ptr %31, !tbaa !2
  %108 = sitofp i16 %107 to double
  %109 = fdiv double %108, %106
  %110 = fsub double %105, %109
  store double %110, ptr %32, !tbaa !2
  %111 = load double, ptr @$float5, !tbaa !2
  %112 = load double, ptr %49, !tbaa !2
  store i16 160, ptr %29, !tbaa !2
  %113 = load i16, ptr %29, !tbaa !2
  %114 = sitofp i16 %113 to double
  %115 = fdiv double %114, %112
  %116 = fadd double %111, %115
  store double %116, ptr %30, !tbaa !2
  %117 = mul i16 0, 2
  %118 = getelementptr i8, ptr @"FRACTAL1%", i16 2
  %119 = load i16, ptr %118, !tbaa !2
  %120 = add i16 0, %117
  %121 = inttoptr i16 %119 to ptr addrspace(2)
  %122 = addrspacecast ptr addrspace(2) %121 to ptr addrspace(1)
  %123 = getelementptr i8, ptr addrspace(1) %122, i16 %120
  %124 = addrspacecast ptr addrspace(1) %123 to ptr addrspace(2)
  %125 = ptrtoint ptr addrspace(2) %124 to i16
  store i16 %125, ptr @b$seg, !tbaa !2
  store i16 4, ptr %27, !tbaa !2
  %126 = load i16, ptr %27, !tbaa !2
  %127 = sitofp i16 %126 to double
  store double %127, ptr %28, !tbaa !2
  %128 = load i16, ptr %44, !tbaa !2
  %129 = add i16 %128, 100
  store i16 %129, ptr %26, !tbaa !2
  call cc1000 addrspace(1) void @FRACLINE(ptr %26, ptr %40, ptr %40, ptr %32, ptr %30, ptr %28)
  %130 = load i16, ptr %42, !tbaa !2
  %131 = add i16 %130, 100
  store i16 %131, ptr %25, !tbaa !2
  call cc1000 addrspace(1) void @FRACLINE(ptr %25, ptr %36, ptr %36, ptr %32, ptr %30, ptr %28)
  %132 = load i16, ptr %43, !tbaa !2
  %133 = add i16 %132, 100
  store i16 %133, ptr %24, !tbaa !2
  call cc1000 addrspace(1) void @FRACLINE2(ptr %24, ptr %38, ptr %38, ptr %32, ptr %30, ptr %28)
  %134 = load i16, ptr %41, !tbaa !2
  %135 = add i16 %134, 100
  store i16 %135, ptr %23, !tbaa !2
  call cc1000 addrspace(1) void @FRACLINE2(ptr %23, ptr %34, ptr %34, ptr %32, ptr %30, ptr %28)
  %136 = load i16, ptr %50, !tbaa !2
  %137 = add i16 %136, 4
  store i16 %137, ptr %50, !tbaa !2
  %138 = load i16, ptr %50, !tbaa !2
  %139 = icmp sge i16 %138, 100
  %140 = sext i1 %139 to i16
  %141 = icmp ne i16 %140, 0
  br i1 %141, label %b7, label %b8

b6:
  call cc1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %51)
  ret void

b7:
  %142 = sub i16 0, 100
  store i16 %142, ptr %50, !tbaa !2
  %143 = load double, ptr %49, !tbaa !2
  store i16 2, ptr %22, !tbaa !2
  %144 = load i16, ptr %22, !tbaa !2
  %145 = sitofp i16 %144 to double
  %146 = fmul double %143, %145
  store double %146, ptr %49, !tbaa !2
  store i16 16080, ptr %21, !tbaa !2
  store i16 0, ptr %20, !tbaa !2
  store i16 99, ptr %19, !tbaa !2
  store i16 1, ptr %18, !tbaa !2
  br label %b10

b8:
  br label %b9

b9:
  %147 = mul i16 0, 2
  %148 = getelementptr i8, ptr @"FRACTAL2%", i16 2
  %149 = load i16, ptr %148, !tbaa !2
  %150 = add i16 0, %147
  %151 = inttoptr i16 %149 to ptr addrspace(2)
  %152 = addrspacecast ptr addrspace(2) %151 to ptr addrspace(1)
  %153 = getelementptr i8, ptr addrspace(1) %152, i16 %150
  %154 = addrspacecast ptr addrspace(1) %153 to ptr addrspace(2)
  %155 = ptrtoint ptr addrspace(2) %154 to i16
  store i16 %155, ptr @b$seg, !tbaa !2
  %156 = load i16, ptr %47, !tbaa !2
  %157 = sext i16 %156 to i32
  %158 = srem i32 %157, 50
  %159 = trunc i32 %158 to i16
  store i16 %159, ptr %8, !tbaa !2
  %160 = load i16, ptr %8, !tbaa !2
  %161 = load float, ptr @$float6, !tbaa !2
  store i16 %160, ptr %6, !tbaa !2
  %162 = load i16, ptr %6, !tbaa !2
  %163 = sitofp i16 %162 to float
  %164 = fmul float %163, %161
  %165 = call i16 @llvm.lrint.i16.f32(float %164)
  store i16 %165, ptr %7, !tbaa !2
  %166 = load i16, ptr %8, !tbaa !2
  store i16 %166, ptr %5, !tbaa !2
  %167 = load i16, ptr %8, !tbaa !2
  %168 = load float, ptr @$float6, !tbaa !2
  store i16 %167, ptr %3, !tbaa !2
  %169 = load i16, ptr %3, !tbaa !2
  %170 = sitofp i16 %169 to float
  %171 = fmul float %170, %168
  store i16 320, ptr %2, !tbaa !2
  %172 = load i16, ptr %2, !tbaa !2
  %173 = sitofp i16 %172 to float
  %174 = fsub float %173, %171
  %175 = call i16 @llvm.lrint.i16.f32(float %174)
  store i16 %175, ptr %4, !tbaa !2
  %176 = load i16, ptr %8, !tbaa !2
  %177 = sub i16 200, %176
  store i16 %177, ptr %1, !tbaa !2
  call cc1000 addrspace(1) void @RENDER(ptr %7, ptr %5, ptr %4, ptr %1)
  %178 = call cc1000 addrspace(1) ptr @llrm.qb.B$INKY()
  %179 = call cc1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %178, ptr @$string7)
  %180 = icmp sgt i16 %179, 0
  %181 = sext i1 %180 to i16
  %182 = icmp ne i16 %181, 0
  br i1 %182, label %b35, label %b36

b10:
  %183 = load i16, ptr %18, !tbaa !2
  %184 = icmp sge i16 %183, 0
  %185 = sext i1 %184 to i16
  %186 = icmp ne i16 %185, 0
  br i1 %186, label %b11, label %b12

b11:
  %187 = load i16, ptr %20, !tbaa !2
  %188 = load i16, ptr %19, !tbaa !2
  %189 = icmp sle i16 %187, %188
  %190 = sext i1 %189 to i16
  %191 = icmp ne i16 %190, 0
  br i1 %191, label %b13, label %b14

b12:
  %192 = load i16, ptr %20, !tbaa !2
  %193 = load i16, ptr %19, !tbaa !2
  %194 = icmp sge i16 %192, %193
  %195 = sext i1 %194 to i16
  %196 = icmp ne i16 %195, 0
  br i1 %196, label %b13, label %b14

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
  %197 = load i16, ptr %15, !tbaa !2
  %198 = icmp sge i16 %197, 0
  %199 = sext i1 %198 to i16
  %200 = icmp ne i16 %199, 0
  br i1 %200, label %b16, label %b17

b16:
  %201 = load i16, ptr %17, !tbaa !2
  %202 = load i16, ptr %16, !tbaa !2
  %203 = icmp sle i16 %201, %202
  %204 = sext i1 %203 to i16
  %205 = icmp ne i16 %204, 0
  br i1 %205, label %b18, label %b19

b17:
  %206 = load i16, ptr %17, !tbaa !2
  %207 = load i16, ptr %16, !tbaa !2
  %208 = icmp sge i16 %206, %207
  %209 = sext i1 %208 to i16
  %210 = icmp ne i16 %209, 0
  br i1 %210, label %b18, label %b19

b18:
  %211 = load i16, ptr %17, !tbaa !2
  %212 = load i16, ptr %20, !tbaa !2
  %213 = mul i16 %212, 161
  %214 = add i16 %213, %211
  %215 = mul i16 %214, 2
  %216 = getelementptr i8, ptr %51, i16 2
  %217 = load i16, ptr %216, !tbaa !2
  %218 = add i16 0, %215
  %219 = inttoptr i16 %217 to ptr addrspace(2)
  %220 = addrspacecast ptr addrspace(2) %219 to ptr addrspace(1)
  %221 = getelementptr i8, ptr addrspace(1) %220, i16 %218
  %222 = load i16, ptr %21, !tbaa !2
  %223 = load i16, ptr @b$seg, !tbaa !2
  %224 = inttoptr i16 %223 to ptr addrspace(2)
  %225 = addrspacecast ptr addrspace(2) %224 to ptr addrspace(1)
  %226 = getelementptr i8, ptr addrspace(1) %225, i16 %222
  %227 = load i8, ptr addrspace(1) %226
  %228 = zext i8 %227 to i16
  store i16 %228, ptr addrspace(1) %221, !tbaa !4
  %229 = load i16, ptr %21, !tbaa !2
  %230 = add i16 %229, 1
  store i16 %230, ptr %21, !tbaa !2
  %231 = load i16, ptr %17, !tbaa !2
  %232 = load i16, ptr %15, !tbaa !2
  %233 = add i16 %231, %232
  store i16 %233, ptr %17, !tbaa !2
  br label %b15

b19:
  %234 = load i16, ptr %21, !tbaa !2
  %235 = add i16 %234, 160
  store i16 %235, ptr %21, !tbaa !2
  %236 = load i16, ptr %20, !tbaa !2
  %237 = load i16, ptr %18, !tbaa !2
  %238 = add i16 %236, %237
  store i16 %238, ptr %20, !tbaa !2
  br label %b10

b20:
  %239 = load i16, ptr %13, !tbaa !2
  %240 = icmp sge i16 %239, 0
  %241 = sext i1 %240 to i16
  %242 = icmp ne i16 %241, 0
  br i1 %242, label %b21, label %b22

b21:
  %243 = load i16, ptr %17, !tbaa !2
  %244 = load i16, ptr %14, !tbaa !2
  %245 = icmp sle i16 %243, %244
  %246 = sext i1 %245 to i16
  %247 = icmp ne i16 %246, 0
  br i1 %247, label %b23, label %b24

b22:
  %248 = load i16, ptr %17, !tbaa !2
  %249 = load i16, ptr %14, !tbaa !2
  %250 = icmp sge i16 %248, %249
  %251 = sext i1 %250 to i16
  %252 = icmp ne i16 %251, 0
  br i1 %252, label %b23, label %b24

b23:
  %253 = load i16, ptr %17, !tbaa !2
  %254 = mul i16 %253, 2
  %255 = getelementptr i8, ptr @"FRACTAL2%", i16 2
  %256 = load i16, ptr %255, !tbaa !2
  %257 = add i16 0, %254
  %258 = inttoptr i16 %256 to ptr addrspace(2)
  %259 = addrspacecast ptr addrspace(2) %258 to ptr addrspace(1)
  %260 = getelementptr i8, ptr addrspace(1) %259, i16 %257
  %261 = load i16, ptr %17, !tbaa !2
  %262 = mul i16 %261, 2
  %263 = getelementptr i8, ptr @"FRACTAL1%", i16 2
  %264 = load i16, ptr %263, !tbaa !2
  %265 = add i16 0, %262
  %266 = inttoptr i16 %264 to ptr addrspace(2)
  %267 = addrspacecast ptr addrspace(2) %266 to ptr addrspace(1)
  %268 = getelementptr i8, ptr addrspace(1) %267, i16 %265
  %269 = load i16, ptr addrspace(1) %268, !tbaa !4
  store i16 %269, ptr addrspace(1) %260, !tbaa !4
  %270 = load i16, ptr %17, !tbaa !2
  %271 = mul i16 %270, 2
  %272 = getelementptr i8, ptr @"FRACTAL1%", i16 2
  %273 = load i16, ptr %272, !tbaa !2
  %274 = add i16 0, %271
  %275 = inttoptr i16 %273 to ptr addrspace(2)
  %276 = addrspacecast ptr addrspace(2) %275 to ptr addrspace(1)
  %277 = getelementptr i8, ptr addrspace(1) %276, i16 %274
  store i16 0, ptr addrspace(1) %277, !tbaa !4
  %278 = load i16, ptr %17, !tbaa !2
  %279 = load i16, ptr %13, !tbaa !2
  %280 = add i16 %278, %279
  store i16 %280, ptr %17, !tbaa !2
  br label %b20

b24:
  store i16 0, ptr %20, !tbaa !2
  store i16 99, ptr %12, !tbaa !2
  store i16 1, ptr %11, !tbaa !2
  br label %b25

b25:
  %281 = load i16, ptr %11, !tbaa !2
  %282 = icmp sge i16 %281, 0
  %283 = sext i1 %282 to i16
  %284 = icmp ne i16 %283, 0
  br i1 %284, label %b26, label %b27

b26:
  %285 = load i16, ptr %20, !tbaa !2
  %286 = load i16, ptr %12, !tbaa !2
  %287 = icmp sle i16 %285, %286
  %288 = sext i1 %287 to i16
  %289 = icmp ne i16 %288, 0
  br i1 %289, label %b28, label %b29

b27:
  %290 = load i16, ptr %20, !tbaa !2
  %291 = load i16, ptr %12, !tbaa !2
  %292 = icmp sge i16 %290, %291
  %293 = sext i1 %292 to i16
  %294 = icmp ne i16 %293, 0
  br i1 %294, label %b28, label %b29

b28:
  store i16 0, ptr %17, !tbaa !2
  store i16 159, ptr %10, !tbaa !2
  store i16 1, ptr %9, !tbaa !2
  br label %b30

b29:
  br label %b9

b30:
  %295 = load i16, ptr %9, !tbaa !2
  %296 = icmp sge i16 %295, 0
  %297 = sext i1 %296 to i16
  %298 = icmp ne i16 %297, 0
  br i1 %298, label %b31, label %b32

b31:
  %299 = load i16, ptr %17, !tbaa !2
  %300 = load i16, ptr %10, !tbaa !2
  %301 = icmp sle i16 %299, %300
  %302 = sext i1 %301 to i16
  %303 = icmp ne i16 %302, 0
  br i1 %303, label %b33, label %b34

b32:
  %304 = load i16, ptr %17, !tbaa !2
  %305 = load i16, ptr %10, !tbaa !2
  %306 = icmp sge i16 %304, %305
  %307 = sext i1 %306 to i16
  %308 = icmp ne i16 %307, 0
  br i1 %308, label %b33, label %b34

b33:
  %309 = load i16, ptr %17, !tbaa !2
  %310 = mul i16 %309, 2
  %311 = load i16, ptr %20, !tbaa !2
  %312 = mul i16 %311, 640
  %313 = add i16 %310, %312
  %314 = load i16, ptr %17, !tbaa !2
  %315 = load i16, ptr %20, !tbaa !2
  %316 = mul i16 %315, 161
  %317 = add i16 %316, %314
  %318 = mul i16 %317, 2
  %319 = getelementptr i8, ptr %51, i16 2
  %320 = load i16, ptr %319, !tbaa !2
  %321 = add i16 0, %318
  %322 = inttoptr i16 %320 to ptr addrspace(2)
  %323 = addrspacecast ptr addrspace(2) %322 to ptr addrspace(1)
  %324 = getelementptr i8, ptr addrspace(1) %323, i16 %321
  %325 = load i16, ptr addrspace(1) %324, !tbaa !4
  %326 = trunc i16 %325 to i8
  %327 = load i16, ptr @b$seg, !tbaa !2
  %328 = inttoptr i16 %327 to ptr addrspace(2)
  %329 = addrspacecast ptr addrspace(2) %328 to ptr addrspace(1)
  %330 = getelementptr i8, ptr addrspace(1) %329, i16 %313
  store i8 %326, ptr addrspace(1) %330
  %331 = load i16, ptr %17, !tbaa !2
  %332 = load i16, ptr %9, !tbaa !2
  %333 = add i16 %331, %332
  store i16 %333, ptr %17, !tbaa !2
  br label %b30

b34:
  %334 = load i16, ptr %20, !tbaa !2
  %335 = load i16, ptr %11, !tbaa !2
  %336 = add i16 %334, %335
  store i16 %336, ptr %20, !tbaa !2
  br label %b25

b35:
  call cc1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %51)
  ret void

b36:
  br label %b37

b37:
  %337 = load i16, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %338 = add i16 %337, 1
  store i16 %338, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %339 = load i16, ptr %47, !tbaa !2
  %340 = load i16, ptr %45, !tbaa !2
  %341 = add i16 %339, %340
  store i16 %341, ptr %47, !tbaa !2
  br label %b2
}

define cc1000 void @OHCANADA(ptr %0) addrspace(1) {
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
  store i16 0, ptr %1
  store i16 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %4
  store i16 0, ptr %5
  store i16 0, ptr %6
  store i16 0, ptr %7
  store i16 0, ptr %8
  store i16 0, ptr %9
  store i16 0, ptr %9, !tbaa !2
  store i16 255, ptr %8, !tbaa !2
  store i16 1, ptr %7, !tbaa !2
  br label %b2

b2:
  %10 = load i16, ptr %7, !tbaa !2
  %11 = icmp sge i16 %10, 0
  %12 = sext i1 %11 to i16
  %13 = icmp ne i16 %12, 0
  br i1 %13, label %b3, label %b4

b3:
  %14 = load i16, ptr %9, !tbaa !2
  %15 = load i16, ptr %8, !tbaa !2
  %16 = icmp sle i16 %14, %15
  %17 = sext i1 %16 to i16
  %18 = icmp ne i16 %17, 0
  br i1 %18, label %b5, label %b6

b4:
  %19 = load i16, ptr %9, !tbaa !2
  %20 = load i16, ptr %8, !tbaa !2
  %21 = icmp sge i16 %19, %20
  %22 = sext i1 %21 to i16
  %23 = icmp ne i16 %22, 0
  br i1 %23, label %b5, label %b6

b5:
  %24 = load i16, ptr %9, !tbaa !2
  %25 = trunc i16 %24 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %25)
  call void @llrm.ia16.out.i8(i16 969, i8 63)
  call void @llrm.ia16.out.i8(i16 969, i8 63)
  call void @llrm.ia16.out.i8(i16 969, i8 63)
  %26 = load i16, ptr %9, !tbaa !2
  %27 = load i16, ptr %7, !tbaa !2
  %28 = add i16 %26, %27
  store i16 %28, ptr %9, !tbaa !2
  br label %b2

b6:
  store i16 -24576, ptr @b$seg, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$BLOD(ptr @$string8, i16 0, i16 1)
  store i16 0, ptr %9, !tbaa !2
  store i16 255, ptr %6, !tbaa !2
  store i16 1, ptr %5, !tbaa !2
  br label %b7

b7:
  %29 = load i16, ptr %5, !tbaa !2
  %30 = icmp sge i16 %29, 0
  %31 = sext i1 %30 to i16
  %32 = icmp ne i16 %31, 0
  br i1 %32, label %b8, label %b9

b8:
  %33 = load i16, ptr %9, !tbaa !2
  %34 = load i16, ptr %6, !tbaa !2
  %35 = icmp sle i16 %33, %34
  %36 = sext i1 %35 to i16
  %37 = icmp ne i16 %36, 0
  br i1 %37, label %b10, label %b11

b9:
  %38 = load i16, ptr %9, !tbaa !2
  %39 = load i16, ptr %6, !tbaa !2
  %40 = icmp sge i16 %38, %39
  %41 = sext i1 %40 to i16
  %42 = icmp ne i16 %41, 0
  br i1 %42, label %b10, label %b11

b10:
  %43 = load i16, ptr %9, !tbaa !2
  %44 = trunc i16 %43 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %44)
  call void @llrm.ia16.out.i8(i16 969, i8 63)
  %45 = load i16, ptr %9, !tbaa !2
  %46 = sext i16 %45 to i32
  %47 = sdiv i32 %46, 4
  %48 = trunc i32 %47 to i16
  %49 = trunc i16 %48 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %49)
  %50 = load i16, ptr %9, !tbaa !2
  %51 = sext i16 %50 to i32
  %52 = sdiv i32 %51, 4
  %53 = trunc i32 %52 to i16
  %54 = trunc i16 %53 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %54)
  %55 = load i16, ptr %9, !tbaa !2
  %56 = load i16, ptr %5, !tbaa !2
  %57 = add i16 %55, %56
  store i16 %57, ptr %9, !tbaa !2
  br label %b7

b11:
  store i16 256, ptr %4, !tbaa !2
  call cc1000 addrspace(1) void @UNWHITEFADE(ptr %4)
  %58 = load i16, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %59 = add i16 %58, 16
  store i16 %59, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  store i16 1, ptr %3, !tbaa !2
  %60 = load i16, ptr %0
  store i16 %60, ptr %2, !tbaa !2
  store i16 1, ptr %1, !tbaa !2
  br label %b12

b12:
  %61 = load i16, ptr %1, !tbaa !2
  %62 = icmp sge i16 %61, 0
  %63 = sext i1 %62 to i16
  %64 = icmp ne i16 %63, 0
  br i1 %64, label %b13, label %b14

b13:
  %65 = load i16, ptr %3, !tbaa !2
  %66 = load i16, ptr %2, !tbaa !2
  %67 = icmp sle i16 %65, %66
  %68 = sext i1 %67 to i16
  %69 = icmp ne i16 %68, 0
  br i1 %69, label %b15, label %b16

b14:
  %70 = load i16, ptr %3, !tbaa !2
  %71 = load i16, ptr %2, !tbaa !2
  %72 = icmp sge i16 %70, %71
  %73 = sext i1 %72 to i16
  %74 = icmp ne i16 %73, 0
  br i1 %74, label %b15, label %b16

b15:
  %75 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %75, ptr @"BENCHFRAME&", !tbaa !2
  %76 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %76, ptr @"BENCHFRAME&", !tbaa !2
  %77 = load i16, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %78 = add i16 %77, 1
  store i16 %78, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %79 = call cc1000 addrspace(1) ptr @llrm.qb.B$INKY()
  %80 = call cc1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %79, ptr @$string9)
  %81 = icmp sgt i16 %80, 0
  %82 = sext i1 %81 to i16
  %83 = icmp ne i16 %82, 0
  br i1 %83, label %b17, label %b18

b16:
  ret void

b17:
  ret void

b18:
  br label %b19

b19:
  %84 = load i16, ptr %3, !tbaa !2
  %85 = load i16, ptr %1, !tbaa !2
  %86 = add i16 %84, %85
  store i16 %86, ptr %3, !tbaa !2
  br label %b12
}

define cc1000 void @PLASMA(ptr %0) addrspace(1) {
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
  store i16 0, ptr %1
  store i16 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %4
  store i16 0, ptr %5
  store i16 0, ptr %6
  store i16 0, ptr %7
  store i16 0, ptr %8
  store i16 0, ptr %9
  store i16 0, ptr %10
  store i16 0, ptr %11
  store i16 0, ptr %12
  store i16 0, ptr %13
  store i16 0, ptr %14
  store i16 0, ptr %15
  store i16 0, ptr %16
  store i16 0, ptr %17
  store i16 0, ptr %18
  store i16 0, ptr %19
  store i16 0, ptr %20
  store i16 0, ptr %21
  store i16 0, ptr %22
  store i16 0, ptr %23
  store i16 0, ptr %24
  store i16 0, ptr %25
  store i16 0, ptr %26
  call void @llvm.memset.p0.i16(ptr %27, i8 0, i16 22, i1 false)
  call void @llvm.memset.p0.i16(ptr %28, i8 0, i16 18, i1 false)
  call void @llvm.memset.p0.i16(ptr %29, i8 0, i16 18, i1 false)
  call void @llvm.memset.p0.i16(ptr %30, i8 0, i16 18, i1 false)
  call cc1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 320, i16 2, i16 257, ptr %30)
  call cc1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 320, i16 2, i16 257, ptr %29)
  call cc1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 512, i16 2, i16 257, ptr %28)
  call cc1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 128, i16 0, i16 128, i16 2, i16 258, ptr %27)
  store i16 -24576, ptr @b$seg, !tbaa !2
  store i16 0, ptr %26, !tbaa !2
  store i16 512, ptr %25, !tbaa !2
  store i16 1, ptr %24, !tbaa !2
  br label %b2

b2:
  %31 = load i16, ptr %24, !tbaa !2
  %32 = icmp sge i16 %31, 0
  %33 = sext i1 %32 to i16
  %34 = icmp ne i16 %33, 0
  br i1 %34, label %b3, label %b4

b3:
  %35 = load i16, ptr %26, !tbaa !2
  %36 = load i16, ptr %25, !tbaa !2
  %37 = icmp sle i16 %35, %36
  %38 = sext i1 %37 to i16
  %39 = icmp ne i16 %38, 0
  br i1 %39, label %b5, label %b6

b4:
  %40 = load i16, ptr %26, !tbaa !2
  %41 = load i16, ptr %25, !tbaa !2
  %42 = icmp sge i16 %40, %41
  %43 = sext i1 %42 to i16
  %44 = icmp ne i16 %43, 0
  br i1 %44, label %b5, label %b6

b5:
  %45 = load i16, ptr %26, !tbaa !2
  %46 = mul i16 %45, 2
  %47 = getelementptr i8, ptr %28, i16 2
  %48 = load i16, ptr %47, !tbaa !2
  %49 = add i16 0, %46
  %50 = inttoptr i16 %48 to ptr addrspace(2)
  %51 = addrspacecast ptr addrspace(2) %50 to ptr addrspace(1)
  %52 = getelementptr i8, ptr addrspace(1) %51, i16 %49
  %53 = load i16, ptr %26, !tbaa !2
  %54 = load float, ptr @$float10, !tbaa !2
  store i16 %53, ptr %23, !tbaa !2
  %55 = load i16, ptr %23, !tbaa !2
  %56 = sitofp i16 %55 to float
  %57 = fmul float %56, %54
  store i16 256, ptr %22, !tbaa !2
  %58 = load i16, ptr %22, !tbaa !2
  %59 = sitofp i16 %58 to float
  %60 = fdiv float %57, %59
  %61 = call float @llvm.sin.f32(float %60)
  store i16 32, ptr %21, !tbaa !2
  %62 = load i16, ptr %21, !tbaa !2
  %63 = sitofp i16 %62 to float
  %64 = fmul float %61, %63
  store i16 32, ptr %20, !tbaa !2
  %65 = load i16, ptr %20, !tbaa !2
  %66 = sitofp i16 %65 to float
  %67 = fadd float %64, %66
  %68 = call i16 @llvm.lrint.i16.f32(float %67)
  store i16 %68, ptr addrspace(1) %52, !tbaa !4
  %69 = load i16, ptr %26, !tbaa !2
  %70 = load i16, ptr %24, !tbaa !2
  %71 = add i16 %69, %70
  store i16 %71, ptr %26, !tbaa !2
  br label %b2

b6:
  store i16 1, ptr %19, !tbaa !2
  %72 = load i16, ptr %0
  store i16 %72, ptr %18, !tbaa !2
  store i16 1, ptr %17, !tbaa !2
  br label %b7

b7:
  %73 = load i16, ptr %17, !tbaa !2
  %74 = icmp sge i16 %73, 0
  %75 = sext i1 %74 to i16
  %76 = icmp ne i16 %75, 0
  br i1 %76, label %b8, label %b9

b8:
  %77 = load i16, ptr %19, !tbaa !2
  %78 = load i16, ptr %18, !tbaa !2
  %79 = icmp sle i16 %77, %78
  %80 = sext i1 %79 to i16
  %81 = icmp ne i16 %80, 0
  br i1 %81, label %b10, label %b11

b9:
  %82 = load i16, ptr %19, !tbaa !2
  %83 = load i16, ptr %18, !tbaa !2
  %84 = icmp sge i16 %82, %83
  %85 = sext i1 %84 to i16
  %86 = icmp ne i16 %85, 0
  br i1 %86, label %b10, label %b11

b10:
  store i16 0, ptr %26, !tbaa !2
  store i16 320, ptr %16, !tbaa !2
  store i16 1, ptr %15, !tbaa !2
  br label %b12

b11:
  call cc1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %27)
  call cc1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %28)
  call cc1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %30)
  call cc1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %29)
  ret void

b12:
  %87 = load i16, ptr %15, !tbaa !2
  %88 = icmp sge i16 %87, 0
  %89 = sext i1 %88 to i16
  %90 = icmp ne i16 %89, 0
  br i1 %90, label %b13, label %b14

b13:
  %91 = load i16, ptr %26, !tbaa !2
  %92 = load i16, ptr %16, !tbaa !2
  %93 = icmp sle i16 %91, %92
  %94 = sext i1 %93 to i16
  %95 = icmp ne i16 %94, 0
  br i1 %95, label %b15, label %b16

b14:
  %96 = load i16, ptr %26, !tbaa !2
  %97 = load i16, ptr %16, !tbaa !2
  %98 = icmp sge i16 %96, %97
  %99 = sext i1 %98 to i16
  %100 = icmp ne i16 %99, 0
  br i1 %100, label %b15, label %b16

b15:
  %101 = load i16, ptr %26, !tbaa !2
  %102 = mul i16 %101, 2
  %103 = getelementptr i8, ptr %30, i16 2
  %104 = load i16, ptr %103, !tbaa !2
  %105 = add i16 0, %102
  %106 = inttoptr i16 %104 to ptr addrspace(2)
  %107 = addrspacecast ptr addrspace(2) %106 to ptr addrspace(1)
  %108 = getelementptr i8, ptr addrspace(1) %107, i16 %105
  %109 = load i16, ptr %26, !tbaa !2
  %110 = load i16, ptr %19, !tbaa !2
  %111 = add i16 %109, %110
  %112 = and i16 %111, 511
  %113 = mul i16 %112, 2
  %114 = getelementptr i8, ptr %28, i16 2
  %115 = load i16, ptr %114, !tbaa !2
  %116 = add i16 0, %113
  %117 = inttoptr i16 %115 to ptr addrspace(2)
  %118 = addrspacecast ptr addrspace(2) %117 to ptr addrspace(1)
  %119 = getelementptr i8, ptr addrspace(1) %118, i16 %116
  %120 = load i16, ptr addrspace(1) %119, !tbaa !4
  %121 = load i16, ptr %26, !tbaa !2
  %122 = mul i16 3, %121
  %123 = load i16, ptr %19, !tbaa !2
  %124 = mul i16 7, %123
  %125 = add i16 %122, %124
  %126 = add i16 %125, 3
  %127 = and i16 %126, 511
  %128 = mul i16 %127, 2
  %129 = getelementptr i8, ptr %28, i16 2
  %130 = load i16, ptr %129, !tbaa !2
  %131 = add i16 0, %128
  %132 = inttoptr i16 %130 to ptr addrspace(2)
  %133 = addrspacecast ptr addrspace(2) %132 to ptr addrspace(1)
  %134 = getelementptr i8, ptr addrspace(1) %133, i16 %131
  %135 = load i16, ptr addrspace(1) %134, !tbaa !4
  %136 = add i16 %120, %135
  store i16 %136, ptr addrspace(1) %108, !tbaa !4
  %137 = load i16, ptr %26, !tbaa !2
  %138 = load i16, ptr %15, !tbaa !2
  %139 = add i16 %137, %138
  store i16 %139, ptr %26, !tbaa !2
  br label %b12

b16:
  store i16 0, ptr %14, !tbaa !2
  store i16 0, ptr %13, !tbaa !2
  store i16 128, ptr %12, !tbaa !2
  store i16 1, ptr %11, !tbaa !2
  br label %b17

b17:
  %140 = load i16, ptr %11, !tbaa !2
  %141 = icmp sge i16 %140, 0
  %142 = sext i1 %141 to i16
  %143 = icmp ne i16 %142, 0
  br i1 %143, label %b18, label %b19

b18:
  %144 = load i16, ptr %13, !tbaa !2
  %145 = load i16, ptr %12, !tbaa !2
  %146 = icmp sle i16 %144, %145
  %147 = sext i1 %146 to i16
  %148 = icmp ne i16 %147, 0
  br i1 %148, label %b20, label %b21

b19:
  %149 = load i16, ptr %13, !tbaa !2
  %150 = load i16, ptr %12, !tbaa !2
  %151 = icmp sge i16 %149, %150
  %152 = sext i1 %151 to i16
  %153 = icmp ne i16 %152, 0
  br i1 %153, label %b20, label %b21

b20:
  %154 = load i16, ptr %13, !tbaa !2
  %155 = mul i16 %154, 7
  %156 = load i16, ptr %19, !tbaa !2
  %157 = mul i16 %156, 5
  %158 = add i16 %155, %157
  %159 = and i16 %158, 511
  %160 = mul i16 %159, 2
  %161 = getelementptr i8, ptr %28, i16 2
  %162 = load i16, ptr %161, !tbaa !2
  %163 = add i16 0, %160
  %164 = inttoptr i16 %162 to ptr addrspace(2)
  %165 = addrspacecast ptr addrspace(2) %164 to ptr addrspace(1)
  %166 = getelementptr i8, ptr addrspace(1) %165, i16 %163
  %167 = load i16, ptr addrspace(1) %166, !tbaa !4
  %168 = load i16, ptr %13, !tbaa !2
  %169 = mul i16 %168, 14
  %170 = load i16, ptr %19, !tbaa !2
  %171 = mul i16 %170, 11
  %172 = add i16 %169, %171
  %173 = add i16 %172, 1943
  %174 = and i16 %173, 511
  %175 = mul i16 %174, 2
  %176 = getelementptr i8, ptr %28, i16 2
  %177 = load i16, ptr %176, !tbaa !2
  %178 = add i16 0, %175
  %179 = inttoptr i16 %177 to ptr addrspace(2)
  %180 = addrspacecast ptr addrspace(2) %179 to ptr addrspace(1)
  %181 = getelementptr i8, ptr addrspace(1) %180, i16 %178
  %182 = load i16, ptr addrspace(1) %181, !tbaa !4
  %183 = add i16 %167, %182
  store i16 %183, ptr %10, !tbaa !2
  store i16 0, ptr %26, !tbaa !2
  store i16 128, ptr %9, !tbaa !2
  store i16 1, ptr %8, !tbaa !2
  br label %b22

b21:
  store i16 0, ptr %26, !tbaa !2
  store i16 320, ptr %7, !tbaa !2
  store i16 1, ptr %6, !tbaa !2
  br label %b27

b22:
  %184 = load i16, ptr %8, !tbaa !2
  %185 = icmp sge i16 %184, 0
  %186 = sext i1 %185 to i16
  %187 = icmp ne i16 %186, 0
  br i1 %187, label %b23, label %b24

b23:
  %188 = load i16, ptr %26, !tbaa !2
  %189 = load i16, ptr %9, !tbaa !2
  %190 = icmp sle i16 %188, %189
  %191 = sext i1 %190 to i16
  %192 = icmp ne i16 %191, 0
  br i1 %192, label %b25, label %b26

b24:
  %193 = load i16, ptr %26, !tbaa !2
  %194 = load i16, ptr %9, !tbaa !2
  %195 = icmp sge i16 %193, %194
  %196 = sext i1 %195 to i16
  %197 = icmp ne i16 %196, 0
  br i1 %197, label %b25, label %b26

b25:
  %198 = load i16, ptr %26, !tbaa !2
  %199 = load i16, ptr %13, !tbaa !2
  %200 = mul i16 %199, 129
  %201 = add i16 %200, %198
  %202 = mul i16 %201, 2
  %203 = getelementptr i8, ptr %27, i16 2
  %204 = load i16, ptr %203, !tbaa !2
  %205 = add i16 0, %202
  %206 = inttoptr i16 %204 to ptr addrspace(2)
  %207 = addrspacecast ptr addrspace(2) %206 to ptr addrspace(1)
  %208 = getelementptr i8, ptr addrspace(1) %207, i16 %205
  %209 = load i16, ptr %26, !tbaa !2
  %210 = mul i16 %209, 2
  %211 = getelementptr i8, ptr %30, i16 2
  %212 = load i16, ptr %211, !tbaa !2
  %213 = add i16 0, %210
  %214 = inttoptr i16 %212 to ptr addrspace(2)
  %215 = addrspacecast ptr addrspace(2) %214 to ptr addrspace(1)
  %216 = getelementptr i8, ptr addrspace(1) %215, i16 %213
  %217 = load i16, ptr addrspace(1) %216, !tbaa !4
  %218 = load i16, ptr %10, !tbaa !2
  %219 = add i16 %217, %218
  store i16 %219, ptr addrspace(1) %208, !tbaa !4
  %220 = load i16, ptr %14, !tbaa !2
  %221 = add i16 %220, 1
  store i16 %221, ptr %14, !tbaa !2
  %222 = load i16, ptr %26, !tbaa !2
  %223 = load i16, ptr %8, !tbaa !2
  %224 = add i16 %222, %223
  store i16 %224, ptr %26, !tbaa !2
  br label %b22

b26:
  %225 = load i16, ptr %13, !tbaa !2
  %226 = load i16, ptr %11, !tbaa !2
  %227 = add i16 %225, %226
  store i16 %227, ptr %13, !tbaa !2
  br label %b17

b27:
  %228 = load i16, ptr %6, !tbaa !2
  %229 = icmp sge i16 %228, 0
  %230 = sext i1 %229 to i16
  %231 = icmp ne i16 %230, 0
  br i1 %231, label %b28, label %b29

b28:
  %232 = load i16, ptr %26, !tbaa !2
  %233 = load i16, ptr %7, !tbaa !2
  %234 = icmp sle i16 %232, %233
  %235 = sext i1 %234 to i16
  %236 = icmp ne i16 %235, 0
  br i1 %236, label %b30, label %b31

b29:
  %237 = load i16, ptr %26, !tbaa !2
  %238 = load i16, ptr %7, !tbaa !2
  %239 = icmp sge i16 %237, %238
  %240 = sext i1 %239 to i16
  %241 = icmp ne i16 %240, 0
  br i1 %241, label %b30, label %b31

b30:
  %242 = load i16, ptr %26, !tbaa !2
  %243 = mul i16 %242, 2
  %244 = getelementptr i8, ptr %30, i16 2
  %245 = load i16, ptr %244, !tbaa !2
  %246 = add i16 0, %243
  %247 = inttoptr i16 %245 to ptr addrspace(2)
  %248 = addrspacecast ptr addrspace(2) %247 to ptr addrspace(1)
  %249 = getelementptr i8, ptr addrspace(1) %248, i16 %246
  %250 = load i16, ptr %26, !tbaa !2
  %251 = mul i16 %250, 11
  %252 = load i16, ptr %19, !tbaa !2
  %253 = mul i16 %252, 7
  %254 = add i16 %251, %253
  %255 = and i16 %254, 511
  %256 = mul i16 %255, 2
  %257 = getelementptr i8, ptr %28, i16 2
  %258 = load i16, ptr %257, !tbaa !2
  %259 = add i16 0, %256
  %260 = inttoptr i16 %258 to ptr addrspace(2)
  %261 = addrspacecast ptr addrspace(2) %260 to ptr addrspace(1)
  %262 = getelementptr i8, ptr addrspace(1) %261, i16 %259
  %263 = load i16, ptr addrspace(1) %262, !tbaa !4
  %264 = load i16, ptr %26, !tbaa !2
  %265 = mul i16 3, %264
  %266 = load i16, ptr %19, !tbaa !2
  %267 = mul i16 7, %266
  %268 = add i16 %265, %267
  %269 = add i16 %268, 3
  %270 = and i16 %269, 511
  %271 = mul i16 %270, 2
  %272 = getelementptr i8, ptr %28, i16 2
  %273 = load i16, ptr %272, !tbaa !2
  %274 = add i16 0, %271
  %275 = inttoptr i16 %273 to ptr addrspace(2)
  %276 = addrspacecast ptr addrspace(2) %275 to ptr addrspace(1)
  %277 = getelementptr i8, ptr addrspace(1) %276, i16 %274
  %278 = load i16, ptr addrspace(1) %277, !tbaa !4
  %279 = add i16 %263, %278
  store i16 %279, ptr addrspace(1) %249, !tbaa !4
  %280 = load i16, ptr %26, !tbaa !2
  %281 = mul i16 %280, 2
  %282 = getelementptr i8, ptr %29, i16 2
  %283 = load i16, ptr %282, !tbaa !2
  %284 = add i16 0, %281
  %285 = inttoptr i16 %283 to ptr addrspace(2)
  %286 = addrspacecast ptr addrspace(2) %285 to ptr addrspace(1)
  %287 = getelementptr i8, ptr addrspace(1) %286, i16 %284
  %288 = load i16, ptr %26, !tbaa !2
  %289 = mul i16 %288, 4
  %290 = load i16, ptr %19, !tbaa !2
  %291 = mul i16 %290, 5
  %292 = add i16 %289, %291
  %293 = and i16 %292, 511
  %294 = mul i16 %293, 2
  %295 = getelementptr i8, ptr %28, i16 2
  %296 = load i16, ptr %295, !tbaa !2
  %297 = add i16 0, %294
  %298 = inttoptr i16 %296 to ptr addrspace(2)
  %299 = addrspacecast ptr addrspace(2) %298 to ptr addrspace(1)
  %300 = getelementptr i8, ptr addrspace(1) %299, i16 %297
  %301 = load i16, ptr addrspace(1) %300, !tbaa !4
  %302 = load i16, ptr %26, !tbaa !2
  %303 = mul i16 9, %302
  %304 = load i16, ptr %19, !tbaa !2
  %305 = mul i16 2, %304
  %306 = add i16 %303, %305
  %307 = add i16 %306, 371
  %308 = and i16 %307, 511
  %309 = mul i16 %308, 2
  %310 = getelementptr i8, ptr %28, i16 2
  %311 = load i16, ptr %310, !tbaa !2
  %312 = add i16 0, %309
  %313 = inttoptr i16 %311 to ptr addrspace(2)
  %314 = addrspacecast ptr addrspace(2) %313 to ptr addrspace(1)
  %315 = getelementptr i8, ptr addrspace(1) %314, i16 %312
  %316 = load i16, ptr addrspace(1) %315, !tbaa !4
  %317 = add i16 %301, %316
  store i16 %317, ptr addrspace(1) %287, !tbaa !4
  %318 = load i16, ptr %26, !tbaa !2
  %319 = load i16, ptr %6, !tbaa !2
  %320 = add i16 %318, %319
  store i16 %320, ptr %26, !tbaa !2
  br label %b27

b31:
  store i16 0, ptr %14, !tbaa !2
  store i16 0, ptr %13, !tbaa !2
  store i16 199, ptr %5, !tbaa !2
  store i16 1, ptr %4, !tbaa !2
  br label %b32

b32:
  %321 = load i16, ptr %4, !tbaa !2
  %322 = icmp sge i16 %321, 0
  %323 = sext i1 %322 to i16
  %324 = icmp ne i16 %323, 0
  br i1 %324, label %b33, label %b34

b33:
  %325 = load i16, ptr %13, !tbaa !2
  %326 = load i16, ptr %5, !tbaa !2
  %327 = icmp sle i16 %325, %326
  %328 = sext i1 %327 to i16
  %329 = icmp ne i16 %328, 0
  br i1 %329, label %b35, label %b36

b34:
  %330 = load i16, ptr %13, !tbaa !2
  %331 = load i16, ptr %5, !tbaa !2
  %332 = icmp sge i16 %330, %331
  %333 = sext i1 %332 to i16
  %334 = icmp ne i16 %333, 0
  br i1 %334, label %b35, label %b36

b35:
  %335 = load i16, ptr %13, !tbaa !2
  %336 = mul i16 %335, 11
  %337 = load i16, ptr %19, !tbaa !2
  %338 = mul i16 %337, 6
  %339 = add i16 %336, %338
  %340 = and i16 %339, 511
  %341 = mul i16 %340, 2
  %342 = getelementptr i8, ptr %28, i16 2
  %343 = load i16, ptr %342, !tbaa !2
  %344 = add i16 0, %341
  %345 = inttoptr i16 %343 to ptr addrspace(2)
  %346 = addrspacecast ptr addrspace(2) %345 to ptr addrspace(1)
  %347 = getelementptr i8, ptr addrspace(1) %346, i16 %344
  %348 = load i16, ptr addrspace(1) %347, !tbaa !4
  %349 = load i16, ptr %13, !tbaa !2
  %350 = mul i16 %349, 14
  %351 = load i16, ptr %19, !tbaa !2
  %352 = mul i16 %351, 11
  %353 = add i16 %350, %352
  %354 = add i16 %353, 1943
  %355 = and i16 %354, 511
  %356 = mul i16 %355, 2
  %357 = getelementptr i8, ptr %28, i16 2
  %358 = load i16, ptr %357, !tbaa !2
  %359 = add i16 0, %356
  %360 = inttoptr i16 %358 to ptr addrspace(2)
  %361 = addrspacecast ptr addrspace(2) %360 to ptr addrspace(1)
  %362 = getelementptr i8, ptr addrspace(1) %361, i16 %359
  %363 = load i16, ptr addrspace(1) %362, !tbaa !4
  %364 = add i16 %348, %363
  store i16 %364, ptr %10, !tbaa !2
  %365 = load i16, ptr %13, !tbaa !2
  %366 = mul i16 %365, 9
  %367 = load i16, ptr %19, !tbaa !2
  %368 = mul i16 %367, 4
  %369 = add i16 %366, %368
  %370 = and i16 %369, 511
  %371 = mul i16 %370, 2
  %372 = getelementptr i8, ptr %28, i16 2
  %373 = load i16, ptr %372, !tbaa !2
  %374 = add i16 0, %371
  %375 = inttoptr i16 %373 to ptr addrspace(2)
  %376 = addrspacecast ptr addrspace(2) %375 to ptr addrspace(1)
  %377 = getelementptr i8, ptr addrspace(1) %376, i16 %374
  %378 = load i16, ptr addrspace(1) %377, !tbaa !4
  %379 = load i16, ptr %13, !tbaa !2
  %380 = mul i16 %379, 17
  %381 = load i16, ptr %19, !tbaa !2
  %382 = mul i16 %381, 23
  %383 = add i16 %380, %382
  %384 = add i16 %383, 1943
  %385 = and i16 %384, 511
  %386 = mul i16 %385, 2
  %387 = getelementptr i8, ptr %28, i16 2
  %388 = load i16, ptr %387, !tbaa !2
  %389 = add i16 0, %386
  %390 = inttoptr i16 %388 to ptr addrspace(2)
  %391 = addrspacecast ptr addrspace(2) %390 to ptr addrspace(1)
  %392 = getelementptr i8, ptr addrspace(1) %391, i16 %389
  %393 = load i16, ptr addrspace(1) %392, !tbaa !4
  %394 = add i16 %378, %393
  store i16 %394, ptr %3, !tbaa !2
  store i16 0, ptr %26, !tbaa !2
  store i16 319, ptr %2, !tbaa !2
  store i16 1, ptr %1, !tbaa !2
  br label %b37

b36:
  call cc1000 addrspace(1) void @UPDPALPLASMA(ptr %19)
  %395 = load i16, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %396 = add i16 %395, 1
  store i16 %396, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %397 = call cc1000 addrspace(1) ptr @llrm.qb.B$INKY()
  %398 = call cc1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %397, ptr @$string11)
  %399 = icmp sgt i16 %398, 0
  %400 = sext i1 %399 to i16
  %401 = icmp ne i16 %400, 0
  br i1 %401, label %b42, label %b43

b37:
  %402 = load i16, ptr %1, !tbaa !2
  %403 = icmp sge i16 %402, 0
  %404 = sext i1 %403 to i16
  %405 = icmp ne i16 %404, 0
  br i1 %405, label %b38, label %b39

b38:
  %406 = load i16, ptr %26, !tbaa !2
  %407 = load i16, ptr %2, !tbaa !2
  %408 = icmp sle i16 %406, %407
  %409 = sext i1 %408 to i16
  %410 = icmp ne i16 %409, 0
  br i1 %410, label %b40, label %b41

b39:
  %411 = load i16, ptr %26, !tbaa !2
  %412 = load i16, ptr %2, !tbaa !2
  %413 = icmp sge i16 %411, %412
  %414 = sext i1 %413 to i16
  %415 = icmp ne i16 %414, 0
  br i1 %415, label %b40, label %b41

b40:
  %416 = load i16, ptr %14, !tbaa !2
  %417 = load i16, ptr %26, !tbaa !2
  %418 = mul i16 %417, 2
  %419 = getelementptr i8, ptr %30, i16 2
  %420 = load i16, ptr %419, !tbaa !2
  %421 = add i16 0, %418
  %422 = inttoptr i16 %420 to ptr addrspace(2)
  %423 = addrspacecast ptr addrspace(2) %422 to ptr addrspace(1)
  %424 = getelementptr i8, ptr addrspace(1) %423, i16 %421
  %425 = load i16, ptr addrspace(1) %424, !tbaa !4
  %426 = load i16, ptr %10, !tbaa !2
  %427 = add i16 %425, %426
  %428 = and i16 %427, 127
  %429 = load i16, ptr %26, !tbaa !2
  %430 = mul i16 %429, 2
  %431 = getelementptr i8, ptr %29, i16 2
  %432 = load i16, ptr %431, !tbaa !2
  %433 = add i16 0, %430
  %434 = inttoptr i16 %432 to ptr addrspace(2)
  %435 = addrspacecast ptr addrspace(2) %434 to ptr addrspace(1)
  %436 = getelementptr i8, ptr addrspace(1) %435, i16 %433
  %437 = load i16, ptr addrspace(1) %436, !tbaa !4
  %438 = load i16, ptr %3, !tbaa !2
  %439 = add i16 %437, %438
  %440 = and i16 %439, 127
  %441 = mul i16 %440, 129
  %442 = add i16 %441, %428
  %443 = mul i16 %442, 2
  %444 = getelementptr i8, ptr %27, i16 2
  %445 = load i16, ptr %444, !tbaa !2
  %446 = add i16 0, %443
  %447 = inttoptr i16 %445 to ptr addrspace(2)
  %448 = addrspacecast ptr addrspace(2) %447 to ptr addrspace(1)
  %449 = getelementptr i8, ptr addrspace(1) %448, i16 %446
  %450 = load i16, ptr addrspace(1) %449, !tbaa !4
  %451 = trunc i16 %450 to i8
  %452 = load i16, ptr @b$seg, !tbaa !2
  %453 = inttoptr i16 %452 to ptr addrspace(2)
  %454 = addrspacecast ptr addrspace(2) %453 to ptr addrspace(1)
  %455 = getelementptr i8, ptr addrspace(1) %454, i16 %416
  store i8 %451, ptr addrspace(1) %455
  %456 = load i16, ptr %14, !tbaa !2
  %457 = add i16 %456, 1
  store i16 %457, ptr %14, !tbaa !2
  %458 = load i16, ptr %26, !tbaa !2
  %459 = load i16, ptr %1, !tbaa !2
  %460 = add i16 %458, %459
  store i16 %460, ptr %26, !tbaa !2
  br label %b37

b41:
  %461 = load i16, ptr %13, !tbaa !2
  %462 = load i16, ptr %4, !tbaa !2
  %463 = add i16 %461, %462
  store i16 %463, ptr %13, !tbaa !2
  br label %b32

b42:
  call cc1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %27)
  call cc1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %28)
  call cc1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %30)
  call cc1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %29)
  ret void

b43:
  br label %b44

b44:
  %464 = load i16, ptr %19, !tbaa !2
  %465 = load i16, ptr %17, !tbaa !2
  %466 = add i16 %464, %465
  store i16 %466, ptr %19, !tbaa !2
  br label %b7
}

define cc1000 void @RENDER(ptr %0, ptr %1, ptr %2, ptr %3) addrspace(1) {
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
  store i16 0, ptr %4
  store i16 0, ptr %5
  store i16 0, ptr %6
  store i16 0, ptr %7
  store i16 0, ptr %8
  store i16 0, ptr %9
  store i16 0, ptr %10
  store i16 0, ptr %11
  store i16 0, ptr %12
  store i16 0, ptr %13
  store i16 0, ptr %14
  store i16 0, ptr %15
  store i16 0, ptr %16
  store i16 0, ptr %17
  store i16 0, ptr %18
  store i16 0, ptr %19
  store i16 0, ptr %20
  store i16 0, ptr %21
  store i16 0, ptr %22
  store i16 0, ptr %23
  call void @llvm.memset.p0.i16(ptr %24, i8 0, i16 22, i1 false)
  call cc1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 160, i16 0, i16 100, i16 2, i16 258, ptr %24)
  %25 = load i16, ptr %3
  %26 = load i16, ptr %1
  %27 = sub i16 %25, %26
  store i16 %27, ptr %23, !tbaa !2
  %28 = load i16, ptr %2
  %29 = load i16, ptr %0
  %30 = sub i16 %28, %29
  store i16 %30, ptr %22, !tbaa !2
  %31 = load i16, ptr %23, !tbaa !2
  %32 = sext i16 %31 to i32
  %33 = sdiv i32 %32, 100
  %34 = trunc i32 %33 to i16
  store i16 %34, ptr %21, !tbaa !2
  %35 = load i16, ptr %22, !tbaa !2
  %36 = sext i16 %35 to i32
  %37 = sdiv i32 %36, 160
  %38 = trunc i32 %37 to i16
  store i16 %38, ptr %20, !tbaa !2
  %39 = load i16, ptr %22, !tbaa !2
  %40 = sext i16 %39 to i32
  %41 = srem i32 %40, 160
  %42 = trunc i32 %41 to i16
  store i16 %42, ptr %22, !tbaa !2
  %43 = load i16, ptr %23, !tbaa !2
  %44 = sext i16 %43 to i32
  %45 = srem i32 %44, 100
  %46 = trunc i32 %45 to i16
  store i16 %46, ptr %23, !tbaa !2
  %47 = load i16, ptr %1
  store i16 %47, ptr %19, !tbaa !2
  store i16 0, ptr %18, !tbaa !2
  store i16 99, ptr %17, !tbaa !2
  store i16 1, ptr %16, !tbaa !2
  br label %b2

b2:
  %48 = load i16, ptr %16, !tbaa !2
  %49 = icmp sge i16 %48, 0
  %50 = sext i1 %49 to i16
  %51 = icmp ne i16 %50, 0
  br i1 %51, label %b3, label %b4

b3:
  %52 = load i16, ptr %18, !tbaa !2
  %53 = load i16, ptr %17, !tbaa !2
  %54 = icmp sle i16 %52, %53
  %55 = sext i1 %54 to i16
  %56 = icmp ne i16 %55, 0
  br i1 %56, label %b5, label %b6

b4:
  %57 = load i16, ptr %18, !tbaa !2
  %58 = load i16, ptr %17, !tbaa !2
  %59 = icmp sge i16 %57, %58
  %60 = sext i1 %59 to i16
  %61 = icmp ne i16 %60, 0
  br i1 %61, label %b5, label %b6

b5:
  %62 = load i16, ptr %0
  store i16 %62, ptr %15, !tbaa !2
  store i16 0, ptr %14, !tbaa !2
  %63 = load i16, ptr %19, !tbaa !2
  %64 = mul i16 %63, 320
  store i16 %64, ptr %13, !tbaa !2
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
  %65 = load i16, ptr %10, !tbaa !2
  %66 = icmp sge i16 %65, 0
  %67 = sext i1 %66 to i16
  %68 = icmp ne i16 %67, 0
  br i1 %68, label %b8, label %b9

b8:
  %69 = load i16, ptr %12, !tbaa !2
  %70 = load i16, ptr %11, !tbaa !2
  %71 = icmp sle i16 %69, %70
  %72 = sext i1 %71 to i16
  %73 = icmp ne i16 %72, 0
  br i1 %73, label %b10, label %b11

b9:
  %74 = load i16, ptr %12, !tbaa !2
  %75 = load i16, ptr %11, !tbaa !2
  %76 = icmp sge i16 %74, %75
  %77 = sext i1 %76 to i16
  %78 = icmp ne i16 %77, 0
  br i1 %78, label %b10, label %b11

b10:
  %79 = load i16, ptr %12, !tbaa !2
  %80 = load i16, ptr %18, !tbaa !2
  %81 = mul i16 %80, 161
  %82 = add i16 %81, %79
  %83 = mul i16 %82, 2
  %84 = getelementptr i8, ptr %24, i16 2
  %85 = load i16, ptr %84, !tbaa !2
  %86 = add i16 0, %83
  %87 = inttoptr i16 %85 to ptr addrspace(2)
  %88 = addrspacecast ptr addrspace(2) %87 to ptr addrspace(1)
  %89 = getelementptr i8, ptr addrspace(1) %88, i16 %86
  %90 = load i16, ptr %15, !tbaa !2
  %91 = load i16, ptr %13, !tbaa !2
  %92 = add i16 %90, %91
  %93 = load i16, ptr @b$seg, !tbaa !2
  %94 = inttoptr i16 %93 to ptr addrspace(2)
  %95 = addrspacecast ptr addrspace(2) %94 to ptr addrspace(1)
  %96 = getelementptr i8, ptr addrspace(1) %95, i16 %92
  %97 = load i8, ptr addrspace(1) %96
  %98 = zext i8 %97 to i16
  store i16 %98, ptr addrspace(1) %89, !tbaa !4
  %99 = load i16, ptr %14, !tbaa !2
  %100 = load i16, ptr %22, !tbaa !2
  %101 = add i16 %99, %100
  store i16 %101, ptr %14, !tbaa !2
  %102 = load i16, ptr %14, !tbaa !2
  %103 = icmp sgt i16 %102, 160
  %104 = sext i1 %103 to i16
  %105 = icmp ne i16 %104, 0
  br i1 %105, label %b12, label %b13

b11:
  %106 = load i16, ptr %9, !tbaa !2
  %107 = load i16, ptr %23, !tbaa !2
  %108 = add i16 %106, %107
  store i16 %108, ptr %9, !tbaa !2
  %109 = load i16, ptr %9, !tbaa !2
  %110 = icmp sgt i16 %109, 100
  %111 = sext i1 %110 to i16
  %112 = icmp ne i16 %111, 0
  br i1 %112, label %b15, label %b16

b12:
  %113 = load i16, ptr %14, !tbaa !2
  %114 = sub i16 %113, 160
  store i16 %114, ptr %14, !tbaa !2
  %115 = load i16, ptr %15, !tbaa !2
  %116 = add i16 %115, 1
  store i16 %116, ptr %15, !tbaa !2
  br label %b14

b13:
  br label %b14

b14:
  %117 = load i16, ptr %15, !tbaa !2
  %118 = load i16, ptr %20, !tbaa !2
  %119 = add i16 %117, %118
  store i16 %119, ptr %15, !tbaa !2
  %120 = load i16, ptr %12, !tbaa !2
  %121 = load i16, ptr %10, !tbaa !2
  %122 = add i16 %120, %121
  store i16 %122, ptr %12, !tbaa !2
  br label %b7

b15:
  %123 = load i16, ptr %9, !tbaa !2
  %124 = sub i16 %123, 100
  store i16 %124, ptr %9, !tbaa !2
  %125 = load i16, ptr %19, !tbaa !2
  %126 = add i16 %125, 1
  store i16 %126, ptr %19, !tbaa !2
  br label %b17

b16:
  br label %b17

b17:
  %127 = load i16, ptr %19, !tbaa !2
  %128 = load i16, ptr %21, !tbaa !2
  %129 = add i16 %127, %128
  store i16 %129, ptr %19, !tbaa !2
  %130 = load i16, ptr %18, !tbaa !2
  %131 = load i16, ptr %16, !tbaa !2
  %132 = add i16 %130, %131
  store i16 %132, ptr %18, !tbaa !2
  br label %b2

b18:
  %133 = load i16, ptr %6, !tbaa !2
  %134 = icmp sge i16 %133, 0
  %135 = sext i1 %134 to i16
  %136 = icmp ne i16 %135, 0
  br i1 %136, label %b19, label %b20

b19:
  %137 = load i16, ptr %19, !tbaa !2
  %138 = load i16, ptr %7, !tbaa !2
  %139 = icmp sle i16 %137, %138
  %140 = sext i1 %139 to i16
  %141 = icmp ne i16 %140, 0
  br i1 %141, label %b21, label %b22

b20:
  %142 = load i16, ptr %19, !tbaa !2
  %143 = load i16, ptr %7, !tbaa !2
  %144 = icmp sge i16 %142, %143
  %145 = sext i1 %144 to i16
  %146 = icmp ne i16 %145, 0
  br i1 %146, label %b21, label %b22

b21:
  store i16 0, ptr %15, !tbaa !2
  store i16 159, ptr %5, !tbaa !2
  store i16 1, ptr %4, !tbaa !2
  br label %b23

b22:
  %147 = mul i16 0, 2
  %148 = getelementptr i8, ptr @"FRACTAL2%", i16 2
  %149 = load i16, ptr %148, !tbaa !2
  %150 = add i16 0, %147
  %151 = inttoptr i16 %149 to ptr addrspace(2)
  %152 = addrspacecast ptr addrspace(2) %151 to ptr addrspace(1)
  %153 = getelementptr i8, ptr addrspace(1) %152, i16 %150
  %154 = addrspacecast ptr addrspace(1) %153 to ptr addrspace(2)
  %155 = ptrtoint ptr addrspace(2) %154 to i16
  store i16 %155, ptr @b$seg, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %24)
  ret void

b23:
  %156 = load i16, ptr %4, !tbaa !2
  %157 = icmp sge i16 %156, 0
  %158 = sext i1 %157 to i16
  %159 = icmp ne i16 %158, 0
  br i1 %159, label %b24, label %b25

b24:
  %160 = load i16, ptr %15, !tbaa !2
  %161 = load i16, ptr %5, !tbaa !2
  %162 = icmp sle i16 %160, %161
  %163 = sext i1 %162 to i16
  %164 = icmp ne i16 %163, 0
  br i1 %164, label %b26, label %b27

b25:
  %165 = load i16, ptr %15, !tbaa !2
  %166 = load i16, ptr %5, !tbaa !2
  %167 = icmp sge i16 %165, %166
  %168 = sext i1 %167 to i16
  %169 = icmp ne i16 %168, 0
  br i1 %169, label %b26, label %b27

b26:
  %170 = load i16, ptr %8, !tbaa !2
  %171 = load i16, ptr %15, !tbaa !2
  %172 = add i16 %170, %171
  %173 = load i16, ptr %15, !tbaa !2
  %174 = load i16, ptr %19, !tbaa !2
  %175 = mul i16 %174, 161
  %176 = add i16 %175, %173
  %177 = mul i16 %176, 2
  %178 = getelementptr i8, ptr %24, i16 2
  %179 = load i16, ptr %178, !tbaa !2
  %180 = add i16 0, %177
  %181 = inttoptr i16 %179 to ptr addrspace(2)
  %182 = addrspacecast ptr addrspace(2) %181 to ptr addrspace(1)
  %183 = getelementptr i8, ptr addrspace(1) %182, i16 %180
  %184 = load i16, ptr addrspace(1) %183, !tbaa !4
  %185 = trunc i16 %184 to i8
  %186 = load i16, ptr @b$seg, !tbaa !2
  %187 = inttoptr i16 %186 to ptr addrspace(2)
  %188 = addrspacecast ptr addrspace(2) %187 to ptr addrspace(1)
  %189 = getelementptr i8, ptr addrspace(1) %188, i16 %172
  store i8 %185, ptr addrspace(1) %189
  %190 = load i16, ptr %15, !tbaa !2
  %191 = load i16, ptr %4, !tbaa !2
  %192 = add i16 %190, %191
  store i16 %192, ptr %15, !tbaa !2
  br label %b23

b27:
  %193 = load i16, ptr %8, !tbaa !2
  %194 = add i16 %193, 320
  store i16 %194, ptr %8, !tbaa !2
  %195 = load i16, ptr %19, !tbaa !2
  %196 = load i16, ptr %6, !tbaa !2
  %197 = add i16 %195, %196
  store i16 %197, ptr %19, !tbaa !2
  br label %b18
}

define cc1000 void @SHADEBOBEFFECT(ptr %0) addrspace(1) {
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
  store i16 0, ptr %1
  store i16 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %4
  store i16 0, ptr %5
  store i16 0, ptr %6
  store i16 0, ptr %7
  store i16 0, ptr %8
  store i16 0, ptr %9
  store i16 0, ptr %10
  store i16 0, ptr %11
  store i16 0, ptr %12
  store i16 0, ptr %13
  store i16 0, ptr %14
  store i16 0, ptr %15
  store i16 0, ptr %16
  store i16 0, ptr %17
  store i16 0, ptr %18
  store i16 0, ptr %19
  store i16 0, ptr %20
  store i16 0, ptr %21
  store i16 0, ptr %22
  store i16 0, ptr %23
  store i16 0, ptr %24
  store i16 0, ptr %25
  store i16 0, ptr %26
  store i16 0, ptr %27
  store i16 0, ptr %28
  store i16 0, ptr %29
  store i16 0, ptr %30
  call void @llvm.memset.p0.i16(ptr %31, i8 0, i16 18, i1 false)
  call cc1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 4096, i16 2, i16 257, ptr %31)
  store i16 0, ptr %30, !tbaa !2
  store i16 -24576, ptr @b$seg, !tbaa !2
  store i16 1, ptr %29, !tbaa !2
  %32 = load i16, ptr %0
  store i16 %32, ptr %28, !tbaa !2
  store i16 1, ptr %27, !tbaa !2
  br label %b2

b2:
  %33 = load i16, ptr %27, !tbaa !2
  %34 = icmp sge i16 %33, 0
  %35 = sext i1 %34 to i16
  %36 = icmp ne i16 %35, 0
  br i1 %36, label %b3, label %b4

b3:
  %37 = load i16, ptr %29, !tbaa !2
  %38 = load i16, ptr %28, !tbaa !2
  %39 = icmp sle i16 %37, %38
  %40 = sext i1 %39 to i16
  %41 = icmp ne i16 %40, 0
  br i1 %41, label %b5, label %b6

b4:
  %42 = load i16, ptr %29, !tbaa !2
  %43 = load i16, ptr %28, !tbaa !2
  %44 = icmp sge i16 %42, %43
  %45 = sext i1 %44 to i16
  %46 = icmp ne i16 %45, 0
  br i1 %46, label %b5, label %b6

b5:
  %47 = load i16, ptr %30, !tbaa !2
  %48 = mul i16 %47, 2
  %49 = getelementptr i8, ptr %31, i16 2
  %50 = load i16, ptr %49, !tbaa !2
  %51 = add i16 0, %48
  %52 = inttoptr i16 %50 to ptr addrspace(2)
  %53 = addrspacecast ptr addrspace(2) %52 to ptr addrspace(1)
  %54 = getelementptr i8, ptr addrspace(1) %53, i16 %51
  %55 = load i16, ptr addrspace(1) %54, !tbaa !4
  store i16 %55, ptr %26, !tbaa !2
  call cc1000 addrspace(1) void @UNDRAWBOB(ptr %26)
  %56 = load i16, ptr %29, !tbaa !2
  store i16 %56, ptr %24, !tbaa !2
  %57 = load i16, ptr %24, !tbaa !2
  %58 = sitofp i16 %57 to float
  store i16 71, ptr %23, !tbaa !2
  %59 = load i16, ptr %23, !tbaa !2
  %60 = sitofp i16 %59 to float
  %61 = fdiv float %58, %60
  %62 = call float @llvm.sin.f32(float %61)
  %63 = load i16, ptr %29, !tbaa !2
  store i16 %63, ptr %22, !tbaa !2
  %64 = load i16, ptr %22, !tbaa !2
  %65 = sitofp i16 %64 to float
  store i16 47, ptr %21, !tbaa !2
  %66 = load i16, ptr %21, !tbaa !2
  %67 = sitofp i16 %66 to float
  %68 = fdiv float %65, %67
  store i16 2, ptr %20, !tbaa !2
  %69 = load i16, ptr %20, !tbaa !2
  %70 = sitofp i16 %69 to float
  %71 = fadd float %68, %70
  %72 = call float @llvm.cos.f32(float %71)
  %73 = fadd float %62, %72
  %74 = load i16, ptr %29, !tbaa !2
  store i16 %74, ptr %19, !tbaa !2
  %75 = load i16, ptr %19, !tbaa !2
  %76 = sitofp i16 %75 to float
  store i16 91, ptr %18, !tbaa !2
  %77 = load i16, ptr %18, !tbaa !2
  %78 = sitofp i16 %77 to float
  %79 = fdiv float %76, %78
  store i16 7, ptr %17, !tbaa !2
  %80 = load i16, ptr %17, !tbaa !2
  %81 = sitofp i16 %80 to float
  %82 = fadd float %79, %81
  %83 = call float @llvm.cos.f32(float %82)
  %84 = fadd float %73, %83
  store i16 48, ptr %16, !tbaa !2
  %85 = load i16, ptr %16, !tbaa !2
  %86 = sitofp i16 %85 to float
  %87 = fmul float %84, %86
  store i16 160, ptr %15, !tbaa !2
  %88 = load i16, ptr %15, !tbaa !2
  %89 = sitofp i16 %88 to float
  %90 = fadd float %87, %89
  %91 = call i16 @llvm.lrint.i16.f32(float %90)
  store i16 %91, ptr %25, !tbaa !2
  %92 = load i16, ptr %29, !tbaa !2
  store i16 %92, ptr %13, !tbaa !2
  %93 = load i16, ptr %13, !tbaa !2
  %94 = sitofp i16 %93 to float
  store i16 49, ptr %12, !tbaa !2
  %95 = load i16, ptr %12, !tbaa !2
  %96 = sitofp i16 %95 to float
  %97 = fdiv float %94, %96
  store i16 3, ptr %11, !tbaa !2
  %98 = load i16, ptr %11, !tbaa !2
  %99 = sitofp i16 %98 to float
  %100 = fadd float %97, %99
  %101 = call float @llvm.cos.f32(float %100)
  %102 = load i16, ptr %29, !tbaa !2
  store i16 %102, ptr %10, !tbaa !2
  %103 = load i16, ptr %10, !tbaa !2
  %104 = sitofp i16 %103 to float
  store i16 41, ptr %9, !tbaa !2
  %105 = load i16, ptr %9, !tbaa !2
  %106 = sitofp i16 %105 to float
  %107 = fdiv float %104, %106
  store i16 2, ptr %8, !tbaa !2
  %108 = load i16, ptr %8, !tbaa !2
  %109 = sitofp i16 %108 to float
  %110 = fadd float %107, %109
  %111 = call float @llvm.sin.f32(float %110)
  %112 = fadd float %101, %111
  %113 = load i16, ptr %29, !tbaa !2
  store i16 %113, ptr %7, !tbaa !2
  %114 = load i16, ptr %7, !tbaa !2
  %115 = sitofp i16 %114 to float
  store i16 97, ptr %6, !tbaa !2
  %116 = load i16, ptr %6, !tbaa !2
  %117 = sitofp i16 %116 to float
  %118 = fdiv float %115, %117
  store i16 7, ptr %5, !tbaa !2
  %119 = load i16, ptr %5, !tbaa !2
  %120 = sitofp i16 %119 to float
  %121 = fadd float %118, %120
  %122 = call float @llvm.sin.f32(float %121)
  %123 = fadd float %112, %122
  store i16 28, ptr %4, !tbaa !2
  %124 = load i16, ptr %4, !tbaa !2
  %125 = sitofp i16 %124 to float
  %126 = fmul float %123, %125
  store i16 100, ptr %3, !tbaa !2
  %127 = load i16, ptr %3, !tbaa !2
  %128 = sitofp i16 %127 to float
  %129 = fadd float %126, %128
  %130 = call i16 @llvm.lrint.i16.f32(float %129)
  store i16 %130, ptr %14, !tbaa !2
  %131 = load i16, ptr %30, !tbaa !2
  %132 = mul i16 %131, 2
  %133 = getelementptr i8, ptr %31, i16 2
  %134 = load i16, ptr %133, !tbaa !2
  %135 = add i16 0, %132
  %136 = inttoptr i16 %134 to ptr addrspace(2)
  %137 = addrspacecast ptr addrspace(2) %136 to ptr addrspace(1)
  %138 = getelementptr i8, ptr addrspace(1) %137, i16 %135
  %139 = load i16, ptr %25, !tbaa !2
  %140 = load i16, ptr %14, !tbaa !2
  %141 = mul i16 %140, 320
  %142 = add i16 %139, %141
  store i16 %142, ptr addrspace(1) %138, !tbaa !4
  %143 = load i16, ptr %30, !tbaa !2
  %144 = mul i16 %143, 2
  %145 = getelementptr i8, ptr %31, i16 2
  %146 = load i16, ptr %145, !tbaa !2
  %147 = add i16 0, %144
  %148 = inttoptr i16 %146 to ptr addrspace(2)
  %149 = addrspacecast ptr addrspace(2) %148 to ptr addrspace(1)
  %150 = getelementptr i8, ptr addrspace(1) %149, i16 %147
  %151 = load i16, ptr addrspace(1) %150, !tbaa !4
  store i16 %151, ptr %2, !tbaa !2
  call cc1000 addrspace(1) void @DRAWBOB(ptr %2)
  %152 = load i16, ptr %29, !tbaa !2
  %153 = sext i16 %152 to i32
  %154 = sdiv i32 %153, 2
  %155 = trunc i32 %154 to i16
  %156 = add i16 %155, 1
  store i16 %156, ptr %1, !tbaa !2
  %157 = load i16, ptr %30, !tbaa !2
  %158 = add i16 %157, 1
  store i16 %158, ptr %30, !tbaa !2
  %159 = load i16, ptr %30, !tbaa !2
  %160 = load i16, ptr %1, !tbaa !2
  %161 = sext i16 %159 to i32
  %162 = sext i16 %160 to i32
  %163 = srem i32 %161, %162
  %164 = trunc i32 %163 to i16
  store i16 %164, ptr %30, !tbaa !2
  %165 = load i16, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %166 = add i16 %165, 1
  store i16 %166, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %167 = call cc1000 addrspace(1) ptr @llrm.qb.B$INKY()
  %168 = call cc1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %167, ptr @$string12)
  %169 = icmp sgt i16 %168, 0
  %170 = sext i1 %169 to i16
  %171 = icmp ne i16 %170, 0
  br i1 %171, label %b7, label %b8

b6:
  call cc1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %31)
  call cc1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %31)
  ret void

b7:
  call cc1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %31)
  ret void

b8:
  br label %b9

b9:
  %172 = load i16, ptr %29, !tbaa !2
  %173 = load i16, ptr %27, !tbaa !2
  %174 = add i16 %172, %173
  store i16 %174, ptr %29, !tbaa !2
  br label %b2
}

define cc1000 void @UNDRAWBOB(ptr %0) addrspace(1) {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  store i16 0, ptr %1
  store i16 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %4
  store i16 0, ptr %5
  store i16 0, ptr %6
  store i16 0, ptr %6, !tbaa !2
  store i16 31, ptr %5, !tbaa !2
  store i16 1, ptr %4, !tbaa !2
  br label %b2

b2:
  %7 = load i16, ptr %4, !tbaa !2
  %8 = icmp sge i16 %7, 0
  %9 = sext i1 %8 to i16
  %10 = icmp ne i16 %9, 0
  br i1 %10, label %b3, label %b4

b3:
  %11 = load i16, ptr %6, !tbaa !2
  %12 = load i16, ptr %5, !tbaa !2
  %13 = icmp sle i16 %11, %12
  %14 = sext i1 %13 to i16
  %15 = icmp ne i16 %14, 0
  br i1 %15, label %b5, label %b6

b4:
  %16 = load i16, ptr %6, !tbaa !2
  %17 = load i16, ptr %5, !tbaa !2
  %18 = icmp sge i16 %16, %17
  %19 = sext i1 %18 to i16
  %20 = icmp ne i16 %19, 0
  br i1 %20, label %b5, label %b6

b5:
  %21 = load i16, ptr %0
  %22 = add i16 %21, 288
  store i16 %22, ptr %0
  store i16 0, ptr %3, !tbaa !2
  store i16 31, ptr %2, !tbaa !2
  store i16 1, ptr %1, !tbaa !2
  br label %b7

b6:
  ret void

b7:
  %23 = load i16, ptr %1, !tbaa !2
  %24 = icmp sge i16 %23, 0
  %25 = sext i1 %24 to i16
  %26 = icmp ne i16 %25, 0
  br i1 %26, label %b8, label %b9

b8:
  %27 = load i16, ptr %3, !tbaa !2
  %28 = load i16, ptr %2, !tbaa !2
  %29 = icmp sle i16 %27, %28
  %30 = sext i1 %29 to i16
  %31 = icmp ne i16 %30, 0
  br i1 %31, label %b10, label %b11

b9:
  %32 = load i16, ptr %3, !tbaa !2
  %33 = load i16, ptr %2, !tbaa !2
  %34 = icmp sge i16 %32, %33
  %35 = sext i1 %34 to i16
  %36 = icmp ne i16 %35, 0
  br i1 %36, label %b10, label %b11

b10:
  %37 = load i16, ptr %0
  %38 = load i16, ptr %0
  %39 = load i16, ptr @b$seg, !tbaa !2
  %40 = inttoptr i16 %39 to ptr addrspace(2)
  %41 = addrspacecast ptr addrspace(2) %40 to ptr addrspace(1)
  %42 = getelementptr i8, ptr addrspace(1) %41, i16 %38
  %43 = load i8, ptr addrspace(1) %42
  %44 = zext i8 %43 to i16
  %45 = load i16, ptr %3, !tbaa !2
  %46 = load i16, ptr %6, !tbaa !2
  %47 = mul i16 %46, 33
  %48 = add i16 %47, %45
  %49 = mul i16 %48, 2
  %50 = getelementptr i8, ptr @"BOBSPRITE%", i16 2
  %51 = load i16, ptr %50, !tbaa !2
  %52 = add i16 0, %49
  %53 = inttoptr i16 %51 to ptr addrspace(2)
  %54 = addrspacecast ptr addrspace(2) %53 to ptr addrspace(1)
  %55 = getelementptr i8, ptr addrspace(1) %54, i16 %52
  %56 = load i16, ptr addrspace(1) %55, !tbaa !4
  %57 = sub i16 %44, %56
  %58 = trunc i16 %57 to i8
  %59 = load i16, ptr @b$seg, !tbaa !2
  %60 = inttoptr i16 %59 to ptr addrspace(2)
  %61 = addrspacecast ptr addrspace(2) %60 to ptr addrspace(1)
  %62 = getelementptr i8, ptr addrspace(1) %61, i16 %37
  store i8 %58, ptr addrspace(1) %62
  %63 = load i16, ptr %0
  %64 = add i16 %63, 1
  store i16 %64, ptr %0
  %65 = load i16, ptr %3, !tbaa !2
  %66 = load i16, ptr %1, !tbaa !2
  %67 = add i16 %65, %66
  store i16 %67, ptr %3, !tbaa !2
  br label %b7

b11:
  %68 = load i16, ptr %6, !tbaa !2
  %69 = load i16, ptr %4, !tbaa !2
  %70 = add i16 %68, %69
  store i16 %70, ptr %6, !tbaa !2
  br label %b2
}

define cc1000 void @UNWHITEFADE(ptr %0) addrspace(1) {
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
  store i16 0, ptr %1
  store i16 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %4
  store i16 0, ptr %5
  store i16 0, ptr %6
  store i16 0, ptr %7
  store i16 0, ptr %8
  store i16 0, ptr %9
  store i16 0, ptr %10
  store i16 0, ptr %11
  store i16 0, ptr %12
  store i16 0, ptr %13
  store float 0.000000e+00, ptr %14
  store i16 0, ptr %15
  store i16 0, ptr %16
  store i16 0, ptr %17
  store i16 0, ptr %18
  store i16 0, ptr %19
  store i16 0, ptr %20
  call void @llvm.memset.p0.i16(ptr %21, i8 0, i16 22, i1 false)
  call cc1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 256, i16 0, i16 3, i16 2, i16 258, ptr %21)
  store i16 0, ptr %20, !tbaa !2
  store i16 255, ptr %19, !tbaa !2
  store i16 1, ptr %18, !tbaa !2
  br label %b2

b2:
  %22 = load i16, ptr %18, !tbaa !2
  %23 = icmp sge i16 %22, 0
  %24 = sext i1 %23 to i16
  %25 = icmp ne i16 %24, 0
  br i1 %25, label %b3, label %b4

b3:
  %26 = load i16, ptr %20, !tbaa !2
  %27 = load i16, ptr %19, !tbaa !2
  %28 = icmp sle i16 %26, %27
  %29 = sext i1 %28 to i16
  %30 = icmp ne i16 %29, 0
  br i1 %30, label %b5, label %b6

b4:
  %31 = load i16, ptr %20, !tbaa !2
  %32 = load i16, ptr %19, !tbaa !2
  %33 = icmp sge i16 %31, %32
  %34 = sext i1 %33 to i16
  %35 = icmp ne i16 %34, 0
  br i1 %35, label %b5, label %b6

b5:
  %36 = load i16, ptr %20, !tbaa !2
  %37 = trunc i16 %36 to i8
  call void @llrm.ia16.out.i8(i16 967, i8 %37)
  %38 = load i16, ptr %20, !tbaa !2
  %39 = mul i16 0, 257
  %40 = add i16 %39, %38
  %41 = mul i16 %40, 2
  %42 = getelementptr i8, ptr %21, i16 2
  %43 = load i16, ptr %42, !tbaa !2
  %44 = add i16 0, %41
  %45 = inttoptr i16 %43 to ptr addrspace(2)
  %46 = addrspacecast ptr addrspace(2) %45 to ptr addrspace(1)
  %47 = getelementptr i8, ptr addrspace(1) %46, i16 %44
  %48 = call i8 @llrm.ia16.in.i8(i16 969)
  %49 = zext i8 %48 to i16
  store i16 %49, ptr addrspace(1) %47, !tbaa !4
  %50 = load i16, ptr %20, !tbaa !2
  %51 = mul i16 1, 257
  %52 = add i16 %51, %50
  %53 = mul i16 %52, 2
  %54 = getelementptr i8, ptr %21, i16 2
  %55 = load i16, ptr %54, !tbaa !2
  %56 = add i16 0, %53
  %57 = inttoptr i16 %55 to ptr addrspace(2)
  %58 = addrspacecast ptr addrspace(2) %57 to ptr addrspace(1)
  %59 = getelementptr i8, ptr addrspace(1) %58, i16 %56
  %60 = call i8 @llrm.ia16.in.i8(i16 969)
  %61 = zext i8 %60 to i16
  store i16 %61, ptr addrspace(1) %59, !tbaa !4
  %62 = load i16, ptr %20, !tbaa !2
  %63 = mul i16 2, 257
  %64 = add i16 %63, %62
  %65 = mul i16 %64, 2
  %66 = getelementptr i8, ptr %21, i16 2
  %67 = load i16, ptr %66, !tbaa !2
  %68 = add i16 0, %65
  %69 = inttoptr i16 %67 to ptr addrspace(2)
  %70 = addrspacecast ptr addrspace(2) %69 to ptr addrspace(1)
  %71 = getelementptr i8, ptr addrspace(1) %70, i16 %68
  %72 = call i8 @llrm.ia16.in.i8(i16 969)
  %73 = zext i8 %72 to i16
  store i16 %73, ptr addrspace(1) %71, !tbaa !4
  %74 = load i16, ptr %20, !tbaa !2
  %75 = load i16, ptr %18, !tbaa !2
  %76 = add i16 %74, %75
  store i16 %76, ptr %20, !tbaa !2
  br label %b2

b6:
  store i16 0, ptr %17, !tbaa !2
  %77 = load i16, ptr %0
  store i16 %77, ptr %16, !tbaa !2
  store i16 1, ptr %15, !tbaa !2
  br label %b7

b7:
  %78 = load i16, ptr %15, !tbaa !2
  %79 = icmp sge i16 %78, 0
  %80 = sext i1 %79 to i16
  %81 = icmp ne i16 %80, 0
  br i1 %81, label %b8, label %b9

b8:
  %82 = load i16, ptr %17, !tbaa !2
  %83 = load i16, ptr %16, !tbaa !2
  %84 = icmp sle i16 %82, %83
  %85 = sext i1 %84 to i16
  %86 = icmp ne i16 %85, 0
  br i1 %86, label %b10, label %b11

b9:
  %87 = load i16, ptr %17, !tbaa !2
  %88 = load i16, ptr %16, !tbaa !2
  %89 = icmp sge i16 %87, %88
  %90 = sext i1 %89 to i16
  %91 = icmp ne i16 %90, 0
  br i1 %91, label %b10, label %b11

b10:
  %92 = load i16, ptr %17, !tbaa !2
  %93 = load i16, ptr %0
  store i16 %92, ptr %13, !tbaa !2
  %94 = load i16, ptr %13, !tbaa !2
  %95 = sitofp i16 %94 to float
  store i16 %93, ptr %12, !tbaa !2
  %96 = load i16, ptr %12, !tbaa !2
  %97 = sitofp i16 %96 to float
  %98 = fdiv float %95, %97
  store float %98, ptr %14, !tbaa !2
  %99 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %99, ptr @"BENCHFRAME&", !tbaa !2
  %100 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %100, ptr @"BENCHFRAME&", !tbaa !2
  store i16 0, ptr %20, !tbaa !2
  store i16 255, ptr %11, !tbaa !2
  store i16 1, ptr %10, !tbaa !2
  br label %b12

b11:
  call cc1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %21)
  ret void

b12:
  %101 = load i16, ptr %10, !tbaa !2
  %102 = icmp sge i16 %101, 0
  %103 = sext i1 %102 to i16
  %104 = icmp ne i16 %103, 0
  br i1 %104, label %b13, label %b14

b13:
  %105 = load i16, ptr %20, !tbaa !2
  %106 = load i16, ptr %11, !tbaa !2
  %107 = icmp sle i16 %105, %106
  %108 = sext i1 %107 to i16
  %109 = icmp ne i16 %108, 0
  br i1 %109, label %b15, label %b16

b14:
  %110 = load i16, ptr %20, !tbaa !2
  %111 = load i16, ptr %11, !tbaa !2
  %112 = icmp sge i16 %110, %111
  %113 = sext i1 %112 to i16
  %114 = icmp ne i16 %113, 0
  br i1 %114, label %b15, label %b16

b15:
  %115 = load i16, ptr %20, !tbaa !2
  %116 = trunc i16 %115 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %116)
  %117 = load i16, ptr %20, !tbaa !2
  %118 = mul i16 0, 257
  %119 = add i16 %118, %117
  %120 = mul i16 %119, 2
  %121 = getelementptr i8, ptr %21, i16 2
  %122 = load i16, ptr %121, !tbaa !2
  %123 = add i16 0, %120
  %124 = inttoptr i16 %122 to ptr addrspace(2)
  %125 = addrspacecast ptr addrspace(2) %124 to ptr addrspace(1)
  %126 = getelementptr i8, ptr addrspace(1) %125, i16 %123
  %127 = load i16, ptr addrspace(1) %126, !tbaa !4
  %128 = load float, ptr %14, !tbaa !2
  store i16 %127, ptr %9, !tbaa !2
  %129 = load i16, ptr %9, !tbaa !2
  %130 = sitofp i16 %129 to float
  %131 = fmul float %130, %128
  %132 = load float, ptr %14, !tbaa !2
  store i16 1, ptr %8, !tbaa !2
  %133 = load i16, ptr %8, !tbaa !2
  %134 = sitofp i16 %133 to float
  %135 = fsub float %134, %132
  store i16 63, ptr %7, !tbaa !2
  %136 = load i16, ptr %7, !tbaa !2
  %137 = sitofp i16 %136 to float
  %138 = fmul float %137, %135
  %139 = fadd float %131, %138
  %140 = call i16 @llvm.lrint.i16.f32(float %139)
  %141 = trunc i16 %140 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %141)
  %142 = load i16, ptr %20, !tbaa !2
  %143 = mul i16 1, 257
  %144 = add i16 %143, %142
  %145 = mul i16 %144, 2
  %146 = getelementptr i8, ptr %21, i16 2
  %147 = load i16, ptr %146, !tbaa !2
  %148 = add i16 0, %145
  %149 = inttoptr i16 %147 to ptr addrspace(2)
  %150 = addrspacecast ptr addrspace(2) %149 to ptr addrspace(1)
  %151 = getelementptr i8, ptr addrspace(1) %150, i16 %148
  %152 = load i16, ptr addrspace(1) %151, !tbaa !4
  %153 = load float, ptr %14, !tbaa !2
  store i16 %152, ptr %6, !tbaa !2
  %154 = load i16, ptr %6, !tbaa !2
  %155 = sitofp i16 %154 to float
  %156 = fmul float %155, %153
  %157 = load float, ptr %14, !tbaa !2
  store i16 1, ptr %5, !tbaa !2
  %158 = load i16, ptr %5, !tbaa !2
  %159 = sitofp i16 %158 to float
  %160 = fsub float %159, %157
  store i16 63, ptr %4, !tbaa !2
  %161 = load i16, ptr %4, !tbaa !2
  %162 = sitofp i16 %161 to float
  %163 = fmul float %162, %160
  %164 = fadd float %156, %163
  %165 = call i16 @llvm.lrint.i16.f32(float %164)
  %166 = trunc i16 %165 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %166)
  %167 = load i16, ptr %20, !tbaa !2
  %168 = mul i16 2, 257
  %169 = add i16 %168, %167
  %170 = mul i16 %169, 2
  %171 = getelementptr i8, ptr %21, i16 2
  %172 = load i16, ptr %171, !tbaa !2
  %173 = add i16 0, %170
  %174 = inttoptr i16 %172 to ptr addrspace(2)
  %175 = addrspacecast ptr addrspace(2) %174 to ptr addrspace(1)
  %176 = getelementptr i8, ptr addrspace(1) %175, i16 %173
  %177 = load i16, ptr addrspace(1) %176, !tbaa !4
  %178 = load float, ptr %14, !tbaa !2
  store i16 %177, ptr %3, !tbaa !2
  %179 = load i16, ptr %3, !tbaa !2
  %180 = sitofp i16 %179 to float
  %181 = fmul float %180, %178
  %182 = load float, ptr %14, !tbaa !2
  store i16 1, ptr %2, !tbaa !2
  %183 = load i16, ptr %2, !tbaa !2
  %184 = sitofp i16 %183 to float
  %185 = fsub float %184, %182
  store i16 63, ptr %1, !tbaa !2
  %186 = load i16, ptr %1, !tbaa !2
  %187 = sitofp i16 %186 to float
  %188 = fmul float %187, %185
  %189 = fadd float %181, %188
  %190 = call i16 @llvm.lrint.i16.f32(float %189)
  %191 = trunc i16 %190 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %191)
  %192 = load i16, ptr %20, !tbaa !2
  %193 = load i16, ptr %10, !tbaa !2
  %194 = add i16 %192, %193
  store i16 %194, ptr %20, !tbaa !2
  br label %b12

b16:
  %195 = load i16, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %196 = add i16 %195, 1
  store i16 %196, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %197 = load i16, ptr %17, !tbaa !2
  %198 = load i16, ptr %15, !tbaa !2
  %199 = add i16 %197, %198
  store i16 %199, ptr %17, !tbaa !2
  br label %b7
}

define cc1000 void @UPDPALPLASMA(ptr %0) addrspace(1) {
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
  store i16 0, ptr %1
  store i16 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %4
  store i16 0, ptr %5
  store i16 0, ptr %6
  store i16 0, ptr %7
  store i16 0, ptr %8
  store i16 0, ptr %9
  store i16 0, ptr %10
  store i16 0, ptr %11
  store i16 0, ptr %12
  store i16 0, ptr %13
  store i16 0, ptr %14
  store i16 0, ptr %15
  store i16 0, ptr %16
  store i16 0, ptr %17
  store i16 0, ptr %18
  store i16 0, ptr %18, !tbaa !2
  store i16 255, ptr %17, !tbaa !2
  store i16 1, ptr %16, !tbaa !2
  br label %b2

b2:
  %19 = load i16, ptr %16, !tbaa !2
  %20 = icmp sge i16 %19, 0
  %21 = sext i1 %20 to i16
  %22 = icmp ne i16 %21, 0
  br i1 %22, label %b3, label %b4

b3:
  %23 = load i16, ptr %18, !tbaa !2
  %24 = load i16, ptr %17, !tbaa !2
  %25 = icmp sle i16 %23, %24
  %26 = sext i1 %25 to i16
  %27 = icmp ne i16 %26, 0
  br i1 %27, label %b5, label %b6

b4:
  %28 = load i16, ptr %18, !tbaa !2
  %29 = load i16, ptr %17, !tbaa !2
  %30 = icmp sge i16 %28, %29
  %31 = sext i1 %30 to i16
  %32 = icmp ne i16 %31, 0
  br i1 %32, label %b5, label %b6

b5:
  %33 = load i16, ptr %18, !tbaa !2
  %34 = trunc i16 %33 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %34)
  %35 = load i16, ptr %18, !tbaa !2
  %36 = load double, ptr @$float13, !tbaa !2
  store i16 %35, ptr %15, !tbaa !2
  %37 = load i16, ptr %15, !tbaa !2
  %38 = sitofp i16 %37 to double
  %39 = fmul double %38, %36
  store i16 128, ptr %14, !tbaa !2
  %40 = load i16, ptr %14, !tbaa !2
  %41 = sitofp i16 %40 to double
  %42 = fdiv double %39, %41
  %43 = load i16, ptr %0
  %44 = load float, ptr @$float14, !tbaa !2
  store i16 %43, ptr %13, !tbaa !2
  %45 = load i16, ptr %13, !tbaa !2
  %46 = sitofp i16 %45 to float
  %47 = fmul float %46, %44
  %48 = fpext float %47 to double
  %49 = fadd double %42, %48
  %50 = call double @llvm.cos.f64(double %49)
  store i16 31, ptr %12, !tbaa !2
  %51 = load i16, ptr %12, !tbaa !2
  %52 = sitofp i16 %51 to double
  %53 = fmul double %52, %50
  store i16 32, ptr %11, !tbaa !2
  %54 = load i16, ptr %11, !tbaa !2
  %55 = sitofp i16 %54 to double
  %56 = fsub double %55, %53
  %57 = call i16 @llvm.lrint.i16.f64(double %56)
  %58 = trunc i16 %57 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %58)
  %59 = load i16, ptr %18, !tbaa !2
  %60 = load double, ptr @$float13, !tbaa !2
  store i16 %59, ptr %10, !tbaa !2
  %61 = load i16, ptr %10, !tbaa !2
  %62 = sitofp i16 %61 to double
  %63 = fmul double %62, %60
  store i16 128, ptr %9, !tbaa !2
  %64 = load i16, ptr %9, !tbaa !2
  %65 = sitofp i16 %64 to double
  %66 = fdiv double %63, %65
  %67 = load i16, ptr %0
  %68 = load float, ptr @$float15, !tbaa !2
  store i16 %67, ptr %8, !tbaa !2
  %69 = load i16, ptr %8, !tbaa !2
  %70 = sitofp i16 %69 to float
  %71 = fmul float %70, %68
  %72 = fpext float %71 to double
  %73 = fadd double %66, %72
  %74 = call double @llvm.cos.f64(double %73)
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
  %84 = load double, ptr @$float13, !tbaa !2
  store i16 %83, ptr %5, !tbaa !2
  %85 = load i16, ptr %5, !tbaa !2
  %86 = sitofp i16 %85 to double
  %87 = fmul double %86, %84
  store i16 64, ptr %4, !tbaa !2
  %88 = load i16, ptr %4, !tbaa !2
  %89 = sitofp i16 %88 to double
  %90 = fdiv double %87, %89
  %91 = load i16, ptr %0
  %92 = load float, ptr @$float16, !tbaa !2
  store i16 %91, ptr %3, !tbaa !2
  %93 = load i16, ptr %3, !tbaa !2
  %94 = sitofp i16 %93 to float
  %95 = fmul float %94, %92
  %96 = fpext float %95 to double
  %97 = fadd double %90, %96
  %98 = call double @llvm.cos.f64(double %97)
  store i16 31, ptr %2, !tbaa !2
  %99 = load i16, ptr %2, !tbaa !2
  %100 = sitofp i16 %99 to double
  %101 = fmul double %100, %98
  store i16 32, ptr %1, !tbaa !2
  %102 = load i16, ptr %1, !tbaa !2
  %103 = sitofp i16 %102 to double
  %104 = fsub double %103, %101
  %105 = call i16 @llvm.lrint.i16.f64(double %104)
  %106 = trunc i16 %105 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %106)
  %107 = load i16, ptr %18, !tbaa !2
  %108 = load i16, ptr %16, !tbaa !2
  %109 = add i16 %107, %108
  store i16 %109, ptr %18, !tbaa !2
  br label %b2

b6:
  ret void
}

define cc1000 void @WHITEFADE(ptr %0) addrspace(1) {
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
  store i16 0, ptr %1
  store i16 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %4
  store i16 0, ptr %5
  store i16 0, ptr %6
  store i16 0, ptr %7
  store i16 0, ptr %8
  store i16 0, ptr %9
  store i16 0, ptr %10
  store i16 0, ptr %11
  store i16 0, ptr %12
  store i16 0, ptr %13
  store float 0.000000e+00, ptr %14
  store i16 0, ptr %15
  store i16 0, ptr %16
  store i16 0, ptr %17
  store i16 0, ptr %18
  store i16 0, ptr %19
  store i16 0, ptr %20
  call void @llvm.memset.p0.i16(ptr %21, i8 0, i16 22, i1 false)
  call cc1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 256, i16 0, i16 3, i16 2, i16 258, ptr %21)
  store i16 0, ptr %20, !tbaa !2
  store i16 255, ptr %19, !tbaa !2
  store i16 1, ptr %18, !tbaa !2
  br label %b2

b2:
  %22 = load i16, ptr %18, !tbaa !2
  %23 = icmp sge i16 %22, 0
  %24 = sext i1 %23 to i16
  %25 = icmp ne i16 %24, 0
  br i1 %25, label %b3, label %b4

b3:
  %26 = load i16, ptr %20, !tbaa !2
  %27 = load i16, ptr %19, !tbaa !2
  %28 = icmp sle i16 %26, %27
  %29 = sext i1 %28 to i16
  %30 = icmp ne i16 %29, 0
  br i1 %30, label %b5, label %b6

b4:
  %31 = load i16, ptr %20, !tbaa !2
  %32 = load i16, ptr %19, !tbaa !2
  %33 = icmp sge i16 %31, %32
  %34 = sext i1 %33 to i16
  %35 = icmp ne i16 %34, 0
  br i1 %35, label %b5, label %b6

b5:
  %36 = load i16, ptr %20, !tbaa !2
  %37 = trunc i16 %36 to i8
  call void @llrm.ia16.out.i8(i16 967, i8 %37)
  %38 = load i16, ptr %20, !tbaa !2
  %39 = mul i16 0, 257
  %40 = add i16 %39, %38
  %41 = mul i16 %40, 2
  %42 = getelementptr i8, ptr %21, i16 2
  %43 = load i16, ptr %42, !tbaa !2
  %44 = add i16 0, %41
  %45 = inttoptr i16 %43 to ptr addrspace(2)
  %46 = addrspacecast ptr addrspace(2) %45 to ptr addrspace(1)
  %47 = getelementptr i8, ptr addrspace(1) %46, i16 %44
  %48 = call i8 @llrm.ia16.in.i8(i16 969)
  %49 = zext i8 %48 to i16
  store i16 %49, ptr addrspace(1) %47, !tbaa !4
  %50 = load i16, ptr %20, !tbaa !2
  %51 = mul i16 1, 257
  %52 = add i16 %51, %50
  %53 = mul i16 %52, 2
  %54 = getelementptr i8, ptr %21, i16 2
  %55 = load i16, ptr %54, !tbaa !2
  %56 = add i16 0, %53
  %57 = inttoptr i16 %55 to ptr addrspace(2)
  %58 = addrspacecast ptr addrspace(2) %57 to ptr addrspace(1)
  %59 = getelementptr i8, ptr addrspace(1) %58, i16 %56
  %60 = call i8 @llrm.ia16.in.i8(i16 969)
  %61 = zext i8 %60 to i16
  store i16 %61, ptr addrspace(1) %59, !tbaa !4
  %62 = load i16, ptr %20, !tbaa !2
  %63 = mul i16 2, 257
  %64 = add i16 %63, %62
  %65 = mul i16 %64, 2
  %66 = getelementptr i8, ptr %21, i16 2
  %67 = load i16, ptr %66, !tbaa !2
  %68 = add i16 0, %65
  %69 = inttoptr i16 %67 to ptr addrspace(2)
  %70 = addrspacecast ptr addrspace(2) %69 to ptr addrspace(1)
  %71 = getelementptr i8, ptr addrspace(1) %70, i16 %68
  %72 = call i8 @llrm.ia16.in.i8(i16 969)
  %73 = zext i8 %72 to i16
  store i16 %73, ptr addrspace(1) %71, !tbaa !4
  %74 = load i16, ptr %20, !tbaa !2
  %75 = load i16, ptr %18, !tbaa !2
  %76 = add i16 %74, %75
  store i16 %76, ptr %20, !tbaa !2
  br label %b2

b6:
  %77 = load i16, ptr %0
  store i16 %77, ptr %17, !tbaa !2
  store i16 0, ptr %16, !tbaa !2
  %78 = sub i16 0, 1
  store i16 %78, ptr %15, !tbaa !2
  br label %b7

b7:
  %79 = load i16, ptr %15, !tbaa !2
  %80 = icmp sge i16 %79, 0
  %81 = sext i1 %80 to i16
  %82 = icmp ne i16 %81, 0
  br i1 %82, label %b8, label %b9

b8:
  %83 = load i16, ptr %17, !tbaa !2
  %84 = load i16, ptr %16, !tbaa !2
  %85 = icmp sle i16 %83, %84
  %86 = sext i1 %85 to i16
  %87 = icmp ne i16 %86, 0
  br i1 %87, label %b10, label %b11

b9:
  %88 = load i16, ptr %17, !tbaa !2
  %89 = load i16, ptr %16, !tbaa !2
  %90 = icmp sge i16 %88, %89
  %91 = sext i1 %90 to i16
  %92 = icmp ne i16 %91, 0
  br i1 %92, label %b10, label %b11

b10:
  %93 = load i16, ptr %17, !tbaa !2
  %94 = load i16, ptr %0
  store i16 %93, ptr %13, !tbaa !2
  %95 = load i16, ptr %13, !tbaa !2
  %96 = sitofp i16 %95 to float
  store i16 %94, ptr %12, !tbaa !2
  %97 = load i16, ptr %12, !tbaa !2
  %98 = sitofp i16 %97 to float
  %99 = fdiv float %96, %98
  store float %99, ptr %14, !tbaa !2
  %100 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %100, ptr @"BENCHFRAME&", !tbaa !2
  %101 = load i32, ptr @"BENCHFRAME&", !tbaa !2
  store i32 %101, ptr @"BENCHFRAME&", !tbaa !2
  store i16 0, ptr %20, !tbaa !2
  store i16 255, ptr %11, !tbaa !2
  store i16 1, ptr %10, !tbaa !2
  br label %b12

b11:
  call cc1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %21)
  ret void

b12:
  %102 = load i16, ptr %10, !tbaa !2
  %103 = icmp sge i16 %102, 0
  %104 = sext i1 %103 to i16
  %105 = icmp ne i16 %104, 0
  br i1 %105, label %b13, label %b14

b13:
  %106 = load i16, ptr %20, !tbaa !2
  %107 = load i16, ptr %11, !tbaa !2
  %108 = icmp sle i16 %106, %107
  %109 = sext i1 %108 to i16
  %110 = icmp ne i16 %109, 0
  br i1 %110, label %b15, label %b16

b14:
  %111 = load i16, ptr %20, !tbaa !2
  %112 = load i16, ptr %11, !tbaa !2
  %113 = icmp sge i16 %111, %112
  %114 = sext i1 %113 to i16
  %115 = icmp ne i16 %114, 0
  br i1 %115, label %b15, label %b16

b15:
  %116 = load i16, ptr %20, !tbaa !2
  %117 = trunc i16 %116 to i8
  call void @llrm.ia16.out.i8(i16 968, i8 %117)
  %118 = load i16, ptr %20, !tbaa !2
  %119 = mul i16 0, 257
  %120 = add i16 %119, %118
  %121 = mul i16 %120, 2
  %122 = getelementptr i8, ptr %21, i16 2
  %123 = load i16, ptr %122, !tbaa !2
  %124 = add i16 0, %121
  %125 = inttoptr i16 %123 to ptr addrspace(2)
  %126 = addrspacecast ptr addrspace(2) %125 to ptr addrspace(1)
  %127 = getelementptr i8, ptr addrspace(1) %126, i16 %124
  %128 = load i16, ptr addrspace(1) %127, !tbaa !4
  %129 = load float, ptr %14, !tbaa !2
  store i16 %128, ptr %9, !tbaa !2
  %130 = load i16, ptr %9, !tbaa !2
  %131 = sitofp i16 %130 to float
  %132 = fmul float %131, %129
  %133 = load float, ptr %14, !tbaa !2
  store i16 1, ptr %8, !tbaa !2
  %134 = load i16, ptr %8, !tbaa !2
  %135 = sitofp i16 %134 to float
  %136 = fsub float %135, %133
  store i16 63, ptr %7, !tbaa !2
  %137 = load i16, ptr %7, !tbaa !2
  %138 = sitofp i16 %137 to float
  %139 = fmul float %138, %136
  %140 = fadd float %132, %139
  %141 = call i16 @llvm.lrint.i16.f32(float %140)
  %142 = trunc i16 %141 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %142)
  %143 = load i16, ptr %20, !tbaa !2
  %144 = mul i16 1, 257
  %145 = add i16 %144, %143
  %146 = mul i16 %145, 2
  %147 = getelementptr i8, ptr %21, i16 2
  %148 = load i16, ptr %147, !tbaa !2
  %149 = add i16 0, %146
  %150 = inttoptr i16 %148 to ptr addrspace(2)
  %151 = addrspacecast ptr addrspace(2) %150 to ptr addrspace(1)
  %152 = getelementptr i8, ptr addrspace(1) %151, i16 %149
  %153 = load i16, ptr addrspace(1) %152, !tbaa !4
  %154 = load float, ptr %14, !tbaa !2
  store i16 %153, ptr %6, !tbaa !2
  %155 = load i16, ptr %6, !tbaa !2
  %156 = sitofp i16 %155 to float
  %157 = fmul float %156, %154
  %158 = load float, ptr %14, !tbaa !2
  store i16 1, ptr %5, !tbaa !2
  %159 = load i16, ptr %5, !tbaa !2
  %160 = sitofp i16 %159 to float
  %161 = fsub float %160, %158
  store i16 63, ptr %4, !tbaa !2
  %162 = load i16, ptr %4, !tbaa !2
  %163 = sitofp i16 %162 to float
  %164 = fmul float %163, %161
  %165 = fadd float %157, %164
  %166 = call i16 @llvm.lrint.i16.f32(float %165)
  %167 = trunc i16 %166 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %167)
  %168 = load i16, ptr %20, !tbaa !2
  %169 = mul i16 2, 257
  %170 = add i16 %169, %168
  %171 = mul i16 %170, 2
  %172 = getelementptr i8, ptr %21, i16 2
  %173 = load i16, ptr %172, !tbaa !2
  %174 = add i16 0, %171
  %175 = inttoptr i16 %173 to ptr addrspace(2)
  %176 = addrspacecast ptr addrspace(2) %175 to ptr addrspace(1)
  %177 = getelementptr i8, ptr addrspace(1) %176, i16 %174
  %178 = load i16, ptr addrspace(1) %177, !tbaa !4
  %179 = load float, ptr %14, !tbaa !2
  store i16 %178, ptr %3, !tbaa !2
  %180 = load i16, ptr %3, !tbaa !2
  %181 = sitofp i16 %180 to float
  %182 = fmul float %181, %179
  %183 = load float, ptr %14, !tbaa !2
  store i16 1, ptr %2, !tbaa !2
  %184 = load i16, ptr %2, !tbaa !2
  %185 = sitofp i16 %184 to float
  %186 = fsub float %185, %183
  store i16 63, ptr %1, !tbaa !2
  %187 = load i16, ptr %1, !tbaa !2
  %188 = sitofp i16 %187 to float
  %189 = fmul float %188, %186
  %190 = fadd float %182, %189
  %191 = call i16 @llvm.lrint.i16.f32(float %190)
  %192 = trunc i16 %191 to i8
  call void @llrm.ia16.out.i8(i16 969, i8 %192)
  %193 = load i16, ptr %20, !tbaa !2
  %194 = load i16, ptr %10, !tbaa !2
  %195 = add i16 %193, %194
  store i16 %195, ptr %20, !tbaa !2
  br label %b12

b16:
  %196 = load i16, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %197 = add i16 %196, 1
  store i16 %197, ptr @"TOTALFRAMECOUNT%", !tbaa !2
  %198 = load i16, ptr %17, !tbaa !2
  %199 = load i16, ptr %15, !tbaa !2
  %200 = add i16 %198, %199
  store i16 %200, ptr %17, !tbaa !2
  br label %b7
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
  %49 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string17, ptr %48)
  %50 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %49, ptr @$string18)
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
  %90 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string19, ptr %89)
  %91 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %90, ptr @$string20)
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
  %160 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string21, ptr %159)
  %161 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %160, ptr @$string22)
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

declare cc1000 void @llrm.qb.B$CSCN(i16, i16, i16) addrspace(1)

declare cc1000 void @llrm.qb.B$DDIM(i16, i16, i16, i16, ptr) addrspace(1)

declare void @llrm.ia16.out.i8(i16, i8) nocallback nofree nounwind willreturn memory(read, inaccessiblemem: readwrite)

declare cc1000 ptr @llrm.qb.B$TIMR() addrspace(1)

declare cc1000 void @llrm.qb.B$ERAS(ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$SCLS(i16) addrspace(1)

declare cc1000 void @llrm.qb.B$CEND() addrspace(1)

declare float @llvm.sqrt.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i16 @llvm.lrint.i16.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare cc1000 ptr @llrm.qb.B$INKY() addrspace(1)

declare cc1000 i16 @llrm.qb.B$SCMP(ptr, ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$BLOD(ptr, i16, i16) addrspace(1)

declare float @llvm.sin.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare float @llvm.cos.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i8 @llrm.ia16.in.i8(i16) nocallback nofree nounwind willreturn memory(read, inaccessiblemem: readwrite)

declare double @llvm.cos.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i16 @llvm.lrint.i16.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc1000 void @TSCSNAP(ptr, ptr) addrspace(1)

declare cc1000 ptr @llrm.qb.B$STI2(i16) addrspace(1)

declare cc1000 ptr @llrm.qb.B$LTRM(ptr) addrspace(1)

declare cc1000 ptr @llrm.qb.B$SCAT(ptr, ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$BSAV(ptr, i16, i16) addrspace(1)

declare cc1000 void @llrm.qb.B$DSG0() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
