target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [8 x i8] c"\08\00\01\00\01\00 \00"

define internal i16 @trace(ptr addrspace(1) noalias readonly dereferenceable(10) %0) addrspace(1) {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  store i16 0, ptr %1
  store i16 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %3, !tbaa !2
  %4 = load i16, ptr addrspace(1) %0
  store i16 0, ptr %2, !tbaa !2
  store i16 %4, ptr %1, !tbaa !2
  br label %b2

b2:
  %5 = load i16, ptr %2, !tbaa !2
  %6 = load i16, ptr %1, !tbaa !2
  %7 = icmp ult i16 %5, %6
  %8 = sext i1 %7 to i8
  %9 = icmp ne i8 %8, 0
  br i1 %9, label %b3, label %b5

b3:
  %10 = load i16, ptr %3, !tbaa !2
  %11 = load i16, ptr %2, !tbaa !2
  %12 = load i16, ptr %2, !tbaa !2
  %13 = load i16, ptr addrspace(1) %0
  %14 = icmp ult i16 %11, %13
  %15 = sext i1 %14 to i8
  %16 = icmp ne i8 %15, 0
  br i1 %16, label %b6, label %b7

b4:
  %17 = load i16, ptr %2, !tbaa !2
  %18 = add i16 %17, 1
  store i16 %18, ptr %2, !tbaa !2
  br label %b2

b5:
  %19 = load i16, ptr %3, !tbaa !2
  ret i16 %19

b6:
  %20 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %21 = load i16, ptr addrspace(1) %20
  %22 = icmp ult i16 %12, %21
  %23 = sext i1 %22 to i8
  %24 = icmp ne i8 %23, 0
  br i1 %24, label %b8, label %b9

b7:
  call addrspace(1) void @N$EBND()
  unreachable

b8:
  %25 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %26 = load i16, ptr addrspace(1) %25
  %27 = mul i16 %26, 1
  %28 = mul i16 %11, %27
  %29 = mul i16 %12, 1
  %30 = add i16 %28, %29
  %31 = getelementptr i8, ptr addrspace(1) %0, i16 6
  %32 = load ptr addrspace(1), ptr addrspace(1) %31
  %33 = mul i16 %30, 2
  %34 = getelementptr i8, ptr addrspace(1) %32, i16 %33
  %35 = load i16, ptr addrspace(1) %34
  %36 = add i16 %10, %35
  store i16 %36, ptr %3, !tbaa !2
  br label %b4

b9:
  call addrspace(1) void @N$EBND()
  unreachable
}

define internal void @bump(ptr addrspace(1) noalias readonly dereferenceable(10) %0) addrspace(1) {
b1:
  %1 = load i16, ptr addrspace(1) %0
  %2 = icmp ult i16 1, %1
  %3 = sext i1 %2 to i8
  %4 = icmp ne i8 %3, 0
  br i1 %4, label %b2, label %b3

b2:
  %5 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %6 = load i16, ptr addrspace(1) %5
  %7 = icmp ult i16 2, %6
  %8 = sext i1 %7 to i8
  %9 = icmp ne i8 %8, 0
  br i1 %9, label %b4, label %b5

b3:
  call addrspace(1) void @N$EBND()
  unreachable

b4:
  %10 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %11 = load i16, ptr addrspace(1) %10
  %12 = mul i16 %11, 1
  %13 = mul i16 1, %12
  %14 = add i16 %13, 2
  %15 = getelementptr i8, ptr addrspace(1) %0, i16 6
  %16 = load ptr addrspace(1), ptr addrspace(1) %15
  %17 = mul i16 %14, 2
  %18 = getelementptr i8, ptr addrspace(1) %16, i16 %17
  %19 = load i16, ptr addrspace(1) %18
  %20 = add i16 %19, 100
  store i16 %20, ptr addrspace(1) %18
  ret void

b5:
  call addrspace(1) void @N$EBND()
  unreachable
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca i16
  %1 = alloca [10 x i8]
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca [10 x i8]
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
  %23 = alloca [64 x i8]
  %24 = alloca i32
  %25 = alloca i16
  %26 = alloca i16
  %27 = alloca i16
  %28 = alloca i16
  %29 = alloca i16
  %30 = alloca i16
  %31 = alloca i16
  %32 = alloca i16
  %33 = alloca i16
  %34 = alloca i16
  %35 = alloca [24 x i8]
  %36 = alloca i8
  %37 = alloca i16
  %38 = alloca i16
  %39 = alloca i16
  %40 = alloca [18 x i8]
  store i16 0, ptr %0
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 10, i1 false)
  store i16 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %4
  store i16 0, ptr %5
  store i16 0, ptr %6
  store i16 0, ptr %7
  store i16 0, ptr %8
  call void @llvm.memset.p0.i16(ptr %9, i8 0, i16 10, i1 false)
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
  call void @llvm.memset.p0.i16(ptr %23, i8 0, i16 64, i1 false)
  store i32 0, ptr %24
  store i16 0, ptr %25
  store i16 0, ptr %26
  store i16 0, ptr %27
  store i16 0, ptr %28
  store i16 0, ptr %29
  store i16 0, ptr %30
  store i16 0, ptr %31
  store i16 0, ptr %32
  store i16 0, ptr %33
  store i16 0, ptr %34
  call void @llvm.memset.p0.i16(ptr %35, i8 0, i16 24, i1 false)
  store i8 0, ptr %36
  store i16 0, ptr %37
  store i16 0, ptr %38
  store i16 0, ptr %39
  call void @llvm.memset.p0.i16(ptr %40, i8 0, i16 18, i1 false)
  store i16 3, ptr %37, !tbaa !2
  store i16 3, ptr %38, !tbaa !2
  store i16 9, ptr %39, !tbaa !2
  %41 = sub i16 0, 0
  %42 = sub i16 0, 0
  %43 = mul i16 %41, 3
  %44 = add i16 %43, %42
  %45 = getelementptr inbounds i16, ptr %40, i16 %44
  store i16 1, ptr %45, !tbaa !2
  %46 = sub i16 0, 0
  %47 = sub i16 1, 0
  %48 = mul i16 %46, 3
  %49 = add i16 %48, %47
  %50 = getelementptr inbounds i16, ptr %40, i16 %49
  store i16 2, ptr %50, !tbaa !2
  %51 = sub i16 0, 0
  %52 = sub i16 2, 0
  %53 = mul i16 %51, 3
  %54 = add i16 %53, %52
  %55 = getelementptr inbounds i16, ptr %40, i16 %54
  store i16 3, ptr %55, !tbaa !2
  %56 = sub i16 1, 0
  %57 = sub i16 0, 0
  %58 = mul i16 %56, 3
  %59 = add i16 %58, %57
  %60 = getelementptr inbounds i16, ptr %40, i16 %59
  store i16 4, ptr %60, !tbaa !2
  %61 = sub i16 1, 0
  %62 = sub i16 1, 0
  %63 = mul i16 %61, 3
  %64 = add i16 %63, %62
  %65 = getelementptr inbounds i16, ptr %40, i16 %64
  store i16 5, ptr %65, !tbaa !2
  %66 = sub i16 1, 0
  %67 = sub i16 2, 0
  %68 = mul i16 %66, 3
  %69 = add i16 %68, %67
  %70 = getelementptr inbounds i16, ptr %40, i16 %69
  store i16 6, ptr %70, !tbaa !2
  %71 = sub i16 2, 0
  %72 = sub i16 0, 0
  %73 = mul i16 %71, 3
  %74 = add i16 %73, %72
  %75 = getelementptr inbounds i16, ptr %40, i16 %74
  store i16 7, ptr %75, !tbaa !2
  %76 = sub i16 2, 0
  %77 = sub i16 1, 0
  %78 = mul i16 %76, 3
  %79 = add i16 %78, %77
  %80 = getelementptr inbounds i16, ptr %40, i16 %79
  store i16 8, ptr %80, !tbaa !2
  %81 = sub i16 2, 0
  %82 = sub i16 2, 0
  %83 = mul i16 %81, 3
  %84 = add i16 %83, %82
  %85 = getelementptr inbounds i16, ptr %40, i16 %84
  store i16 9, ptr %85, !tbaa !2
  store i8 7, ptr %36, !tbaa !2
  store i16 2, ptr %31, !tbaa !2
  store i16 3, ptr %32, !tbaa !2
  store i16 4, ptr %33, !tbaa !2
  store i16 24, ptr %34, !tbaa !2
  store i16 0, ptr %30, !tbaa !2
  store i16 2, ptr %29, !tbaa !2
  br label %b2

b2:
  %86 = load i16, ptr %30, !tbaa !2
  %87 = load i16, ptr %29, !tbaa !2
  %88 = icmp slt i16 %86, %87
  %89 = sext i1 %88 to i8
  %90 = icmp ne i8 %89, 0
  br i1 %90, label %b3, label %b5

b3:
  store i16 0, ptr %28, !tbaa !2
  store i16 3, ptr %27, !tbaa !2
  br label %b6

b4:
  %91 = load i16, ptr %30, !tbaa !2
  %92 = add i16 %91, 1
  store i16 %92, ptr %30, !tbaa !2
  br label %b2

b5:
  %93 = sub i16 1, 0
  %94 = sub i16 2, 0
  %95 = mul i16 %93, 3
  %96 = add i16 %95, %94
  %97 = sub i16 3, 0
  %98 = mul i16 %96, 4
  %99 = add i16 %98, %97
  %100 = getelementptr inbounds i8, ptr %35, i16 %99
  store i8 1, ptr %100, !tbaa !2
  store i32 0, ptr %24, !tbaa !2
  store i16 2, ptr %18, !tbaa !2
  store i16 2, ptr %19, !tbaa !2
  store i16 2, ptr %20, !tbaa !2
  store i16 2, ptr %21, !tbaa !2
  store i16 16, ptr %22, !tbaa !2
  store i16 0, ptr %17, !tbaa !2
  store i16 2, ptr %16, !tbaa !2
  br label %b14

b6:
  %101 = load i16, ptr %28, !tbaa !2
  %102 = load i16, ptr %27, !tbaa !2
  %103 = icmp slt i16 %101, %102
  %104 = sext i1 %103 to i8
  %105 = icmp ne i8 %104, 0
  br i1 %105, label %b7, label %b9

b7:
  store i16 0, ptr %26, !tbaa !2
  store i16 4, ptr %25, !tbaa !2
  br label %b10

b8:
  %106 = load i16, ptr %28, !tbaa !2
  %107 = add i16 %106, 1
  store i16 %107, ptr %28, !tbaa !2
  br label %b6

b9:
  br label %b4

b10:
  %108 = load i16, ptr %26, !tbaa !2
  %109 = load i16, ptr %25, !tbaa !2
  %110 = icmp slt i16 %108, %109
  %111 = sext i1 %110 to i8
  %112 = icmp ne i8 %111, 0
  br i1 %112, label %b11, label %b13

b11:
  %113 = load i16, ptr %30, !tbaa !2
  %114 = load i16, ptr %28, !tbaa !2
  %115 = load i16, ptr %26, !tbaa !2
  %116 = load i8, ptr %36, !tbaa !2
  %117 = sub i16 %113, 0
  %118 = sub i16 %114, 0
  %119 = mul i16 %117, 3
  %120 = add i16 %119, %118
  %121 = sub i16 %115, 0
  %122 = mul i16 %120, 4
  %123 = add i16 %122, %121
  %124 = getelementptr inbounds i8, ptr %35, i16 %123
  store i8 %116, ptr %124, !tbaa !2
  br label %b12

b12:
  %125 = load i16, ptr %26, !tbaa !2
  %126 = add i16 %125, 1
  store i16 %126, ptr %26, !tbaa !2
  br label %b10

b13:
  br label %b8

b14:
  %127 = load i16, ptr %17, !tbaa !2
  %128 = load i16, ptr %16, !tbaa !2
  %129 = icmp slt i16 %127, %128
  %130 = sext i1 %129 to i8
  %131 = icmp ne i8 %130, 0
  br i1 %131, label %b15, label %b17

b15:
  store i16 0, ptr %15, !tbaa !2
  store i16 2, ptr %14, !tbaa !2
  br label %b18

b16:
  %132 = load i16, ptr %17, !tbaa !2
  %133 = add i16 %132, 1
  store i16 %133, ptr %17, !tbaa !2
  br label %b14

b17:
  %134 = sub i16 1, 0
  %135 = sub i16 0, 0
  %136 = mul i16 %134, 2
  %137 = add i16 %136, %135
  %138 = sub i16 1, 0
  %139 = mul i16 %137, 2
  %140 = add i16 %139, %138
  %141 = sub i16 0, 0
  %142 = mul i16 %140, 2
  %143 = add i16 %142, %141
  %144 = getelementptr inbounds i32, ptr %23, i16 %143
  store i32 42, ptr %144, !tbaa !2
  %145 = addrspacecast ptr %40 to ptr addrspace(1)
  store i16 3, ptr %9, !tbaa !2
  %146 = getelementptr inbounds i8, ptr %9, i16 2
  store i16 3, ptr %146, !tbaa !2
  %147 = getelementptr inbounds i8, ptr %9, i16 4
  store i16 9, ptr %147, !tbaa !2
  %148 = getelementptr inbounds i8, ptr %9, i16 6
  store ptr addrspace(1) %145, ptr %148, !tbaa !2
  %149 = addrspacecast ptr %9 to ptr addrspace(1)
  call addrspace(1) void @bump(ptr addrspace(1) %149)
  store i16 0, ptr %8, !tbaa !2
  store i16 0, ptr %7, !tbaa !2
  store i16 2, ptr %6, !tbaa !2
  br label %b30

b18:
  %150 = load i16, ptr %15, !tbaa !2
  %151 = load i16, ptr %14, !tbaa !2
  %152 = icmp slt i16 %150, %151
  %153 = sext i1 %152 to i8
  %154 = icmp ne i8 %153, 0
  br i1 %154, label %b19, label %b21

b19:
  store i16 0, ptr %13, !tbaa !2
  store i16 2, ptr %12, !tbaa !2
  br label %b22

b20:
  %155 = load i16, ptr %15, !tbaa !2
  %156 = add i16 %155, 1
  store i16 %156, ptr %15, !tbaa !2
  br label %b18

b21:
  br label %b16

b22:
  %157 = load i16, ptr %13, !tbaa !2
  %158 = load i16, ptr %12, !tbaa !2
  %159 = icmp slt i16 %157, %158
  %160 = sext i1 %159 to i8
  %161 = icmp ne i8 %160, 0
  br i1 %161, label %b23, label %b25

b23:
  store i16 0, ptr %11, !tbaa !2
  store i16 2, ptr %10, !tbaa !2
  br label %b26

b24:
  %162 = load i16, ptr %13, !tbaa !2
  %163 = add i16 %162, 1
  store i16 %163, ptr %13, !tbaa !2
  br label %b22

b25:
  br label %b20

b26:
  %164 = load i16, ptr %11, !tbaa !2
  %165 = load i16, ptr %10, !tbaa !2
  %166 = icmp slt i16 %164, %165
  %167 = sext i1 %166 to i8
  %168 = icmp ne i8 %167, 0
  br i1 %168, label %b27, label %b29

b27:
  %169 = load i16, ptr %17, !tbaa !2
  %170 = load i16, ptr %15, !tbaa !2
  %171 = load i16, ptr %13, !tbaa !2
  %172 = load i16, ptr %11, !tbaa !2
  %173 = load i32, ptr %24, !tbaa !2
  %174 = sub i16 %169, 0
  %175 = sub i16 %170, 0
  %176 = mul i16 %174, 2
  %177 = add i16 %176, %175
  %178 = sub i16 %171, 0
  %179 = mul i16 %177, 2
  %180 = add i16 %179, %178
  %181 = sub i16 %172, 0
  %182 = mul i16 %180, 2
  %183 = add i16 %182, %181
  %184 = getelementptr inbounds i32, ptr %23, i16 %183
  store i32 %173, ptr %184, !tbaa !2
  br label %b28

b28:
  %185 = load i16, ptr %11, !tbaa !2
  %186 = add i16 %185, 1
  store i16 %186, ptr %11, !tbaa !2
  br label %b26

b29:
  br label %b24

b30:
  %187 = load i16, ptr %7, !tbaa !2
  %188 = load i16, ptr %6, !tbaa !2
  %189 = icmp slt i16 %187, %188
  %190 = sext i1 %189 to i8
  %191 = icmp ne i8 %190, 0
  br i1 %191, label %b31, label %b33

b31:
  store i16 0, ptr %5, !tbaa !2
  store i16 3, ptr %4, !tbaa !2
  br label %b34

b32:
  %192 = load i16, ptr %7, !tbaa !2
  %193 = add i16 %192, 1
  store i16 %193, ptr %7, !tbaa !2
  br label %b30

b33:
  %194 = addrspacecast ptr %40 to ptr addrspace(1)
  store i16 3, ptr %1, !tbaa !2
  %195 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 3, ptr %195, !tbaa !2
  %196 = getelementptr inbounds i8, ptr %1, i16 4
  store i16 9, ptr %196, !tbaa !2
  %197 = getelementptr inbounds i8, ptr %1, i16 6
  store ptr addrspace(1) %194, ptr %197, !tbaa !2
  %198 = addrspacecast ptr %1 to ptr addrspace(1)
  %199 = call addrspace(1) i16 @trace(ptr addrspace(1) %198)
  store i16 %199, ptr %0, !tbaa !2
  %200 = load i16, ptr %0, !tbaa !2
  call addrspace(1) void @N$PI2(i16 %200)
  %201 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %201)
  %202 = sub i16 1, 0
  %203 = sub i16 2, 0
  %204 = mul i16 %202, 3
  %205 = add i16 %204, %203
  %206 = getelementptr inbounds i16, ptr %40, i16 %205
  %207 = load i16, ptr %206, !tbaa !2
  call addrspace(1) void @N$PI2(i16 %207)
  %208 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %208)
  %209 = load i16, ptr %8, !tbaa !2
  call addrspace(1) void @N$PI2(i16 %209)
  %210 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %210)
  %211 = sub i16 1, 0
  %212 = sub i16 0, 0
  %213 = mul i16 %211, 2
  %214 = add i16 %213, %212
  %215 = sub i16 1, 0
  %216 = mul i16 %214, 2
  %217 = add i16 %216, %215
  %218 = sub i16 0, 0
  %219 = mul i16 %217, 2
  %220 = add i16 %219, %218
  %221 = getelementptr inbounds i32, ptr %23, i16 %220
  %222 = load i32, ptr %221, !tbaa !2
  call addrspace(1) void @N$PI4(i32 %222)
  %223 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %223)
  call addrspace(1) void @N$PU2(i16 9)
  %224 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %224)
  call addrspace(1) void @N$PU2(i16 3)
  %225 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %225)
  call addrspace(1) void @N$PU2(i16 4)
  call addrspace(1) void @N$PN()
  ret i16 0

b34:
  %226 = load i16, ptr %5, !tbaa !2
  %227 = load i16, ptr %4, !tbaa !2
  %228 = icmp slt i16 %226, %227
  %229 = sext i1 %228 to i8
  %230 = icmp ne i8 %229, 0
  br i1 %230, label %b35, label %b37

b35:
  store i16 0, ptr %3, !tbaa !2
  store i16 4, ptr %2, !tbaa !2
  br label %b38

b36:
  %231 = load i16, ptr %5, !tbaa !2
  %232 = add i16 %231, 1
  store i16 %232, ptr %5, !tbaa !2
  br label %b34

b37:
  br label %b32

b38:
  %233 = load i16, ptr %3, !tbaa !2
  %234 = load i16, ptr %2, !tbaa !2
  %235 = icmp slt i16 %233, %234
  %236 = sext i1 %235 to i8
  %237 = icmp ne i8 %236, 0
  br i1 %237, label %b39, label %b41

b39:
  %238 = load i16, ptr %8, !tbaa !2
  %239 = load i16, ptr %7, !tbaa !2
  %240 = load i16, ptr %5, !tbaa !2
  %241 = load i16, ptr %3, !tbaa !2
  %242 = icmp ult i16 %239, 2
  %243 = sext i1 %242 to i8
  %244 = icmp ne i8 %243, 0
  br i1 %244, label %b42, label %b43

b40:
  %245 = load i16, ptr %3, !tbaa !2
  %246 = add i16 %245, 1
  store i16 %246, ptr %3, !tbaa !2
  br label %b38

b41:
  br label %b36

b42:
  %247 = icmp ult i16 %240, 3
  %248 = sext i1 %247 to i8
  %249 = icmp ne i8 %248, 0
  br i1 %249, label %b44, label %b45

b43:
  call addrspace(1) void @N$EBND()
  unreachable

b44:
  %250 = icmp ult i16 %241, 4
  %251 = sext i1 %250 to i8
  %252 = icmp ne i8 %251, 0
  br i1 %252, label %b46, label %b47

b45:
  call addrspace(1) void @N$EBND()
  unreachable

b46:
  %253 = sub i16 %239, 0
  %254 = sub i16 %240, 0
  %255 = mul i16 %253, 3
  %256 = add i16 %255, %254
  %257 = sub i16 %241, 0
  %258 = mul i16 %256, 4
  %259 = add i16 %258, %257
  %260 = getelementptr inbounds i8, ptr %35, i16 %259
  %261 = load i8, ptr %260, !tbaa !2
  %262 = zext i8 %261 to i16
  %263 = add i16 %238, %262
  store i16 %263, ptr %8, !tbaa !2
  br label %b40

b47:
  call addrspace(1) void @N$EBND()
  unreachable
}

declare void @N$EBND() addrspace(1)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$PI2(i16) addrspace(1)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PI4(i32) addrspace(1)

declare void @N$PU2(i16) addrspace(1)

declare void @N$PN() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
