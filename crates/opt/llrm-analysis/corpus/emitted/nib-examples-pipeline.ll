target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [14 x i8] c"\08\00\07\00\07\00closed \00"
@$str2 = internal constant [35 x i8] c"\08\00\1C\00\1C\0012 15 x 11 30 28 err 27 9 10\00"
@$str3 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str4 = internal constant [15 x i8] c"\08\00\08\00\08\00skipped \00"
@$str5 = internal constant [11 x i8] c"\08\00\04\00\04\00feed\00"
@$str6 = internal constant [12 x i8] c"\08\00\05\00\05\00mean \00"
@$str7 = internal constant [12 x i8] c"\08\00\05\00\05\00alarm\00"
@$str8 = internal constant [16 x i8] c"\08\00\09\00\09\00alarm at \00"
@$str9 = internal constant [11 x i8] c"\08\00\04\00\04\00done\00"

define internal void @Source.drop(ptr addrspace(1) %0) addrspace(1) {
b1:
  %1 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %1)
  %2 = load ptr, ptr addrspace(1) %0
  call addrspace(1) void @N$PS(ptr %2)
  call addrspace(1) void @N$PN()
  ret void
}

define internal i32 @number(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) {
b1:
  %1 = alloca i8
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca [4 x i8]
  store i8 0, ptr %1
  store i16 0, ptr %2
  store i16 0, ptr %3
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 4, i1 false)
  store i16 0, ptr %3, !tbaa !2
  %5 = load i16, ptr addrspace(1) %0
  store i16 0, ptr %2, !tbaa !2
  br label %b2

b2:
  %6 = load i16, ptr %2, !tbaa !2
  %7 = icmp ult i16 %6, %5
  %8 = sext i1 %7 to i8
  %9 = icmp ne i8 %8, 0
  br i1 %9, label %b3, label %b5

b3:
  %10 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %11 = load ptr addrspace(1), ptr addrspace(1) %10
  %12 = getelementptr i8, ptr addrspace(1) %11, i16 %6
  %13 = load i8, ptr addrspace(1) %12
  %14 = icmp ult i8 %13, 48
  %15 = sext i1 %14 to i8
  store i8 %15, ptr %1, !tbaa !2
  %16 = icmp ne i8 %15, 0
  br i1 %16, label %b7, label %b6

b4:
  %17 = load i16, ptr %2, !tbaa !2
  %18 = add i16 %17, 1
  store i16 %18, ptr %2, !tbaa !2
  br label %b2

b5:
  %19 = load i16, ptr %3, !tbaa !2
  store i8 0, ptr %4, !tbaa !2
  %20 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 %19, ptr %20, !tbaa !2
  %21 = addrspacecast ptr %4 to ptr addrspace(1)
  %22 = load i32, ptr addrspace(1) %21, !tbaa !2
  ret i32 %22

b6:
  %23 = load i8, ptr addrspace(1) %12
  %24 = icmp ugt i8 %23, 57
  %25 = sext i1 %24 to i8
  store i8 %25, ptr %1, !tbaa !2
  br label %b7

b7:
  %26 = load i8, ptr %1, !tbaa !2
  %27 = icmp ne i8 %26, 0
  br i1 %27, label %b8, label %b9

b8:
  store i8 1, ptr %4, !tbaa !2
  %28 = addrspacecast ptr %4 to ptr addrspace(1)
  %29 = load i32, ptr addrspace(1) %28, !tbaa !2
  ret i32 %29

b9:
  br label %b10

b10:
  %30 = load i16, ptr %3, !tbaa !2
  %31 = mul i16 %30, 10
  %32 = load i8, ptr addrspace(1) %12
  %33 = zext i8 %32 to i16
  %34 = add i16 %31, %33
  %35 = zext i8 48 to i16
  %36 = sub i16 %34, %35
  store i16 %36, ptr %3, !tbaa !2
  br label %b4
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca i16
  %1 = alloca [4 x i8]
  %2 = alloca i8
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca [54 x i8]
  %8 = alloca i8
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca [8 x i8]
  %12 = alloca [54 x i8]
  %13 = alloca i16
  %14 = alloca [4 x i8]
  %15 = alloca i8
  %16 = alloca i16
  %17 = alloca i16
  %18 = alloca i16
  %19 = alloca i16
  %20 = alloca [54 x i8]
  %21 = alloca i8
  %22 = alloca i16
  %23 = alloca i16
  %24 = alloca [8 x i8]
  %25 = alloca [54 x i8]
  %26 = alloca ptr
  store i16 0, ptr %0
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 4, i1 false)
  store i8 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %4
  store i16 0, ptr %5
  store i16 0, ptr %6
  call void @llvm.memset.p0.i16(ptr %7, i8 0, i16 54, i1 false)
  store i8 0, ptr %8
  store i16 0, ptr %9
  store i16 0, ptr %10
  call void @llvm.memset.p0.i16(ptr %11, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %12, i8 0, i16 54, i1 false)
  store i16 0, ptr %13
  call void @llvm.memset.p0.i16(ptr %14, i8 0, i16 4, i1 false)
  store i8 0, ptr %15
  store i16 0, ptr %16
  store i16 0, ptr %17
  store i16 0, ptr %18
  store i16 0, ptr %19
  call void @llvm.memset.p0.i16(ptr %20, i8 0, i16 54, i1 false)
  store i8 0, ptr %21
  store i16 0, ptr %22
  store i16 0, ptr %23
  call void @llvm.memset.p0.i16(ptr %24, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %25, i8 0, i16 54, i1 false)
  store ptr null, ptr %26
  %27 = getelementptr i8, ptr @$str2, i16 6
  store ptr %27, ptr %26, !tbaa !2
  %28 = getelementptr i8, ptr @$str5, i16 6
  %29 = load ptr, ptr %26, !tbaa !2
  %30 = getelementptr i8, ptr %29, i16 -4
  %31 = load i16, ptr %30
  %32 = addrspacecast ptr %29 to ptr addrspace(1)
  store i16 %31, ptr %24, !tbaa !2
  %33 = getelementptr inbounds i8, ptr %24, i16 2
  store i16 %31, ptr %33, !tbaa !2
  %34 = getelementptr inbounds i8, ptr %24, i16 4
  store ptr addrspace(1) %32, ptr %34, !tbaa !2
  %35 = addrspacecast ptr %24 to ptr addrspace(1)
  %36 = getelementptr inbounds i8, ptr %25, i16 8
  %37 = addrspacecast ptr %36 to ptr addrspace(1)
  %38 = load i16, ptr addrspace(1) %35, !tbaa !2
  store i16 %38, ptr addrspace(1) %37
  %39 = getelementptr i8, ptr addrspace(1) %35, i16 2
  %40 = load i16, ptr addrspace(1) %39, !tbaa !2
  %41 = getelementptr i8, ptr addrspace(1) %37, i16 2
  store i16 %40, ptr addrspace(1) %41
  %42 = getelementptr i8, ptr addrspace(1) %35, i16 4
  %43 = load ptr addrspace(1), ptr addrspace(1) %42, !tbaa !2
  %44 = getelementptr i8, ptr addrspace(1) %37, i16 4
  store ptr addrspace(1) %43, ptr addrspace(1) %44
  store i16 0, ptr %25, !tbaa !2
  %45 = getelementptr inbounds i8, ptr %25, i16 2
  store i16 0, ptr %45, !tbaa !2
  %46 = getelementptr inbounds i8, ptr %25, i16 4
  store i16 0, ptr %46, !tbaa !2
  %47 = getelementptr inbounds i8, ptr %25, i16 6
  store ptr %28, ptr %47, !tbaa !2
  %48 = getelementptr inbounds i8, ptr %25, i16 16
  store ptr null, ptr %48, !tbaa !2
  %49 = getelementptr inbounds i8, ptr %25, i16 18
  store i16 0, ptr %49, !tbaa !2
  %50 = getelementptr inbounds i8, ptr %25, i16 20
  store i16 0, ptr %50, !tbaa !2
  %51 = getelementptr inbounds i8, ptr %25, i16 22
  store i16 0, ptr %51, !tbaa !2
  %52 = getelementptr inbounds i8, ptr %25, i16 24
  store ptr addrspace(1) null, ptr %52, !tbaa !2
  %53 = getelementptr inbounds i8, ptr %25, i16 28
  store ptr null, ptr %53, !tbaa !2
  %54 = getelementptr inbounds i8, ptr %25, i16 30
  store i16 0, ptr %54, !tbaa !2
  %55 = getelementptr inbounds i8, ptr %25, i16 32
  store ptr null, ptr %55, !tbaa !2
  %56 = getelementptr inbounds i8, ptr %25, i16 34
  store i8 0, ptr %56, !tbaa !2
  %57 = getelementptr inbounds i8, ptr %25, i16 36
  store ptr null, ptr %57, !tbaa !2
  %58 = getelementptr inbounds i8, ptr %25, i16 38
  store i16 0, ptr %58, !tbaa !2
  %59 = getelementptr inbounds i8, ptr %25, i16 40
  store i8 -1, ptr %59, !tbaa !2
  %60 = getelementptr inbounds i8, ptr %25, i16 42
  %61 = addrspacecast ptr %60 to ptr addrspace(1)
  store i16 0, ptr %23, !tbaa !2
  store i16 3, ptr %22, !tbaa !2
  br label %b2

b2:
  %62 = load i16, ptr %23, !tbaa !2
  %63 = load i16, ptr %22, !tbaa !2
  %64 = icmp slt i16 %62, %63
  %65 = sext i1 %64 to i8
  %66 = icmp ne i8 %65, 0
  br i1 %66, label %b3, label %b5

b3:
  %67 = load i16, ptr %23, !tbaa !2
  %68 = mul i16 %67, 2
  %69 = getelementptr i8, ptr addrspace(1) %61, i16 %68
  store i16 0, ptr addrspace(1) %69
  br label %b4

b4:
  %70 = load i16, ptr %23, !tbaa !2
  %71 = add i16 %70, 1
  store i16 %71, ptr %23, !tbaa !2
  br label %b2

b5:
  %72 = getelementptr inbounds i8, ptr %25, i16 48
  store i16 0, ptr %72, !tbaa !2
  %73 = getelementptr inbounds i8, ptr %25, i16 50
  store i16 0, ptr %73, !tbaa !2
  %74 = getelementptr inbounds i8, ptr %25, i16 52
  store i8 -1, ptr %74, !tbaa !2
  store i8 -1, ptr %21, !tbaa !2
  %75 = load i16, ptr %25, !tbaa !2
  %76 = getelementptr inbounds i8, ptr %25, i16 2
  %77 = load i16, ptr %76, !tbaa !2
  %78 = getelementptr inbounds i8, ptr %25, i16 4
  %79 = load i16, ptr %78, !tbaa !2
  %80 = getelementptr inbounds i8, ptr %25, i16 6
  %81 = load ptr, ptr %80, !tbaa !2
  %82 = getelementptr inbounds i8, ptr %25, i16 8
  %83 = load i16, ptr %82, !tbaa !2
  %84 = getelementptr inbounds i8, ptr %25, i16 10
  %85 = load i16, ptr %84, !tbaa !2
  %86 = getelementptr inbounds i8, ptr %25, i16 12
  %87 = load ptr addrspace(1), ptr %86, !tbaa !2
  %88 = getelementptr inbounds i8, ptr %25, i16 16
  %89 = load ptr, ptr %88, !tbaa !2
  %90 = getelementptr inbounds i8, ptr %25, i16 18
  %91 = load i16, ptr %90, !tbaa !2
  %92 = getelementptr inbounds i8, ptr %25, i16 20
  %93 = load i16, ptr %92, !tbaa !2
  %94 = getelementptr inbounds i8, ptr %25, i16 22
  %95 = load i16, ptr %94, !tbaa !2
  %96 = getelementptr inbounds i8, ptr %25, i16 24
  %97 = load ptr addrspace(1), ptr %96, !tbaa !2
  %98 = getelementptr inbounds i8, ptr %25, i16 28
  %99 = load ptr, ptr %98, !tbaa !2
  %100 = getelementptr inbounds i8, ptr %25, i16 30
  %101 = load i16, ptr %100, !tbaa !2
  %102 = getelementptr inbounds i8, ptr %25, i16 32
  %103 = load ptr, ptr %102, !tbaa !2
  %104 = getelementptr inbounds i8, ptr %25, i16 34
  %105 = load i8, ptr %104, !tbaa !2
  %106 = getelementptr inbounds i8, ptr %25, i16 36
  %107 = load ptr, ptr %106, !tbaa !2
  %108 = getelementptr inbounds i8, ptr %25, i16 38
  %109 = load i16, ptr %108, !tbaa !2
  %110 = getelementptr inbounds i8, ptr %25, i16 40
  %111 = load i8, ptr %110, !tbaa !2
  %112 = getelementptr inbounds i8, ptr %25, i16 48
  %113 = load i16, ptr %112, !tbaa !2
  %114 = getelementptr inbounds i8, ptr %25, i16 50
  %115 = load i16, ptr %114, !tbaa !2
  %116 = getelementptr inbounds i8, ptr %25, i16 52
  %117 = load i8, ptr %116, !tbaa !2
  store i8 0, ptr %21, !tbaa !2
  store i16 %75, ptr %20, !tbaa !2
  %118 = getelementptr inbounds i8, ptr %20, i16 2
  store i16 %77, ptr %118, !tbaa !2
  %119 = getelementptr inbounds i8, ptr %20, i16 4
  store i16 %79, ptr %119, !tbaa !2
  %120 = getelementptr inbounds i8, ptr %20, i16 6
  store ptr %81, ptr %120, !tbaa !2
  %121 = getelementptr inbounds i8, ptr %20, i16 8
  store i16 %83, ptr %121, !tbaa !2
  %122 = getelementptr inbounds i8, ptr %20, i16 10
  store i16 %85, ptr %122, !tbaa !2
  %123 = getelementptr inbounds i8, ptr %20, i16 12
  store ptr addrspace(1) %87, ptr %123, !tbaa !2
  %124 = getelementptr inbounds i8, ptr %20, i16 16
  store ptr %89, ptr %124, !tbaa !2
  %125 = getelementptr inbounds i8, ptr %20, i16 18
  store i16 %91, ptr %125, !tbaa !2
  %126 = getelementptr inbounds i8, ptr %20, i16 20
  store i16 %93, ptr %126, !tbaa !2
  %127 = getelementptr inbounds i8, ptr %20, i16 22
  store i16 %95, ptr %127, !tbaa !2
  %128 = getelementptr inbounds i8, ptr %20, i16 24
  store ptr addrspace(1) %97, ptr %128, !tbaa !2
  %129 = getelementptr inbounds i8, ptr %20, i16 28
  store ptr %99, ptr %129, !tbaa !2
  %130 = getelementptr inbounds i8, ptr %20, i16 30
  store i16 %101, ptr %130, !tbaa !2
  %131 = getelementptr inbounds i8, ptr %20, i16 32
  store ptr %103, ptr %131, !tbaa !2
  %132 = getelementptr inbounds i8, ptr %20, i16 34
  store i8 %105, ptr %132, !tbaa !2
  %133 = getelementptr inbounds i8, ptr %20, i16 36
  store ptr %107, ptr %133, !tbaa !2
  %134 = getelementptr inbounds i8, ptr %20, i16 38
  store i16 %109, ptr %134, !tbaa !2
  %135 = getelementptr inbounds i8, ptr %20, i16 40
  store i8 %111, ptr %135, !tbaa !2
  %136 = getelementptr inbounds i8, ptr %20, i16 42
  %137 = addrspacecast ptr %136 to ptr addrspace(1)
  %138 = getelementptr inbounds i8, ptr %25, i16 42
  %139 = addrspacecast ptr %138 to ptr addrspace(1)
  store i16 0, ptr %19, !tbaa !2
  store i16 3, ptr %18, !tbaa !2
  br label %b6

b6:
  %140 = load i16, ptr %19, !tbaa !2
  %141 = load i16, ptr %18, !tbaa !2
  %142 = icmp slt i16 %140, %141
  %143 = sext i1 %142 to i8
  %144 = icmp ne i8 %143, 0
  br i1 %144, label %b7, label %b9

b7:
  %145 = load i16, ptr %19, !tbaa !2
  %146 = mul i16 %145, 2
  %147 = getelementptr i8, ptr addrspace(1) %137, i16 %146
  %148 = load i16, ptr %19, !tbaa !2
  %149 = mul i16 %148, 2
  %150 = getelementptr i8, ptr addrspace(1) %139, i16 %149
  %151 = load i16, ptr addrspace(1) %150
  store i16 %151, ptr addrspace(1) %147
  br label %b8

b8:
  %152 = load i16, ptr %19, !tbaa !2
  %153 = add i16 %152, 1
  store i16 %153, ptr %19, !tbaa !2
  br label %b6

b9:
  %154 = getelementptr inbounds i8, ptr %20, i16 48
  store i16 %113, ptr %154, !tbaa !2
  %155 = getelementptr inbounds i8, ptr %20, i16 50
  store i16 %115, ptr %155, !tbaa !2
  %156 = getelementptr inbounds i8, ptr %20, i16 52
  store i8 %117, ptr %156, !tbaa !2
  store i16 0, ptr %25, !tbaa !2
  %157 = getelementptr inbounds i8, ptr %25, i16 2
  store i16 0, ptr %157, !tbaa !2
  %158 = getelementptr inbounds i8, ptr %25, i16 4
  store i16 0, ptr %158, !tbaa !2
  %159 = getelementptr inbounds i8, ptr %25, i16 6
  store ptr null, ptr %159, !tbaa !2
  %160 = getelementptr inbounds i8, ptr %25, i16 8
  store i16 0, ptr %160, !tbaa !2
  %161 = getelementptr inbounds i8, ptr %25, i16 10
  store i16 0, ptr %161, !tbaa !2
  %162 = getelementptr inbounds i8, ptr %25, i16 12
  store ptr addrspace(1) null, ptr %162, !tbaa !2
  %163 = getelementptr inbounds i8, ptr %25, i16 16
  store ptr null, ptr %163, !tbaa !2
  %164 = getelementptr inbounds i8, ptr %25, i16 18
  store i16 0, ptr %164, !tbaa !2
  %165 = getelementptr inbounds i8, ptr %25, i16 20
  store i16 0, ptr %165, !tbaa !2
  %166 = getelementptr inbounds i8, ptr %25, i16 22
  store i16 0, ptr %166, !tbaa !2
  %167 = getelementptr inbounds i8, ptr %25, i16 24
  store ptr addrspace(1) null, ptr %167, !tbaa !2
  %168 = getelementptr inbounds i8, ptr %25, i16 28
  store ptr null, ptr %168, !tbaa !2
  %169 = getelementptr inbounds i8, ptr %25, i16 30
  store i16 0, ptr %169, !tbaa !2
  %170 = getelementptr inbounds i8, ptr %25, i16 32
  store ptr null, ptr %170, !tbaa !2
  %171 = getelementptr inbounds i8, ptr %25, i16 34
  store i8 0, ptr %171, !tbaa !2
  %172 = getelementptr inbounds i8, ptr %25, i16 36
  store ptr null, ptr %172, !tbaa !2
  %173 = getelementptr inbounds i8, ptr %25, i16 38
  store i16 0, ptr %173, !tbaa !2
  %174 = getelementptr inbounds i8, ptr %25, i16 40
  store i8 0, ptr %174, !tbaa !2
  %175 = getelementptr inbounds i8, ptr %25, i16 42
  %176 = addrspacecast ptr %175 to ptr addrspace(1)
  store i16 0, ptr %17, !tbaa !2
  store i16 3, ptr %16, !tbaa !2
  br label %b10

b10:
  %177 = load i16, ptr %17, !tbaa !2
  %178 = load i16, ptr %16, !tbaa !2
  %179 = icmp slt i16 %177, %178
  %180 = sext i1 %179 to i8
  %181 = icmp ne i8 %180, 0
  br i1 %181, label %b11, label %b13

b11:
  %182 = load i16, ptr %17, !tbaa !2
  %183 = mul i16 %182, 2
  %184 = getelementptr i8, ptr addrspace(1) %176, i16 %183
  store i16 0, ptr addrspace(1) %184
  br label %b12

b12:
  %185 = load i16, ptr %17, !tbaa !2
  %186 = add i16 %185, 1
  store i16 %186, ptr %17, !tbaa !2
  br label %b10

b13:
  %187 = getelementptr inbounds i8, ptr %25, i16 48
  store i16 0, ptr %187, !tbaa !2
  %188 = getelementptr inbounds i8, ptr %25, i16 50
  store i16 0, ptr %188, !tbaa !2
  %189 = getelementptr inbounds i8, ptr %25, i16 52
  store i8 0, ptr %189, !tbaa !2
  store i8 -1, ptr %15, !tbaa !2
  br label %b14

b14:
  br label %b15

b15:
  %190 = addrspacecast ptr %20 to ptr addrspace(1)
  %191 = call addrspace(1) i32 @$state3.next(ptr addrspace(1) %190)
  %192 = addrspacecast ptr %14 to ptr addrspace(1)
  store i32 %191, ptr addrspace(1) %192, !tbaa !2
  %193 = load i8, ptr %14, !tbaa !2
  %194 = icmp eq i8 %193, 0
  %195 = sext i1 %194 to i8
  %196 = icmp ne i8 %195, 0
  br i1 %196, label %b19, label %b18

b16:
  %197 = load i8, ptr %15, !tbaa !2
  %198 = icmp ne i8 %197, 0
  %199 = sext i1 %198 to i8
  %200 = icmp ne i8 %199, 0
  br i1 %200, label %b22, label %b21

b17:
  br label %b14

b18:
  br label %b16

b19:
  %201 = getelementptr inbounds i8, ptr %14, i16 2
  %202 = load i16, ptr %201, !tbaa !2
  %203 = getelementptr inbounds i8, ptr %14, i16 2
  %204 = load i16, ptr %203, !tbaa !2
  store i16 %204, ptr %13, !tbaa !2
  %205 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @N$PS(ptr %205)
  %206 = load i16, ptr %13, !tbaa !2
  call addrspace(1) void @N$PI2(i16 %206)
  call addrspace(1) void @N$PN()
  br label %b17

b21:
  %207 = getelementptr i8, ptr @$str7, i16 6
  %208 = load ptr, ptr %26, !tbaa !2
  %209 = getelementptr i8, ptr %208, i16 -4
  %210 = load i16, ptr %209
  %211 = addrspacecast ptr %208 to ptr addrspace(1)
  store i16 %210, ptr %11, !tbaa !2
  %212 = getelementptr inbounds i8, ptr %11, i16 2
  store i16 %210, ptr %212, !tbaa !2
  %213 = getelementptr inbounds i8, ptr %11, i16 4
  store ptr addrspace(1) %211, ptr %213, !tbaa !2
  %214 = addrspacecast ptr %11 to ptr addrspace(1)
  %215 = getelementptr inbounds i8, ptr %12, i16 8
  %216 = addrspacecast ptr %215 to ptr addrspace(1)
  %217 = load i16, ptr addrspace(1) %214, !tbaa !2
  store i16 %217, ptr addrspace(1) %216
  %218 = getelementptr i8, ptr addrspace(1) %214, i16 2
  %219 = load i16, ptr addrspace(1) %218, !tbaa !2
  %220 = getelementptr i8, ptr addrspace(1) %216, i16 2
  store i16 %219, ptr addrspace(1) %220
  %221 = getelementptr i8, ptr addrspace(1) %214, i16 4
  %222 = load ptr addrspace(1), ptr addrspace(1) %221, !tbaa !2
  %223 = getelementptr i8, ptr addrspace(1) %216, i16 4
  store ptr addrspace(1) %222, ptr addrspace(1) %223
  store i16 0, ptr %12, !tbaa !2
  %224 = getelementptr inbounds i8, ptr %12, i16 2
  store i16 0, ptr %224, !tbaa !2
  %225 = getelementptr inbounds i8, ptr %12, i16 4
  store i16 0, ptr %225, !tbaa !2
  %226 = getelementptr inbounds i8, ptr %12, i16 6
  store ptr %207, ptr %226, !tbaa !2
  %227 = getelementptr inbounds i8, ptr %12, i16 16
  store ptr null, ptr %227, !tbaa !2
  %228 = getelementptr inbounds i8, ptr %12, i16 18
  store i16 0, ptr %228, !tbaa !2
  %229 = getelementptr inbounds i8, ptr %12, i16 20
  store i16 0, ptr %229, !tbaa !2
  %230 = getelementptr inbounds i8, ptr %12, i16 22
  store i16 0, ptr %230, !tbaa !2
  %231 = getelementptr inbounds i8, ptr %12, i16 24
  store ptr addrspace(1) null, ptr %231, !tbaa !2
  %232 = getelementptr inbounds i8, ptr %12, i16 28
  store ptr null, ptr %232, !tbaa !2
  %233 = getelementptr inbounds i8, ptr %12, i16 30
  store i16 0, ptr %233, !tbaa !2
  %234 = getelementptr inbounds i8, ptr %12, i16 32
  store ptr null, ptr %234, !tbaa !2
  %235 = getelementptr inbounds i8, ptr %12, i16 34
  store i8 0, ptr %235, !tbaa !2
  %236 = getelementptr inbounds i8, ptr %12, i16 36
  store ptr null, ptr %236, !tbaa !2
  %237 = getelementptr inbounds i8, ptr %12, i16 38
  store i16 0, ptr %237, !tbaa !2
  %238 = getelementptr inbounds i8, ptr %12, i16 40
  store i8 -1, ptr %238, !tbaa !2
  %239 = getelementptr inbounds i8, ptr %12, i16 42
  %240 = addrspacecast ptr %239 to ptr addrspace(1)
  store i16 0, ptr %10, !tbaa !2
  store i16 3, ptr %9, !tbaa !2
  br label %b29

b22:
  %241 = getelementptr inbounds i8, ptr %20, i16 52
  %242 = load i8, ptr %241, !tbaa !2
  %243 = icmp ne i8 %242, 0
  %244 = sext i1 %243 to i8
  %245 = icmp ne i8 %244, 0
  br i1 %245, label %b24, label %b23

b23:
  br label %b21

b24:
  %246 = getelementptr inbounds i8, ptr %20, i16 36
  %247 = load ptr, ptr %246, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %247)
  %248 = getelementptr inbounds i8, ptr %20, i16 40
  %249 = load i8, ptr %248, !tbaa !2
  %250 = icmp ne i8 %249, 0
  %251 = sext i1 %250 to i8
  %252 = icmp ne i8 %251, 0
  br i1 %252, label %b26, label %b25

b25:
  br label %b23

b26:
  %253 = getelementptr inbounds i8, ptr %20, i16 6
  %254 = load ptr, ptr %253, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %254)
  %255 = getelementptr inbounds i8, ptr %20, i16 34
  %256 = load i8, ptr %255, !tbaa !2
  %257 = icmp ne i8 %256, 0
  %258 = sext i1 %257 to i8
  %259 = icmp ne i8 %258, 0
  br i1 %259, label %b28, label %b27

b27:
  %260 = getelementptr inbounds i8, ptr %20, i16 28
  %261 = load ptr, ptr %260, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %261)
  %262 = getelementptr inbounds i8, ptr %20, i16 32
  %263 = load ptr, ptr %262, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %263)
  br label %b25

b28:
  %264 = getelementptr inbounds i8, ptr %20, i16 16
  %265 = addrspacecast ptr %264 to ptr addrspace(1)
  call addrspace(1) void @Source.drop(ptr addrspace(1) %265)
  %266 = getelementptr inbounds i8, ptr %20, i16 16
  %267 = load ptr, ptr %266, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %267)
  br label %b27

b29:
  %268 = load i16, ptr %10, !tbaa !2
  %269 = load i16, ptr %9, !tbaa !2
  %270 = icmp slt i16 %268, %269
  %271 = sext i1 %270 to i8
  %272 = icmp ne i8 %271, 0
  br i1 %272, label %b30, label %b32

b30:
  %273 = load i16, ptr %10, !tbaa !2
  %274 = mul i16 %273, 2
  %275 = getelementptr i8, ptr addrspace(1) %240, i16 %274
  store i16 0, ptr addrspace(1) %275
  br label %b31

b31:
  %276 = load i16, ptr %10, !tbaa !2
  %277 = add i16 %276, 1
  store i16 %277, ptr %10, !tbaa !2
  br label %b29

b32:
  %278 = getelementptr inbounds i8, ptr %12, i16 48
  store i16 0, ptr %278, !tbaa !2
  %279 = getelementptr inbounds i8, ptr %12, i16 50
  store i16 0, ptr %279, !tbaa !2
  %280 = getelementptr inbounds i8, ptr %12, i16 52
  store i8 -1, ptr %280, !tbaa !2
  store i8 -1, ptr %8, !tbaa !2
  %281 = load i16, ptr %12, !tbaa !2
  %282 = getelementptr inbounds i8, ptr %12, i16 2
  %283 = load i16, ptr %282, !tbaa !2
  %284 = getelementptr inbounds i8, ptr %12, i16 4
  %285 = load i16, ptr %284, !tbaa !2
  %286 = getelementptr inbounds i8, ptr %12, i16 6
  %287 = load ptr, ptr %286, !tbaa !2
  %288 = getelementptr inbounds i8, ptr %12, i16 8
  %289 = load i16, ptr %288, !tbaa !2
  %290 = getelementptr inbounds i8, ptr %12, i16 10
  %291 = load i16, ptr %290, !tbaa !2
  %292 = getelementptr inbounds i8, ptr %12, i16 12
  %293 = load ptr addrspace(1), ptr %292, !tbaa !2
  %294 = getelementptr inbounds i8, ptr %12, i16 16
  %295 = load ptr, ptr %294, !tbaa !2
  %296 = getelementptr inbounds i8, ptr %12, i16 18
  %297 = load i16, ptr %296, !tbaa !2
  %298 = getelementptr inbounds i8, ptr %12, i16 20
  %299 = load i16, ptr %298, !tbaa !2
  %300 = getelementptr inbounds i8, ptr %12, i16 22
  %301 = load i16, ptr %300, !tbaa !2
  %302 = getelementptr inbounds i8, ptr %12, i16 24
  %303 = load ptr addrspace(1), ptr %302, !tbaa !2
  %304 = getelementptr inbounds i8, ptr %12, i16 28
  %305 = load ptr, ptr %304, !tbaa !2
  %306 = getelementptr inbounds i8, ptr %12, i16 30
  %307 = load i16, ptr %306, !tbaa !2
  %308 = getelementptr inbounds i8, ptr %12, i16 32
  %309 = load ptr, ptr %308, !tbaa !2
  %310 = getelementptr inbounds i8, ptr %12, i16 34
  %311 = load i8, ptr %310, !tbaa !2
  %312 = getelementptr inbounds i8, ptr %12, i16 36
  %313 = load ptr, ptr %312, !tbaa !2
  %314 = getelementptr inbounds i8, ptr %12, i16 38
  %315 = load i16, ptr %314, !tbaa !2
  %316 = getelementptr inbounds i8, ptr %12, i16 40
  %317 = load i8, ptr %316, !tbaa !2
  %318 = getelementptr inbounds i8, ptr %12, i16 48
  %319 = load i16, ptr %318, !tbaa !2
  %320 = getelementptr inbounds i8, ptr %12, i16 50
  %321 = load i16, ptr %320, !tbaa !2
  %322 = getelementptr inbounds i8, ptr %12, i16 52
  %323 = load i8, ptr %322, !tbaa !2
  store i8 0, ptr %8, !tbaa !2
  store i16 %281, ptr %7, !tbaa !2
  %324 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 %283, ptr %324, !tbaa !2
  %325 = getelementptr inbounds i8, ptr %7, i16 4
  store i16 %285, ptr %325, !tbaa !2
  %326 = getelementptr inbounds i8, ptr %7, i16 6
  store ptr %287, ptr %326, !tbaa !2
  %327 = getelementptr inbounds i8, ptr %7, i16 8
  store i16 %289, ptr %327, !tbaa !2
  %328 = getelementptr inbounds i8, ptr %7, i16 10
  store i16 %291, ptr %328, !tbaa !2
  %329 = getelementptr inbounds i8, ptr %7, i16 12
  store ptr addrspace(1) %293, ptr %329, !tbaa !2
  %330 = getelementptr inbounds i8, ptr %7, i16 16
  store ptr %295, ptr %330, !tbaa !2
  %331 = getelementptr inbounds i8, ptr %7, i16 18
  store i16 %297, ptr %331, !tbaa !2
  %332 = getelementptr inbounds i8, ptr %7, i16 20
  store i16 %299, ptr %332, !tbaa !2
  %333 = getelementptr inbounds i8, ptr %7, i16 22
  store i16 %301, ptr %333, !tbaa !2
  %334 = getelementptr inbounds i8, ptr %7, i16 24
  store ptr addrspace(1) %303, ptr %334, !tbaa !2
  %335 = getelementptr inbounds i8, ptr %7, i16 28
  store ptr %305, ptr %335, !tbaa !2
  %336 = getelementptr inbounds i8, ptr %7, i16 30
  store i16 %307, ptr %336, !tbaa !2
  %337 = getelementptr inbounds i8, ptr %7, i16 32
  store ptr %309, ptr %337, !tbaa !2
  %338 = getelementptr inbounds i8, ptr %7, i16 34
  store i8 %311, ptr %338, !tbaa !2
  %339 = getelementptr inbounds i8, ptr %7, i16 36
  store ptr %313, ptr %339, !tbaa !2
  %340 = getelementptr inbounds i8, ptr %7, i16 38
  store i16 %315, ptr %340, !tbaa !2
  %341 = getelementptr inbounds i8, ptr %7, i16 40
  store i8 %317, ptr %341, !tbaa !2
  %342 = getelementptr inbounds i8, ptr %7, i16 42
  %343 = addrspacecast ptr %342 to ptr addrspace(1)
  %344 = getelementptr inbounds i8, ptr %12, i16 42
  %345 = addrspacecast ptr %344 to ptr addrspace(1)
  store i16 0, ptr %6, !tbaa !2
  store i16 3, ptr %5, !tbaa !2
  br label %b33

b33:
  %346 = load i16, ptr %6, !tbaa !2
  %347 = load i16, ptr %5, !tbaa !2
  %348 = icmp slt i16 %346, %347
  %349 = sext i1 %348 to i8
  %350 = icmp ne i8 %349, 0
  br i1 %350, label %b34, label %b36

b34:
  %351 = load i16, ptr %6, !tbaa !2
  %352 = mul i16 %351, 2
  %353 = getelementptr i8, ptr addrspace(1) %343, i16 %352
  %354 = load i16, ptr %6, !tbaa !2
  %355 = mul i16 %354, 2
  %356 = getelementptr i8, ptr addrspace(1) %345, i16 %355
  %357 = load i16, ptr addrspace(1) %356
  store i16 %357, ptr addrspace(1) %353
  br label %b35

b35:
  %358 = load i16, ptr %6, !tbaa !2
  %359 = add i16 %358, 1
  store i16 %359, ptr %6, !tbaa !2
  br label %b33

b36:
  %360 = getelementptr inbounds i8, ptr %7, i16 48
  store i16 %319, ptr %360, !tbaa !2
  %361 = getelementptr inbounds i8, ptr %7, i16 50
  store i16 %321, ptr %361, !tbaa !2
  %362 = getelementptr inbounds i8, ptr %7, i16 52
  store i8 %323, ptr %362, !tbaa !2
  store i16 0, ptr %12, !tbaa !2
  %363 = getelementptr inbounds i8, ptr %12, i16 2
  store i16 0, ptr %363, !tbaa !2
  %364 = getelementptr inbounds i8, ptr %12, i16 4
  store i16 0, ptr %364, !tbaa !2
  %365 = getelementptr inbounds i8, ptr %12, i16 6
  store ptr null, ptr %365, !tbaa !2
  %366 = getelementptr inbounds i8, ptr %12, i16 8
  store i16 0, ptr %366, !tbaa !2
  %367 = getelementptr inbounds i8, ptr %12, i16 10
  store i16 0, ptr %367, !tbaa !2
  %368 = getelementptr inbounds i8, ptr %12, i16 12
  store ptr addrspace(1) null, ptr %368, !tbaa !2
  %369 = getelementptr inbounds i8, ptr %12, i16 16
  store ptr null, ptr %369, !tbaa !2
  %370 = getelementptr inbounds i8, ptr %12, i16 18
  store i16 0, ptr %370, !tbaa !2
  %371 = getelementptr inbounds i8, ptr %12, i16 20
  store i16 0, ptr %371, !tbaa !2
  %372 = getelementptr inbounds i8, ptr %12, i16 22
  store i16 0, ptr %372, !tbaa !2
  %373 = getelementptr inbounds i8, ptr %12, i16 24
  store ptr addrspace(1) null, ptr %373, !tbaa !2
  %374 = getelementptr inbounds i8, ptr %12, i16 28
  store ptr null, ptr %374, !tbaa !2
  %375 = getelementptr inbounds i8, ptr %12, i16 30
  store i16 0, ptr %375, !tbaa !2
  %376 = getelementptr inbounds i8, ptr %12, i16 32
  store ptr null, ptr %376, !tbaa !2
  %377 = getelementptr inbounds i8, ptr %12, i16 34
  store i8 0, ptr %377, !tbaa !2
  %378 = getelementptr inbounds i8, ptr %12, i16 36
  store ptr null, ptr %378, !tbaa !2
  %379 = getelementptr inbounds i8, ptr %12, i16 38
  store i16 0, ptr %379, !tbaa !2
  %380 = getelementptr inbounds i8, ptr %12, i16 40
  store i8 0, ptr %380, !tbaa !2
  %381 = getelementptr inbounds i8, ptr %12, i16 42
  %382 = addrspacecast ptr %381 to ptr addrspace(1)
  store i16 0, ptr %4, !tbaa !2
  store i16 3, ptr %3, !tbaa !2
  br label %b37

b37:
  %383 = load i16, ptr %4, !tbaa !2
  %384 = load i16, ptr %3, !tbaa !2
  %385 = icmp slt i16 %383, %384
  %386 = sext i1 %385 to i8
  %387 = icmp ne i8 %386, 0
  br i1 %387, label %b38, label %b40

b38:
  %388 = load i16, ptr %4, !tbaa !2
  %389 = mul i16 %388, 2
  %390 = getelementptr i8, ptr addrspace(1) %382, i16 %389
  store i16 0, ptr addrspace(1) %390
  br label %b39

b39:
  %391 = load i16, ptr %4, !tbaa !2
  %392 = add i16 %391, 1
  store i16 %392, ptr %4, !tbaa !2
  br label %b37

b40:
  %393 = getelementptr inbounds i8, ptr %12, i16 48
  store i16 0, ptr %393, !tbaa !2
  %394 = getelementptr inbounds i8, ptr %12, i16 50
  store i16 0, ptr %394, !tbaa !2
  %395 = getelementptr inbounds i8, ptr %12, i16 52
  store i8 0, ptr %395, !tbaa !2
  store i8 -1, ptr %2, !tbaa !2
  br label %b41

b41:
  br label %b42

b42:
  %396 = addrspacecast ptr %7 to ptr addrspace(1)
  %397 = call addrspace(1) i32 @$state3.next(ptr addrspace(1) %396)
  %398 = addrspacecast ptr %1 to ptr addrspace(1)
  store i32 %397, ptr addrspace(1) %398, !tbaa !2
  %399 = load i8, ptr %1, !tbaa !2
  %400 = icmp eq i8 %399, 0
  %401 = sext i1 %400 to i8
  %402 = icmp ne i8 %401, 0
  br i1 %402, label %b46, label %b45

b43:
  %403 = load i8, ptr %2, !tbaa !2
  %404 = icmp ne i8 %403, 0
  %405 = sext i1 %404 to i8
  %406 = icmp ne i8 %405, 0
  br i1 %406, label %b52, label %b51

b44:
  br label %b41

b45:
  br label %b43

b46:
  %407 = getelementptr inbounds i8, ptr %1, i16 2
  %408 = load i16, ptr %407, !tbaa !2
  %409 = getelementptr inbounds i8, ptr %1, i16 2
  %410 = load i16, ptr %409, !tbaa !2
  store i16 %410, ptr %0, !tbaa !2
  %411 = load i16, ptr %0, !tbaa !2
  %412 = icmp sgt i16 %411, 20
  %413 = sext i1 %412 to i8
  %414 = icmp ne i8 %413, 0
  br i1 %414, label %b47, label %b48

b47:
  %415 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @N$PS(ptr %415)
  %416 = load i16, ptr %0, !tbaa !2
  call addrspace(1) void @N$PI2(i16 %416)
  call addrspace(1) void @N$PN()
  br label %b43

b48:
  br label %b49

b49:
  br label %b44

b51:
  %417 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %417)
  call addrspace(1) void @N$PN()
  %418 = load i8, ptr %8, !tbaa !2
  %419 = icmp ne i8 %418, 0
  %420 = sext i1 %419 to i8
  %421 = icmp ne i8 %420, 0
  br i1 %421, label %b60, label %b59

b52:
  %422 = getelementptr inbounds i8, ptr %7, i16 52
  %423 = load i8, ptr %422, !tbaa !2
  %424 = icmp ne i8 %423, 0
  %425 = sext i1 %424 to i8
  %426 = icmp ne i8 %425, 0
  br i1 %426, label %b54, label %b53

b53:
  br label %b51

b54:
  %427 = getelementptr inbounds i8, ptr %7, i16 36
  %428 = load ptr, ptr %427, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %428)
  %429 = getelementptr inbounds i8, ptr %7, i16 40
  %430 = load i8, ptr %429, !tbaa !2
  %431 = icmp ne i8 %430, 0
  %432 = sext i1 %431 to i8
  %433 = icmp ne i8 %432, 0
  br i1 %433, label %b56, label %b55

b55:
  br label %b53

b56:
  %434 = getelementptr inbounds i8, ptr %7, i16 6
  %435 = load ptr, ptr %434, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %435)
  %436 = getelementptr inbounds i8, ptr %7, i16 34
  %437 = load i8, ptr %436, !tbaa !2
  %438 = icmp ne i8 %437, 0
  %439 = sext i1 %438 to i8
  %440 = icmp ne i8 %439, 0
  br i1 %440, label %b58, label %b57

b57:
  %441 = getelementptr inbounds i8, ptr %7, i16 28
  %442 = load ptr, ptr %441, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %442)
  %443 = getelementptr inbounds i8, ptr %7, i16 32
  %444 = load ptr, ptr %443, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %444)
  br label %b55

b58:
  %445 = getelementptr inbounds i8, ptr %7, i16 16
  %446 = addrspacecast ptr %445 to ptr addrspace(1)
  call addrspace(1) void @Source.drop(ptr addrspace(1) %446)
  %447 = getelementptr inbounds i8, ptr %7, i16 16
  %448 = load ptr, ptr %447, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %448)
  br label %b57

b59:
  %449 = load i8, ptr %21, !tbaa !2
  %450 = icmp ne i8 %449, 0
  %451 = sext i1 %450 to i8
  %452 = icmp ne i8 %451, 0
  br i1 %452, label %b68, label %b67

b60:
  %453 = getelementptr inbounds i8, ptr %12, i16 52
  %454 = load i8, ptr %453, !tbaa !2
  %455 = icmp ne i8 %454, 0
  %456 = sext i1 %455 to i8
  %457 = icmp ne i8 %456, 0
  br i1 %457, label %b62, label %b61

b61:
  br label %b59

b62:
  %458 = getelementptr inbounds i8, ptr %12, i16 36
  %459 = load ptr, ptr %458, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %459)
  %460 = getelementptr inbounds i8, ptr %12, i16 40
  %461 = load i8, ptr %460, !tbaa !2
  %462 = icmp ne i8 %461, 0
  %463 = sext i1 %462 to i8
  %464 = icmp ne i8 %463, 0
  br i1 %464, label %b64, label %b63

b63:
  br label %b61

b64:
  %465 = getelementptr inbounds i8, ptr %12, i16 6
  %466 = load ptr, ptr %465, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %466)
  %467 = getelementptr inbounds i8, ptr %12, i16 34
  %468 = load i8, ptr %467, !tbaa !2
  %469 = icmp ne i8 %468, 0
  %470 = sext i1 %469 to i8
  %471 = icmp ne i8 %470, 0
  br i1 %471, label %b66, label %b65

b65:
  %472 = getelementptr inbounds i8, ptr %12, i16 28
  %473 = load ptr, ptr %472, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %473)
  %474 = getelementptr inbounds i8, ptr %12, i16 32
  %475 = load ptr, ptr %474, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %475)
  br label %b63

b66:
  %476 = getelementptr inbounds i8, ptr %12, i16 16
  %477 = addrspacecast ptr %476 to ptr addrspace(1)
  call addrspace(1) void @Source.drop(ptr addrspace(1) %477)
  %478 = getelementptr inbounds i8, ptr %12, i16 16
  %479 = load ptr, ptr %478, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %479)
  br label %b65

b67:
  %480 = load ptr, ptr %26, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %480)
  ret i16 0

b68:
  %481 = getelementptr inbounds i8, ptr %25, i16 52
  %482 = load i8, ptr %481, !tbaa !2
  %483 = icmp ne i8 %482, 0
  %484 = sext i1 %483 to i8
  %485 = icmp ne i8 %484, 0
  br i1 %485, label %b70, label %b69

b69:
  br label %b67

b70:
  %486 = getelementptr inbounds i8, ptr %25, i16 36
  %487 = load ptr, ptr %486, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %487)
  %488 = getelementptr inbounds i8, ptr %25, i16 40
  %489 = load i8, ptr %488, !tbaa !2
  %490 = icmp ne i8 %489, 0
  %491 = sext i1 %490 to i8
  %492 = icmp ne i8 %491, 0
  br i1 %492, label %b72, label %b71

b71:
  br label %b69

b72:
  %493 = getelementptr inbounds i8, ptr %25, i16 6
  %494 = load ptr, ptr %493, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %494)
  %495 = getelementptr inbounds i8, ptr %25, i16 34
  %496 = load i8, ptr %495, !tbaa !2
  %497 = icmp ne i8 %496, 0
  %498 = sext i1 %497 to i8
  %499 = icmp ne i8 %498, 0
  br i1 %499, label %b74, label %b73

b73:
  %500 = getelementptr inbounds i8, ptr %25, i16 28
  %501 = load ptr, ptr %500, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %501)
  %502 = getelementptr inbounds i8, ptr %25, i16 32
  %503 = load ptr, ptr %502, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %503)
  br label %b71

b74:
  %504 = getelementptr inbounds i8, ptr %25, i16 16
  %505 = addrspacecast ptr %504 to ptr addrspace(1)
  call addrspace(1) void @Source.drop(ptr addrspace(1) %505)
  %506 = getelementptr inbounds i8, ptr %25, i16 16
  %507 = load ptr, ptr %506, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %507)
  br label %b73
}

define internal i32 @$state3.next(ptr addrspace(1) %0) addrspace(1) {
b1:
  %1 = alloca i16
  %2 = alloca [4 x i8]
  %3 = alloca [4 x i8]
  store i16 0, ptr %1
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 4, i1 false)
  br label %b2

b2:
  br label %b3

b3:
  %4 = load i16, ptr addrspace(1) %0
  %5 = icmp eq i16 %4, 0
  %6 = sext i1 %5 to i8
  %7 = icmp ne i8 %6, 0
  br i1 %7, label %b7, label %b6

b6:
  %8 = icmp eq i16 %4, 2
  %9 = sext i1 %8 to i8
  %10 = icmp ne i8 %9, 0
  br i1 %10, label %b9, label %b8

b7:
  %11 = getelementptr i8, ptr addrspace(1) %0, i16 42
  store i16 0, ptr addrspace(1) %11
  %12 = getelementptr i8, ptr addrspace(1) %0, i16 44
  store i16 0, ptr addrspace(1) %12
  %13 = getelementptr i8, ptr addrspace(1) %0, i16 46
  store i16 0, ptr addrspace(1) %13
  %14 = getelementptr i8, ptr addrspace(1) %0, i16 48
  store i16 0, ptr addrspace(1) %14
  store i16 2, ptr addrspace(1) %0
  br label %b2

b8:
  %15 = icmp eq i16 %4, 3
  %16 = sext i1 %15 to i8
  %17 = icmp ne i8 %16, 0
  br i1 %17, label %b15, label %b14

b9:
  %18 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %19 = call addrspace(1) i32 @$state2.next(ptr addrspace(1) %18)
  %20 = addrspacecast ptr %2 to ptr addrspace(1)
  store i32 %19, ptr addrspace(1) %20, !tbaa !2
  %21 = load i8, ptr %2, !tbaa !2
  %22 = icmp eq i8 %21, 0
  %23 = sext i1 %22 to i8
  %24 = icmp ne i8 %23, 0
  br i1 %24, label %b12, label %b11

b11:
  store i16 4, ptr addrspace(1) %0
  br label %b2

b12:
  %25 = getelementptr inbounds i8, ptr %2, i16 2
  %26 = load i16, ptr %25, !tbaa !2
  %27 = getelementptr inbounds i8, ptr %2, i16 2
  %28 = load i16, ptr %27, !tbaa !2
  store i16 %28, ptr %1, !tbaa !2
  %29 = load i16, ptr %1, !tbaa !2
  %30 = getelementptr i8, ptr addrspace(1) %0, i16 50
  store i16 %29, ptr addrspace(1) %30
  store i16 3, ptr addrspace(1) %0
  br label %b2

b14:
  %31 = icmp eq i16 %4, 4
  %32 = sext i1 %31 to i8
  %33 = icmp ne i8 %32, 0
  br i1 %33, label %b22, label %b21

b15:
  %34 = getelementptr i8, ptr addrspace(1) %0, i16 42
  %35 = getelementptr i8, ptr addrspace(1) %0, i16 48
  %36 = load i16, ptr addrspace(1) %35
  %37 = urem i16 %36, 3
  %38 = icmp ult i16 %37, 3
  %39 = sext i1 %38 to i8
  %40 = icmp ne i8 %39, 0
  br i1 %40, label %b16, label %b17

b16:
  %41 = mul i16 %37, 2
  %42 = getelementptr i8, ptr addrspace(1) %34, i16 %41
  %43 = getelementptr i8, ptr addrspace(1) %0, i16 50
  %44 = load i16, ptr addrspace(1) %43
  store i16 %44, ptr addrspace(1) %42
  %45 = getelementptr i8, ptr addrspace(1) %0, i16 48
  %46 = load i16, ptr addrspace(1) %45
  %47 = add i16 %46, 1
  %48 = getelementptr i8, ptr addrspace(1) %0, i16 48
  store i16 %47, ptr addrspace(1) %48
  %49 = getelementptr i8, ptr addrspace(1) %0, i16 48
  %50 = load i16, ptr addrspace(1) %49
  %51 = icmp uge i16 %50, 3
  %52 = sext i1 %51 to i8
  %53 = icmp ne i8 %52, 0
  br i1 %53, label %b18, label %b19

b17:
  call addrspace(1) void @N$EBND()
  unreachable

b18:
  store i16 5, ptr addrspace(1) %0
  br label %b2

b19:
  store i16 6, ptr addrspace(1) %0
  br label %b2

b21:
  %54 = icmp eq i16 %4, 5
  %55 = sext i1 %54 to i8
  %56 = icmp ne i8 %55, 0
  br i1 %56, label %b30, label %b29

b22:
  %57 = getelementptr i8, ptr addrspace(1) %0, i16 52
  %58 = load i8, ptr addrspace(1) %57
  %59 = icmp ne i8 %58, 0
  %60 = sext i1 %59 to i8
  %61 = icmp ne i8 %60, 0
  br i1 %61, label %b24, label %b23

b23:
  %62 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 0, ptr addrspace(1) %62
  %63 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 0, ptr addrspace(1) %63
  %64 = getelementptr i8, ptr addrspace(1) %0, i16 6
  store ptr null, ptr addrspace(1) %64
  %65 = getelementptr i8, ptr addrspace(1) %0, i16 8
  store i16 0, ptr addrspace(1) %65
  %66 = getelementptr i8, ptr addrspace(1) %0, i16 10
  store i16 0, ptr addrspace(1) %66
  %67 = getelementptr i8, ptr addrspace(1) %0, i16 12
  store ptr addrspace(1) null, ptr addrspace(1) %67
  %68 = getelementptr i8, ptr addrspace(1) %0, i16 16
  store ptr null, ptr addrspace(1) %68
  %69 = getelementptr i8, ptr addrspace(1) %0, i16 18
  store i16 0, ptr addrspace(1) %69
  %70 = getelementptr i8, ptr addrspace(1) %0, i16 20
  store i16 0, ptr addrspace(1) %70
  %71 = getelementptr i8, ptr addrspace(1) %0, i16 22
  store i16 0, ptr addrspace(1) %71
  %72 = getelementptr i8, ptr addrspace(1) %0, i16 24
  store ptr addrspace(1) null, ptr addrspace(1) %72
  %73 = getelementptr i8, ptr addrspace(1) %0, i16 28
  store ptr null, ptr addrspace(1) %73
  %74 = getelementptr i8, ptr addrspace(1) %0, i16 30
  store i16 0, ptr addrspace(1) %74
  %75 = getelementptr i8, ptr addrspace(1) %0, i16 32
  store ptr null, ptr addrspace(1) %75
  %76 = getelementptr i8, ptr addrspace(1) %0, i16 34
  store i8 0, ptr addrspace(1) %76
  %77 = getelementptr i8, ptr addrspace(1) %0, i16 36
  store ptr null, ptr addrspace(1) %77
  %78 = getelementptr i8, ptr addrspace(1) %0, i16 38
  store i16 0, ptr addrspace(1) %78
  %79 = getelementptr i8, ptr addrspace(1) %0, i16 40
  store i8 0, ptr addrspace(1) %79
  %80 = getelementptr i8, ptr addrspace(1) %0, i16 52
  store i8 0, ptr addrspace(1) %80
  store i16 1, ptr addrspace(1) %0
  br label %b2

b24:
  %81 = getelementptr i8, ptr addrspace(1) %0, i16 36
  %82 = load ptr, ptr addrspace(1) %81
  call addrspace(1) void @N$BDRP(ptr %82)
  %83 = getelementptr i8, ptr addrspace(1) %0, i16 40
  %84 = load i8, ptr addrspace(1) %83
  %85 = icmp ne i8 %84, 0
  %86 = sext i1 %85 to i8
  %87 = icmp ne i8 %86, 0
  br i1 %87, label %b26, label %b25

b25:
  br label %b23

b26:
  %88 = getelementptr i8, ptr addrspace(1) %0, i16 6
  %89 = load ptr, ptr addrspace(1) %88
  call addrspace(1) void @N$BDRP(ptr %89)
  %90 = getelementptr i8, ptr addrspace(1) %0, i16 34
  %91 = load i8, ptr addrspace(1) %90
  %92 = icmp ne i8 %91, 0
  %93 = sext i1 %92 to i8
  %94 = icmp ne i8 %93, 0
  br i1 %94, label %b28, label %b27

b27:
  %95 = getelementptr i8, ptr addrspace(1) %0, i16 28
  %96 = load ptr, ptr addrspace(1) %95
  call addrspace(1) void @N$BDRP(ptr %96)
  %97 = getelementptr i8, ptr addrspace(1) %0, i16 32
  %98 = load ptr, ptr addrspace(1) %97
  call addrspace(1) void @N$BDRP(ptr %98)
  br label %b25

b28:
  %99 = getelementptr i8, ptr addrspace(1) %0, i16 16
  call addrspace(1) void @Source.drop(ptr addrspace(1) %99)
  %100 = getelementptr i8, ptr addrspace(1) %0, i16 16
  %101 = load ptr, ptr addrspace(1) %100
  call addrspace(1) void @N$BDRP(ptr %101)
  br label %b27

b29:
  %102 = icmp eq i16 %4, 6
  %103 = sext i1 %102 to i8
  %104 = icmp ne i8 %103, 0
  br i1 %104, label %b32, label %b31

b30:
  store i16 8, ptr addrspace(1) %0
  %105 = getelementptr i8, ptr addrspace(1) %0, i16 42
  %106 = getelementptr i8, ptr addrspace(1) %105, i16 0
  %107 = load i16, ptr addrspace(1) %106
  %108 = getelementptr i8, ptr addrspace(1) %0, i16 42
  %109 = getelementptr i8, ptr addrspace(1) %108, i16 2
  %110 = load i16, ptr addrspace(1) %109
  %111 = add i16 %107, %110
  %112 = getelementptr i8, ptr addrspace(1) %0, i16 42
  %113 = getelementptr i8, ptr addrspace(1) %112, i16 4
  %114 = load i16, ptr addrspace(1) %113
  %115 = add i16 %111, %114
  %116 = sdiv i16 %115, 3
  %117 = srem i16 %115, 3
  %118 = icmp ne i16 %117, 0
  %119 = sext i1 %118 to i8
  %120 = xor i16 %117, 3
  %121 = icmp slt i16 %120, 0
  %122 = sext i1 %121 to i8
  %123 = and i8 %119, %122
  %124 = sext i8 %123 to i16
  %125 = and i16 %124, 1
  %126 = sub i16 %116, %125
  store i8 0, ptr %3, !tbaa !2
  %127 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 %126, ptr %127, !tbaa !2
  %128 = addrspacecast ptr %3 to ptr addrspace(1)
  %129 = load i32, ptr addrspace(1) %128, !tbaa !2
  ret i32 %129

b31:
  %130 = icmp eq i16 %4, 7
  %131 = sext i1 %130 to i8
  %132 = icmp ne i8 %131, 0
  br i1 %132, label %b34, label %b33

b32:
  store i16 7, ptr addrspace(1) %0
  br label %b2

b33:
  %133 = icmp eq i16 %4, 8
  %134 = sext i1 %133 to i8
  %135 = icmp ne i8 %134, 0
  br i1 %135, label %b36, label %b35

b34:
  store i16 2, ptr addrspace(1) %0
  br label %b2

b35:
  store i16 1, ptr addrspace(1) %0
  store i8 1, ptr %3, !tbaa !2
  %136 = addrspacecast ptr %3 to ptr addrspace(1)
  %137 = load i32, ptr addrspace(1) %136, !tbaa !2
  ret i32 %137

b36:
  store i16 7, ptr addrspace(1) %0
  br label %b2
}

define internal i32 @$state2.next(ptr addrspace(1) %0) addrspace(1) {
b1:
  %1 = alloca i16
  %2 = alloca [8 x i8]
  %3 = alloca [4 x i8]
  %4 = alloca ptr
  %5 = alloca [4 x i8]
  %6 = alloca [4 x i8]
  store i16 0, ptr %1
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 4, i1 false)
  store ptr null, ptr %4
  call void @llvm.memset.p0.i16(ptr %5, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr %6, i8 0, i16 4, i1 false)
  br label %b2

b2:
  br label %b3

b3:
  %7 = load i16, ptr addrspace(1) %0
  %8 = icmp eq i16 %7, 0
  %9 = sext i1 %8 to i8
  %10 = icmp ne i8 %9, 0
  br i1 %10, label %b7, label %b6

b6:
  %11 = icmp eq i16 %7, 2
  %12 = sext i1 %11 to i8
  %13 = icmp ne i8 %12, 0
  br i1 %13, label %b9, label %b8

b7:
  store i16 2, ptr addrspace(1) %0
  br label %b2

b8:
  %14 = icmp eq i16 %7, 3
  %15 = sext i1 %14 to i8
  %16 = icmp ne i8 %15, 0
  br i1 %16, label %b17, label %b16

b9:
  %17 = addrspacecast ptr %5 to ptr addrspace(1)
  %18 = getelementptr i8, ptr addrspace(1) %0, i16 2
  call addrspace(1) void @$state1.next(ptr addrspace(1) %17, ptr addrspace(1) %18)
  %19 = load i8, ptr %5, !tbaa !2
  %20 = icmp eq i8 %19, 0
  %21 = sext i1 %20 to i8
  %22 = icmp ne i8 %21, 0
  br i1 %22, label %b12, label %b11

b11:
  store i16 4, ptr addrspace(1) %0
  br label %b2

b12:
  %23 = getelementptr inbounds i8, ptr %5, i16 2
  %24 = load ptr, ptr %23, !tbaa !2
  %25 = getelementptr inbounds i8, ptr %5, i16 2
  %26 = load ptr, ptr %25, !tbaa !2
  store ptr %26, ptr %4, !tbaa !2
  %27 = load ptr, ptr %4, !tbaa !2
  store ptr null, ptr %4, !tbaa !2
  %28 = getelementptr i8, ptr addrspace(1) %0, i16 34
  %29 = load ptr, ptr addrspace(1) %28
  call addrspace(1) void @N$BDRP(ptr %29)
  %30 = getelementptr i8, ptr addrspace(1) %0, i16 34
  store ptr %27, ptr addrspace(1) %30
  store i16 3, ptr addrspace(1) %0
  %31 = load ptr, ptr %4, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %31)
  br label %b2

b16:
  %32 = icmp eq i16 %7, 4
  %33 = sext i1 %32 to i8
  %34 = icmp ne i8 %33, 0
  br i1 %34, label %b23, label %b22

b17:
  %35 = getelementptr i8, ptr addrspace(1) %0, i16 34
  %36 = load ptr, ptr addrspace(1) %35
  %37 = getelementptr i8, ptr %36, i16 -4
  %38 = load i16, ptr %37
  %39 = addrspacecast ptr %36 to ptr addrspace(1)
  store i16 %38, ptr %2, !tbaa !2
  %40 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 %38, ptr %40, !tbaa !2
  %41 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %39, ptr %41, !tbaa !2
  %42 = addrspacecast ptr %2 to ptr addrspace(1)
  %43 = call addrspace(1) i32 @number(ptr addrspace(1) %42)
  %44 = addrspacecast ptr %3 to ptr addrspace(1)
  store i32 %43, ptr addrspace(1) %44, !tbaa !2
  %45 = load i8, ptr %3, !tbaa !2
  %46 = icmp eq i8 %45, 0
  %47 = sext i1 %46 to i8
  %48 = icmp ne i8 %47, 0
  br i1 %48, label %b20, label %b19

b19:
  store i16 8, ptr addrspace(1) %0
  br label %b2

b20:
  %49 = getelementptr inbounds i8, ptr %3, i16 2
  %50 = load i16, ptr %49, !tbaa !2
  %51 = getelementptr inbounds i8, ptr %3, i16 2
  %52 = load i16, ptr %51, !tbaa !2
  store i16 %52, ptr %1, !tbaa !2
  %53 = load i16, ptr %1, !tbaa !2
  %54 = getelementptr i8, ptr addrspace(1) %0, i16 36
  store i16 %53, ptr addrspace(1) %54
  store i16 6, ptr addrspace(1) %0
  br label %b2

b22:
  %55 = icmp eq i16 %7, 5
  %56 = sext i1 %55 to i8
  %57 = icmp ne i8 %56, 0
  br i1 %57, label %b29, label %b28

b23:
  %58 = getelementptr i8, ptr addrspace(1) %0, i16 38
  %59 = load i8, ptr addrspace(1) %58
  %60 = icmp ne i8 %59, 0
  %61 = sext i1 %60 to i8
  %62 = icmp ne i8 %61, 0
  br i1 %62, label %b25, label %b24

b24:
  %63 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 0, ptr addrspace(1) %63
  %64 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store ptr null, ptr addrspace(1) %64
  %65 = getelementptr i8, ptr addrspace(1) %0, i16 6
  store i16 0, ptr addrspace(1) %65
  %66 = getelementptr i8, ptr addrspace(1) %0, i16 8
  store i16 0, ptr addrspace(1) %66
  %67 = getelementptr i8, ptr addrspace(1) %0, i16 10
  store ptr addrspace(1) null, ptr addrspace(1) %67
  %68 = getelementptr i8, ptr addrspace(1) %0, i16 14
  store ptr null, ptr addrspace(1) %68
  %69 = getelementptr i8, ptr addrspace(1) %0, i16 16
  store i16 0, ptr addrspace(1) %69
  %70 = getelementptr i8, ptr addrspace(1) %0, i16 18
  store i16 0, ptr addrspace(1) %70
  %71 = getelementptr i8, ptr addrspace(1) %0, i16 20
  store i16 0, ptr addrspace(1) %71
  %72 = getelementptr i8, ptr addrspace(1) %0, i16 22
  store ptr addrspace(1) null, ptr addrspace(1) %72
  %73 = getelementptr i8, ptr addrspace(1) %0, i16 26
  store ptr null, ptr addrspace(1) %73
  %74 = getelementptr i8, ptr addrspace(1) %0, i16 28
  store i16 0, ptr addrspace(1) %74
  %75 = getelementptr i8, ptr addrspace(1) %0, i16 30
  store ptr null, ptr addrspace(1) %75
  %76 = getelementptr i8, ptr addrspace(1) %0, i16 32
  store i8 0, ptr addrspace(1) %76
  %77 = getelementptr i8, ptr addrspace(1) %0, i16 38
  store i8 0, ptr addrspace(1) %77
  store i16 1, ptr addrspace(1) %0
  br label %b2

b25:
  %78 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %79 = load ptr, ptr addrspace(1) %78
  call addrspace(1) void @N$BDRP(ptr %79)
  %80 = getelementptr i8, ptr addrspace(1) %0, i16 32
  %81 = load i8, ptr addrspace(1) %80
  %82 = icmp ne i8 %81, 0
  %83 = sext i1 %82 to i8
  %84 = icmp ne i8 %83, 0
  br i1 %84, label %b27, label %b26

b26:
  %85 = getelementptr i8, ptr addrspace(1) %0, i16 26
  %86 = load ptr, ptr addrspace(1) %85
  call addrspace(1) void @N$BDRP(ptr %86)
  %87 = getelementptr i8, ptr addrspace(1) %0, i16 30
  %88 = load ptr, ptr addrspace(1) %87
  call addrspace(1) void @N$BDRP(ptr %88)
  br label %b24

b27:
  %89 = getelementptr i8, ptr addrspace(1) %0, i16 14
  call addrspace(1) void @Source.drop(ptr addrspace(1) %89)
  %90 = getelementptr i8, ptr addrspace(1) %0, i16 14
  %91 = load ptr, ptr addrspace(1) %90
  call addrspace(1) void @N$BDRP(ptr %91)
  br label %b26

b28:
  %92 = icmp eq i16 %7, 6
  %93 = sext i1 %92 to i8
  %94 = icmp ne i8 %93, 0
  br i1 %94, label %b31, label %b30

b29:
  %95 = getelementptr i8, ptr addrspace(1) %0, i16 34
  %96 = load ptr, ptr addrspace(1) %95
  call addrspace(1) void @N$BDRP(ptr %96)
  %97 = getelementptr i8, ptr addrspace(1) %0, i16 34
  store ptr null, ptr addrspace(1) %97
  store i16 2, ptr addrspace(1) %0
  br label %b2

b30:
  %98 = icmp eq i16 %7, 7
  %99 = sext i1 %98 to i8
  %100 = icmp ne i8 %99, 0
  br i1 %100, label %b33, label %b32

b31:
  store i16 7, ptr addrspace(1) %0
  %101 = getelementptr i8, ptr addrspace(1) %0, i16 36
  %102 = load i16, ptr addrspace(1) %101
  store i8 0, ptr %6, !tbaa !2
  %103 = getelementptr inbounds i8, ptr %6, i16 2
  store i16 %102, ptr %103, !tbaa !2
  %104 = addrspacecast ptr %6 to ptr addrspace(1)
  %105 = load i32, ptr addrspace(1) %104, !tbaa !2
  ret i32 %105

b32:
  %106 = icmp eq i16 %7, 8
  %107 = sext i1 %106 to i8
  %108 = icmp ne i8 %107, 0
  br i1 %108, label %b35, label %b34

b33:
  store i16 5, ptr addrspace(1) %0
  br label %b2

b34:
  store i16 1, ptr addrspace(1) %0
  store i8 1, ptr %6, !tbaa !2
  %109 = addrspacecast ptr %6 to ptr addrspace(1)
  %110 = load i32, ptr addrspace(1) %109, !tbaa !2
  ret i32 %110

b35:
  %111 = getelementptr i8, ptr @$str4, i16 6
  call addrspace(1) void @N$PS(ptr %111)
  %112 = getelementptr i8, ptr addrspace(1) %0, i16 34
  %113 = load ptr, ptr addrspace(1) %112
  call addrspace(1) void @N$PS(ptr %113)
  call addrspace(1) void @N$PN()
  store i16 5, ptr addrspace(1) %0
  br label %b2
}

define internal void @$state1.next(ptr addrspace(1) %0, ptr addrspace(1) %1) addrspace(1) {
b1:
  %2 = alloca ptr
  %3 = alloca [4 x i8]
  store ptr null, ptr %2
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 4, i1 false)
  %4 = getelementptr i8, ptr addrspace(1) %1, i16 4
  br label %b2

b2:
  br label %b3

b3:
  %5 = load i16, ptr addrspace(1) %1
  %6 = icmp eq i16 %5, 0
  %7 = sext i1 %6 to i8
  %8 = icmp ne i8 %7, 0
  br i1 %8, label %b7, label %b6

b6:
  %9 = icmp eq i16 %5, 2
  %10 = sext i1 %9 to i8
  %11 = icmp ne i8 %10, 0
  br i1 %11, label %b11, label %b10

b7:
  %12 = getelementptr i8, ptr addrspace(1) %1, i16 2
  %13 = load ptr, ptr addrspace(1) %12
  %14 = getelementptr i8, ptr addrspace(1) %1, i16 2
  store ptr null, ptr addrspace(1) %14
  %15 = getelementptr i8, ptr addrspace(1) %1, i16 30
  %16 = load i8, ptr addrspace(1) %15
  %17 = icmp ne i8 %16, 0
  %18 = sext i1 %17 to i8
  %19 = icmp ne i8 %18, 0
  br i1 %19, label %b9, label %b8

b8:
  %20 = getelementptr i8, ptr addrspace(1) %1, i16 12
  store ptr %13, ptr addrspace(1) %20
  %21 = getelementptr i8, ptr addrspace(1) %1, i16 30
  store i8 -1, ptr addrspace(1) %21
  %22 = getelementptr i8, ptr addrspace(1) %1, i16 16
  %23 = load i16, ptr addrspace(1) %4
  store i16 %23, ptr addrspace(1) %22
  %24 = getelementptr i8, ptr addrspace(1) %4, i16 2
  %25 = load i16, ptr addrspace(1) %24
  %26 = getelementptr i8, ptr addrspace(1) %22, i16 2
  store i16 %25, ptr addrspace(1) %26
  %27 = getelementptr i8, ptr addrspace(1) %4, i16 4
  %28 = load ptr addrspace(1), ptr addrspace(1) %27
  %29 = getelementptr i8, ptr addrspace(1) %22, i16 4
  store ptr addrspace(1) %28, ptr addrspace(1) %29
  %30 = getelementptr i8, ptr addrspace(1) %1, i16 24
  %31 = load ptr, ptr addrspace(1) %30
  call addrspace(1) void @N$BDRP(ptr %31)
  %32 = getelementptr i8, ptr addrspace(1) %1, i16 14
  store i16 0, ptr addrspace(1) %32
  %33 = getelementptr i8, ptr addrspace(1) %1, i16 24
  store ptr null, ptr addrspace(1) %33
  %34 = getelementptr i8, ptr addrspace(1) %1, i16 26
  store i16 0, ptr addrspace(1) %34
  store i16 2, ptr addrspace(1) %1
  br label %b2

b9:
  %35 = getelementptr i8, ptr addrspace(1) %1, i16 12
  call addrspace(1) void @Source.drop(ptr addrspace(1) %35)
  %36 = getelementptr i8, ptr addrspace(1) %1, i16 12
  %37 = load ptr, ptr addrspace(1) %36
  call addrspace(1) void @N$BDRP(ptr %37)
  br label %b8

b10:
  %38 = icmp eq i16 %5, 3
  %39 = sext i1 %38 to i8
  %40 = icmp ne i8 %39, 0
  br i1 %40, label %b19, label %b18

b11:
  %41 = addrspacecast ptr %3 to ptr addrspace(1)
  %42 = getelementptr i8, ptr addrspace(1) %1, i16 14
  call addrspace(1) void @$state0.next(ptr addrspace(1) %41, ptr addrspace(1) %42)
  %43 = load i8, ptr %3, !tbaa !2
  %44 = icmp eq i8 %43, 0
  %45 = sext i1 %44 to i8
  %46 = icmp ne i8 %45, 0
  br i1 %46, label %b14, label %b13

b13:
  store i16 4, ptr addrspace(1) %1
  br label %b2

b14:
  %47 = getelementptr inbounds i8, ptr %3, i16 2
  %48 = load ptr, ptr %47, !tbaa !2
  %49 = getelementptr inbounds i8, ptr %3, i16 2
  %50 = load ptr, ptr %49, !tbaa !2
  store ptr %50, ptr %2, !tbaa !2
  %51 = load ptr, ptr %2, !tbaa !2
  store ptr null, ptr %2, !tbaa !2
  %52 = getelementptr i8, ptr addrspace(1) %1, i16 28
  %53 = load ptr, ptr addrspace(1) %52
  call addrspace(1) void @N$BDRP(ptr %53)
  %54 = getelementptr i8, ptr addrspace(1) %1, i16 28
  store ptr %51, ptr addrspace(1) %54
  store i16 3, ptr addrspace(1) %1
  %55 = load ptr, ptr %2, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %55)
  br label %b2

b18:
  %56 = icmp eq i16 %5, 4
  %57 = sext i1 %56 to i8
  %58 = icmp ne i8 %57, 0
  br i1 %58, label %b21, label %b20

b19:
  store i16 5, ptr addrspace(1) %1
  %59 = getelementptr i8, ptr addrspace(1) %1, i16 28
  %60 = load ptr, ptr addrspace(1) %59
  %61 = getelementptr i8, ptr addrspace(1) %1, i16 28
  store ptr null, ptr addrspace(1) %61
  store i8 0, ptr addrspace(1) %0
  %62 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr %60, ptr addrspace(1) %62
  ret void

b20:
  %63 = icmp eq i16 %5, 5
  %64 = sext i1 %63 to i8
  %65 = icmp ne i8 %64, 0
  br i1 %65, label %b25, label %b24

b21:
  %66 = getelementptr i8, ptr addrspace(1) %1, i16 24
  %67 = load ptr, ptr addrspace(1) %66
  call addrspace(1) void @N$BDRP(ptr %67)
  %68 = getelementptr i8, ptr addrspace(1) %1, i16 14
  store i16 0, ptr addrspace(1) %68
  %69 = getelementptr i8, ptr addrspace(1) %1, i16 16
  store i16 0, ptr addrspace(1) %69
  %70 = getelementptr i8, ptr addrspace(1) %1, i16 18
  store i16 0, ptr addrspace(1) %70
  %71 = getelementptr i8, ptr addrspace(1) %1, i16 20
  store ptr addrspace(1) null, ptr addrspace(1) %71
  %72 = getelementptr i8, ptr addrspace(1) %1, i16 24
  store ptr null, ptr addrspace(1) %72
  %73 = getelementptr i8, ptr addrspace(1) %1, i16 26
  store i16 0, ptr addrspace(1) %73
  %74 = getelementptr i8, ptr addrspace(1) %1, i16 30
  %75 = load i8, ptr addrspace(1) %74
  %76 = icmp ne i8 %75, 0
  %77 = sext i1 %76 to i8
  %78 = icmp ne i8 %77, 0
  br i1 %78, label %b23, label %b22

b22:
  %79 = getelementptr i8, ptr addrspace(1) %1, i16 12
  store ptr null, ptr addrspace(1) %79
  %80 = getelementptr i8, ptr addrspace(1) %1, i16 30
  store i8 0, ptr addrspace(1) %80
  %81 = getelementptr i8, ptr addrspace(1) %1, i16 2
  %82 = load ptr, ptr addrspace(1) %81
  call addrspace(1) void @N$BDRP(ptr %82)
  %83 = getelementptr i8, ptr addrspace(1) %1, i16 2
  store ptr null, ptr addrspace(1) %83
  store i16 1, ptr addrspace(1) %1
  br label %b2

b23:
  %84 = getelementptr i8, ptr addrspace(1) %1, i16 12
  call addrspace(1) void @Source.drop(ptr addrspace(1) %84)
  %85 = getelementptr i8, ptr addrspace(1) %1, i16 12
  %86 = load ptr, ptr addrspace(1) %85
  call addrspace(1) void @N$BDRP(ptr %86)
  br label %b22

b24:
  store i16 1, ptr addrspace(1) %1
  store i8 1, ptr addrspace(1) %0
  ret void

b25:
  %87 = getelementptr i8, ptr addrspace(1) %1, i16 28
  %88 = load ptr, ptr addrspace(1) %87
  call addrspace(1) void @N$BDRP(ptr %88)
  %89 = getelementptr i8, ptr addrspace(1) %1, i16 28
  store ptr null, ptr addrspace(1) %89
  store i16 2, ptr addrspace(1) %1
  br label %b2
}

define internal void @$state0.next(ptr addrspace(1) %0, ptr addrspace(1) %1) addrspace(1) {
b1:
  %2 = getelementptr i8, ptr addrspace(1) %1, i16 2
  br label %b2

b2:
  br label %b3

b3:
  %3 = load i16, ptr addrspace(1) %1
  %4 = icmp eq i16 %3, 0
  %5 = sext i1 %4 to i8
  %6 = icmp ne i8 %5, 0
  br i1 %6, label %b7, label %b6

b6:
  %7 = icmp eq i16 %3, 2
  %8 = sext i1 %7 to i8
  %9 = icmp ne i8 %8, 0
  br i1 %9, label %b9, label %b8

b7:
  %10 = getelementptr i8, ptr @$str3, i16 6
  %11 = getelementptr i8, ptr addrspace(1) %1, i16 10
  %12 = load ptr, ptr addrspace(1) %11
  call addrspace(1) void @N$BDRP(ptr %12)
  %13 = getelementptr i8, ptr addrspace(1) %1, i16 10
  store ptr %10, ptr addrspace(1) %13
  %14 = getelementptr i8, ptr addrspace(1) %1, i16 12
  store i16 0, ptr addrspace(1) %14
  store i16 2, ptr addrspace(1) %1
  br label %b2

b8:
  %15 = icmp eq i16 %3, 3
  %16 = sext i1 %15 to i8
  %17 = icmp ne i8 %16, 0
  br i1 %17, label %b14, label %b13

b9:
  %18 = getelementptr i8, ptr addrspace(1) %1, i16 12
  %19 = load i16, ptr addrspace(1) %18
  %20 = load i16, ptr addrspace(1) %2
  %21 = icmp ult i16 %19, %20
  %22 = sext i1 %21 to i8
  %23 = icmp ne i8 %22, 0
  br i1 %23, label %b10, label %b11

b10:
  store i16 3, ptr addrspace(1) %1
  br label %b2

b11:
  store i16 5, ptr addrspace(1) %1
  br label %b2

b13:
  %24 = icmp eq i16 %3, 4
  %25 = sext i1 %24 to i8
  %26 = icmp ne i8 %25, 0
  br i1 %26, label %b21, label %b20

b14:
  %27 = getelementptr i8, ptr addrspace(1) %1, i16 12
  %28 = load i16, ptr addrspace(1) %27
  %29 = load i16, ptr addrspace(1) %2
  %30 = icmp ult i16 %28, %29
  %31 = sext i1 %30 to i8
  %32 = icmp ne i8 %31, 0
  br i1 %32, label %b15, label %b16

b15:
  %33 = getelementptr i8, ptr addrspace(1) %2, i16 4
  %34 = load ptr addrspace(1), ptr addrspace(1) %33
  %35 = getelementptr i8, ptr addrspace(1) %34, i16 %28
  %36 = load i8, ptr addrspace(1) %35
  %37 = icmp eq i8 %36, 32
  %38 = sext i1 %37 to i8
  %39 = icmp ne i8 %38, 0
  br i1 %39, label %b17, label %b18

b16:
  call addrspace(1) void @N$EBND()
  unreachable

b17:
  store i16 6, ptr addrspace(1) %1
  br label %b2

b18:
  store i16 7, ptr addrspace(1) %1
  br label %b2

b20:
  %40 = icmp eq i16 %3, 5
  %41 = sext i1 %40 to i8
  %42 = icmp ne i8 %41, 0
  br i1 %42, label %b23, label %b22

b21:
  %43 = getelementptr i8, ptr addrspace(1) %1, i16 12
  %44 = load i16, ptr addrspace(1) %43
  %45 = add i16 %44, 1
  %46 = getelementptr i8, ptr addrspace(1) %1, i16 12
  store i16 %45, ptr addrspace(1) %46
  store i16 2, ptr addrspace(1) %1
  br label %b2

b22:
  %47 = icmp eq i16 %3, 6
  %48 = sext i1 %47 to i8
  %49 = icmp ne i8 %48, 0
  br i1 %49, label %b28, label %b27

b23:
  %50 = getelementptr i8, ptr addrspace(1) %1, i16 10
  %51 = load ptr, ptr addrspace(1) %50
  %52 = getelementptr i8, ptr %51, i16 -4
  %53 = load i16, ptr %52
  %54 = icmp ugt i16 %53, 0
  %55 = sext i1 %54 to i8
  %56 = icmp ne i8 %55, 0
  br i1 %56, label %b24, label %b25

b24:
  store i16 13, ptr addrspace(1) %1
  br label %b2

b25:
  store i16 14, ptr addrspace(1) %1
  br label %b2

b27:
  %57 = icmp eq i16 %3, 7
  %58 = sext i1 %57 to i8
  %59 = icmp ne i8 %58, 0
  br i1 %59, label %b33, label %b32

b28:
  %60 = getelementptr i8, ptr addrspace(1) %1, i16 10
  %61 = load ptr, ptr addrspace(1) %60
  %62 = getelementptr i8, ptr %61, i16 -4
  %63 = load i16, ptr %62
  %64 = icmp ugt i16 %63, 0
  %65 = sext i1 %64 to i8
  %66 = icmp ne i8 %65, 0
  br i1 %66, label %b29, label %b30

b29:
  store i16 9, ptr addrspace(1) %1
  br label %b2

b30:
  store i16 10, ptr addrspace(1) %1
  br label %b2

b32:
  %67 = icmp eq i16 %3, 8
  %68 = sext i1 %67 to i8
  %69 = icmp ne i8 %68, 0
  br i1 %69, label %b37, label %b36

b33:
  %70 = getelementptr i8, ptr addrspace(1) %1, i16 10
  %71 = load ptr, ptr addrspace(1) %70
  %72 = getelementptr i8, ptr %71, i16 -4
  %73 = load i16, ptr %72
  %74 = call addrspace(1) ptr @N$BGRW(ptr %71, i16 1, i16 1)
  %75 = getelementptr i8, ptr addrspace(1) %1, i16 10
  store ptr %74, ptr addrspace(1) %75
  %76 = getelementptr i8, ptr %74, i16 %73
  %77 = getelementptr i8, ptr addrspace(1) %1, i16 12
  %78 = load i16, ptr addrspace(1) %77
  %79 = load i16, ptr addrspace(1) %2
  %80 = icmp ult i16 %78, %79
  %81 = sext i1 %80 to i8
  %82 = icmp ne i8 %81, 0
  br i1 %82, label %b34, label %b35

b34:
  %83 = getelementptr i8, ptr addrspace(1) %2, i16 4
  %84 = load ptr addrspace(1), ptr addrspace(1) %83
  %85 = getelementptr i8, ptr addrspace(1) %84, i16 %78
  %86 = load i8, ptr addrspace(1) %85
  store i8 %86, ptr %76
  %87 = getelementptr i8, ptr %74, i16 -4
  %88 = load i16, ptr %87
  %89 = getelementptr i8, ptr %74, i16 %88
  store i8 0, ptr %89
  store i16 8, ptr addrspace(1) %1
  br label %b2

b35:
  call addrspace(1) void @N$EBND()
  unreachable

b36:
  %90 = icmp eq i16 %3, 9
  %91 = sext i1 %90 to i8
  %92 = icmp ne i8 %91, 0
  br i1 %92, label %b39, label %b38

b37:
  store i16 4, ptr addrspace(1) %1
  br label %b2

b38:
  %93 = icmp eq i16 %3, 10
  %94 = sext i1 %93 to i8
  %95 = icmp ne i8 %94, 0
  br i1 %95, label %b41, label %b40

b39:
  store i16 12, ptr addrspace(1) %1
  %96 = getelementptr i8, ptr addrspace(1) %1, i16 10
  %97 = load ptr, ptr addrspace(1) %96
  %98 = getelementptr i8, ptr addrspace(1) %1, i16 10
  store ptr null, ptr addrspace(1) %98
  store i8 0, ptr addrspace(1) %0
  %99 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr %97, ptr addrspace(1) %99
  ret void

b40:
  %100 = icmp eq i16 %3, 11
  %101 = sext i1 %100 to i8
  %102 = icmp ne i8 %101, 0
  br i1 %102, label %b43, label %b42

b41:
  store i16 11, ptr addrspace(1) %1
  br label %b2

b42:
  %103 = icmp eq i16 %3, 12
  %104 = sext i1 %103 to i8
  %105 = icmp ne i8 %104, 0
  br i1 %105, label %b45, label %b44

b43:
  store i16 8, ptr addrspace(1) %1
  br label %b2

b44:
  %106 = icmp eq i16 %3, 13
  %107 = sext i1 %106 to i8
  %108 = icmp ne i8 %107, 0
  br i1 %108, label %b47, label %b46

b45:
  %109 = getelementptr i8, ptr @$str3, i16 6
  %110 = getelementptr i8, ptr addrspace(1) %1, i16 10
  %111 = load ptr, ptr addrspace(1) %110
  call addrspace(1) void @N$BDRP(ptr %111)
  %112 = getelementptr i8, ptr addrspace(1) %1, i16 10
  store ptr %109, ptr addrspace(1) %112
  store i16 11, ptr addrspace(1) %1
  br label %b2

b46:
  %113 = icmp eq i16 %3, 14
  %114 = sext i1 %113 to i8
  %115 = icmp ne i8 %114, 0
  br i1 %115, label %b49, label %b48

b47:
  store i16 16, ptr addrspace(1) %1
  %116 = getelementptr i8, ptr addrspace(1) %1, i16 10
  %117 = load ptr, ptr addrspace(1) %116
  %118 = getelementptr i8, ptr addrspace(1) %1, i16 10
  store ptr null, ptr addrspace(1) %118
  store i8 0, ptr addrspace(1) %0
  %119 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr %117, ptr addrspace(1) %119
  ret void

b48:
  %120 = icmp eq i16 %3, 15
  %121 = sext i1 %120 to i8
  %122 = icmp ne i8 %121, 0
  br i1 %122, label %b51, label %b50

b49:
  store i16 15, ptr addrspace(1) %1
  br label %b2

b50:
  %123 = icmp eq i16 %3, 16
  %124 = sext i1 %123 to i8
  %125 = icmp ne i8 %124, 0
  br i1 %125, label %b53, label %b52

b51:
  %126 = getelementptr i8, ptr addrspace(1) %1, i16 10
  %127 = load ptr, ptr addrspace(1) %126
  call addrspace(1) void @N$BDRP(ptr %127)
  %128 = getelementptr i8, ptr addrspace(1) %1, i16 10
  store ptr null, ptr addrspace(1) %128
  store i16 1, ptr addrspace(1) %1
  br label %b2

b52:
  store i16 1, ptr addrspace(1) %1
  store i8 1, ptr addrspace(1) %0
  ret void

b53:
  store i16 15, ptr addrspace(1) %1
  br label %b2
}

declare void @N$PS(ptr) addrspace(1)

declare void @N$PN() addrspace(1)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$PI2(i16) addrspace(1)

declare void @N$BDRP(ptr) addrspace(1)

declare void @N$EBND() addrspace(1)

declare ptr @N$BGRW(ptr, i16, i16) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
