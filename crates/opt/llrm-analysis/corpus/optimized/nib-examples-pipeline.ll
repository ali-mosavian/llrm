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

define internal i32 @number(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) willreturn {
b1:
  %1 = alloca [4 x i8]
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 4, i1 false)
  %2 = load i16, ptr addrspace(1) %0
  %3 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %4 = load ptr addrspace(1), ptr addrspace(1) %3
  br label %b2

b2:
  %5 = phi i16 [ 0, %b1 ], [ %26, %b10 ]
  %6 = phi i16 [ 0, %b1 ], [ %27, %b10 ]
  %7 = icmp ult i16 %6, %2
  br i1 %7, label %b3, label %b5

b3:
  %8 = getelementptr i8, ptr addrspace(1) %4, i16 %6
  %9 = load i8, ptr addrspace(1) %8
  %10 = icmp ult i8 %9, 48
  %11 = sext i1 %10 to i8
  br i1 %10, label %b7, label %b6

b5:
  store i8 0, ptr %1, !tbaa !2
  %12 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 %5, ptr %12, !tbaa !2
  %13 = addrspacecast ptr %1 to ptr addrspace(1)
  %14 = load i32, ptr addrspace(1) %13, !tbaa !2
  ret i32 %14

b6:
  %15 = load i8, ptr addrspace(1) %8
  %16 = icmp ugt i8 %15, 57
  %17 = sext i1 %16 to i8
  br label %b7

b7:
  %18 = phi i8 [ %11, %b3 ], [ %17, %b6 ]
  %19 = icmp ne i8 %18, 0
  br i1 %19, label %b8, label %b10

b8:
  store i8 1, ptr %1, !tbaa !2
  %20 = addrspacecast ptr %1 to ptr addrspace(1)
  %21 = load i32, ptr addrspace(1) %20, !tbaa !2
  ret i32 %21

b10:
  %22 = mul i16 %5, 10
  %23 = load i8, ptr addrspace(1) %8
  %24 = zext i8 %23 to i16
  %25 = add i16 %22, %24
  %26 = add i16 %25, -48
  %27 = add i16 %6, 1
  br label %b2
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca [4 x i8]
  %1 = alloca [54 x i8]
  %2 = alloca [54 x i8]
  %3 = alloca [4 x i8]
  %4 = alloca [54 x i8]
  %5 = alloca [54 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 54, i1 false)
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 54, i1 false)
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 54, i1 false)
  call void @llvm.memset.p0.i16(ptr %5, i8 0, i16 54, i1 false)
  %6 = getelementptr i8, ptr @$str2, i16 6
  %7 = getelementptr i8, ptr @$str5, i16 6
  %8 = getelementptr i8, ptr %6, i16 -4
  %9 = load i16, ptr %8
  %10 = addrspacecast ptr %6 to ptr addrspace(1)
  %11 = getelementptr inbounds i8, ptr %5, i16 8
  %12 = addrspacecast ptr %11 to ptr addrspace(1)
  store i16 %9, ptr addrspace(1) %12
  %13 = getelementptr i8, ptr addrspace(1) %12, i16 2
  store i16 %9, ptr addrspace(1) %13
  %14 = getelementptr i8, ptr addrspace(1) %12, i16 4
  store ptr addrspace(1) %10, ptr addrspace(1) %14
  store i16 0, ptr %5, !tbaa !2
  %15 = getelementptr inbounds i8, ptr %5, i16 2
  store i16 0, ptr %15, !tbaa !2
  %16 = getelementptr inbounds i8, ptr %5, i16 4
  store i16 0, ptr %16, !tbaa !2
  %17 = getelementptr inbounds i8, ptr %5, i16 6
  store ptr %7, ptr %17, !tbaa !2
  %18 = getelementptr inbounds i8, ptr %5, i16 16
  store ptr null, ptr %18, !tbaa !2
  %19 = getelementptr inbounds i8, ptr %5, i16 18
  store i16 0, ptr %19, !tbaa !2
  %20 = getelementptr inbounds i8, ptr %5, i16 20
  store i16 0, ptr %20, !tbaa !2
  %21 = getelementptr inbounds i8, ptr %5, i16 22
  store i16 0, ptr %21, !tbaa !2
  %22 = getelementptr inbounds i8, ptr %5, i16 24
  store ptr addrspace(1) null, ptr %22, !tbaa !2
  %23 = getelementptr inbounds i8, ptr %5, i16 28
  store ptr null, ptr %23, !tbaa !2
  %24 = getelementptr inbounds i8, ptr %5, i16 30
  store i16 0, ptr %24, !tbaa !2
  %25 = getelementptr inbounds i8, ptr %5, i16 32
  store ptr null, ptr %25, !tbaa !2
  %26 = getelementptr inbounds i8, ptr %5, i16 34
  store i8 0, ptr %26, !tbaa !2
  %27 = getelementptr inbounds i8, ptr %5, i16 36
  store ptr null, ptr %27, !tbaa !2
  %28 = getelementptr inbounds i8, ptr %5, i16 38
  store i16 0, ptr %28, !tbaa !2
  %29 = getelementptr inbounds i8, ptr %5, i16 40
  store i8 -1, ptr %29, !tbaa !2
  %30 = getelementptr inbounds i8, ptr %5, i16 42
  %31 = addrspacecast ptr %30 to ptr addrspace(1)
  br label %b2

b2:
  %32 = phi i16 [ 0, %b1 ], [ %36, %b3 ]
  %33 = icmp slt i16 %32, 3
  br i1 %33, label %b3, label %b5

b3:
  %34 = shl i16 %32, 1
  %35 = getelementptr i8, ptr addrspace(1) %31, i16 %34
  store i16 0, ptr addrspace(1) %35
  %36 = add i16 %32, 1
  br label %b2

b5:
  %37 = getelementptr inbounds i8, ptr %5, i16 48
  store i16 0, ptr %37, !tbaa !2
  %38 = getelementptr inbounds i8, ptr %5, i16 50
  store i16 0, ptr %38, !tbaa !2
  %39 = getelementptr inbounds i8, ptr %5, i16 52
  store i8 -1, ptr %39, !tbaa !2
  %40 = load i16, ptr %5, !tbaa !2
  %41 = load i16, ptr %15, !tbaa !2
  %42 = load i16, ptr %16, !tbaa !2
  %43 = load ptr, ptr %17, !tbaa !2
  %44 = load i16, ptr %11, !tbaa !2
  %45 = getelementptr inbounds i8, ptr %5, i16 10
  %46 = load i16, ptr %45, !tbaa !2
  %47 = getelementptr inbounds i8, ptr %5, i16 12
  %48 = load ptr addrspace(1), ptr %47, !tbaa !2
  %49 = load ptr, ptr %18, !tbaa !2
  %50 = load i16, ptr %19, !tbaa !2
  %51 = load i16, ptr %20, !tbaa !2
  %52 = load i16, ptr %21, !tbaa !2
  %53 = load ptr addrspace(1), ptr %22, !tbaa !2
  %54 = load ptr, ptr %23, !tbaa !2
  %55 = load i16, ptr %24, !tbaa !2
  %56 = load ptr, ptr %25, !tbaa !2
  %57 = load i8, ptr %26, !tbaa !2
  %58 = load ptr, ptr %27, !tbaa !2
  %59 = load i16, ptr %28, !tbaa !2
  %60 = load i8, ptr %29, !tbaa !2
  store i16 %40, ptr %4, !tbaa !2
  %61 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 %41, ptr %61, !tbaa !2
  %62 = getelementptr inbounds i8, ptr %4, i16 4
  store i16 %42, ptr %62, !tbaa !2
  %63 = getelementptr inbounds i8, ptr %4, i16 6
  store ptr %43, ptr %63, !tbaa !2
  %64 = getelementptr inbounds i8, ptr %4, i16 8
  store i16 %44, ptr %64, !tbaa !2
  %65 = getelementptr inbounds i8, ptr %4, i16 10
  store i16 %46, ptr %65, !tbaa !2
  %66 = getelementptr inbounds i8, ptr %4, i16 12
  store ptr addrspace(1) %48, ptr %66, !tbaa !2
  %67 = getelementptr inbounds i8, ptr %4, i16 16
  store ptr %49, ptr %67, !tbaa !2
  %68 = getelementptr inbounds i8, ptr %4, i16 18
  store i16 %50, ptr %68, !tbaa !2
  %69 = getelementptr inbounds i8, ptr %4, i16 20
  store i16 %51, ptr %69, !tbaa !2
  %70 = getelementptr inbounds i8, ptr %4, i16 22
  store i16 %52, ptr %70, !tbaa !2
  %71 = getelementptr inbounds i8, ptr %4, i16 24
  store ptr addrspace(1) %53, ptr %71, !tbaa !2
  %72 = getelementptr inbounds i8, ptr %4, i16 28
  store ptr %54, ptr %72, !tbaa !2
  %73 = getelementptr inbounds i8, ptr %4, i16 30
  store i16 %55, ptr %73, !tbaa !2
  %74 = getelementptr inbounds i8, ptr %4, i16 32
  store ptr %56, ptr %74, !tbaa !2
  %75 = getelementptr inbounds i8, ptr %4, i16 34
  store i8 %57, ptr %75, !tbaa !2
  %76 = getelementptr inbounds i8, ptr %4, i16 36
  store ptr %58, ptr %76, !tbaa !2
  %77 = getelementptr inbounds i8, ptr %4, i16 38
  store i16 %59, ptr %77, !tbaa !2
  %78 = getelementptr inbounds i8, ptr %4, i16 40
  store i8 %60, ptr %78, !tbaa !2
  %79 = getelementptr inbounds i8, ptr %4, i16 42
  %80 = addrspacecast ptr %79 to ptr addrspace(1)
  br label %b6

b6:
  %81 = phi i16 [ 0, %b5 ], [ %87, %b7 ]
  %82 = icmp slt i16 %81, 3
  br i1 %82, label %b7, label %b9

b7:
  %83 = shl i16 %81, 1
  %84 = getelementptr i8, ptr addrspace(1) %80, i16 %83
  %85 = getelementptr i8, ptr addrspace(1) %31, i16 %83
  %86 = load i16, ptr addrspace(1) %85
  store i16 %86, ptr addrspace(1) %84
  %87 = add i16 %81, 1
  br label %b6

b9:
  %88 = getelementptr inbounds i8, ptr %4, i16 48
  store i16 0, ptr %88, !tbaa !2
  %89 = getelementptr inbounds i8, ptr %4, i16 50
  store i16 0, ptr %89, !tbaa !2
  %90 = getelementptr inbounds i8, ptr %4, i16 52
  store i8 -1, ptr %90, !tbaa !2
  store i16 0, ptr %5, !tbaa !2
  store i16 0, ptr %15, !tbaa !2
  store i16 0, ptr %16, !tbaa !2
  store ptr null, ptr %17, !tbaa !2
  store i16 0, ptr %11, !tbaa !2
  store i16 0, ptr %45, !tbaa !2
  store ptr addrspace(1) null, ptr %47, !tbaa !2
  store ptr null, ptr %18, !tbaa !2
  store i16 0, ptr %19, !tbaa !2
  store i16 0, ptr %20, !tbaa !2
  store i16 0, ptr %21, !tbaa !2
  store ptr addrspace(1) null, ptr %22, !tbaa !2
  store ptr null, ptr %23, !tbaa !2
  store i16 0, ptr %24, !tbaa !2
  store ptr null, ptr %25, !tbaa !2
  store i8 0, ptr %26, !tbaa !2
  store ptr null, ptr %27, !tbaa !2
  store i16 0, ptr %28, !tbaa !2
  store i8 0, ptr %29, !tbaa !2
  br label %b10

b10:
  %91 = phi i16 [ 0, %b9 ], [ %95, %b11 ]
  %92 = icmp slt i16 %91, 3
  br i1 %92, label %b11, label %b13

b11:
  %93 = shl i16 %91, 1
  %94 = getelementptr i8, ptr addrspace(1) %31, i16 %93
  store i16 0, ptr addrspace(1) %94
  %95 = add i16 %91, 1
  br label %b10

b13:
  store i16 0, ptr %37, !tbaa !2
  store i16 0, ptr %38, !tbaa !2
  store i8 0, ptr %39, !tbaa !2
  %96 = addrspacecast ptr %4 to ptr addrspace(1)
  %97 = addrspacecast ptr %3 to ptr addrspace(1)
  %98 = getelementptr inbounds i8, ptr %3, i16 2
  %99 = getelementptr i8, ptr @$str6, i16 6
  br label %b15

b15:
  %100 = call addrspace(1) i32 @$state3.next(ptr addrspace(1) %96)
  store i32 %100, ptr addrspace(1) %97, !tbaa !2
  %101 = load i8, ptr %3, !tbaa !2
  %102 = icmp eq i8 %101, 0
  br i1 %102, label %b19, label %b22

b19:
  %103 = load i16, ptr %98, !tbaa !2
  call addrspace(1) void @N$PS(ptr %99)
  call addrspace(1) void @N$PI2(i16 %103)
  call addrspace(1) void @N$PN()
  br label %b15

b21:
  %104 = getelementptr i8, ptr @$str7, i16 6
  %105 = load i16, ptr %8
  %106 = getelementptr inbounds i8, ptr %2, i16 8
  %107 = addrspacecast ptr %106 to ptr addrspace(1)
  store i16 %105, ptr addrspace(1) %107
  %108 = getelementptr i8, ptr addrspace(1) %107, i16 2
  store i16 %105, ptr addrspace(1) %108
  %109 = getelementptr i8, ptr addrspace(1) %107, i16 4
  store ptr addrspace(1) %10, ptr addrspace(1) %109
  store i16 0, ptr %2, !tbaa !2
  %110 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 0, ptr %110, !tbaa !2
  %111 = getelementptr inbounds i8, ptr %2, i16 4
  store i16 0, ptr %111, !tbaa !2
  %112 = getelementptr inbounds i8, ptr %2, i16 6
  store ptr %104, ptr %112, !tbaa !2
  %113 = getelementptr inbounds i8, ptr %2, i16 16
  store ptr null, ptr %113, !tbaa !2
  %114 = getelementptr inbounds i8, ptr %2, i16 18
  store i16 0, ptr %114, !tbaa !2
  %115 = getelementptr inbounds i8, ptr %2, i16 20
  store i16 0, ptr %115, !tbaa !2
  %116 = getelementptr inbounds i8, ptr %2, i16 22
  store i16 0, ptr %116, !tbaa !2
  %117 = getelementptr inbounds i8, ptr %2, i16 24
  store ptr addrspace(1) null, ptr %117, !tbaa !2
  %118 = getelementptr inbounds i8, ptr %2, i16 28
  store ptr null, ptr %118, !tbaa !2
  %119 = getelementptr inbounds i8, ptr %2, i16 30
  store i16 0, ptr %119, !tbaa !2
  %120 = getelementptr inbounds i8, ptr %2, i16 32
  store ptr null, ptr %120, !tbaa !2
  %121 = getelementptr inbounds i8, ptr %2, i16 34
  store i8 0, ptr %121, !tbaa !2
  %122 = getelementptr inbounds i8, ptr %2, i16 36
  store ptr null, ptr %122, !tbaa !2
  %123 = getelementptr inbounds i8, ptr %2, i16 38
  store i16 0, ptr %123, !tbaa !2
  %124 = getelementptr inbounds i8, ptr %2, i16 40
  store i8 -1, ptr %124, !tbaa !2
  %125 = getelementptr inbounds i8, ptr %2, i16 42
  %126 = addrspacecast ptr %125 to ptr addrspace(1)
  br label %b29

b22:
  %127 = load i8, ptr %90, !tbaa !2
  %128 = icmp ne i8 %127, 0
  br i1 %128, label %b24, label %b21

b24:
  %129 = load ptr, ptr %76, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %129)
  %130 = load i8, ptr %78, !tbaa !2
  %131 = icmp ne i8 %130, 0
  br i1 %131, label %b26, label %b21

b26:
  %132 = load ptr, ptr %63, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %132)
  %133 = load i8, ptr %75, !tbaa !2
  %134 = icmp ne i8 %133, 0
  br i1 %134, label %b28, label %b27

b27:
  %135 = load ptr, ptr %72, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %135)
  %136 = load ptr, ptr %74, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %136)
  br label %b21

b28:
  %137 = addrspacecast ptr %67 to ptr addrspace(1)
  %138 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %138)
  %139 = load ptr, ptr addrspace(1) %137
  call addrspace(1) void @N$PS(ptr %139)
  call addrspace(1) void @N$PN()
  %140 = load ptr, ptr %67, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %140)
  br label %b27

b29:
  %141 = phi i16 [ 0, %b21 ], [ %145, %b30 ]
  %142 = icmp slt i16 %141, 3
  br i1 %142, label %b30, label %b32

b30:
  %143 = shl i16 %141, 1
  %144 = getelementptr i8, ptr addrspace(1) %126, i16 %143
  store i16 0, ptr addrspace(1) %144
  %145 = add i16 %141, 1
  br label %b29

b32:
  %146 = getelementptr inbounds i8, ptr %2, i16 48
  store i16 0, ptr %146, !tbaa !2
  %147 = getelementptr inbounds i8, ptr %2, i16 50
  store i16 0, ptr %147, !tbaa !2
  %148 = getelementptr inbounds i8, ptr %2, i16 52
  store i8 -1, ptr %148, !tbaa !2
  %149 = load i16, ptr %2, !tbaa !2
  %150 = load i16, ptr %110, !tbaa !2
  %151 = load i16, ptr %111, !tbaa !2
  %152 = load ptr, ptr %112, !tbaa !2
  %153 = load i16, ptr %106, !tbaa !2
  %154 = getelementptr inbounds i8, ptr %2, i16 10
  %155 = load i16, ptr %154, !tbaa !2
  %156 = getelementptr inbounds i8, ptr %2, i16 12
  %157 = load ptr addrspace(1), ptr %156, !tbaa !2
  %158 = load ptr, ptr %113, !tbaa !2
  %159 = load i16, ptr %114, !tbaa !2
  %160 = load i16, ptr %115, !tbaa !2
  %161 = load i16, ptr %116, !tbaa !2
  %162 = load ptr addrspace(1), ptr %117, !tbaa !2
  %163 = load ptr, ptr %118, !tbaa !2
  %164 = load i16, ptr %119, !tbaa !2
  %165 = load ptr, ptr %120, !tbaa !2
  %166 = load i8, ptr %121, !tbaa !2
  %167 = load ptr, ptr %122, !tbaa !2
  %168 = load i16, ptr %123, !tbaa !2
  %169 = load i8, ptr %124, !tbaa !2
  store i16 %149, ptr %1, !tbaa !2
  %170 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 %150, ptr %170, !tbaa !2
  %171 = getelementptr inbounds i8, ptr %1, i16 4
  store i16 %151, ptr %171, !tbaa !2
  %172 = getelementptr inbounds i8, ptr %1, i16 6
  store ptr %152, ptr %172, !tbaa !2
  %173 = getelementptr inbounds i8, ptr %1, i16 8
  store i16 %153, ptr %173, !tbaa !2
  %174 = getelementptr inbounds i8, ptr %1, i16 10
  store i16 %155, ptr %174, !tbaa !2
  %175 = getelementptr inbounds i8, ptr %1, i16 12
  store ptr addrspace(1) %157, ptr %175, !tbaa !2
  %176 = getelementptr inbounds i8, ptr %1, i16 16
  store ptr %158, ptr %176, !tbaa !2
  %177 = getelementptr inbounds i8, ptr %1, i16 18
  store i16 %159, ptr %177, !tbaa !2
  %178 = getelementptr inbounds i8, ptr %1, i16 20
  store i16 %160, ptr %178, !tbaa !2
  %179 = getelementptr inbounds i8, ptr %1, i16 22
  store i16 %161, ptr %179, !tbaa !2
  %180 = getelementptr inbounds i8, ptr %1, i16 24
  store ptr addrspace(1) %162, ptr %180, !tbaa !2
  %181 = getelementptr inbounds i8, ptr %1, i16 28
  store ptr %163, ptr %181, !tbaa !2
  %182 = getelementptr inbounds i8, ptr %1, i16 30
  store i16 %164, ptr %182, !tbaa !2
  %183 = getelementptr inbounds i8, ptr %1, i16 32
  store ptr %165, ptr %183, !tbaa !2
  %184 = getelementptr inbounds i8, ptr %1, i16 34
  store i8 %166, ptr %184, !tbaa !2
  %185 = getelementptr inbounds i8, ptr %1, i16 36
  store ptr %167, ptr %185, !tbaa !2
  %186 = getelementptr inbounds i8, ptr %1, i16 38
  store i16 %168, ptr %186, !tbaa !2
  %187 = getelementptr inbounds i8, ptr %1, i16 40
  store i8 %169, ptr %187, !tbaa !2
  %188 = getelementptr inbounds i8, ptr %1, i16 42
  %189 = addrspacecast ptr %188 to ptr addrspace(1)
  br label %b33

b33:
  %190 = phi i16 [ 0, %b32 ], [ %196, %b34 ]
  %191 = icmp slt i16 %190, 3
  br i1 %191, label %b34, label %b36

b34:
  %192 = shl i16 %190, 1
  %193 = getelementptr i8, ptr addrspace(1) %189, i16 %192
  %194 = getelementptr i8, ptr addrspace(1) %126, i16 %192
  %195 = load i16, ptr addrspace(1) %194
  store i16 %195, ptr addrspace(1) %193
  %196 = add i16 %190, 1
  br label %b33

b36:
  %197 = getelementptr inbounds i8, ptr %1, i16 48
  store i16 0, ptr %197, !tbaa !2
  %198 = getelementptr inbounds i8, ptr %1, i16 50
  store i16 0, ptr %198, !tbaa !2
  %199 = getelementptr inbounds i8, ptr %1, i16 52
  store i8 -1, ptr %199, !tbaa !2
  store i16 0, ptr %2, !tbaa !2
  store i16 0, ptr %110, !tbaa !2
  store i16 0, ptr %111, !tbaa !2
  store ptr null, ptr %112, !tbaa !2
  store i16 0, ptr %106, !tbaa !2
  store i16 0, ptr %154, !tbaa !2
  store ptr addrspace(1) null, ptr %156, !tbaa !2
  store ptr null, ptr %113, !tbaa !2
  store i16 0, ptr %114, !tbaa !2
  store i16 0, ptr %115, !tbaa !2
  store i16 0, ptr %116, !tbaa !2
  store ptr addrspace(1) null, ptr %117, !tbaa !2
  store ptr null, ptr %118, !tbaa !2
  store i16 0, ptr %119, !tbaa !2
  store ptr null, ptr %120, !tbaa !2
  store i8 0, ptr %121, !tbaa !2
  store ptr null, ptr %122, !tbaa !2
  store i16 0, ptr %123, !tbaa !2
  store i8 0, ptr %124, !tbaa !2
  br label %b37

b37:
  %200 = phi i16 [ 0, %b36 ], [ %204, %b38 ]
  %201 = icmp slt i16 %200, 3
  br i1 %201, label %b38, label %b40

b38:
  %202 = shl i16 %200, 1
  %203 = getelementptr i8, ptr addrspace(1) %126, i16 %202
  store i16 0, ptr addrspace(1) %203
  %204 = add i16 %200, 1
  br label %b37

b40:
  store i16 0, ptr %146, !tbaa !2
  store i16 0, ptr %147, !tbaa !2
  store i8 0, ptr %148, !tbaa !2
  %205 = addrspacecast ptr %1 to ptr addrspace(1)
  %206 = addrspacecast ptr %0 to ptr addrspace(1)
  %207 = getelementptr inbounds i8, ptr %0, i16 2
  br label %b42

b42:
  %208 = call addrspace(1) i32 @$state3.next(ptr addrspace(1) %205)
  store i32 %208, ptr addrspace(1) %206, !tbaa !2
  %209 = load i8, ptr %0, !tbaa !2
  %210 = icmp eq i8 %209, 0
  br i1 %210, label %b46, label %b52

b46:
  %211 = load i16, ptr %207, !tbaa !2
  %212 = icmp sgt i16 %211, 20
  br i1 %212, label %b47, label %b48

b47:
  %213 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @N$PS(ptr %213)
  call addrspace(1) void @N$PI2(i16 %211)
  call addrspace(1) void @N$PN()
  br label %b52

b48:
  br label %b42

b51:
  %214 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %214)
  call addrspace(1) void @N$PN()
  call addrspace(1) void @N$BDRP(ptr %6)
  ret i16 0

b52:
  %215 = load i8, ptr %199, !tbaa !2
  %216 = icmp ne i8 %215, 0
  br i1 %216, label %b54, label %b51

b54:
  %217 = load ptr, ptr %185, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %217)
  %218 = load i8, ptr %187, !tbaa !2
  %219 = icmp ne i8 %218, 0
  br i1 %219, label %b56, label %b51

b56:
  %220 = load ptr, ptr %172, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %220)
  %221 = load i8, ptr %184, !tbaa !2
  %222 = icmp ne i8 %221, 0
  br i1 %222, label %b58, label %b57

b57:
  %223 = load ptr, ptr %181, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %223)
  %224 = load ptr, ptr %183, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %224)
  br label %b51

b58:
  %225 = addrspacecast ptr %176 to ptr addrspace(1)
  %226 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %226)
  %227 = load ptr, ptr addrspace(1) %225
  call addrspace(1) void @N$PS(ptr %227)
  call addrspace(1) void @N$PN()
  %228 = load ptr, ptr %176, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %228)
  br label %b57
}

define internal i32 @$state3.next(ptr addrspace(1) %0) addrspace(1) {
b1:
  %1 = alloca [4 x i8]
  %2 = alloca [4 x i8]
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 4, i1 false)
  %3 = getelementptr i8, ptr addrspace(1) %0, i16 42
  %4 = getelementptr i8, ptr addrspace(1) %0, i16 44
  %5 = getelementptr i8, ptr addrspace(1) %0, i16 46
  %6 = getelementptr i8, ptr addrspace(1) %0, i16 48
  %7 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %8 = addrspacecast ptr %1 to ptr addrspace(1)
  %9 = getelementptr inbounds i8, ptr %1, i16 2
  %10 = getelementptr i8, ptr addrspace(1) %0, i16 50
  %11 = getelementptr i8, ptr addrspace(1) %0, i16 52
  %12 = getelementptr i8, ptr addrspace(1) %0, i16 36
  %13 = getelementptr i8, ptr addrspace(1) %0, i16 40
  %14 = getelementptr i8, ptr addrspace(1) %0, i16 6
  %15 = getelementptr i8, ptr addrspace(1) %0, i16 34
  %16 = getelementptr i8, ptr addrspace(1) %0, i16 16
  %17 = getelementptr i8, ptr @$str1, i16 6
  %18 = getelementptr i8, ptr addrspace(1) %0, i16 28
  %19 = getelementptr i8, ptr addrspace(1) %0, i16 32
  %20 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %21 = getelementptr i8, ptr addrspace(1) %0, i16 8
  %22 = getelementptr i8, ptr addrspace(1) %0, i16 10
  %23 = getelementptr i8, ptr addrspace(1) %0, i16 12
  %24 = getelementptr i8, ptr addrspace(1) %0, i16 18
  %25 = getelementptr i8, ptr addrspace(1) %0, i16 20
  %26 = getelementptr i8, ptr addrspace(1) %0, i16 22
  %27 = getelementptr i8, ptr addrspace(1) %0, i16 24
  %28 = getelementptr i8, ptr addrspace(1) %0, i16 30
  %29 = getelementptr i8, ptr addrspace(1) %0, i16 38
  br label %b2

b2:
  %30 = load i16, ptr addrspace(1) %0
  %31 = icmp eq i16 %30, 0
  br i1 %31, label %b7, label %b6

b6:
  %32 = icmp eq i16 %30, 2
  br i1 %32, label %b9, label %b8

b7:
  store i16 0, ptr addrspace(1) %3
  store i16 0, ptr addrspace(1) %4
  store i16 0, ptr addrspace(1) %5
  store i16 0, ptr addrspace(1) %6
  store i16 2, ptr addrspace(1) %0
  br label %b2

b8:
  %33 = icmp eq i16 %30, 3
  br i1 %33, label %b15, label %b14

b9:
  %34 = call addrspace(1) i32 @$state2.next(ptr addrspace(1) %7)
  store i32 %34, ptr addrspace(1) %8, !tbaa !2
  %35 = load i8, ptr %1, !tbaa !2
  %36 = icmp eq i8 %35, 0
  br i1 %36, label %b12, label %b11

b11:
  store i16 4, ptr addrspace(1) %0
  br label %b2

b12:
  %37 = load i16, ptr %9, !tbaa !2
  store i16 %37, ptr addrspace(1) %10
  store i16 3, ptr addrspace(1) %0
  br label %b2

b14:
  %38 = icmp eq i16 %30, 4
  br i1 %38, label %b22, label %b21

b15:
  %39 = load i16, ptr addrspace(1) %6
  %40 = urem i16 %39, 3
  %41 = icmp ult i16 %40, 3
  br i1 %41, label %b16, label %b17

b16:
  %42 = shl i16 %40, 1
  %43 = getelementptr i8, ptr addrspace(1) %3, i16 %42
  %44 = load i16, ptr addrspace(1) %10
  store i16 %44, ptr addrspace(1) %43
  %45 = load i16, ptr addrspace(1) %6
  %46 = add i16 %45, 1
  store i16 %46, ptr addrspace(1) %6
  %47 = icmp uge i16 %46, 3
  br i1 %47, label %b18, label %b19

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
  %48 = icmp eq i16 %30, 5
  br i1 %48, label %b30, label %b29

b22:
  %49 = load i8, ptr addrspace(1) %11
  %50 = icmp ne i8 %49, 0
  br i1 %50, label %b24, label %b23

b23:
  store i16 0, ptr addrspace(1) %7
  store i16 0, ptr addrspace(1) %20
  store ptr null, ptr addrspace(1) %14
  store i16 0, ptr addrspace(1) %21
  store i16 0, ptr addrspace(1) %22
  store ptr addrspace(1) null, ptr addrspace(1) %23
  store ptr null, ptr addrspace(1) %16
  store i16 0, ptr addrspace(1) %24
  store i16 0, ptr addrspace(1) %25
  store i16 0, ptr addrspace(1) %26
  store ptr addrspace(1) null, ptr addrspace(1) %27
  store ptr null, ptr addrspace(1) %18
  store i16 0, ptr addrspace(1) %28
  store ptr null, ptr addrspace(1) %19
  store i8 0, ptr addrspace(1) %15
  store ptr null, ptr addrspace(1) %12
  store i16 0, ptr addrspace(1) %29
  store i8 0, ptr addrspace(1) %13
  store i8 0, ptr addrspace(1) %11
  store i16 1, ptr addrspace(1) %0
  br label %b2

b24:
  %51 = load ptr, ptr addrspace(1) %12
  call addrspace(1) void @N$BDRP(ptr %51)
  %52 = load i8, ptr addrspace(1) %13
  %53 = icmp ne i8 %52, 0
  br i1 %53, label %b26, label %b23

b26:
  %54 = load ptr, ptr addrspace(1) %14
  call addrspace(1) void @N$BDRP(ptr %54)
  %55 = load i8, ptr addrspace(1) %15
  %56 = icmp ne i8 %55, 0
  br i1 %56, label %b28, label %b27

b27:
  %57 = load ptr, ptr addrspace(1) %18
  call addrspace(1) void @N$BDRP(ptr %57)
  %58 = load ptr, ptr addrspace(1) %19
  call addrspace(1) void @N$BDRP(ptr %58)
  br label %b23

b28:
  call addrspace(1) void @N$PS(ptr %17)
  %59 = load ptr, ptr addrspace(1) %16
  call addrspace(1) void @N$PS(ptr %59)
  call addrspace(1) void @N$PN()
  %60 = load ptr, ptr addrspace(1) %16
  call addrspace(1) void @N$BDRP(ptr %60)
  br label %b27

b29:
  %61 = icmp eq i16 %30, 6
  br i1 %61, label %b32, label %b31

b30:
  store i16 8, ptr addrspace(1) %0
  %62 = getelementptr i8, ptr addrspace(1) %3, i16 0
  %63 = load i16, ptr addrspace(1) %62
  %64 = getelementptr i8, ptr addrspace(1) %3, i16 2
  %65 = load i16, ptr addrspace(1) %64
  %66 = add i16 %63, %65
  %67 = getelementptr i8, ptr addrspace(1) %3, i16 4
  %68 = load i16, ptr addrspace(1) %67
  %69 = add i16 %66, %68
  %70 = sdiv i16 %69, 3
  %71 = srem i16 %69, 3
  %72 = icmp ne i16 %71, 0
  %73 = sext i1 %72 to i8
  %74 = xor i16 %71, 3
  %75 = icmp slt i16 %74, 0
  %76 = sext i1 %75 to i8
  %77 = and i8 %73, %76
  %78 = sext i8 %77 to i16
  %79 = and i16 %78, 1
  %80 = sub i16 %70, %79
  store i8 0, ptr %2, !tbaa !2
  %81 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 %80, ptr %81, !tbaa !2
  %82 = addrspacecast ptr %2 to ptr addrspace(1)
  %83 = load i32, ptr addrspace(1) %82, !tbaa !2
  ret i32 %83

b31:
  %84 = icmp eq i16 %30, 7
  br i1 %84, label %b34, label %b33

b32:
  store i16 7, ptr addrspace(1) %0
  br label %b2

b33:
  %85 = icmp eq i16 %30, 8
  br i1 %85, label %b36, label %b35

b34:
  store i16 2, ptr addrspace(1) %0
  br label %b2

b35:
  store i16 1, ptr addrspace(1) %0
  store i8 1, ptr %2, !tbaa !2
  %86 = addrspacecast ptr %2 to ptr addrspace(1)
  %87 = load i32, ptr addrspace(1) %86, !tbaa !2
  ret i32 %87

b36:
  store i16 7, ptr addrspace(1) %0
  br label %b2
}

define internal i32 @$state2.next(ptr addrspace(1) %0) addrspace(1) {
b1:
  %1 = alloca [4 x i8]
  %2 = alloca [4 x i8]
  %3 = alloca [4 x i8]
  %4 = alloca [4 x i8]
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 4, i1 false)
  %5 = addrspacecast ptr %3 to ptr addrspace(1)
  %6 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %7 = getelementptr inbounds i8, ptr %3, i16 2
  %8 = getelementptr i8, ptr addrspace(1) %0, i16 34
  %9 = addrspacecast ptr %2 to ptr addrspace(1)
  %10 = getelementptr inbounds i8, ptr %2, i16 2
  %11 = getelementptr i8, ptr addrspace(1) %0, i16 36
  %12 = getelementptr inbounds i8, ptr %1, i16 2
  %13 = addrspacecast ptr %1 to ptr addrspace(1)
  %14 = getelementptr i8, ptr addrspace(1) %0, i16 38
  %15 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %16 = getelementptr i8, ptr addrspace(1) %0, i16 32
  %17 = getelementptr i8, ptr addrspace(1) %0, i16 14
  %18 = getelementptr i8, ptr @$str1, i16 6
  %19 = getelementptr i8, ptr addrspace(1) %0, i16 26
  %20 = getelementptr i8, ptr addrspace(1) %0, i16 30
  %21 = getelementptr i8, ptr addrspace(1) %0, i16 6
  %22 = getelementptr i8, ptr addrspace(1) %0, i16 8
  %23 = getelementptr i8, ptr addrspace(1) %0, i16 10
  %24 = getelementptr i8, ptr addrspace(1) %0, i16 16
  %25 = getelementptr i8, ptr addrspace(1) %0, i16 18
  %26 = getelementptr i8, ptr addrspace(1) %0, i16 20
  %27 = getelementptr i8, ptr addrspace(1) %0, i16 22
  %28 = getelementptr i8, ptr addrspace(1) %0, i16 28
  %29 = getelementptr i8, ptr @$str4, i16 6
  br label %b2

b2:
  %30 = load i16, ptr addrspace(1) %0
  %31 = icmp eq i16 %30, 0
  br i1 %31, label %b7, label %b6

b6:
  %32 = icmp eq i16 %30, 2
  br i1 %32, label %b9, label %b8

b7:
  store i16 2, ptr addrspace(1) %0
  br label %b2

b8:
  %33 = icmp eq i16 %30, 3
  br i1 %33, label %b17, label %b16

b9:
  call addrspace(1) void @$state1.next(ptr addrspace(1) %5, ptr addrspace(1) %6)
  %34 = load i8, ptr %3, !tbaa !2
  %35 = icmp eq i8 %34, 0
  br i1 %35, label %b12, label %b11

b11:
  store i16 4, ptr addrspace(1) %0
  br label %b2

b12:
  %36 = load ptr, ptr %7, !tbaa !2
  %37 = load ptr, ptr addrspace(1) %8
  call addrspace(1) void @N$BDRP(ptr %37)
  store ptr %36, ptr addrspace(1) %8
  store i16 3, ptr addrspace(1) %0
  call addrspace(1) void @N$BDRP(ptr null)
  br label %b2

b16:
  %38 = icmp eq i16 %30, 4
  br i1 %38, label %b23, label %b22

b17:
  %39 = load ptr, ptr addrspace(1) %8
  %40 = getelementptr i8, ptr %39, i16 -4
  %41 = load i16, ptr %40
  %42 = addrspacecast ptr %39 to ptr addrspace(1)
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 4, i1 false)
  br label %43

43:
  %44 = phi i16 [ 0, %b17 ], [ %68, %63 ]
  %45 = phi i16 [ 0, %b17 ], [ %69, %63 ]
  %46 = icmp ult i16 %45, %41
  br i1 %46, label %47, label %52

47:
  %48 = getelementptr i8, ptr addrspace(1) %42, i16 %45
  %49 = load i8, ptr addrspace(1) %48
  %50 = icmp ult i8 %49, 48
  %51 = sext i1 %50 to i8
  br i1 %50, label %58, label %54

52:
  store i8 0, ptr %1
  store i16 %44, ptr %12
  %53 = load i32, ptr addrspace(1) %13
  br label %70

54:
  %55 = load i8, ptr addrspace(1) %48
  %56 = icmp ugt i8 %55, 57
  %57 = sext i1 %56 to i8
  br label %58

58:
  %59 = phi i8 [ %51, %47 ], [ %57, %54 ]
  %60 = icmp ne i8 %59, 0
  br i1 %60, label %61, label %63

61:
  store i8 1, ptr %1
  %62 = load i32, ptr addrspace(1) %13
  br label %70

63:
  %64 = mul i16 %44, 10
  %65 = load i8, ptr addrspace(1) %48
  %66 = zext i8 %65 to i16
  %67 = add i16 %64, %66
  %68 = add i16 %67, -48
  %69 = add i16 %45, 1
  br label %43

70:
  %71 = phi i32 [ %53, %52 ], [ %62, %61 ]
  store i32 %71, ptr addrspace(1) %9, !tbaa !2
  %72 = load i8, ptr %2, !tbaa !2
  %73 = icmp eq i8 %72, 0
  br i1 %73, label %b20, label %b19

b19:
  store i16 8, ptr addrspace(1) %0
  br label %b2

b20:
  %74 = load i16, ptr %10, !tbaa !2
  store i16 %74, ptr addrspace(1) %11
  store i16 6, ptr addrspace(1) %0
  br label %b2

b22:
  %75 = icmp eq i16 %30, 5
  br i1 %75, label %b29, label %b28

b23:
  %76 = load i8, ptr addrspace(1) %14
  %77 = icmp ne i8 %76, 0
  br i1 %77, label %b25, label %b24

b24:
  store i16 0, ptr addrspace(1) %6
  store ptr null, ptr addrspace(1) %15
  store i16 0, ptr addrspace(1) %21
  store i16 0, ptr addrspace(1) %22
  store ptr addrspace(1) null, ptr addrspace(1) %23
  store ptr null, ptr addrspace(1) %17
  store i16 0, ptr addrspace(1) %24
  store i16 0, ptr addrspace(1) %25
  store i16 0, ptr addrspace(1) %26
  store ptr addrspace(1) null, ptr addrspace(1) %27
  store ptr null, ptr addrspace(1) %19
  store i16 0, ptr addrspace(1) %28
  store ptr null, ptr addrspace(1) %20
  store i8 0, ptr addrspace(1) %16
  store i8 0, ptr addrspace(1) %14
  store i16 1, ptr addrspace(1) %0
  br label %b2

b25:
  %78 = load ptr, ptr addrspace(1) %15
  call addrspace(1) void @N$BDRP(ptr %78)
  %79 = load i8, ptr addrspace(1) %16
  %80 = icmp ne i8 %79, 0
  br i1 %80, label %b27, label %b26

b26:
  %81 = load ptr, ptr addrspace(1) %19
  call addrspace(1) void @N$BDRP(ptr %81)
  %82 = load ptr, ptr addrspace(1) %20
  call addrspace(1) void @N$BDRP(ptr %82)
  br label %b24

b27:
  call addrspace(1) void @N$PS(ptr %18)
  %83 = load ptr, ptr addrspace(1) %17
  call addrspace(1) void @N$PS(ptr %83)
  call addrspace(1) void @N$PN()
  %84 = load ptr, ptr addrspace(1) %17
  call addrspace(1) void @N$BDRP(ptr %84)
  br label %b26

b28:
  %85 = icmp eq i16 %30, 6
  br i1 %85, label %b31, label %b30

b29:
  %86 = load ptr, ptr addrspace(1) %8
  call addrspace(1) void @N$BDRP(ptr %86)
  store ptr null, ptr addrspace(1) %8
  store i16 2, ptr addrspace(1) %0
  br label %b2

b30:
  %87 = icmp eq i16 %30, 7
  br i1 %87, label %b33, label %b32

b31:
  store i16 7, ptr addrspace(1) %0
  %88 = load i16, ptr addrspace(1) %11
  store i8 0, ptr %4, !tbaa !2
  %89 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 %88, ptr %89, !tbaa !2
  %90 = addrspacecast ptr %4 to ptr addrspace(1)
  %91 = load i32, ptr addrspace(1) %90, !tbaa !2
  ret i32 %91

b32:
  %92 = icmp eq i16 %30, 8
  br i1 %92, label %b35, label %b34

b33:
  store i16 5, ptr addrspace(1) %0
  br label %b2

b34:
  store i16 1, ptr addrspace(1) %0
  store i8 1, ptr %4, !tbaa !2
  %93 = addrspacecast ptr %4 to ptr addrspace(1)
  %94 = load i32, ptr addrspace(1) %93, !tbaa !2
  ret i32 %94

b35:
  call addrspace(1) void @N$PS(ptr %29)
  %95 = load ptr, ptr addrspace(1) %8
  call addrspace(1) void @N$PS(ptr %95)
  call addrspace(1) void @N$PN()
  store i16 5, ptr addrspace(1) %0
  br label %b2
}

define internal void @$state1.next(ptr addrspace(1) %0, ptr addrspace(1) %1) addrspace(1) {
b1:
  %2 = alloca [4 x i8]
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 4, i1 false)
  %3 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %4 = getelementptr i8, ptr addrspace(1) %1, i16 2
  %5 = getelementptr i8, ptr addrspace(1) %1, i16 30
  %6 = getelementptr i8, ptr addrspace(1) %1, i16 12
  %7 = getelementptr i8, ptr @$str1, i16 6
  %8 = getelementptr i8, ptr addrspace(1) %1, i16 16
  %9 = getelementptr i8, ptr addrspace(1) %3, i16 2
  %10 = getelementptr i8, ptr addrspace(1) %8, i16 2
  %11 = getelementptr i8, ptr addrspace(1) %3, i16 4
  %12 = getelementptr i8, ptr addrspace(1) %8, i16 4
  %13 = getelementptr i8, ptr addrspace(1) %1, i16 24
  %14 = getelementptr i8, ptr addrspace(1) %1, i16 14
  %15 = getelementptr i8, ptr addrspace(1) %1, i16 26
  %16 = addrspacecast ptr %2 to ptr addrspace(1)
  %17 = getelementptr inbounds i8, ptr %2, i16 2
  %18 = getelementptr i8, ptr addrspace(1) %1, i16 28
  %19 = getelementptr i8, ptr addrspace(1) %1, i16 18
  %20 = getelementptr i8, ptr addrspace(1) %1, i16 20
  br label %b2

b2:
  %21 = load i16, ptr addrspace(1) %1
  %22 = icmp eq i16 %21, 0
  br i1 %22, label %b7, label %b6

b6:
  %23 = icmp eq i16 %21, 2
  br i1 %23, label %b11, label %b10

b7:
  %24 = load ptr, ptr addrspace(1) %4
  store ptr null, ptr addrspace(1) %4
  %25 = load i8, ptr addrspace(1) %5
  %26 = icmp ne i8 %25, 0
  br i1 %26, label %b9, label %b8

b8:
  store ptr %24, ptr addrspace(1) %6
  store i8 -1, ptr addrspace(1) %5
  %27 = load i16, ptr addrspace(1) %3
  store i16 %27, ptr addrspace(1) %8
  %28 = load i16, ptr addrspace(1) %9
  store i16 %28, ptr addrspace(1) %10
  %29 = load ptr addrspace(1), ptr addrspace(1) %11
  store ptr addrspace(1) %29, ptr addrspace(1) %12
  %30 = load ptr, ptr addrspace(1) %13
  call addrspace(1) void @N$BDRP(ptr %30)
  store i16 0, ptr addrspace(1) %14
  store ptr null, ptr addrspace(1) %13
  store i16 0, ptr addrspace(1) %15
  store i16 2, ptr addrspace(1) %1
  br label %b2

b9:
  call addrspace(1) void @N$PS(ptr %7)
  %31 = load ptr, ptr addrspace(1) %6
  call addrspace(1) void @N$PS(ptr %31)
  call addrspace(1) void @N$PN()
  %32 = load ptr, ptr addrspace(1) %6
  call addrspace(1) void @N$BDRP(ptr %32)
  br label %b8

b10:
  %33 = icmp eq i16 %21, 3
  br i1 %33, label %b19, label %b18

b11:
  call addrspace(1) void @$state0.next(ptr addrspace(1) %16, ptr addrspace(1) %14)
  %34 = load i8, ptr %2, !tbaa !2
  %35 = icmp eq i8 %34, 0
  br i1 %35, label %b14, label %b13

b13:
  store i16 4, ptr addrspace(1) %1
  br label %b2

b14:
  %36 = load ptr, ptr %17, !tbaa !2
  %37 = load ptr, ptr addrspace(1) %18
  call addrspace(1) void @N$BDRP(ptr %37)
  store ptr %36, ptr addrspace(1) %18
  store i16 3, ptr addrspace(1) %1
  call addrspace(1) void @N$BDRP(ptr null)
  br label %b2

b18:
  %38 = icmp eq i16 %21, 4
  br i1 %38, label %b21, label %b20

b19:
  store i16 5, ptr addrspace(1) %1
  %39 = load ptr, ptr addrspace(1) %18
  store ptr null, ptr addrspace(1) %18
  store i8 0, ptr addrspace(1) %0
  %40 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr %39, ptr addrspace(1) %40
  ret void

b20:
  %41 = icmp eq i16 %21, 5
  br i1 %41, label %b25, label %b24

b21:
  %42 = load ptr, ptr addrspace(1) %13
  call addrspace(1) void @N$BDRP(ptr %42)
  store i16 0, ptr addrspace(1) %14
  store i16 0, ptr addrspace(1) %8
  store i16 0, ptr addrspace(1) %19
  store ptr addrspace(1) null, ptr addrspace(1) %20
  store ptr null, ptr addrspace(1) %13
  store i16 0, ptr addrspace(1) %15
  %43 = load i8, ptr addrspace(1) %5
  %44 = icmp ne i8 %43, 0
  br i1 %44, label %b23, label %b22

b22:
  store ptr null, ptr addrspace(1) %6
  store i8 0, ptr addrspace(1) %5
  %45 = load ptr, ptr addrspace(1) %4
  call addrspace(1) void @N$BDRP(ptr %45)
  store ptr null, ptr addrspace(1) %4
  store i16 1, ptr addrspace(1) %1
  br label %b2

b23:
  call addrspace(1) void @N$PS(ptr %7)
  %46 = load ptr, ptr addrspace(1) %6
  call addrspace(1) void @N$PS(ptr %46)
  call addrspace(1) void @N$PN()
  %47 = load ptr, ptr addrspace(1) %6
  call addrspace(1) void @N$BDRP(ptr %47)
  br label %b22

b24:
  store i16 1, ptr addrspace(1) %1
  store i8 1, ptr addrspace(1) %0
  ret void

b25:
  %48 = load ptr, ptr addrspace(1) %18
  call addrspace(1) void @N$BDRP(ptr %48)
  store ptr null, ptr addrspace(1) %18
  store i16 2, ptr addrspace(1) %1
  br label %b2
}

define internal void @$state0.next(ptr addrspace(1) %0, ptr addrspace(1) %1) addrspace(1) {
b1:
  %2 = getelementptr i8, ptr addrspace(1) %1, i16 2
  %3 = getelementptr i8, ptr @$str3, i16 6
  %4 = getelementptr i8, ptr addrspace(1) %1, i16 10
  %5 = getelementptr i8, ptr addrspace(1) %1, i16 12
  %6 = getelementptr i8, ptr addrspace(1) %2, i16 4
  br label %b2

b2:
  %7 = load i16, ptr addrspace(1) %1
  %8 = icmp eq i16 %7, 0
  br i1 %8, label %b7, label %b6

b6:
  %9 = icmp eq i16 %7, 2
  br i1 %9, label %b9, label %b8

b7:
  %10 = load ptr, ptr addrspace(1) %4
  call addrspace(1) void @N$BDRP(ptr %10)
  store ptr %3, ptr addrspace(1) %4
  store i16 0, ptr addrspace(1) %5
  store i16 2, ptr addrspace(1) %1
  br label %b2

b8:
  %11 = icmp eq i16 %7, 3
  br i1 %11, label %b14, label %b13

b9:
  %12 = load i16, ptr addrspace(1) %5
  %13 = load i16, ptr addrspace(1) %2
  %14 = icmp ult i16 %12, %13
  br i1 %14, label %b10, label %b11

b10:
  store i16 3, ptr addrspace(1) %1
  br label %b2

b11:
  store i16 5, ptr addrspace(1) %1
  br label %b2

b13:
  %15 = icmp eq i16 %7, 4
  br i1 %15, label %b21, label %b20

b14:
  %16 = load i16, ptr addrspace(1) %5
  %17 = load i16, ptr addrspace(1) %2
  %18 = icmp ult i16 %16, %17
  br i1 %18, label %b15, label %b16

b15:
  %19 = load ptr addrspace(1), ptr addrspace(1) %6
  %20 = getelementptr i8, ptr addrspace(1) %19, i16 %16
  %21 = load i8, ptr addrspace(1) %20
  %22 = icmp eq i8 %21, 32
  br i1 %22, label %b17, label %b18

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
  %23 = icmp eq i16 %7, 5
  br i1 %23, label %b23, label %b22

b21:
  %24 = load i16, ptr addrspace(1) %5
  %25 = add i16 %24, 1
  store i16 %25, ptr addrspace(1) %5
  store i16 2, ptr addrspace(1) %1
  br label %b2

b22:
  %26 = icmp eq i16 %7, 6
  br i1 %26, label %b28, label %b27

b23:
  %27 = load ptr, ptr addrspace(1) %4
  %28 = getelementptr i8, ptr %27, i16 -4
  %29 = load i16, ptr %28
  %30 = icmp ugt i16 %29, 0
  br i1 %30, label %b24, label %b25

b24:
  store i16 13, ptr addrspace(1) %1
  br label %b2

b25:
  store i16 14, ptr addrspace(1) %1
  br label %b2

b27:
  %31 = icmp eq i16 %7, 7
  br i1 %31, label %b33, label %b32

b28:
  %32 = load ptr, ptr addrspace(1) %4
  %33 = getelementptr i8, ptr %32, i16 -4
  %34 = load i16, ptr %33
  %35 = icmp ugt i16 %34, 0
  br i1 %35, label %b29, label %b30

b29:
  store i16 9, ptr addrspace(1) %1
  br label %b2

b30:
  store i16 10, ptr addrspace(1) %1
  br label %b2

b32:
  %36 = icmp eq i16 %7, 8
  br i1 %36, label %b37, label %b36

b33:
  %37 = load ptr, ptr addrspace(1) %4
  %38 = getelementptr i8, ptr %37, i16 -4
  %39 = load i16, ptr %38
  %40 = call addrspace(1) ptr @N$BGRW(ptr %37, i16 1, i16 1)
  store ptr %40, ptr addrspace(1) %4
  %41 = getelementptr i8, ptr %40, i16 %39
  %42 = load i16, ptr addrspace(1) %5
  %43 = load i16, ptr addrspace(1) %2
  %44 = icmp ult i16 %42, %43
  br i1 %44, label %b34, label %b35

b34:
  %45 = load ptr addrspace(1), ptr addrspace(1) %6
  %46 = getelementptr i8, ptr addrspace(1) %45, i16 %42
  %47 = load i8, ptr addrspace(1) %46
  store i8 %47, ptr %41
  %48 = getelementptr i8, ptr %40, i16 -4
  %49 = load i16, ptr %48
  %50 = getelementptr i8, ptr %40, i16 %49
  store i8 0, ptr %50
  store i16 8, ptr addrspace(1) %1
  br label %b2

b35:
  call addrspace(1) void @N$EBND()
  unreachable

b36:
  %51 = icmp eq i16 %7, 9
  br i1 %51, label %b39, label %b38

b37:
  store i16 4, ptr addrspace(1) %1
  br label %b2

b38:
  %52 = icmp eq i16 %7, 10
  br i1 %52, label %b41, label %b40

b39:
  store i16 12, ptr addrspace(1) %1
  %53 = load ptr, ptr addrspace(1) %4
  store ptr null, ptr addrspace(1) %4
  store i8 0, ptr addrspace(1) %0
  %54 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr %53, ptr addrspace(1) %54
  ret void

b40:
  %55 = icmp eq i16 %7, 11
  br i1 %55, label %b43, label %b42

b41:
  store i16 11, ptr addrspace(1) %1
  br label %b2

b42:
  %56 = icmp eq i16 %7, 12
  br i1 %56, label %b45, label %b44

b43:
  store i16 8, ptr addrspace(1) %1
  br label %b2

b44:
  %57 = icmp eq i16 %7, 13
  br i1 %57, label %b47, label %b46

b45:
  %58 = load ptr, ptr addrspace(1) %4
  call addrspace(1) void @N$BDRP(ptr %58)
  store ptr %3, ptr addrspace(1) %4
  store i16 11, ptr addrspace(1) %1
  br label %b2

b46:
  %59 = icmp eq i16 %7, 14
  br i1 %59, label %b49, label %b48

b47:
  store i16 16, ptr addrspace(1) %1
  %60 = load ptr, ptr addrspace(1) %4
  store ptr null, ptr addrspace(1) %4
  store i8 0, ptr addrspace(1) %0
  %61 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr %60, ptr addrspace(1) %61
  ret void

b48:
  %62 = icmp eq i16 %7, 15
  br i1 %62, label %b51, label %b50

b49:
  store i16 15, ptr addrspace(1) %1
  br label %b2

b50:
  %63 = icmp eq i16 %7, 16
  br i1 %63, label %b53, label %b52

b51:
  %64 = load ptr, ptr addrspace(1) %4
  call addrspace(1) void @N$BDRP(ptr %64)
  store ptr null, ptr addrspace(1) %4
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
