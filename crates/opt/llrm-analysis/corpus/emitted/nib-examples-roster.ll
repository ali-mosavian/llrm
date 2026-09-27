target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [9 x i8] c"\08\00\02\00\02\00 (\00"
@$str2 = internal constant [8 x i8] c"\08\00\01\00\01\00)\00"
@$str3 = internal constant [13 x i8] c"\08\00\06\00\06\00master\00"
@$str4 = internal constant [13 x i8] c"\08\00\06\00\06\00expert\00"
@$str5 = internal constant [11 x i8] c"\08\00\04\00\04\00club\00"
@$str6 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str7 = internal constant [10 x i8] c"\08\00\03\00\03\00ada\00"
@$str8 = internal constant [10 x i8] c"\08\00\03\00\03\00bob\00"
@$str9 = internal constant [9 x i8] c"\08\00\02\00\02\00cy\00"
@$str10 = internal constant [10 x i8] c"\08\00\03\00\03\00dee\00"
@$str11 = internal constant [17 x i8] c"\08\00\0A\00\0A\00signed dee\00"
@$str12 = internal constant [22 x i8] c"\08\00\0F\00\0F\00refused rating \00"
@$str13 = internal constant [9 x i8] c"\08\00\02\00\02\00 *\00"
@$str14 = internal constant [8 x i8] c"\08\00\01\00\01\00 \00"
@$str15 = internal constant [26 x i8] c"\08\00\13\00\13\00 above 1500, first \00"
@$str16 = internal constant [11 x i8] c"\08\00\04\00\04\00 at \00"

define internal void @Player.new(ptr addrspace(1) %0, ptr addrspace(1) noalias readonly dereferenceable(8) %1, i16 %2) addrspace(1) {
b1:
  %3 = icmp slt i16 %2, 0
  %4 = sext i1 %3 to i8
  %5 = icmp ne i8 %4, 0
  br i1 %5, label %b2, label %b3

b2:
  store i8 1, ptr addrspace(1) %0
  %6 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 0, ptr addrspace(1) %6
  %7 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %2, ptr addrspace(1) %7
  ret void

b3:
  br label %b4

b4:
  %8 = call addrspace(1) ptr @N$VCPY(ptr addrspace(1) %1)
  store i8 0, ptr addrspace(1) %0
  %9 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr %8, ptr addrspace(1) %9
  %10 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %2, ptr addrspace(1) %10
  ret void
}

define internal ptr @Player.display(ptr addrspace(1) %0) addrspace(1) {
b1:
  call addrspace(1) void @N$PBEG()
  %1 = load ptr, ptr addrspace(1) %0
  call addrspace(1) void @N$PS(ptr %1)
  %2 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %2)
  %3 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %4 = load i16, ptr addrspace(1) %3
  call addrspace(1) void @N$PI2(i16 %4)
  %5 = getelementptr i8, ptr @$str2, i16 6
  call addrspace(1) void @N$PS(ptr %5)
  %6 = call addrspace(1) ptr @N$PEND()
  ret ptr %6
}

define internal ptr @Player.grade(ptr addrspace(1) %0) addrspace(1) {
b1:
  %1 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %2 = load i16, ptr addrspace(1) %1
  %3 = icmp sge i16 %2, 2000
  %4 = sext i1 %3 to i8
  %5 = icmp ne i8 %4, 0
  br i1 %5, label %b2, label %b3

b2:
  %6 = getelementptr i8, ptr @$str3, i16 6
  ret ptr %6

b3:
  %7 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %8 = load i16, ptr addrspace(1) %7
  %9 = icmp sge i16 %8, 1500
  %10 = sext i1 %9 to i8
  %11 = icmp ne i8 %10, 0
  br i1 %11, label %b5, label %b6

b5:
  %12 = getelementptr i8, ptr @$str4, i16 6
  ret ptr %12

b6:
  %13 = getelementptr i8, ptr @$str5, i16 6
  ret ptr %13
}

define internal ptr addrspace(1) @best(ptr addrspace(1) %0) addrspace(1) {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  store i16 0, ptr %1
  store i16 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %3, !tbaa !2
  %4 = load ptr, ptr addrspace(1) %0
  %5 = getelementptr i8, ptr %4, i16 -4
  %6 = load i16, ptr %5
  store i16 0, ptr %2, !tbaa !2
  store i16 %6, ptr %1, !tbaa !2
  br label %b2

b2:
  %7 = load i16, ptr %2, !tbaa !2
  %8 = load i16, ptr %1, !tbaa !2
  %9 = icmp ult i16 %7, %8
  %10 = sext i1 %9 to i8
  %11 = icmp ne i8 %10, 0
  br i1 %11, label %b3, label %b5

b3:
  %12 = load ptr, ptr addrspace(1) %0
  %13 = load i16, ptr %2, !tbaa !2
  %14 = getelementptr i8, ptr %12, i16 -4
  %15 = load i16, ptr %14
  %16 = icmp ult i16 %13, %15
  %17 = sext i1 %16 to i8
  %18 = icmp ne i8 %17, 0
  br i1 %18, label %b6, label %b7

b4:
  %19 = load i16, ptr %2, !tbaa !2
  %20 = add i16 %19, 1
  store i16 %20, ptr %2, !tbaa !2
  br label %b2

b5:
  %21 = load ptr, ptr addrspace(1) %0
  %22 = load i16, ptr %3, !tbaa !2
  %23 = getelementptr i8, ptr %21, i16 -4
  %24 = load i16, ptr %23
  %25 = icmp ult i16 %22, %24
  %26 = sext i1 %25 to i8
  %27 = icmp ne i8 %26, 0
  br i1 %27, label %b13, label %b14

b6:
  %28 = mul i16 %13, 4
  %29 = getelementptr i8, ptr %12, i16 %28
  %30 = getelementptr i8, ptr %29, i16 2
  %31 = load i16, ptr %30
  %32 = load ptr, ptr addrspace(1) %0
  %33 = load i16, ptr %3, !tbaa !2
  %34 = getelementptr i8, ptr %32, i16 -4
  %35 = load i16, ptr %34
  %36 = icmp ult i16 %33, %35
  %37 = sext i1 %36 to i8
  %38 = icmp ne i8 %37, 0
  br i1 %38, label %b8, label %b9

b7:
  call addrspace(1) void @N$EBND()
  unreachable

b8:
  %39 = mul i16 %33, 4
  %40 = getelementptr i8, ptr %32, i16 %39
  %41 = getelementptr i8, ptr %40, i16 2
  %42 = load i16, ptr %41
  %43 = icmp sgt i16 %31, %42
  %44 = sext i1 %43 to i8
  %45 = icmp ne i8 %44, 0
  br i1 %45, label %b10, label %b11

b9:
  call addrspace(1) void @N$EBND()
  unreachable

b10:
  %46 = load i16, ptr %2, !tbaa !2
  store i16 %46, ptr %3, !tbaa !2
  br label %b12

b11:
  br label %b12

b12:
  br label %b4

b13:
  %47 = mul i16 %22, 4
  %48 = getelementptr i8, ptr %21, i16 %47
  %49 = addrspacecast ptr %48 to ptr addrspace(1)
  ret ptr addrspace(1) %49

b14:
  call addrspace(1) void @N$EBND()
  unreachable
}

define internal void @sign_up(ptr addrspace(1) %0, ptr addrspace(1) %1, ptr addrspace(1) noalias readonly dereferenceable(8) %2, i16 %3) addrspace(1) {
b1:
  %4 = alloca [6 x i8]
  %5 = alloca [4 x i8]
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 6, i1 false)
  call void @llvm.memset.p0.i16(ptr %5, i8 0, i16 4, i1 false)
  %6 = addrspacecast ptr %4 to ptr addrspace(1)
  call addrspace(1) void @Player.new(ptr addrspace(1) %6, ptr addrspace(1) %2, i16 %3)
  %7 = load i8, ptr %4, !tbaa !2
  %8 = icmp eq i8 %7, 1
  %9 = sext i1 %8 to i8
  %10 = icmp ne i8 %9, 0
  br i1 %10, label %b2, label %b3

b2:
  %11 = getelementptr inbounds i8, ptr %4, i16 2
  %12 = load i16, ptr %11, !tbaa !2
  %13 = getelementptr inbounds i8, ptr %4, i16 4
  %14 = load i16, ptr %13, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %15 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %12, ptr addrspace(1) %15
  %16 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %14, ptr addrspace(1) %16
  ret void

b3:
  %17 = getelementptr inbounds i8, ptr %4, i16 2
  %18 = load ptr, ptr %17, !tbaa !2
  %19 = getelementptr inbounds i8, ptr %4, i16 4
  %20 = load i16, ptr %19, !tbaa !2
  store ptr %18, ptr %5, !tbaa !2
  %21 = getelementptr inbounds i8, ptr %5, i16 2
  store i16 %20, ptr %21, !tbaa !2
  %22 = load ptr, ptr addrspace(1) %1
  %23 = getelementptr i8, ptr %22, i16 -4
  %24 = load i16, ptr %23
  %25 = call addrspace(1) ptr @N$BGRW(ptr %22, i16 1, i16 4)
  store ptr %25, ptr addrspace(1) %1
  %26 = mul i16 %24, 4
  %27 = getelementptr i8, ptr %25, i16 %26
  %28 = load ptr, ptr %5, !tbaa !2
  %29 = getelementptr inbounds i8, ptr %5, i16 2
  %30 = load i16, ptr %29, !tbaa !2
  store ptr %28, ptr %27
  %31 = getelementptr i8, ptr %27, i16 2
  store i16 %30, ptr %31
  store ptr null, ptr %5, !tbaa !2
  %32 = getelementptr inbounds i8, ptr %5, i16 2
  store i16 0, ptr %32, !tbaa !2
  store i8 0, ptr addrspace(1) %0
  %33 = load ptr, ptr %5, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %33)
  ret void
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca i16
  %1 = alloca ptr
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca ptr
  %5 = alloca ptr
  %6 = alloca ptr
  %7 = alloca ptr
  %8 = alloca ptr
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca [8 x i8]
  %12 = alloca [6 x i8]
  %13 = alloca i16
  %14 = alloca [8 x i8]
  %15 = alloca [6 x i8]
  %16 = alloca i16
  %17 = alloca ptr
  %18 = alloca ptr
  store i16 0, ptr %0
  store ptr null, ptr %1
  store i16 0, ptr %2
  store i16 0, ptr %3
  store ptr null, ptr %4
  store ptr null, ptr %5
  store ptr null, ptr %6
  store ptr null, ptr %7
  store ptr null, ptr %8
  store i16 0, ptr %9
  store i16 0, ptr %10
  call void @llvm.memset.p0.i16(ptr %11, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %12, i8 0, i16 6, i1 false)
  store i16 0, ptr %13
  call void @llvm.memset.p0.i16(ptr %14, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %15, i8 0, i16 6, i1 false)
  store i16 0, ptr %16
  store ptr null, ptr %17
  store ptr null, ptr %18
  %19 = getelementptr i8, ptr @$str6, i16 6
  store ptr %19, ptr %18, !tbaa !2
  %20 = getelementptr i8, ptr @$str6, i16 6
  %21 = call addrspace(1) ptr @N$BGRW(ptr %20, i16 3, i16 4)
  %22 = getelementptr i8, ptr %21, i16 0
  %23 = getelementptr i8, ptr @$str7, i16 6
  store ptr %23, ptr %22
  %24 = getelementptr i8, ptr %22, i16 2
  store i16 2150, ptr %24
  %25 = getelementptr i8, ptr %21, i16 4
  %26 = getelementptr i8, ptr @$str8, i16 6
  store ptr %26, ptr %25
  %27 = getelementptr i8, ptr %25, i16 2
  store i16 1480, ptr %27
  %28 = getelementptr i8, ptr %21, i16 8
  %29 = getelementptr i8, ptr @$str9, i16 6
  store ptr %29, ptr %28
  %30 = getelementptr i8, ptr %28, i16 2
  store i16 1620, ptr %30
  store ptr %21, ptr %17, !tbaa !2
  %31 = load ptr, ptr %17, !tbaa !2
  %32 = getelementptr i8, ptr %31, i16 -4
  %33 = load i16, ptr %32
  store i16 0, ptr %16, !tbaa !2
  br label %b2

b2:
  %34 = load i16, ptr %16, !tbaa !2
  %35 = icmp ult i16 %34, %33
  %36 = sext i1 %35 to i8
  %37 = icmp ne i8 %36, 0
  br i1 %37, label %b3, label %b5

b3:
  %38 = mul i16 %34, 4
  %39 = getelementptr i8, ptr %31, i16 %38
  %40 = load ptr, ptr %39
  %41 = getelementptr i8, ptr %39, i16 2
  %42 = load i16, ptr %41
  %43 = getelementptr i8, ptr %39, i16 2
  %44 = addrspacecast ptr %43 to ptr addrspace(1)
  %45 = addrspacecast ptr %15 to ptr addrspace(1)
  %46 = addrspacecast ptr %18 to ptr addrspace(1)
  %47 = load ptr, ptr %39
  %48 = getelementptr i8, ptr %47, i16 -4
  %49 = load i16, ptr %48
  %50 = addrspacecast ptr %47 to ptr addrspace(1)
  store i16 %49, ptr %14, !tbaa !2
  %51 = getelementptr inbounds i8, ptr %14, i16 2
  store i16 %49, ptr %51, !tbaa !2
  %52 = getelementptr inbounds i8, ptr %14, i16 4
  store ptr addrspace(1) %50, ptr %52, !tbaa !2
  %53 = addrspacecast ptr %14 to ptr addrspace(1)
  %54 = load i16, ptr addrspace(1) %44
  call addrspace(1) void @sign_up(ptr addrspace(1) %45, ptr addrspace(1) %46, ptr addrspace(1) %53, i16 %54)
  br label %b4

b4:
  %55 = load i16, ptr %16, !tbaa !2
  %56 = add i16 %55, 1
  store i16 %56, ptr %16, !tbaa !2
  br label %b2

b5:
  %57 = load ptr, ptr %17, !tbaa !2
  %58 = icmp ne ptr %57, null
  %59 = sext i1 %58 to i8
  %60 = icmp ne i8 %59, 0
  br i1 %60, label %b7, label %b6

b6:
  call addrspace(1) void @N$BDRP(ptr %57)
  %61 = addrspacecast ptr %12 to ptr addrspace(1)
  %62 = addrspacecast ptr %18 to ptr addrspace(1)
  %63 = getelementptr i8, ptr @$str10, i16 6
  %64 = getelementptr i8, ptr %63, i16 -4
  %65 = load i16, ptr %64
  %66 = addrspacecast ptr %63 to ptr addrspace(1)
  store i16 %65, ptr %11, !tbaa !2
  %67 = getelementptr inbounds i8, ptr %11, i16 2
  store i16 %65, ptr %67, !tbaa !2
  %68 = getelementptr inbounds i8, ptr %11, i16 4
  store ptr addrspace(1) %66, ptr %68, !tbaa !2
  %69 = addrspacecast ptr %11 to ptr addrspace(1)
  call addrspace(1) void @sign_up(ptr addrspace(1) %61, ptr addrspace(1) %62, ptr addrspace(1) %69, i16 -3)
  %70 = load i8, ptr %12, !tbaa !2
  %71 = icmp eq i8 %70, 0
  %72 = sext i1 %71 to i8
  %73 = icmp ne i8 %72, 0
  br i1 %73, label %b13, label %b12

b7:
  %74 = getelementptr i8, ptr %57, i16 -4
  %75 = load i16, ptr %74
  store i16 0, ptr %13, !tbaa !2
  br label %b8

b8:
  %76 = load i16, ptr %13, !tbaa !2
  %77 = icmp ult i16 %76, %75
  %78 = sext i1 %77 to i8
  %79 = icmp ne i8 %78, 0
  br i1 %79, label %b10, label %b9

b9:
  br label %b6

b10:
  %80 = mul i16 %76, 4
  %81 = getelementptr i8, ptr %57, i16 %80
  %82 = load ptr, ptr %81
  call addrspace(1) void @N$BDRP(ptr %82)
  %83 = add i16 %76, 1
  store i16 %83, ptr %13, !tbaa !2
  br label %b8

b11:
  %84 = load ptr, ptr %18, !tbaa !2
  %85 = getelementptr i8, ptr %84, i16 -4
  %86 = load i16, ptr %85
  %87 = icmp ult i16 1, %86
  %88 = sext i1 %87 to i8
  %89 = icmp ne i8 %88, 0
  br i1 %89, label %b15, label %b16

b12:
  %90 = getelementptr inbounds i8, ptr %12, i16 4
  %91 = load i16, ptr %90, !tbaa !2
  store i16 %91, ptr %10, !tbaa !2
  %92 = getelementptr i8, ptr @$str12, i16 6
  call addrspace(1) void @N$PS(ptr %92)
  %93 = load i16, ptr %10, !tbaa !2
  call addrspace(1) void @N$PI2(i16 %93)
  call addrspace(1) void @N$PN()
  br label %b11

b13:
  %94 = getelementptr i8, ptr @$str11, i16 6
  call addrspace(1) void @N$PS(ptr %94)
  call addrspace(1) void @N$PN()
  br label %b11

b15:
  %95 = getelementptr i8, ptr %84, i16 4
  %96 = getelementptr i8, ptr %95, i16 2
  %97 = load i16, ptr %96
  %98 = add i16 %97, 25
  %99 = getelementptr i8, ptr %95, i16 2
  store i16 %98, ptr %99
  %100 = addrspacecast ptr %18 to ptr addrspace(1)
  %101 = call addrspace(1) ptr addrspace(1) @best(ptr addrspace(1) %100)
  %102 = load ptr, ptr %18, !tbaa !2
  %103 = getelementptr i8, ptr %102, i16 -4
  %104 = load i16, ptr %103
  store i16 0, ptr %9, !tbaa !2
  br label %b17

b16:
  call addrspace(1) void @N$EBND()
  unreachable

b17:
  %105 = load i16, ptr %9, !tbaa !2
  %106 = icmp ult i16 %105, %104
  %107 = sext i1 %106 to i8
  %108 = icmp ne i8 %107, 0
  br i1 %108, label %b18, label %b20

b18:
  %109 = mul i16 %105, 4
  %110 = getelementptr i8, ptr %102, i16 %109
  %111 = addrspacecast ptr %110 to ptr addrspace(1)
  %112 = icmp eq ptr addrspace(1) %111, %101
  %113 = sext i1 %112 to i8
  %114 = icmp ne i8 %113, 0
  br i1 %114, label %b21, label %b22

b19:
  %115 = load i16, ptr %9, !tbaa !2
  %116 = add i16 %115, 1
  store i16 %116, ptr %9, !tbaa !2
  br label %b17

b20:
  %117 = getelementptr i8, ptr @$str6, i16 6
  store ptr %117, ptr %4, !tbaa !2
  %118 = load ptr, ptr %18, !tbaa !2
  %119 = getelementptr i8, ptr %118, i16 -4
  %120 = load i16, ptr %119
  store i16 0, ptr %3, !tbaa !2
  store i16 %120, ptr %2, !tbaa !2
  br label %b24

b21:
  %121 = getelementptr i8, ptr @$str13, i16 6
  store ptr %121, ptr %8, !tbaa !2
  br label %b23

b22:
  %122 = getelementptr i8, ptr @$str6, i16 6
  store ptr %122, ptr %8, !tbaa !2
  br label %b23

b23:
  %123 = load ptr, ptr %8, !tbaa !2
  store ptr %123, ptr %7, !tbaa !2
  %124 = addrspacecast ptr %110 to ptr addrspace(1)
  %125 = call addrspace(1) ptr @Player.display(ptr addrspace(1) %124)
  store ptr %125, ptr %6, !tbaa !2
  %126 = addrspacecast ptr %110 to ptr addrspace(1)
  %127 = call addrspace(1) ptr @Player.grade(ptr addrspace(1) %126)
  store ptr %127, ptr %5, !tbaa !2
  %128 = load ptr, ptr %6, !tbaa !2
  call addrspace(1) void @N$PS(ptr %128)
  %129 = getelementptr i8, ptr @$str14, i16 6
  call addrspace(1) void @N$PS(ptr %129)
  %130 = load ptr, ptr %5, !tbaa !2
  call addrspace(1) void @N$PS(ptr %130)
  %131 = load ptr, ptr %7, !tbaa !2
  call addrspace(1) void @N$PS(ptr %131)
  call addrspace(1) void @N$PN()
  %132 = load ptr, ptr %5, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %132)
  %133 = load ptr, ptr %6, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %133)
  %134 = load ptr, ptr %7, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %134)
  br label %b19

b24:
  %135 = load i16, ptr %3, !tbaa !2
  %136 = load i16, ptr %2, !tbaa !2
  %137 = icmp ult i16 %135, %136
  %138 = sext i1 %137 to i8
  %139 = icmp ne i8 %138, 0
  br i1 %139, label %b25, label %b27

b25:
  %140 = load ptr, ptr %18, !tbaa !2
  %141 = load i16, ptr %3, !tbaa !2
  %142 = getelementptr i8, ptr %140, i16 -4
  %143 = load i16, ptr %142
  %144 = icmp ult i16 %141, %143
  %145 = sext i1 %144 to i8
  %146 = icmp ne i8 %145, 0
  br i1 %146, label %b28, label %b29

b26:
  %147 = load i16, ptr %3, !tbaa !2
  %148 = add i16 %147, 1
  store i16 %148, ptr %3, !tbaa !2
  br label %b24

b27:
  %149 = load ptr, ptr %4, !tbaa !2
  store ptr null, ptr %4, !tbaa !2
  store ptr %149, ptr %1, !tbaa !2
  %150 = load ptr, ptr %1, !tbaa !2
  %151 = getelementptr i8, ptr %150, i16 -4
  %152 = load i16, ptr %151
  %153 = icmp ult i16 0, %152
  %154 = sext i1 %153 to i8
  %155 = icmp ne i8 %154, 0
  br i1 %155, label %b35, label %b36

b28:
  %156 = mul i16 %141, 4
  %157 = getelementptr i8, ptr %140, i16 %156
  %158 = getelementptr i8, ptr %157, i16 2
  %159 = load i16, ptr %158
  %160 = icmp sgt i16 %159, 1500
  %161 = sext i1 %160 to i8
  %162 = icmp ne i8 %161, 0
  br i1 %162, label %b30, label %b31

b29:
  call addrspace(1) void @N$EBND()
  unreachable

b30:
  %163 = load ptr, ptr %4, !tbaa !2
  %164 = getelementptr i8, ptr %163, i16 -4
  %165 = load i16, ptr %164
  %166 = call addrspace(1) ptr @N$BGRW(ptr %163, i16 1, i16 4)
  store ptr %166, ptr %4, !tbaa !2
  %167 = mul i16 %165, 4
  %168 = getelementptr i8, ptr %166, i16 %167
  %169 = load ptr, ptr %18, !tbaa !2
  %170 = load i16, ptr %3, !tbaa !2
  %171 = getelementptr i8, ptr %169, i16 -4
  %172 = load i16, ptr %171
  %173 = icmp ult i16 %170, %172
  %174 = sext i1 %173 to i8
  %175 = icmp ne i8 %174, 0
  br i1 %175, label %b33, label %b34

b31:
  br label %b32

b32:
  br label %b26

b33:
  %176 = mul i16 %170, 4
  %177 = getelementptr i8, ptr %169, i16 %176
  %178 = getelementptr i8, ptr %177, i16 2
  %179 = load i16, ptr %178
  %180 = load i16, ptr %3, !tbaa !2
  store i16 %179, ptr %168
  %181 = getelementptr i8, ptr %168, i16 2
  store i16 %180, ptr %181
  br label %b32

b34:
  call addrspace(1) void @N$EBND()
  unreachable

b35:
  %182 = getelementptr i8, ptr %150, i16 0
  %183 = load i16, ptr %182
  %184 = getelementptr i8, ptr %182, i16 2
  %185 = load i16, ptr %184
  %186 = getelementptr i8, ptr %182, i16 2
  %187 = addrspacecast ptr %186 to ptr addrspace(1)
  %188 = load ptr, ptr %1, !tbaa !2
  %189 = getelementptr i8, ptr %188, i16 -4
  %190 = load i16, ptr %189
  call addrspace(1) void @N$PU2(i16 %190)
  %191 = getelementptr i8, ptr @$str15, i16 6
  call addrspace(1) void @N$PS(ptr %191)
  %192 = load ptr, ptr %18, !tbaa !2
  %193 = load i16, ptr addrspace(1) %187
  %194 = getelementptr i8, ptr %192, i16 -4
  %195 = load i16, ptr %194
  %196 = icmp ult i16 %193, %195
  %197 = sext i1 %196 to i8
  %198 = icmp ne i8 %197, 0
  br i1 %198, label %b37, label %b38

b36:
  call addrspace(1) void @N$EBND()
  unreachable

b37:
  %199 = mul i16 %193, 4
  %200 = getelementptr i8, ptr %192, i16 %199
  %201 = load ptr, ptr %200
  call addrspace(1) void @N$PS(ptr %201)
  %202 = getelementptr i8, ptr @$str16, i16 6
  call addrspace(1) void @N$PS(ptr %202)
  %203 = load i16, ptr %182
  call addrspace(1) void @N$PI2(i16 %203)
  call addrspace(1) void @N$PN()
  %204 = load ptr, ptr %1, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %204)
  %205 = load ptr, ptr %4, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %205)
  %206 = load ptr, ptr %18, !tbaa !2
  %207 = icmp ne ptr %206, null
  %208 = sext i1 %207 to i8
  %209 = icmp ne i8 %208, 0
  br i1 %209, label %b40, label %b39

b38:
  call addrspace(1) void @N$EBND()
  unreachable

b39:
  call addrspace(1) void @N$BDRP(ptr %206)
  ret i16 0

b40:
  %210 = getelementptr i8, ptr %206, i16 -4
  %211 = load i16, ptr %210
  store i16 0, ptr %0, !tbaa !2
  br label %b41

b41:
  %212 = load i16, ptr %0, !tbaa !2
  %213 = icmp ult i16 %212, %211
  %214 = sext i1 %213 to i8
  %215 = icmp ne i8 %214, 0
  br i1 %215, label %b43, label %b42

b42:
  br label %b39

b43:
  %216 = mul i16 %212, 4
  %217 = getelementptr i8, ptr %206, i16 %216
  %218 = load ptr, ptr %217
  call addrspace(1) void @N$BDRP(ptr %218)
  %219 = add i16 %212, 1
  store i16 %219, ptr %0, !tbaa !2
  br label %b41
}

declare ptr @N$VCPY(ptr addrspace(1)) addrspace(1)

declare void @N$PBEG() addrspace(1)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PI2(i16) addrspace(1)

declare ptr @N$PEND() addrspace(1)

declare void @N$EBND() addrspace(1)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare void @N$BDRP(ptr) addrspace(1)

declare void @N$PN() addrspace(1)

declare void @N$PU2(i16) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
