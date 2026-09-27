target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [15 x i8] c"\08\00\08\00\08\00matmul: \00"

define internal void @multiply(ptr addrspace(1) noalias readonly dereferenceable(10) %0, ptr addrspace(1) noalias readonly dereferenceable(10) %1, ptr addrspace(1) noalias readonly dereferenceable(10) %2) addrspace(1) {
b1:
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i32
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca i16
  store i16 0, ptr %3
  store i16 0, ptr %4
  store i32 0, ptr %5
  store i16 0, ptr %6
  store i16 0, ptr %7
  store i16 0, ptr %8
  store i16 0, ptr %9
  %10 = load i16, ptr addrspace(1) %0
  store i16 0, ptr %9, !tbaa !2
  store i16 %10, ptr %8, !tbaa !2
  br label %b2

b2:
  %11 = load i16, ptr %9, !tbaa !2
  %12 = load i16, ptr %8, !tbaa !2
  %13 = icmp ult i16 %11, %12
  %14 = sext i1 %13 to i8
  %15 = icmp ne i8 %14, 0
  br i1 %15, label %b3, label %b5

b3:
  %16 = getelementptr i8, ptr addrspace(1) %1, i16 2
  %17 = load i16, ptr addrspace(1) %16
  store i16 0, ptr %7, !tbaa !2
  store i16 %17, ptr %6, !tbaa !2
  br label %b6

b4:
  %18 = load i16, ptr %9, !tbaa !2
  %19 = add i16 %18, 1
  store i16 %19, ptr %9, !tbaa !2
  br label %b2

b5:
  ret void

b6:
  %20 = load i16, ptr %7, !tbaa !2
  %21 = load i16, ptr %6, !tbaa !2
  %22 = icmp ult i16 %20, %21
  %23 = sext i1 %22 to i8
  %24 = icmp ne i8 %23, 0
  br i1 %24, label %b7, label %b9

b7:
  store i32 0, ptr %5, !tbaa !2
  %25 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %26 = load i16, ptr addrspace(1) %25
  store i16 0, ptr %4, !tbaa !2
  store i16 %26, ptr %3, !tbaa !2
  br label %b10

b8:
  %27 = load i16, ptr %7, !tbaa !2
  %28 = add i16 %27, 1
  store i16 %28, ptr %7, !tbaa !2
  br label %b6

b9:
  br label %b4

b10:
  %29 = load i16, ptr %4, !tbaa !2
  %30 = load i16, ptr %3, !tbaa !2
  %31 = icmp ult i16 %29, %30
  %32 = sext i1 %31 to i8
  %33 = icmp ne i8 %32, 0
  br i1 %33, label %b11, label %b13

b11:
  %34 = load i32, ptr %5, !tbaa !2
  %35 = load i16, ptr %9, !tbaa !2
  %36 = load i16, ptr %4, !tbaa !2
  %37 = load i16, ptr addrspace(1) %0
  %38 = icmp ult i16 %35, %37
  %39 = sext i1 %38 to i8
  %40 = icmp ne i8 %39, 0
  br i1 %40, label %b14, label %b15

b12:
  %41 = load i16, ptr %4, !tbaa !2
  %42 = add i16 %41, 1
  store i16 %42, ptr %4, !tbaa !2
  br label %b10

b13:
  %43 = load i16, ptr %9, !tbaa !2
  %44 = load i16, ptr %7, !tbaa !2
  %45 = load i16, ptr addrspace(1) %2
  %46 = icmp ult i16 %43, %45
  %47 = sext i1 %46 to i8
  %48 = icmp ne i8 %47, 0
  br i1 %48, label %b22, label %b23

b14:
  %49 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %50 = load i16, ptr addrspace(1) %49
  %51 = icmp ult i16 %36, %50
  %52 = sext i1 %51 to i8
  %53 = icmp ne i8 %52, 0
  br i1 %53, label %b16, label %b17

b15:
  call addrspace(1) void @N$EBND()
  unreachable

b16:
  %54 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %55 = load i16, ptr addrspace(1) %54
  %56 = mul i16 %55, 1
  %57 = mul i16 %35, %56
  %58 = mul i16 %36, 1
  %59 = add i16 %57, %58
  %60 = getelementptr i8, ptr addrspace(1) %0, i16 6
  %61 = load ptr addrspace(1), ptr addrspace(1) %60
  %62 = mul i16 %59, 4
  %63 = getelementptr i8, ptr addrspace(1) %61, i16 %62
  %64 = load i32, ptr addrspace(1) %63
  %65 = load i16, ptr %4, !tbaa !2
  %66 = load i16, ptr %7, !tbaa !2
  %67 = load i16, ptr addrspace(1) %1
  %68 = icmp ult i16 %65, %67
  %69 = sext i1 %68 to i8
  %70 = icmp ne i8 %69, 0
  br i1 %70, label %b18, label %b19

b17:
  call addrspace(1) void @N$EBND()
  unreachable

b18:
  %71 = getelementptr i8, ptr addrspace(1) %1, i16 2
  %72 = load i16, ptr addrspace(1) %71
  %73 = icmp ult i16 %66, %72
  %74 = sext i1 %73 to i8
  %75 = icmp ne i8 %74, 0
  br i1 %75, label %b20, label %b21

b19:
  call addrspace(1) void @N$EBND()
  unreachable

b20:
  %76 = getelementptr i8, ptr addrspace(1) %1, i16 2
  %77 = load i16, ptr addrspace(1) %76
  %78 = mul i16 %77, 1
  %79 = mul i16 %65, %78
  %80 = mul i16 %66, 1
  %81 = add i16 %79, %80
  %82 = getelementptr i8, ptr addrspace(1) %1, i16 6
  %83 = load ptr addrspace(1), ptr addrspace(1) %82
  %84 = mul i16 %81, 4
  %85 = getelementptr i8, ptr addrspace(1) %83, i16 %84
  %86 = load i32, ptr addrspace(1) %85
  %87 = sext i32 %64 to i64
  %88 = sext i32 %86 to i64
  %89 = mul i64 %87, %88
  %90 = ashr i64 %89, 8
  %91 = trunc i64 %90 to i32
  %92 = add i32 %34, %91
  store i32 %92, ptr %5, !tbaa !2
  br label %b12

b21:
  call addrspace(1) void @N$EBND()
  unreachable

b22:
  %93 = getelementptr i8, ptr addrspace(1) %2, i16 2
  %94 = load i16, ptr addrspace(1) %93
  %95 = icmp ult i16 %44, %94
  %96 = sext i1 %95 to i8
  %97 = icmp ne i8 %96, 0
  br i1 %97, label %b24, label %b25

b23:
  call addrspace(1) void @N$EBND()
  unreachable

b24:
  %98 = getelementptr i8, ptr addrspace(1) %2, i16 2
  %99 = load i16, ptr addrspace(1) %98
  %100 = mul i16 %99, 1
  %101 = mul i16 %43, %100
  %102 = mul i16 %44, 1
  %103 = add i16 %101, %102
  %104 = getelementptr i8, ptr addrspace(1) %2, i16 6
  %105 = load ptr addrspace(1), ptr addrspace(1) %104
  %106 = mul i16 %103, 4
  %107 = getelementptr i8, ptr addrspace(1) %105, i16 %106
  %108 = load i32, ptr %5, !tbaa !2
  store i32 %108, ptr addrspace(1) %107
  br label %b8

b25:
  call addrspace(1) void @N$EBND()
  unreachable
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i32
  %5 = alloca [10 x i8]
  %6 = alloca [10 x i8]
  %7 = alloca [10 x i8]
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
  %19 = alloca [256 x i8]
  %20 = alloca i32
  %21 = alloca i16
  %22 = alloca i16
  %23 = alloca i16
  %24 = alloca i16
  %25 = alloca i16
  %26 = alloca i16
  %27 = alloca i16
  %28 = alloca [256 x i8]
  %29 = alloca i32
  %30 = alloca i16
  %31 = alloca i16
  %32 = alloca i16
  %33 = alloca i16
  %34 = alloca i16
  %35 = alloca i16
  %36 = alloca i16
  %37 = alloca [256 x i8]
  %38 = alloca i32
  %39 = alloca i16
  store i16 0, ptr %0
  store i16 0, ptr %1
  store i16 0, ptr %2
  store i16 0, ptr %3
  store i32 0, ptr %4
  call void @llvm.memset.p0.i16(ptr %5, i8 0, i16 10, i1 false)
  call void @llvm.memset.p0.i16(ptr %6, i8 0, i16 10, i1 false)
  call void @llvm.memset.p0.i16(ptr %7, i8 0, i16 10, i1 false)
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
  call void @llvm.memset.p0.i16(ptr %19, i8 0, i16 256, i1 false)
  store i32 0, ptr %20
  store i16 0, ptr %21
  store i16 0, ptr %22
  store i16 0, ptr %23
  store i16 0, ptr %24
  store i16 0, ptr %25
  store i16 0, ptr %26
  store i16 0, ptr %27
  call void @llvm.memset.p0.i16(ptr %28, i8 0, i16 256, i1 false)
  store i32 0, ptr %29
  store i16 0, ptr %30
  store i16 0, ptr %31
  store i16 0, ptr %32
  store i16 0, ptr %33
  store i16 0, ptr %34
  store i16 0, ptr %35
  store i16 0, ptr %36
  call void @llvm.memset.p0.i16(ptr %37, i8 0, i16 256, i1 false)
  store i32 0, ptr %38
  store i16 0, ptr %39
  store i16 1, ptr %39, !tbaa !2
  store i32 0, ptr %38, !tbaa !2
  store i16 8, ptr %34, !tbaa !2
  store i16 8, ptr %35, !tbaa !2
  store i16 64, ptr %36, !tbaa !2
  store i16 0, ptr %33, !tbaa !2
  store i16 8, ptr %32, !tbaa !2
  br label %b2

b2:
  %40 = load i16, ptr %33, !tbaa !2
  %41 = load i16, ptr %32, !tbaa !2
  %42 = icmp slt i16 %40, %41
  %43 = sext i1 %42 to i8
  %44 = icmp ne i8 %43, 0
  br i1 %44, label %b3, label %b5

b3:
  store i16 0, ptr %31, !tbaa !2
  store i16 8, ptr %30, !tbaa !2
  br label %b6

b4:
  %45 = load i16, ptr %33, !tbaa !2
  %46 = add i16 %45, 1
  store i16 %46, ptr %33, !tbaa !2
  br label %b2

b5:
  store i32 0, ptr %29, !tbaa !2
  store i16 8, ptr %25, !tbaa !2
  store i16 8, ptr %26, !tbaa !2
  store i16 64, ptr %27, !tbaa !2
  store i16 0, ptr %24, !tbaa !2
  store i16 8, ptr %23, !tbaa !2
  br label %b10

b6:
  %47 = load i16, ptr %31, !tbaa !2
  %48 = load i16, ptr %30, !tbaa !2
  %49 = icmp slt i16 %47, %48
  %50 = sext i1 %49 to i8
  %51 = icmp ne i8 %50, 0
  br i1 %51, label %b7, label %b9

b7:
  %52 = load i16, ptr %33, !tbaa !2
  %53 = load i16, ptr %31, !tbaa !2
  %54 = load i32, ptr %38, !tbaa !2
  %55 = sub i16 %52, 0
  %56 = sub i16 %53, 0
  %57 = mul i16 %55, 8
  %58 = add i16 %57, %56
  %59 = getelementptr inbounds i32, ptr %37, i16 %58
  store i32 %54, ptr %59, !tbaa !2
  br label %b8

b8:
  %60 = load i16, ptr %31, !tbaa !2
  %61 = add i16 %60, 1
  store i16 %61, ptr %31, !tbaa !2
  br label %b6

b9:
  br label %b4

b10:
  %62 = load i16, ptr %24, !tbaa !2
  %63 = load i16, ptr %23, !tbaa !2
  %64 = icmp slt i16 %62, %63
  %65 = sext i1 %64 to i8
  %66 = icmp ne i8 %65, 0
  br i1 %66, label %b11, label %b13

b11:
  store i16 0, ptr %22, !tbaa !2
  store i16 8, ptr %21, !tbaa !2
  br label %b14

b12:
  %67 = load i16, ptr %24, !tbaa !2
  %68 = add i16 %67, 1
  store i16 %68, ptr %24, !tbaa !2
  br label %b10

b13:
  store i32 0, ptr %20, !tbaa !2
  store i16 8, ptr %16, !tbaa !2
  store i16 8, ptr %17, !tbaa !2
  store i16 64, ptr %18, !tbaa !2
  store i16 0, ptr %15, !tbaa !2
  store i16 8, ptr %14, !tbaa !2
  br label %b18

b14:
  %69 = load i16, ptr %22, !tbaa !2
  %70 = load i16, ptr %21, !tbaa !2
  %71 = icmp slt i16 %69, %70
  %72 = sext i1 %71 to i8
  %73 = icmp ne i8 %72, 0
  br i1 %73, label %b15, label %b17

b15:
  %74 = load i16, ptr %24, !tbaa !2
  %75 = load i16, ptr %22, !tbaa !2
  %76 = load i32, ptr %29, !tbaa !2
  %77 = sub i16 %74, 0
  %78 = sub i16 %75, 0
  %79 = mul i16 %77, 8
  %80 = add i16 %79, %78
  %81 = getelementptr inbounds i32, ptr %28, i16 %80
  store i32 %76, ptr %81, !tbaa !2
  br label %b16

b16:
  %82 = load i16, ptr %22, !tbaa !2
  %83 = add i16 %82, 1
  store i16 %83, ptr %22, !tbaa !2
  br label %b14

b17:
  br label %b12

b18:
  %84 = load i16, ptr %15, !tbaa !2
  %85 = load i16, ptr %14, !tbaa !2
  %86 = icmp slt i16 %84, %85
  %87 = sext i1 %86 to i8
  %88 = icmp ne i8 %87, 0
  br i1 %88, label %b19, label %b21

b19:
  store i16 0, ptr %13, !tbaa !2
  store i16 8, ptr %12, !tbaa !2
  br label %b22

b20:
  %89 = load i16, ptr %15, !tbaa !2
  %90 = add i16 %89, 1
  store i16 %90, ptr %15, !tbaa !2
  br label %b18

b21:
  store i16 0, ptr %11, !tbaa !2
  store i16 8, ptr %10, !tbaa !2
  br label %b26

b22:
  %91 = load i16, ptr %13, !tbaa !2
  %92 = load i16, ptr %12, !tbaa !2
  %93 = icmp slt i16 %91, %92
  %94 = sext i1 %93 to i8
  %95 = icmp ne i8 %94, 0
  br i1 %95, label %b23, label %b25

b23:
  %96 = load i16, ptr %15, !tbaa !2
  %97 = load i16, ptr %13, !tbaa !2
  %98 = load i32, ptr %20, !tbaa !2
  %99 = sub i16 %96, 0
  %100 = sub i16 %97, 0
  %101 = mul i16 %99, 8
  %102 = add i16 %101, %100
  %103 = getelementptr inbounds i32, ptr %19, i16 %102
  store i32 %98, ptr %103, !tbaa !2
  br label %b24

b24:
  %104 = load i16, ptr %13, !tbaa !2
  %105 = add i16 %104, 1
  store i16 %105, ptr %13, !tbaa !2
  br label %b22

b25:
  br label %b20

b26:
  %106 = load i16, ptr %11, !tbaa !2
  %107 = load i16, ptr %10, !tbaa !2
  %108 = icmp slt i16 %106, %107
  %109 = sext i1 %108 to i8
  %110 = icmp ne i8 %109, 0
  br i1 %110, label %b27, label %b29

b27:
  store i16 0, ptr %9, !tbaa !2
  store i16 8, ptr %8, !tbaa !2
  br label %b30

b28:
  %111 = load i16, ptr %11, !tbaa !2
  %112 = add i16 %111, 1
  store i16 %112, ptr %11, !tbaa !2
  br label %b26

b29:
  %113 = addrspacecast ptr %37 to ptr addrspace(1)
  store i16 8, ptr %7, !tbaa !2
  %114 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 8, ptr %114, !tbaa !2
  %115 = getelementptr inbounds i8, ptr %7, i16 4
  store i16 64, ptr %115, !tbaa !2
  %116 = getelementptr inbounds i8, ptr %7, i16 6
  store ptr addrspace(1) %113, ptr %116, !tbaa !2
  %117 = addrspacecast ptr %7 to ptr addrspace(1)
  %118 = addrspacecast ptr %28 to ptr addrspace(1)
  store i16 8, ptr %6, !tbaa !2
  %119 = getelementptr inbounds i8, ptr %6, i16 2
  store i16 8, ptr %119, !tbaa !2
  %120 = getelementptr inbounds i8, ptr %6, i16 4
  store i16 64, ptr %120, !tbaa !2
  %121 = getelementptr inbounds i8, ptr %6, i16 6
  store ptr addrspace(1) %118, ptr %121, !tbaa !2
  %122 = addrspacecast ptr %6 to ptr addrspace(1)
  %123 = addrspacecast ptr %19 to ptr addrspace(1)
  store i16 8, ptr %5, !tbaa !2
  %124 = getelementptr inbounds i8, ptr %5, i16 2
  store i16 8, ptr %124, !tbaa !2
  %125 = getelementptr inbounds i8, ptr %5, i16 4
  store i16 64, ptr %125, !tbaa !2
  %126 = getelementptr inbounds i8, ptr %5, i16 6
  store ptr addrspace(1) %123, ptr %126, !tbaa !2
  %127 = addrspacecast ptr %5 to ptr addrspace(1)
  call addrspace(1) void @multiply(ptr addrspace(1) %117, ptr addrspace(1) %122, ptr addrspace(1) %127)
  store i32 0, ptr %4, !tbaa !2
  store i16 0, ptr %3, !tbaa !2
  store i16 8, ptr %2, !tbaa !2
  br label %b49

b30:
  %128 = load i16, ptr %9, !tbaa !2
  %129 = load i16, ptr %8, !tbaa !2
  %130 = icmp slt i16 %128, %129
  %131 = sext i1 %130 to i8
  %132 = icmp ne i8 %131, 0
  br i1 %132, label %b31, label %b33

b31:
  %133 = load i16, ptr %11, !tbaa !2
  %134 = load i16, ptr %9, !tbaa !2
  %135 = icmp ult i16 %133, 8
  %136 = sext i1 %135 to i8
  %137 = icmp ne i8 %136, 0
  br i1 %137, label %b34, label %b35

b32:
  %138 = load i16, ptr %9, !tbaa !2
  %139 = add i16 %138, 1
  store i16 %139, ptr %9, !tbaa !2
  br label %b30

b33:
  br label %b28

b34:
  %140 = icmp ult i16 %134, 8
  %141 = sext i1 %140 to i8
  %142 = icmp ne i8 %141, 0
  br i1 %142, label %b36, label %b37

b35:
  call addrspace(1) void @N$EBND()
  unreachable

b36:
  %143 = load i16, ptr %11, !tbaa !2
  %144 = mul i16 %143, 3
  %145 = load i16, ptr %9, !tbaa !2
  %146 = add i16 %144, %145
  %147 = add i16 %146, 1
  %148 = load i16, ptr %39, !tbaa !2
  %149 = add i16 %147, %148
  %150 = sext i16 %149 to i32
  %151 = zext i8 8 to i32
  %152 = shl i32 %150, %151
  %153 = sext i32 %152 to i64
  %154 = sext i32 1024 to i64
  %155 = shl i64 %153, 8
  %156 = sdiv i64 %155, %154
  %157 = trunc i64 %156 to i32
  %158 = sub i16 %133, 0
  %159 = sub i16 %134, 0
  %160 = mul i16 %158, 8
  %161 = add i16 %160, %159
  %162 = getelementptr inbounds i32, ptr %37, i16 %161
  store i32 %157, ptr %162, !tbaa !2
  %163 = load i16, ptr %11, !tbaa !2
  %164 = load i16, ptr %9, !tbaa !2
  %165 = icmp eq i16 %163, %164
  %166 = sext i1 %165 to i8
  %167 = icmp ne i8 %166, 0
  br i1 %167, label %b38, label %b39

b37:
  call addrspace(1) void @N$EBND()
  unreachable

b38:
  %168 = load i16, ptr %11, !tbaa !2
  %169 = load i16, ptr %9, !tbaa !2
  %170 = icmp ult i16 %168, 8
  %171 = sext i1 %170 to i8
  %172 = icmp ne i8 %171, 0
  br i1 %172, label %b41, label %b42

b39:
  %173 = load i16, ptr %11, !tbaa !2
  %174 = load i16, ptr %9, !tbaa !2
  %175 = icmp ult i16 %173, 8
  %176 = sext i1 %175 to i8
  %177 = icmp ne i8 %176, 0
  br i1 %177, label %b45, label %b46

b40:
  br label %b32

b41:
  %178 = icmp ult i16 %169, 8
  %179 = sext i1 %178 to i8
  %180 = icmp ne i8 %179, 0
  br i1 %180, label %b43, label %b44

b42:
  call addrspace(1) void @N$EBND()
  unreachable

b43:
  %181 = sub i16 %168, 0
  %182 = sub i16 %169, 0
  %183 = mul i16 %181, 8
  %184 = add i16 %183, %182
  %185 = getelementptr inbounds i32, ptr %28, i16 %184
  store i32 512, ptr %185, !tbaa !2
  br label %b40

b44:
  call addrspace(1) void @N$EBND()
  unreachable

b45:
  %186 = icmp ult i16 %174, 8
  %187 = sext i1 %186 to i8
  %188 = icmp ne i8 %187, 0
  br i1 %188, label %b47, label %b48

b46:
  call addrspace(1) void @N$EBND()
  unreachable

b47:
  %189 = load i16, ptr %11, !tbaa !2
  %190 = load i16, ptr %9, !tbaa !2
  %191 = add i16 %189, %190
  %192 = srem i16 %191, 3
  %193 = sext i16 %192 to i32
  %194 = zext i8 8 to i32
  %195 = shl i32 %193, %194
  %196 = sext i32 %195 to i64
  %197 = sext i32 512 to i64
  %198 = shl i64 %196, 8
  %199 = sdiv i64 %198, %197
  %200 = trunc i64 %199 to i32
  %201 = sub i16 %173, 0
  %202 = sub i16 %174, 0
  %203 = mul i16 %201, 8
  %204 = add i16 %203, %202
  %205 = getelementptr inbounds i32, ptr %28, i16 %204
  store i32 %200, ptr %205, !tbaa !2
  br label %b40

b48:
  call addrspace(1) void @N$EBND()
  unreachable

b49:
  %206 = load i16, ptr %3, !tbaa !2
  %207 = load i16, ptr %2, !tbaa !2
  %208 = icmp slt i16 %206, %207
  %209 = sext i1 %208 to i8
  %210 = icmp ne i8 %209, 0
  br i1 %210, label %b50, label %b52

b50:
  store i16 0, ptr %1, !tbaa !2
  store i16 8, ptr %0, !tbaa !2
  br label %b53

b51:
  %211 = load i16, ptr %3, !tbaa !2
  %212 = add i16 %211, 1
  store i16 %212, ptr %3, !tbaa !2
  br label %b49

b52:
  %213 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %213)
  %214 = load i32, ptr %4, !tbaa !2
  call addrspace(1) void @N$PQ4(i32 %214, i8 8)
  call addrspace(1) void @N$PN()
  ret i16 0

b53:
  %215 = load i16, ptr %1, !tbaa !2
  %216 = load i16, ptr %0, !tbaa !2
  %217 = icmp slt i16 %215, %216
  %218 = sext i1 %217 to i8
  %219 = icmp ne i8 %218, 0
  br i1 %219, label %b54, label %b56

b54:
  %220 = load i32, ptr %4, !tbaa !2
  %221 = load i16, ptr %3, !tbaa !2
  %222 = load i16, ptr %1, !tbaa !2
  %223 = icmp ult i16 %221, 8
  %224 = sext i1 %223 to i8
  %225 = icmp ne i8 %224, 0
  br i1 %225, label %b57, label %b58

b55:
  %226 = load i16, ptr %1, !tbaa !2
  %227 = add i16 %226, 1
  store i16 %227, ptr %1, !tbaa !2
  br label %b53

b56:
  br label %b51

b57:
  %228 = icmp ult i16 %222, 8
  %229 = sext i1 %228 to i8
  %230 = icmp ne i8 %229, 0
  br i1 %230, label %b59, label %b60

b58:
  call addrspace(1) void @N$EBND()
  unreachable

b59:
  %231 = sub i16 %221, 0
  %232 = sub i16 %222, 0
  %233 = mul i16 %231, 8
  %234 = add i16 %233, %232
  %235 = getelementptr inbounds i32, ptr %19, i16 %234
  %236 = load i32, ptr %235, !tbaa !2
  %237 = load i16, ptr %3, !tbaa !2
  %238 = mul i16 %237, 8
  %239 = load i16, ptr %1, !tbaa !2
  %240 = add i16 %238, %239
  %241 = add i16 %240, 1
  %242 = sext i16 %241 to i32
  %243 = zext i8 8 to i32
  %244 = shl i32 %242, %243
  %245 = sext i32 %236 to i64
  %246 = sext i32 %244 to i64
  %247 = mul i64 %245, %246
  %248 = ashr i64 %247, 8
  %249 = trunc i64 %248 to i32
  %250 = add i32 %220, %249
  store i32 %250, ptr %4, !tbaa !2
  br label %b55

b60:
  call addrspace(1) void @N$EBND()
  unreachable
}

declare void @N$EBND() addrspace(1)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PQ4(i32, i8) addrspace(1)

declare void @N$PN() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
