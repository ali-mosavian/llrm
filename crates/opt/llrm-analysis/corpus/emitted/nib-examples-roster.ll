target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

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

define internal cc 1005 void @Player.new(ptr addrspace(1) %0, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %1, i16 %2) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %3 = icmp slt i16 %2, 0
  br i1 %3, label %b2, label %b3

b2:
  store i8 1, ptr addrspace(1) %0
  %4 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 0, ptr addrspace(1) %4
  %5 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %2, ptr addrspace(1) %5
  ret void

b3:
  br label %b4

b4:
  %6 = call cc 1005 addrspace(1) ptr @N$VCPY(ptr addrspace(1) %1)
  store i8 0, ptr addrspace(1) %0
  %7 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr %6, ptr addrspace(1) %7
  %8 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %2, ptr addrspace(1) %8
  ret void
}

define internal cc 1005 ptr @Player.display(ptr addrspace(1) nonnull dereferenceable(4) readonly noalias nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  call cc 1005 addrspace(1) void @N$PBEG()
  %1 = load ptr, ptr addrspace(1) %0
  call cc 1005 addrspace(1) void @N$PS(ptr %1)
  %2 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %2)
  %3 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %4 = load i16, ptr addrspace(1) %3
  call cc 1005 addrspace(1) void @N$PI2(i16 %4)
  %5 = getelementptr i8, ptr @$str2, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %5)
  %6 = call cc 1005 addrspace(1) ptr @N$PEND()
  ret ptr %6
}

define internal cc 1005 ptr @Player.grade(ptr addrspace(1) nonnull dereferenceable(4) readonly noalias nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %2 = load i16, ptr addrspace(1) %1
  %3 = icmp sge i16 %2, 2000
  br i1 %3, label %b2, label %b3

b2:
  %4 = getelementptr i8, ptr @$str3, i16 6
  ret ptr %4

b3:
  %5 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %6 = load i16, ptr addrspace(1) %5
  %7 = icmp sge i16 %6, 1500
  br i1 %7, label %b5, label %b6

b5:
  %8 = getelementptr i8, ptr @$str4, i16 6
  ret ptr %8

b6:
  %9 = getelementptr i8, ptr @$str5, i16 6
  ret ptr %9
}

define internal cc 1005 ptr addrspace(1) @best(ptr addrspace(1) %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
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
  br i1 %9, label %b3, label %b5

b3:
  %10 = load ptr, ptr addrspace(1) %0
  %11 = load i16, ptr %2, !tbaa !2
  %12 = getelementptr i8, ptr %10, i16 -4
  %13 = load i16, ptr %12
  %14 = icmp ult i16 %11, %13
  br i1 %14, label %b9, label %b10

b4:
  %15 = load i16, ptr %2, !tbaa !2
  %16 = add nuw i16 %15, 1
  store i16 %16, ptr %2, !tbaa !2
  br label %b2

b5:
  %17 = load ptr, ptr addrspace(1) %0
  %18 = load i16, ptr %3, !tbaa !2
  %19 = getelementptr i8, ptr %17, i16 -4
  %20 = load i16, ptr %19
  %21 = icmp ult i16 %18, %20
  br i1 %21, label %b13, label %b14

b6:
  %22 = load i16, ptr %2, !tbaa !2
  store i16 %22, ptr %3, !tbaa !2
  br label %b8

b7:
  br label %b8

b8:
  br label %b4

b9:
  %23 = mul i16 %11, 4
  %24 = getelementptr inbounds i8, ptr %10, i16 %23
  %25 = getelementptr i8, ptr %24, i16 2
  %26 = load i16, ptr %25
  %27 = load ptr, ptr addrspace(1) %0
  %28 = load i16, ptr %3, !tbaa !2
  %29 = getelementptr i8, ptr %27, i16 -4
  %30 = load i16, ptr %29
  %31 = icmp ult i16 %28, %30
  br i1 %31, label %b11, label %b12

b10:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b11:
  %32 = mul i16 %28, 4
  %33 = getelementptr inbounds i8, ptr %27, i16 %32
  %34 = getelementptr i8, ptr %33, i16 2
  %35 = load i16, ptr %34
  %36 = icmp sgt i16 %26, %35
  br i1 %36, label %b6, label %b7

b12:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b13:
  %37 = mul i16 %18, 4
  %38 = getelementptr inbounds i8, ptr %17, i16 %37
  %39 = addrspacecast ptr %38 to ptr addrspace(1)
  ret ptr addrspace(1) %39

b14:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

define internal cc 1005 void @sign_up(ptr addrspace(1) %0, ptr addrspace(1) nocapture %1, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %2, i16 %3) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %4 = alloca [6 x i8]
  %5 = alloca [4 x i8]
  %6 = addrspacecast ptr %4 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Player.new(ptr addrspace(1) %6, ptr addrspace(1) %2, i16 %3)
  %7 = load i8, ptr %4, !tbaa !2, !range !5
  %8 = icmp eq i8 %7, 1
  br i1 %8, label %b2, label %b3

b2:
  store i8 1, ptr addrspace(1) %0
  %9 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %10 = getelementptr inbounds i8, ptr %4, i16 2
  call void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) %9, ptr %10, i16 4, i1 false)
  ret void

b3:
  %11 = getelementptr inbounds i8, ptr %4, i16 2
  %12 = load ptr, ptr %11, !tbaa !2
  %13 = getelementptr inbounds i8, ptr %4, i16 4
  %14 = load i16, ptr %13, !tbaa !2
  store ptr %12, ptr %5, !tbaa !2
  %15 = getelementptr inbounds i8, ptr %5, i16 2
  store i16 %14, ptr %15, !tbaa !2
  %16 = load ptr, ptr addrspace(1) %1
  %17 = getelementptr i8, ptr %16, i16 -4
  %18 = load i16, ptr %17
  %19 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %16, i16 1, i16 4)
  store ptr %19, ptr addrspace(1) %1
  %20 = mul i16 %18, 4
  %21 = getelementptr inbounds i8, ptr %19, i16 %20
  %22 = load ptr, ptr %5, !tbaa !2
  %23 = getelementptr inbounds i8, ptr %5, i16 2
  %24 = load i16, ptr %23, !tbaa !2
  store ptr %22, ptr %21
  %25 = getelementptr i8, ptr %21, i16 2
  store i16 %24, ptr %25
  store ptr null, ptr %5, !tbaa !2
  %26 = getelementptr inbounds i8, ptr %5, i16 2
  store i16 0, ptr %26, !tbaa !2
  store i8 0, ptr addrspace(1) %0
  %27 = load ptr, ptr %5, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %27)
  ret void
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
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
  %19 = getelementptr i8, ptr @$str6, i16 6
  store ptr %19, ptr %18, !tbaa !2
  %20 = getelementptr i8, ptr @$str6, i16 6
  %21 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %20, i16 3, i16 4)
  %22 = getelementptr inbounds i8, ptr %21, i16 0
  %23 = getelementptr i8, ptr @$str7, i16 6
  store ptr %23, ptr %22
  %24 = getelementptr i8, ptr %22, i16 2
  store i16 2150, ptr %24
  %25 = getelementptr inbounds i8, ptr %21, i16 4
  %26 = getelementptr i8, ptr @$str8, i16 6
  store ptr %26, ptr %25
  %27 = getelementptr i8, ptr %25, i16 2
  store i16 1480, ptr %27
  %28 = getelementptr inbounds i8, ptr %21, i16 8
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
  br i1 %35, label %b3, label %b5

b3:
  %36 = mul i16 %34, 4
  %37 = getelementptr inbounds i8, ptr %31, i16 %36
  %38 = load ptr, ptr %37
  %39 = getelementptr i8, ptr %37, i16 2
  %40 = load i16, ptr %39
  %41 = getelementptr i8, ptr %37, i16 2
  %42 = addrspacecast ptr %41 to ptr addrspace(1)
  %43 = addrspacecast ptr %15 to ptr addrspace(1)
  %44 = addrspacecast ptr %18 to ptr addrspace(1)
  %45 = load ptr, ptr %37
  %46 = getelementptr i8, ptr %45, i16 -4
  %47 = load i16, ptr %46
  %48 = addrspacecast ptr %45 to ptr addrspace(1)
  store i16 %47, ptr %14, !tbaa !2
  %49 = getelementptr inbounds i8, ptr %14, i16 2
  store i16 %47, ptr %49, !tbaa !2
  %50 = getelementptr inbounds i8, ptr %14, i16 4
  store ptr addrspace(1) %48, ptr %50, !tbaa !2
  %51 = addrspacecast ptr %14 to ptr addrspace(1)
  %52 = load i16, ptr addrspace(1) %42
  call cc 1005 addrspace(1) void @sign_up(ptr addrspace(1) %43, ptr addrspace(1) %44, ptr addrspace(1) %51, i16 %52)
  br label %b4

b4:
  %53 = load i16, ptr %16, !tbaa !2
  %54 = add i16 %53, 1
  store i16 %54, ptr %16, !tbaa !2
  br label %b2

b5:
  %55 = load ptr, ptr %17, !tbaa !2
  %56 = icmp ne ptr %55, null
  br i1 %56, label %b7, label %b6

b6:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %55)
  %57 = addrspacecast ptr %12 to ptr addrspace(1)
  %58 = addrspacecast ptr %18 to ptr addrspace(1)
  %59 = getelementptr i8, ptr @$str10, i16 6
  %60 = getelementptr i8, ptr %59, i16 -4
  %61 = load i16, ptr %60
  %62 = addrspacecast ptr %59 to ptr addrspace(1)
  store i16 %61, ptr %11, !tbaa !2
  %63 = getelementptr inbounds i8, ptr %11, i16 2
  store i16 %61, ptr %63, !tbaa !2
  %64 = getelementptr inbounds i8, ptr %11, i16 4
  store ptr addrspace(1) %62, ptr %64, !tbaa !2
  %65 = addrspacecast ptr %11 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @sign_up(ptr addrspace(1) %57, ptr addrspace(1) %58, ptr addrspace(1) %65, i16 -3)
  %66 = load i8, ptr %12, !tbaa !2, !range !6
  %67 = icmp eq i8 %66, 0
  br i1 %67, label %b13, label %b12

b7:
  %68 = getelementptr i8, ptr %55, i16 -4
  %69 = load i16, ptr %68
  store i16 0, ptr %13, !tbaa !2
  br label %b8

b8:
  %70 = load i16, ptr %13, !tbaa !2
  %71 = icmp ult i16 %70, %69
  br i1 %71, label %b10, label %b9

b9:
  br label %b6

b10:
  %72 = mul i16 %70, 4
  %73 = getelementptr inbounds i8, ptr %55, i16 %72
  %74 = load ptr, ptr %73
  call cc 1005 addrspace(1) void @N$BDRP(ptr %74)
  %75 = add i16 %70, 1
  store i16 %75, ptr %13, !tbaa !2
  br label %b8

b11:
  %76 = load ptr, ptr %18, !tbaa !2
  %77 = getelementptr i8, ptr %76, i16 -4
  %78 = load i16, ptr %77
  %79 = icmp ult i16 1, %78
  br i1 %79, label %b15, label %b16

b12:
  %80 = getelementptr inbounds i8, ptr %12, i16 4
  %81 = load i16, ptr %80, !tbaa !2
  store i16 %81, ptr %10, !tbaa !2
  %82 = getelementptr i8, ptr @$str12, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %82)
  %83 = load i16, ptr %10, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI2(i16 %83)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b11

b13:
  %84 = getelementptr i8, ptr @$str11, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %84)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b11

b15:
  %85 = getelementptr inbounds i8, ptr %76, i16 4
  %86 = getelementptr i8, ptr %85, i16 2
  %87 = load i16, ptr %86
  %88 = add i16 %87, 25
  %89 = getelementptr i8, ptr %85, i16 2
  store i16 %88, ptr %89
  %90 = addrspacecast ptr %18 to ptr addrspace(1)
  %91 = call cc 1005 addrspace(1) ptr addrspace(1) @best(ptr addrspace(1) %90)
  %92 = load ptr, ptr %18, !tbaa !2
  %93 = getelementptr i8, ptr %92, i16 -4
  %94 = load i16, ptr %93
  store i16 0, ptr %9, !tbaa !2
  br label %b17

b16:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b17:
  %95 = load i16, ptr %9, !tbaa !2
  %96 = icmp ult i16 %95, %94
  br i1 %96, label %b18, label %b20

b18:
  %97 = mul i16 %95, 4
  %98 = getelementptr inbounds i8, ptr %92, i16 %97
  %99 = addrspacecast ptr %98 to ptr addrspace(1)
  %100 = icmp eq ptr addrspace(1) %99, %91
  br i1 %100, label %b21, label %b22

b19:
  %101 = load i16, ptr %9, !tbaa !2
  %102 = add i16 %101, 1
  store i16 %102, ptr %9, !tbaa !2
  br label %b17

b20:
  %103 = getelementptr i8, ptr @$str6, i16 6
  store ptr %103, ptr %4, !tbaa !2
  %104 = load ptr, ptr %18, !tbaa !2
  %105 = getelementptr i8, ptr %104, i16 -4
  %106 = load i16, ptr %105
  store i16 0, ptr %3, !tbaa !2
  store i16 %106, ptr %2, !tbaa !2
  br label %b24

b21:
  %107 = getelementptr i8, ptr @$str13, i16 6
  store ptr %107, ptr %8, !tbaa !2
  br label %b23

b22:
  %108 = getelementptr i8, ptr @$str6, i16 6
  store ptr %108, ptr %8, !tbaa !2
  br label %b23

b23:
  %109 = load ptr, ptr %8, !tbaa !2
  store ptr %109, ptr %7, !tbaa !2
  %110 = addrspacecast ptr %98 to ptr addrspace(1)
  %111 = call cc 1005 addrspace(1) ptr @Player.display(ptr addrspace(1) %110)
  store ptr %111, ptr %6, !tbaa !2
  %112 = addrspacecast ptr %98 to ptr addrspace(1)
  %113 = call cc 1005 addrspace(1) ptr @Player.grade(ptr addrspace(1) %112)
  store ptr %113, ptr %5, !tbaa !2
  %114 = load ptr, ptr %6, !tbaa !2
  call cc 1005 addrspace(1) void @N$PS(ptr %114)
  %115 = getelementptr i8, ptr @$str14, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %115)
  %116 = load ptr, ptr %5, !tbaa !2
  call cc 1005 addrspace(1) void @N$PS(ptr %116)
  %117 = load ptr, ptr %7, !tbaa !2
  call cc 1005 addrspace(1) void @N$PS(ptr %117)
  call cc 1005 addrspace(1) void @N$PN()
  %118 = load ptr, ptr %5, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %118)
  %119 = load ptr, ptr %6, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %119)
  %120 = load ptr, ptr %7, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %120)
  br label %b19

b24:
  %121 = load i16, ptr %3, !tbaa !2
  %122 = load i16, ptr %2, !tbaa !2
  %123 = icmp ult i16 %121, %122
  br i1 %123, label %b25, label %b27

b25:
  %124 = load ptr, ptr %18, !tbaa !2
  %125 = load i16, ptr %3, !tbaa !2
  %126 = getelementptr i8, ptr %124, i16 -4
  %127 = load i16, ptr %126
  %128 = icmp ult i16 %125, %127
  br i1 %128, label %b31, label %b32

b26:
  %129 = load i16, ptr %3, !tbaa !2
  %130 = add nuw i16 %129, 1
  store i16 %130, ptr %3, !tbaa !2
  br label %b24

b27:
  %131 = load ptr, ptr %4, !tbaa !2
  store ptr null, ptr %4, !tbaa !2
  store ptr %131, ptr %1, !tbaa !2
  %132 = load ptr, ptr %1, !tbaa !2
  %133 = getelementptr i8, ptr %132, i16 -4
  %134 = load i16, ptr %133
  %135 = icmp ult i16 0, %134
  br i1 %135, label %b35, label %b36

b28:
  %136 = load ptr, ptr %4, !tbaa !2
  %137 = getelementptr i8, ptr %136, i16 -4
  %138 = load i16, ptr %137
  %139 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %136, i16 1, i16 4)
  store ptr %139, ptr %4, !tbaa !2
  %140 = mul i16 %138, 4
  %141 = getelementptr inbounds i8, ptr %139, i16 %140
  %142 = load ptr, ptr %18, !tbaa !2
  %143 = load i16, ptr %3, !tbaa !2
  %144 = getelementptr i8, ptr %142, i16 -4
  %145 = load i16, ptr %144
  %146 = icmp ult i16 %143, %145
  br i1 %146, label %b33, label %b34

b29:
  br label %b30

b30:
  br label %b26

b31:
  %147 = mul i16 %125, 4
  %148 = getelementptr inbounds i8, ptr %124, i16 %147
  %149 = getelementptr i8, ptr %148, i16 2
  %150 = load i16, ptr %149
  %151 = icmp sgt i16 %150, 1500
  br i1 %151, label %b28, label %b29

b32:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b33:
  %152 = mul i16 %143, 4
  %153 = getelementptr inbounds i8, ptr %142, i16 %152
  %154 = getelementptr i8, ptr %153, i16 2
  %155 = load i16, ptr %154
  %156 = load i16, ptr %3, !tbaa !2
  store i16 %155, ptr %141
  %157 = getelementptr i8, ptr %141, i16 2
  store i16 %156, ptr %157
  br label %b30

b34:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b35:
  %158 = getelementptr inbounds i8, ptr %132, i16 0
  %159 = load i16, ptr %158
  %160 = getelementptr i8, ptr %158, i16 2
  %161 = load i16, ptr %160
  %162 = getelementptr i8, ptr %158, i16 2
  %163 = addrspacecast ptr %162 to ptr addrspace(1)
  %164 = load ptr, ptr %1, !tbaa !2
  %165 = getelementptr i8, ptr %164, i16 -4
  %166 = load i16, ptr %165
  call cc 1005 addrspace(1) void @N$PU2(i16 %166)
  %167 = getelementptr i8, ptr @$str15, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %167)
  %168 = load ptr, ptr %18, !tbaa !2
  %169 = load i16, ptr addrspace(1) %163
  %170 = getelementptr i8, ptr %168, i16 -4
  %171 = load i16, ptr %170
  %172 = icmp ult i16 %169, %171
  br i1 %172, label %b37, label %b38

b36:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b37:
  %173 = mul i16 %169, 4
  %174 = getelementptr inbounds i8, ptr %168, i16 %173
  %175 = load ptr, ptr %174
  call cc 1005 addrspace(1) void @N$PS(ptr %175)
  %176 = getelementptr i8, ptr @$str16, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %176)
  %177 = load i16, ptr %158
  call cc 1005 addrspace(1) void @N$PI2(i16 %177)
  call cc 1005 addrspace(1) void @N$PN()
  %178 = load ptr, ptr %1, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %178)
  %179 = load ptr, ptr %4, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %179)
  %180 = load ptr, ptr %18, !tbaa !2
  %181 = icmp ne ptr %180, null
  br i1 %181, label %b40, label %b39

b38:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b39:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %180)
  ret i16 0

b40:
  %182 = getelementptr i8, ptr %180, i16 -4
  %183 = load i16, ptr %182
  store i16 0, ptr %0, !tbaa !2
  br label %b41

b41:
  %184 = load i16, ptr %0, !tbaa !2
  %185 = icmp ult i16 %184, %183
  br i1 %185, label %b43, label %b42

b42:
  br label %b39

b43:
  %186 = mul i16 %184, 4
  %187 = getelementptr inbounds i8, ptr %180, i16 %186
  %188 = load ptr, ptr %187
  call cc 1005 addrspace(1) void @N$BDRP(ptr %188)
  %189 = add i16 %184, 1
  store i16 %189, ptr %0, !tbaa !2
  br label %b41
}

declare cc 1005 ptr @N$VCPY(ptr addrspace(1)) addrspace(1)

declare cc 1005 void @N$PBEG() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 ptr @N$PEND() addrspace(1)

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) noalias nocapture writeonly, ptr noalias nocapture readonly, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)

declare cc 1005 ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PU2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i8 0, i8 2}
!6 = !{i8 0, i8 2}
