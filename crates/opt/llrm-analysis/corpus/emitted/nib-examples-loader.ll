target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str2 = internal constant [10 x i8] c"\08\00\03\00\03\00end\00"
@$str3 = internal constant [9 x i8] c"\08\00\02\00\02\00: \00"
@$str4 = internal constant [15 x i8] c"\08\00\08\00\08\00 entries\00"
@$str5 = internal constant [9 x i8] c"\08\00\02\00\02\00  \00"
@$str6 = internal constant [10 x i8] c"\08\00\03\00\03\00 = \00"
@$str7 = internal constant [14 x i8] c"\08\00\07\00\07\00: line \00"
@$str8 = internal constant [18 x i8] c"\08\00\0B\00\0B\00 has no '='\00"
@$str9 = internal constant [23 x i8] c"\08\00\10\00\10\00 is not a number\00"
@$str10 = internal constant [21 x i8] c"\08\00\0E\00\0E\00 repeats a key\00"
@$str11 = internal constant [12 x i8] c"\08\00\05\00\05\00video\00"
@$str12 = internal constant [35 x i8] c"\08\00\1C\00\1C\00width=320\0Aheight=200\0Adepth=8\00"
@$str13 = internal constant [12 x i8] c"\08\00\05\00\05\00early\00"
@$str14 = internal constant [22 x i8] c"\08\00\0F\00\0F\00a=1\0Ab=2\0Aend\0Ac=3\00"
@$str15 = internal constant [15 x i8] c"\08\00\08\00\08\00nonumber\00"
@$str16 = internal constant [18 x i8] c"\08\00\0B\00\0B\00a=1\0Ab=2\0Ac=x\00"
@$str17 = internal constant [15 x i8] c"\08\00\08\00\08\00noequals\00"
@$str18 = internal constant [12 x i8] c"\08\00\05\00\05\00a=1\0Ab\00"
@$str19 = internal constant [13 x i8] c"\08\00\06\00\06\00repeat\00"
@$str20 = internal constant [18 x i8] c"\08\00\0B\00\0B\00a=1\0Ab=2\0Aa=3\00"

define internal cc 1005 i32 @number(ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca [4 x i8]
  %4 = load i16, ptr addrspace(1) %0
  %5 = icmp eq i16 %4, 0
  br i1 %5, label %b2, label %b3

b2:
  store i32 1, ptr %3, !tbaa !2
  %6 = addrspacecast ptr %3 to ptr addrspace(1)
  %7 = load i32, ptr addrspace(1) %6, !tbaa !2
  ret i32 %7

b3:
  br label %b4

b4:
  store i16 0, ptr %2, !tbaa !2
  %8 = load i16, ptr addrspace(1) %0
  store i16 0, ptr %1, !tbaa !2
  br label %b5

b5:
  %9 = load i16, ptr %1, !tbaa !2
  %10 = icmp ult i16 %9, %8
  br i1 %10, label %b6, label %b8

b6:
  %11 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %12 = load ptr addrspace(1), ptr addrspace(1) %11
  %13 = getelementptr inbounds i8, ptr addrspace(1) %12, i16 %9
  %14 = load i8, ptr addrspace(1) %13
  %15 = icmp ult i8 %14, 48
  br i1 %15, label %b9, label %b12

b7:
  %16 = load i16, ptr %1, !tbaa !2
  %17 = add i16 %16, 1
  store i16 %17, ptr %1, !tbaa !2
  br label %b5

b8:
  %18 = load i16, ptr %2, !tbaa !2
  store i16 0, ptr %3, !tbaa !2
  %19 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 %18, ptr %19, !tbaa !2
  %20 = addrspacecast ptr %3 to ptr addrspace(1)
  %21 = load i32, ptr addrspace(1) %20, !tbaa !2
  ret i32 %21

b9:
  store i32 1, ptr %3, !tbaa !2
  %22 = addrspacecast ptr %3 to ptr addrspace(1)
  %23 = load i32, ptr addrspace(1) %22, !tbaa !2
  ret i32 %23

b10:
  br label %b11

b11:
  %24 = load i16, ptr %2, !tbaa !2
  %25 = mul i16 %24, 10
  %26 = load i8, ptr addrspace(1) %13
  %27 = zext i8 %26 to i16
  %28 = add i16 %25, %27
  %29 = sub i16 %28, 48
  store i16 %29, ptr %2, !tbaa !2
  br label %b7

b12:
  %30 = load i8, ptr addrspace(1) %13
  %31 = icmp ugt i8 %30, 57
  br i1 %31, label %b9, label %b10
}

define internal cc 1005 i8 @Table.has(ptr addrspace(1) nonnull dereferenceable(4) readonly noalias nocapture %0, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca [8 x i8]
  %3 = alloca i16
  %4 = alloca [8 x i8]
  %5 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %6 = load ptr, ptr addrspace(1) %5
  %7 = getelementptr i8, ptr %6, i16 -4
  %8 = load i16, ptr %7
  %9 = addrspacecast ptr %6 to ptr addrspace(1)
  store i16 %8, ptr %4, !tbaa !2
  %10 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 %8, ptr %10, !tbaa !2
  %11 = getelementptr inbounds i8, ptr %4, i16 4
  store ptr addrspace(1) %9, ptr %11, !tbaa !2
  %12 = addrspacecast ptr %4 to ptr addrspace(1)
  %13 = load i16, ptr addrspace(1) %12
  store i16 0, ptr %3, !tbaa !2
  br label %b2

b2:
  %14 = load i16, ptr %3, !tbaa !2
  %15 = icmp ult i16 %14, %13
  br i1 %15, label %b3, label %b5

b3:
  %16 = getelementptr i8, ptr addrspace(1) %12, i16 4
  %17 = load ptr addrspace(1), ptr addrspace(1) %16, !tbaa !2
  %18 = mul i16 %14, 4
  %19 = getelementptr inbounds i8, ptr addrspace(1) %17, i16 %18
  %20 = load ptr, ptr addrspace(1) %19
  %21 = getelementptr i8, ptr %20, i16 -4
  %22 = load i16, ptr %21
  %23 = addrspacecast ptr %20 to ptr addrspace(1)
  store i16 %22, ptr %2, !tbaa !2
  %24 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 %22, ptr %24, !tbaa !2
  %25 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %23, ptr %25, !tbaa !2
  %26 = addrspacecast ptr %2 to ptr addrspace(1)
  %27 = call cc 1005 addrspace(1) i8 @N$VCMP(ptr addrspace(1) %26, ptr addrspace(1) %1)
  %28 = icmp eq i8 %27, 0
  br i1 %28, label %b6, label %b7

b4:
  %29 = load i16, ptr %3, !tbaa !2
  %30 = add i16 %29, 1
  store i16 %30, ptr %3, !tbaa !2
  br label %b2

b5:
  ret i8 0

b6:
  ret i8 1

b7:
  br label %b8

b8:
  br label %b4
}

define internal cc 1005 void @parse_line(ptr addrspace(1) %0, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %1, i16 %2) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %3 = alloca [8 x i8]
  %4 = alloca i16
  %5 = alloca [8 x i8]
  %6 = alloca [4 x i8]
  %7 = alloca i16
  %8 = alloca i16
  %9 = load i16, ptr addrspace(1) %1
  store i16 0, ptr %8, !tbaa !2
  store i16 %9, ptr %7, !tbaa !2
  br label %b2

b2:
  %10 = load i16, ptr %8, !tbaa !2
  %11 = load i16, ptr %7, !tbaa !2
  %12 = icmp ult i16 %10, %11
  br i1 %12, label %b3, label %b5

b3:
  %13 = load i16, ptr %8, !tbaa !2
  %14 = load i16, ptr addrspace(1) %1
  %15 = icmp ult i16 %13, %14
  br i1 %15, label %b9, label %b10

b4:
  %16 = load i16, ptr %8, !tbaa !2
  %17 = add nuw i16 %16, 1
  store i16 %17, ptr %8, !tbaa !2
  br label %b2

b5:
  store i8 1, ptr addrspace(1) %0
  %18 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 0, ptr addrspace(1) %18
  %19 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %2, ptr addrspace(1) %19
  ret void

b6:
  %20 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %21 = load ptr addrspace(1), ptr addrspace(1) %20
  %22 = load i16, ptr addrspace(1) %1
  %23 = load i16, ptr %8, !tbaa !2
  %24 = add i16 %23, 1
  %25 = load i16, ptr addrspace(1) %1
  %26 = icmp ule i16 %25, %22
  br i1 %26, label %b11, label %b12

b7:
  br label %b8

b8:
  br label %b4

b9:
  %27 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %28 = load ptr addrspace(1), ptr addrspace(1) %27
  %29 = getelementptr inbounds i8, ptr addrspace(1) %28, i16 %13
  %30 = load i8, ptr addrspace(1) %29
  %31 = icmp eq i8 %30, 61
  br i1 %31, label %b6, label %b7

b10:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b11:
  %32 = icmp ule i16 %24, %25
  br i1 %32, label %b13, label %b14

b12:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b13:
  %33 = getelementptr inbounds i8, ptr addrspace(1) %21, i16 %24
  %34 = sub i16 %25, %24
  store i16 %34, ptr %5, !tbaa !2
  %35 = getelementptr inbounds i8, ptr %5, i16 2
  store i16 %34, ptr %35, !tbaa !2
  %36 = getelementptr inbounds i8, ptr %5, i16 4
  store ptr addrspace(1) %33, ptr %36, !tbaa !2
  %37 = addrspacecast ptr %5 to ptr addrspace(1)
  %38 = call cc 1005 addrspace(1) i32 @number(ptr addrspace(1) %37)
  %39 = addrspacecast ptr %6 to ptr addrspace(1)
  store i32 %38, ptr addrspace(1) %39, !tbaa !2
  %40 = load i8, ptr %6, !tbaa !2, !range !5
  %41 = icmp eq i8 %40, 0
  br i1 %41, label %b17, label %b16

b14:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b16:
  store i8 1, ptr addrspace(1) %0
  %42 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 1, ptr addrspace(1) %42
  %43 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %2, ptr addrspace(1) %43
  ret void

b17:
  %44 = getelementptr inbounds i8, ptr %6, i16 2
  %45 = load i16, ptr %44, !tbaa !2
  %46 = getelementptr inbounds i8, ptr %6, i16 2
  %47 = load i16, ptr %46, !tbaa !2
  store i16 %47, ptr %4, !tbaa !2
  %48 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %49 = load ptr addrspace(1), ptr addrspace(1) %48
  %50 = load i16, ptr addrspace(1) %1
  %51 = load i16, ptr %8, !tbaa !2
  %52 = icmp ule i16 %51, %50
  br i1 %52, label %b18, label %b19

b18:
  %53 = icmp ule i16 0, %51
  br i1 %53, label %b20, label %b21

b19:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b20:
  store i16 %51, ptr %3, !tbaa !2
  %54 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 %51, ptr %54, !tbaa !2
  %55 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %49, ptr %55, !tbaa !2
  %56 = addrspacecast ptr %3 to ptr addrspace(1)
  %57 = call cc 1005 addrspace(1) ptr @N$VCPY(ptr addrspace(1) %56)
  %58 = load i16, ptr %4, !tbaa !2
  store i8 0, ptr addrspace(1) %0
  %59 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr %57, ptr addrspace(1) %59
  %60 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %58, ptr addrspace(1) %60
  ret void

b21:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

define internal cc 1005 void @load(ptr addrspace(1) %0, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %1, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %2) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca [8 x i8]
  %6 = alloca i16
  %7 = alloca [6 x i8]
  %8 = alloca [4 x i8]
  %9 = alloca [8 x i8]
  %10 = alloca [8 x i8]
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca i16
  %14 = alloca i16
  %15 = alloca [4 x i8]
  %16 = call cc 1005 addrspace(1) ptr @N$VCPY(ptr addrspace(1) %1)
  %17 = getelementptr i8, ptr @$str1, i16 6
  store ptr %16, ptr %15, !tbaa !2
  %18 = getelementptr inbounds i8, ptr %15, i16 2
  store ptr %17, ptr %18, !tbaa !2
  store i16 0, ptr %14, !tbaa !2
  store i16 1, ptr %13, !tbaa !2
  %19 = load i16, ptr addrspace(1) %2
  %20 = add i16 %19, 1
  store i16 0, ptr %12, !tbaa !2
  store i16 %20, ptr %11, !tbaa !2
  br label %b2

b2:
  %21 = load i16, ptr %12, !tbaa !2
  %22 = load i16, ptr %11, !tbaa !2
  %23 = icmp ult i16 %21, %22
  br i1 %23, label %b3, label %b5

b3:
  %24 = load i16, ptr %12, !tbaa !2
  %25 = load i16, ptr addrspace(1) %2
  %26 = icmp ult i16 %24, %25
  br i1 %26, label %b9, label %b7

b4:
  %27 = load i16, ptr %12, !tbaa !2
  %28 = add nuw i16 %27, 1
  store i16 %28, ptr %12, !tbaa !2
  br label %b2

b5:
  %29 = load ptr, ptr %15, !tbaa !2
  %30 = getelementptr inbounds i8, ptr %15, i16 2
  %31 = load ptr, ptr %30, !tbaa !2
  store i8 0, ptr addrspace(1) %0
  %32 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr %29, ptr addrspace(1) %32
  %33 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store ptr %31, ptr addrspace(1) %33
  store ptr null, ptr %15, !tbaa !2
  %34 = getelementptr inbounds i8, ptr %15, i16 2
  store ptr null, ptr %34, !tbaa !2
  %35 = getelementptr inbounds i8, ptr %15, i16 2
  %36 = load ptr, ptr %35, !tbaa !2
  %37 = icmp ne ptr %36, null
  br i1 %37, label %b35, label %b34

b6:
  br label %b4

b7:
  br label %b8

b8:
  %38 = getelementptr i8, ptr addrspace(1) %2, i16 4
  %39 = load ptr addrspace(1), ptr addrspace(1) %38
  %40 = load i16, ptr addrspace(1) %2
  %41 = load i16, ptr %14, !tbaa !2
  %42 = load i16, ptr %12, !tbaa !2
  %43 = icmp ule i16 %42, %40
  br i1 %43, label %b12, label %b13

b9:
  %44 = load i16, ptr %12, !tbaa !2
  %45 = load i16, ptr addrspace(1) %2
  %46 = icmp ult i16 %44, %45
  br i1 %46, label %b10, label %b11

b10:
  %47 = getelementptr i8, ptr addrspace(1) %2, i16 4
  %48 = load ptr addrspace(1), ptr addrspace(1) %47
  %49 = getelementptr inbounds i8, ptr addrspace(1) %48, i16 %44
  %50 = load i8, ptr addrspace(1) %49
  %51 = icmp ne i8 %50, 10
  br i1 %51, label %b6, label %b7

b11:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b12:
  %52 = icmp ule i16 %41, %42
  br i1 %52, label %b14, label %b15

b13:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b14:
  %53 = getelementptr inbounds i8, ptr addrspace(1) %39, i16 %41
  %54 = sub i16 %42, %41
  store i16 %54, ptr %10, !tbaa !2
  %55 = getelementptr inbounds i8, ptr %10, i16 2
  store i16 %54, ptr %55, !tbaa !2
  %56 = getelementptr inbounds i8, ptr %10, i16 4
  store ptr addrspace(1) %53, ptr %56, !tbaa !2
  %57 = addrspacecast ptr %10 to ptr addrspace(1)
  %58 = getelementptr i8, ptr @$str2, i16 6
  %59 = getelementptr i8, ptr %58, i16 -4
  %60 = load i16, ptr %59
  %61 = addrspacecast ptr %58 to ptr addrspace(1)
  store i16 %60, ptr %9, !tbaa !2
  %62 = getelementptr inbounds i8, ptr %9, i16 2
  store i16 %60, ptr %62, !tbaa !2
  %63 = getelementptr inbounds i8, ptr %9, i16 4
  store ptr addrspace(1) %61, ptr %63, !tbaa !2
  %64 = addrspacecast ptr %9 to ptr addrspace(1)
  %65 = call cc 1005 addrspace(1) i8 @N$VCMP(ptr addrspace(1) %57, ptr addrspace(1) %64)
  %66 = icmp eq i8 %65, 0
  br i1 %66, label %b16, label %b17

b15:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b16:
  br label %b5

b17:
  br label %b18

b18:
  %67 = addrspacecast ptr %7 to ptr addrspace(1)
  %68 = load i16, ptr %13, !tbaa !2
  call cc 1005 addrspace(1) void @parse_line(ptr addrspace(1) %67, ptr addrspace(1) %57, i16 %68)
  %69 = load i8, ptr %7, !tbaa !2, !range !6
  %70 = icmp eq i8 %69, 1
  br i1 %70, label %b19, label %b20

b19:
  store i8 1, ptr addrspace(1) %0
  %71 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %72 = getelementptr inbounds i8, ptr %7, i16 2
  call void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) %71, ptr %72, i16 4, i1 false)
  %73 = getelementptr inbounds i8, ptr %15, i16 2
  %74 = load ptr, ptr %73, !tbaa !2
  %75 = icmp ne ptr %74, null
  br i1 %75, label %b22, label %b21

b20:
  %76 = getelementptr inbounds i8, ptr %7, i16 2
  %77 = load ptr, ptr %76, !tbaa !2
  %78 = getelementptr inbounds i8, ptr %7, i16 4
  %79 = load i16, ptr %78, !tbaa !2
  store ptr %77, ptr %8, !tbaa !2
  %80 = getelementptr inbounds i8, ptr %8, i16 2
  store i16 %79, ptr %80, !tbaa !2
  %81 = addrspacecast ptr %15 to ptr addrspace(1)
  %82 = load ptr, ptr %8, !tbaa !2
  %83 = getelementptr i8, ptr %82, i16 -4
  %84 = load i16, ptr %83
  %85 = addrspacecast ptr %82 to ptr addrspace(1)
  store i16 %84, ptr %5, !tbaa !2
  %86 = getelementptr inbounds i8, ptr %5, i16 2
  store i16 %84, ptr %86, !tbaa !2
  %87 = getelementptr inbounds i8, ptr %5, i16 4
  store ptr addrspace(1) %85, ptr %87, !tbaa !2
  %88 = addrspacecast ptr %5 to ptr addrspace(1)
  %89 = call cc 1005 addrspace(1) i8 @Table.has(ptr addrspace(1) %81, ptr addrspace(1) %88)
  %90 = icmp ne i8 %89, 0
  br i1 %90, label %b26, label %b27

b21:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %74)
  %91 = load ptr, ptr %15, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %91)
  ret void

b22:
  %92 = getelementptr i8, ptr %74, i16 -4
  %93 = load i16, ptr %92
  store i16 0, ptr %6, !tbaa !2
  br label %b23

b23:
  %94 = load i16, ptr %6, !tbaa !2
  %95 = icmp ult i16 %94, %93
  br i1 %95, label %b25, label %b24

b24:
  br label %b21

b25:
  %96 = mul i16 %94, 4
  %97 = getelementptr inbounds i8, ptr %74, i16 %96
  %98 = load ptr, ptr %97
  call cc 1005 addrspace(1) void @N$BDRP(ptr %98)
  %99 = add i16 %94, 1
  store i16 %99, ptr %6, !tbaa !2
  br label %b23

b26:
  %100 = load i16, ptr %13, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %101 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 2, ptr addrspace(1) %101
  %102 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %100, ptr addrspace(1) %102
  %103 = load ptr, ptr %8, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %103)
  %104 = getelementptr inbounds i8, ptr %15, i16 2
  %105 = load ptr, ptr %104, !tbaa !2
  %106 = icmp ne ptr %105, null
  br i1 %106, label %b30, label %b29

b27:
  br label %b28

b28:
  %107 = getelementptr inbounds i8, ptr %15, i16 2
  %108 = load ptr, ptr %107, !tbaa !2
  %109 = getelementptr i8, ptr %108, i16 -4
  %110 = load i16, ptr %109
  %111 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %108, i16 1, i16 4)
  %112 = getelementptr inbounds i8, ptr %15, i16 2
  store ptr %111, ptr %112, !tbaa !2
  %113 = mul i16 %110, 4
  %114 = getelementptr inbounds i8, ptr %111, i16 %113
  %115 = load ptr, ptr %8, !tbaa !2
  %116 = getelementptr inbounds i8, ptr %8, i16 2
  %117 = load i16, ptr %116, !tbaa !2
  store ptr %115, ptr %114
  %118 = getelementptr i8, ptr %114, i16 2
  store i16 %117, ptr %118
  store ptr null, ptr %8, !tbaa !2
  %119 = getelementptr inbounds i8, ptr %8, i16 2
  store i16 0, ptr %119, !tbaa !2
  %120 = load i16, ptr %12, !tbaa !2
  %121 = add i16 %120, 1
  store i16 %121, ptr %14, !tbaa !2
  %122 = load i16, ptr %13, !tbaa !2
  %123 = add i16 %122, 1
  store i16 %123, ptr %13, !tbaa !2
  %124 = load ptr, ptr %8, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %124)
  br label %b4

b29:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %105)
  %125 = load ptr, ptr %15, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %125)
  ret void

b30:
  %126 = getelementptr i8, ptr %105, i16 -4
  %127 = load i16, ptr %126
  store i16 0, ptr %4, !tbaa !2
  br label %b31

b31:
  %128 = load i16, ptr %4, !tbaa !2
  %129 = icmp ult i16 %128, %127
  br i1 %129, label %b33, label %b32

b32:
  br label %b29

b33:
  %130 = mul i16 %128, 4
  %131 = getelementptr inbounds i8, ptr %105, i16 %130
  %132 = load ptr, ptr %131
  call cc 1005 addrspace(1) void @N$BDRP(ptr %132)
  %133 = add i16 %128, 1
  store i16 %133, ptr %4, !tbaa !2
  br label %b31

b34:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %36)
  %134 = load ptr, ptr %15, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %134)
  ret void

b35:
  %135 = getelementptr i8, ptr %36, i16 -4
  %136 = load i16, ptr %135
  store i16 0, ptr %3, !tbaa !2
  br label %b36

b36:
  %137 = load i16, ptr %3, !tbaa !2
  %138 = icmp ult i16 %137, %136
  br i1 %138, label %b38, label %b37

b37:
  br label %b34

b38:
  %139 = mul i16 %137, 4
  %140 = getelementptr inbounds i8, ptr %36, i16 %139
  %141 = load ptr, ptr %140
  call cc 1005 addrspace(1) void @N$BDRP(ptr %141)
  %142 = add i16 %137, 1
  store i16 %142, ptr %3, !tbaa !2
  br label %b36
}

define internal cc 1005 void @report(ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %0, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca [8 x i8]
  %9 = alloca [4 x i8]
  %10 = alloca [6 x i8]
  %11 = addrspacecast ptr %10 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @load(ptr addrspace(1) %11, ptr addrspace(1) %0, ptr addrspace(1) %1)
  %12 = load i8, ptr %10, !tbaa !2, !range !7
  %13 = icmp eq i8 %12, 0
  br i1 %13, label %b4, label %b3

b2:
  ret void

b3:
  %14 = load i8, ptr %10, !tbaa !2, !range !7
  %15 = icmp eq i8 %14, 1
  br i1 %15, label %b15, label %b14

b4:
  %16 = getelementptr inbounds i8, ptr %10, i16 2
  %17 = load ptr, ptr %16, !tbaa !2
  %18 = getelementptr inbounds i8, ptr %10, i16 4
  %19 = load ptr, ptr %18, !tbaa !2
  store ptr %17, ptr %9, !tbaa !2
  %20 = getelementptr inbounds i8, ptr %9, i16 2
  store ptr %19, ptr %20, !tbaa !2
  %21 = load ptr, ptr %9, !tbaa !2
  call cc 1005 addrspace(1) void @N$PS(ptr %21)
  %22 = getelementptr i8, ptr @$str3, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %22)
  %23 = getelementptr inbounds i8, ptr %9, i16 2
  %24 = load ptr, ptr %23, !tbaa !2
  %25 = getelementptr i8, ptr %24, i16 -4
  %26 = load i16, ptr %25
  call cc 1005 addrspace(1) void @N$PU2(i16 %26)
  %27 = getelementptr i8, ptr @$str4, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %27)
  call cc 1005 addrspace(1) void @N$PN()
  %28 = getelementptr inbounds i8, ptr %9, i16 2
  %29 = load ptr, ptr %28, !tbaa !2
  %30 = getelementptr i8, ptr %29, i16 -4
  %31 = load i16, ptr %30
  %32 = addrspacecast ptr %29 to ptr addrspace(1)
  store i16 %31, ptr %8, !tbaa !2
  %33 = getelementptr inbounds i8, ptr %8, i16 2
  store i16 %31, ptr %33, !tbaa !2
  %34 = getelementptr inbounds i8, ptr %8, i16 4
  store ptr addrspace(1) %32, ptr %34, !tbaa !2
  %35 = addrspacecast ptr %8 to ptr addrspace(1)
  %36 = load i16, ptr addrspace(1) %35
  store i16 0, ptr %7, !tbaa !2
  br label %b5

b5:
  %37 = load i16, ptr %7, !tbaa !2
  %38 = icmp ult i16 %37, %36
  br i1 %38, label %b6, label %b8

b6:
  %39 = getelementptr i8, ptr addrspace(1) %35, i16 4
  %40 = load ptr addrspace(1), ptr addrspace(1) %39, !tbaa !2
  %41 = mul i16 %37, 4
  %42 = getelementptr inbounds i8, ptr addrspace(1) %40, i16 %41
  %43 = getelementptr i8, ptr @$str5, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %43)
  %44 = load ptr, ptr addrspace(1) %42
  call cc 1005 addrspace(1) void @N$PS(ptr %44)
  %45 = getelementptr i8, ptr @$str6, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %45)
  %46 = getelementptr i8, ptr addrspace(1) %42, i16 2
  %47 = load i16, ptr addrspace(1) %46
  call cc 1005 addrspace(1) void @N$PI2(i16 %47)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b7

b7:
  %48 = load i16, ptr %7, !tbaa !2
  %49 = add i16 %48, 1
  store i16 %49, ptr %7, !tbaa !2
  br label %b5

b8:
  %50 = getelementptr inbounds i8, ptr %9, i16 2
  %51 = load ptr, ptr %50, !tbaa !2
  %52 = icmp ne ptr %51, null
  br i1 %52, label %b10, label %b9

b9:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %51)
  %53 = load ptr, ptr %9, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %53)
  br label %b2

b10:
  %54 = getelementptr i8, ptr %51, i16 -4
  %55 = load i16, ptr %54
  store i16 0, ptr %6, !tbaa !2
  br label %b11

b11:
  %56 = load i16, ptr %6, !tbaa !2
  %57 = icmp ult i16 %56, %55
  br i1 %57, label %b13, label %b12

b12:
  br label %b9

b13:
  %58 = mul i16 %56, 4
  %59 = getelementptr inbounds i8, ptr %51, i16 %58
  %60 = load ptr, ptr %59
  call cc 1005 addrspace(1) void @N$BDRP(ptr %60)
  %61 = add i16 %56, 1
  store i16 %61, ptr %6, !tbaa !2
  br label %b11

b14:
  %62 = load i8, ptr %10, !tbaa !2, !range !7
  %63 = icmp eq i8 %62, 1
  br i1 %63, label %b18, label %b17

b15:
  %64 = getelementptr inbounds i8, ptr %10, i16 2
  %65 = load i8, ptr %64, !tbaa !2, !range !8
  %66 = icmp eq i8 %65, 0
  br i1 %66, label %b16, label %b14

b16:
  %67 = getelementptr inbounds i8, ptr %10, i16 4
  %68 = load i16, ptr %67, !tbaa !2
  %69 = getelementptr inbounds i8, ptr %10, i16 4
  %70 = load i16, ptr %69, !tbaa !2
  store i16 %70, ptr %5, !tbaa !2
  call cc 1005 addrspace(1) void @N$PV(ptr addrspace(1) %0)
  %71 = getelementptr i8, ptr @$str7, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %71)
  %72 = load i16, ptr %5, !tbaa !2
  call cc 1005 addrspace(1) void @N$PU2(i16 %72)
  %73 = getelementptr i8, ptr @$str8, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %73)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b2

b17:
  %74 = getelementptr inbounds i8, ptr %10, i16 4
  %75 = load i16, ptr %74, !tbaa !2
  store i16 %75, ptr %3, !tbaa !2
  call cc 1005 addrspace(1) void @N$PV(ptr addrspace(1) %0)
  %76 = getelementptr i8, ptr @$str7, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %76)
  %77 = load i16, ptr %3, !tbaa !2
  call cc 1005 addrspace(1) void @N$PU2(i16 %77)
  %78 = getelementptr i8, ptr @$str10, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %78)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b2

b18:
  %79 = getelementptr inbounds i8, ptr %10, i16 2
  %80 = load i8, ptr %79, !tbaa !2, !range !8
  %81 = icmp eq i8 %80, 1
  br i1 %81, label %b19, label %b17

b19:
  %82 = getelementptr inbounds i8, ptr %10, i16 4
  %83 = load i16, ptr %82, !tbaa !2
  %84 = getelementptr inbounds i8, ptr %10, i16 4
  %85 = load i16, ptr %84, !tbaa !2
  store i16 %85, ptr %4, !tbaa !2
  call cc 1005 addrspace(1) void @N$PV(ptr addrspace(1) %0)
  %86 = getelementptr i8, ptr @$str7, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %86)
  %87 = load i16, ptr %4, !tbaa !2
  call cc 1005 addrspace(1) void @N$PU2(i16 %87)
  %88 = getelementptr i8, ptr @$str9, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %88)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b2
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca [8 x i8]
  %1 = alloca [8 x i8]
  %2 = alloca [8 x i8]
  %3 = alloca [8 x i8]
  %4 = alloca [8 x i8]
  %5 = alloca [8 x i8]
  %6 = alloca [8 x i8]
  %7 = alloca [8 x i8]
  %8 = alloca [8 x i8]
  %9 = alloca [8 x i8]
  %10 = getelementptr i8, ptr @$str11, i16 6
  %11 = getelementptr i8, ptr %10, i16 -4
  %12 = load i16, ptr %11
  %13 = addrspacecast ptr %10 to ptr addrspace(1)
  store i16 %12, ptr %9, !tbaa !2
  %14 = getelementptr inbounds i8, ptr %9, i16 2
  store i16 %12, ptr %14, !tbaa !2
  %15 = getelementptr inbounds i8, ptr %9, i16 4
  store ptr addrspace(1) %13, ptr %15, !tbaa !2
  %16 = addrspacecast ptr %9 to ptr addrspace(1)
  %17 = getelementptr i8, ptr @$str12, i16 6
  %18 = getelementptr i8, ptr %17, i16 -4
  %19 = load i16, ptr %18
  %20 = addrspacecast ptr %17 to ptr addrspace(1)
  store i16 %19, ptr %8, !tbaa !2
  %21 = getelementptr inbounds i8, ptr %8, i16 2
  store i16 %19, ptr %21, !tbaa !2
  %22 = getelementptr inbounds i8, ptr %8, i16 4
  store ptr addrspace(1) %20, ptr %22, !tbaa !2
  %23 = addrspacecast ptr %8 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @report(ptr addrspace(1) %16, ptr addrspace(1) %23)
  %24 = getelementptr i8, ptr @$str13, i16 6
  %25 = getelementptr i8, ptr %24, i16 -4
  %26 = load i16, ptr %25
  %27 = addrspacecast ptr %24 to ptr addrspace(1)
  store i16 %26, ptr %7, !tbaa !2
  %28 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 %26, ptr %28, !tbaa !2
  %29 = getelementptr inbounds i8, ptr %7, i16 4
  store ptr addrspace(1) %27, ptr %29, !tbaa !2
  %30 = addrspacecast ptr %7 to ptr addrspace(1)
  %31 = getelementptr i8, ptr @$str14, i16 6
  %32 = getelementptr i8, ptr %31, i16 -4
  %33 = load i16, ptr %32
  %34 = addrspacecast ptr %31 to ptr addrspace(1)
  store i16 %33, ptr %6, !tbaa !2
  %35 = getelementptr inbounds i8, ptr %6, i16 2
  store i16 %33, ptr %35, !tbaa !2
  %36 = getelementptr inbounds i8, ptr %6, i16 4
  store ptr addrspace(1) %34, ptr %36, !tbaa !2
  %37 = addrspacecast ptr %6 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @report(ptr addrspace(1) %30, ptr addrspace(1) %37)
  %38 = getelementptr i8, ptr @$str15, i16 6
  %39 = getelementptr i8, ptr %38, i16 -4
  %40 = load i16, ptr %39
  %41 = addrspacecast ptr %38 to ptr addrspace(1)
  store i16 %40, ptr %5, !tbaa !2
  %42 = getelementptr inbounds i8, ptr %5, i16 2
  store i16 %40, ptr %42, !tbaa !2
  %43 = getelementptr inbounds i8, ptr %5, i16 4
  store ptr addrspace(1) %41, ptr %43, !tbaa !2
  %44 = addrspacecast ptr %5 to ptr addrspace(1)
  %45 = getelementptr i8, ptr @$str16, i16 6
  %46 = getelementptr i8, ptr %45, i16 -4
  %47 = load i16, ptr %46
  %48 = addrspacecast ptr %45 to ptr addrspace(1)
  store i16 %47, ptr %4, !tbaa !2
  %49 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 %47, ptr %49, !tbaa !2
  %50 = getelementptr inbounds i8, ptr %4, i16 4
  store ptr addrspace(1) %48, ptr %50, !tbaa !2
  %51 = addrspacecast ptr %4 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @report(ptr addrspace(1) %44, ptr addrspace(1) %51)
  %52 = getelementptr i8, ptr @$str17, i16 6
  %53 = getelementptr i8, ptr %52, i16 -4
  %54 = load i16, ptr %53
  %55 = addrspacecast ptr %52 to ptr addrspace(1)
  store i16 %54, ptr %3, !tbaa !2
  %56 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 %54, ptr %56, !tbaa !2
  %57 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %55, ptr %57, !tbaa !2
  %58 = addrspacecast ptr %3 to ptr addrspace(1)
  %59 = getelementptr i8, ptr @$str18, i16 6
  %60 = getelementptr i8, ptr %59, i16 -4
  %61 = load i16, ptr %60
  %62 = addrspacecast ptr %59 to ptr addrspace(1)
  store i16 %61, ptr %2, !tbaa !2
  %63 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 %61, ptr %63, !tbaa !2
  %64 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %62, ptr %64, !tbaa !2
  %65 = addrspacecast ptr %2 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @report(ptr addrspace(1) %58, ptr addrspace(1) %65)
  %66 = getelementptr i8, ptr @$str19, i16 6
  %67 = getelementptr i8, ptr %66, i16 -4
  %68 = load i16, ptr %67
  %69 = addrspacecast ptr %66 to ptr addrspace(1)
  store i16 %68, ptr %1, !tbaa !2
  %70 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 %68, ptr %70, !tbaa !2
  %71 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %69, ptr %71, !tbaa !2
  %72 = addrspacecast ptr %1 to ptr addrspace(1)
  %73 = getelementptr i8, ptr @$str20, i16 6
  %74 = getelementptr i8, ptr %73, i16 -4
  %75 = load i16, ptr %74
  %76 = addrspacecast ptr %73 to ptr addrspace(1)
  store i16 %75, ptr %0, !tbaa !2
  %77 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 %75, ptr %77, !tbaa !2
  %78 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %76, ptr %78, !tbaa !2
  %79 = addrspacecast ptr %0 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @report(ptr addrspace(1) %72, ptr addrspace(1) %79)
  ret i16 0
}

declare cc 1005 i8 @N$VCMP(ptr addrspace(1), ptr addrspace(1)) addrspace(1) memory(read)

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare cc 1005 ptr @N$VCPY(ptr addrspace(1)) addrspace(1)

declare void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) noalias nocapture writeonly, ptr noalias nocapture readonly, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

declare cc 1005 ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PU2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PV(ptr addrspace(1)) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i8 0, i8 2}
!6 = !{i8 0, i8 2}
!7 = !{i8 0, i8 2}
!8 = !{i8 0, i8 3}
