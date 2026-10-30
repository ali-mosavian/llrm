target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [13 x i8] c"\08\00\06\00\06\00nobody\00"
@$str2 = internal constant [10 x i8] c"\08\00\03\00\03\00ada\00"
@$str3 = internal constant [10 x i8] c"\08\00\03\00\03\00bob\00"
@$str4 = internal constant [14 x i8] c"\08\00\07\00\07\00leader \00"
@$str5 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str6 = internal constant [9 x i8] c"\08\00\02\00\02\00cy\00"
@$str7 = internal constant [9 x i8] c"\08\00\02\00\02\00di\00"
@$str8 = internal constant [9 x i8] c"\08\00\02\00\02\00ed\00"
@$str9 = internal constant [12 x i8] c"\08\00\05\00\05\00best \00"
@$str10 = internal constant [13 x i8] c"\08\00\06\00\06\00 with \00"
@$str11 = internal constant [14 x i8] c"\08\00\07\00\07\00no team\00"
@$str12 = internal constant [13 x i8] c"\08\00\06\00\06\00first \00"
@$str13 = internal constant [27 x i8] c"\08\00\14\00\14\00move north then east\00"
@$str14 = internal constant [8 x i8] c"\08\00\01\00\01\00[\00"
@$str15 = internal constant [8 x i8] c"\08\00\01\00\01\00]\00"

define internal cc 1005 ptr addrspace(1) @leader(ptr addrspace(1) nonnull dereferenceable(4) readonly noalias %0, ptr addrspace(1) nonnull dereferenceable(4) readonly noalias %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca ptr addrspace(1)
  %3 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %4 = load i16, ptr addrspace(1) %3
  %5 = getelementptr i8, ptr addrspace(1) %1, i16 2
  %6 = load i16, ptr addrspace(1) %5
  %7 = icmp sge i16 %4, %6
  br i1 %7, label %b2, label %b3

b2:
  store ptr addrspace(1) %0, ptr %2, !tbaa !2
  br label %b4

b3:
  store ptr addrspace(1) %1, ptr %2, !tbaa !2
  br label %b4

b4:
  %8 = load ptr addrspace(1), ptr %2, !tbaa !2
  ret ptr addrspace(1) %8
}

define internal cc 1005 void @best(ptr addrspace(1) %0, ptr addrspace(1) noalias readonly dereferenceable(8) %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = load i16, ptr addrspace(1) %1
  %6 = icmp eq i16 %5, 0
  br i1 %6, label %b2, label %b3

b2:
  store i8 1, ptr addrspace(1) %0
  ret void

b3:
  br label %b4

b4:
  store i16 0, ptr %4, !tbaa !2
  %7 = load i16, ptr addrspace(1) %1
  store i16 1, ptr %3, !tbaa !2
  store i16 %7, ptr %2, !tbaa !2
  br label %b5

b5:
  %8 = load i16, ptr %3, !tbaa !2
  %9 = load i16, ptr %2, !tbaa !2
  %10 = icmp ult i16 %8, %9
  br i1 %10, label %b6, label %b8

b6:
  %11 = load i16, ptr %3, !tbaa !2
  %12 = load i16, ptr addrspace(1) %1, !range !5
  %13 = icmp ult i16 %11, %12
  br i1 %13, label %b12, label %b13

b7:
  %14 = load i16, ptr %3, !tbaa !2
  %15 = add nuw i16 %14, 1
  store i16 %15, ptr %3, !tbaa !2
  br label %b5

b8:
  %16 = load i16, ptr %4, !tbaa !2
  %17 = load i16, ptr addrspace(1) %1, !range !7
  %18 = icmp ult i16 %16, %17
  br i1 %18, label %b16, label %b17

b9:
  %19 = load i16, ptr %3, !tbaa !2
  store i16 %19, ptr %4, !tbaa !2
  br label %b11

b10:
  br label %b11

b11:
  br label %b7

b12:
  %20 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %21 = load ptr addrspace(1), ptr addrspace(1) %20
  %22 = mul i16 %11, 4
  %23 = getelementptr inbounds i8, ptr addrspace(1) %21, i16 %22
  %24 = getelementptr i8, ptr addrspace(1) %23, i16 2
  %25 = load i16, ptr addrspace(1) %24
  %26 = load i16, ptr %4, !tbaa !2
  %27 = load i16, ptr addrspace(1) %1, !range !6
  %28 = icmp ult i16 %26, %27
  br i1 %28, label %b14, label %b15

b13:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b14:
  %29 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %30 = load ptr addrspace(1), ptr addrspace(1) %29
  %31 = mul i16 %26, 4
  %32 = getelementptr inbounds i8, ptr addrspace(1) %30, i16 %31
  %33 = getelementptr i8, ptr addrspace(1) %32, i16 2
  %34 = load i16, ptr addrspace(1) %33
  %35 = icmp sgt i16 %25, %34
  br i1 %35, label %b9, label %b10

b15:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b16:
  %36 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %37 = load ptr addrspace(1), ptr addrspace(1) %36
  %38 = mul i16 %16, 4
  %39 = getelementptr inbounds i8, ptr addrspace(1) %37, i16 %38
  store i8 0, ptr addrspace(1) %0
  %40 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr addrspace(1) %39, ptr addrspace(1) %40
  ret void

b17:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

define internal cc 1005 void @first_name(ptr addrspace(1) %0, ptr addrspace(1) noalias readonly dereferenceable(8) %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca [8 x i8]
  %3 = alloca [8 x i8]
  %4 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %5 = load ptr addrspace(1), ptr addrspace(1) %4
  %6 = load i16, ptr addrspace(1) %1
  %7 = icmp sge i16 %6, 1
  br i1 %7, label %b4, label %b3

b3:
  %8 = getelementptr i8, ptr @$str1, i16 6
  %9 = getelementptr i8, ptr %8, i16 -4
  %10 = load i16, ptr %9
  %11 = addrspacecast ptr %8 to ptr addrspace(1)
  store i16 %10, ptr %2, !tbaa !2
  %12 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 %10, ptr %12, !tbaa !2
  %13 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %11, ptr %13, !tbaa !2
  %14 = addrspacecast ptr %2 to ptr addrspace(1)
  %15 = load i16, ptr addrspace(1) %14, !tbaa !2
  store i16 %15, ptr addrspace(1) %0
  %16 = getelementptr i8, ptr addrspace(1) %14, i16 2
  %17 = load i16, ptr addrspace(1) %16, !tbaa !2
  %18 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %17, ptr addrspace(1) %18
  %19 = getelementptr i8, ptr addrspace(1) %14, i16 4
  %20 = load ptr addrspace(1), ptr addrspace(1) %19, !tbaa !2
  %21 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store ptr addrspace(1) %20, ptr addrspace(1) %21
  ret void

b4:
  %22 = getelementptr inbounds i8, ptr addrspace(1) %5, i16 0
  %23 = getelementptr inbounds i8, ptr addrspace(1) %5, i16 0
  %24 = load ptr, ptr addrspace(1) %23
  %25 = getelementptr i8, ptr %24, i16 -4
  %26 = load i16, ptr %25
  %27 = addrspacecast ptr %24 to ptr addrspace(1)
  store i16 %26, ptr %3, !tbaa !2
  %28 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 %26, ptr %28, !tbaa !2
  %29 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %27, ptr %29, !tbaa !2
  %30 = addrspacecast ptr %3 to ptr addrspace(1)
  %31 = load i16, ptr addrspace(1) %30, !tbaa !2
  store i16 %31, ptr addrspace(1) %0
  %32 = getelementptr i8, ptr addrspace(1) %30, i16 2
  %33 = load i16, ptr addrspace(1) %32, !tbaa !2
  %34 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %33, ptr addrspace(1) %34
  %35 = getelementptr i8, ptr addrspace(1) %30, i16 4
  %36 = load ptr addrspace(1), ptr addrspace(1) %35, !tbaa !2
  %37 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store ptr addrspace(1) %36, ptr addrspace(1) %37
  ret void
}

define internal cc 1005 void @word(ptr addrspace(1) %0, ptr addrspace(1) noalias readonly dereferenceable(8) %1, i16 %2) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %3 = alloca [8 x i8]
  %4 = alloca i16
  store i16 %2, ptr %4, !tbaa !2
  br label %b2

b2:
  %5 = load i16, ptr %4, !tbaa !2
  %6 = load i16, ptr addrspace(1) %1
  %7 = icmp ult i16 %5, %6
  br i1 %7, label %b5, label %b4

b3:
  %8 = load i16, ptr %4, !tbaa !2
  %9 = add i16 %8, 1
  store i16 %9, ptr %4, !tbaa !2
  br label %b2

b4:
  %10 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %11 = load ptr addrspace(1), ptr addrspace(1) %10
  %12 = load i16, ptr addrspace(1) %1
  %13 = load i16, ptr %4, !tbaa !2
  %14 = icmp ule i16 %13, %12
  br i1 %14, label %b8, label %b9

b5:
  %15 = load i16, ptr %4, !tbaa !2
  %16 = load i16, ptr addrspace(1) %1
  %17 = icmp ult i16 %15, %16
  br i1 %17, label %b6, label %b7

b6:
  %18 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %19 = load ptr addrspace(1), ptr addrspace(1) %18
  %20 = getelementptr inbounds i8, ptr addrspace(1) %19, i16 %15
  %21 = load i8, ptr addrspace(1) %20
  %22 = icmp ne i8 %21, 32
  br i1 %22, label %b3, label %b4

b7:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b8:
  %23 = icmp ule i16 %2, %13
  br i1 %23, label %b10, label %b11

b9:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b10:
  %24 = getelementptr inbounds i8, ptr addrspace(1) %11, i16 %2
  %25 = sub i16 %13, %2
  store i16 %25, ptr %3, !tbaa !2
  %26 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 %25, ptr %26, !tbaa !2
  %27 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %24, ptr %27, !tbaa !2
  %28 = addrspacecast ptr %3 to ptr addrspace(1)
  %29 = load i16, ptr addrspace(1) %28, !tbaa !2
  store i16 %29, ptr addrspace(1) %0
  %30 = getelementptr i8, ptr addrspace(1) %28, i16 2
  %31 = load i16, ptr addrspace(1) %30, !tbaa !2
  %32 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %31, ptr addrspace(1) %32
  %33 = getelementptr i8, ptr addrspace(1) %28, i16 4
  %34 = load ptr addrspace(1), ptr addrspace(1) %33, !tbaa !2
  %35 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store ptr addrspace(1) %34, ptr addrspace(1) %35
  ret void

b11:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca [8 x i8]
  %2 = alloca [8 x i8]
  %3 = alloca [8 x i8]
  %4 = alloca [8 x i8]
  %5 = alloca [8 x i8]
  %6 = alloca i16
  %7 = alloca ptr
  %8 = alloca [8 x i8]
  %9 = alloca [8 x i8]
  %10 = alloca [8 x i8]
  %11 = alloca [6 x i8]
  %12 = alloca ptr
  %13 = alloca [8 x i8]
  %14 = alloca [4 x i8]
  %15 = alloca [4 x i8]
  %16 = getelementptr i8, ptr @$str2, i16 6
  store ptr %16, ptr %15, !tbaa !2
  %17 = getelementptr inbounds i8, ptr %15, i16 2
  store i16 31, ptr %17, !tbaa !2
  %18 = getelementptr i8, ptr @$str3, i16 6
  store ptr %18, ptr %14, !tbaa !2
  %19 = getelementptr inbounds i8, ptr %14, i16 2
  store i16 45, ptr %19, !tbaa !2
  %20 = addrspacecast ptr %15 to ptr addrspace(1)
  %21 = addrspacecast ptr %14 to ptr addrspace(1)
  %22 = call cc 1005 addrspace(1) ptr addrspace(1) @leader(ptr addrspace(1) %20, ptr addrspace(1) %21)
  %23 = load ptr, ptr addrspace(1) %22
  %24 = getelementptr i8, ptr %23, i16 -4
  %25 = load i16, ptr %24
  %26 = addrspacecast ptr %23 to ptr addrspace(1)
  store i16 %25, ptr %13, !tbaa !2
  %27 = getelementptr inbounds i8, ptr %13, i16 2
  store i16 %25, ptr %27, !tbaa !2
  %28 = getelementptr inbounds i8, ptr %13, i16 4
  store ptr addrspace(1) %26, ptr %28, !tbaa !2
  %29 = addrspacecast ptr %13 to ptr addrspace(1)
  %30 = getelementptr i8, ptr @$str4, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %30)
  call cc 1005 addrspace(1) void @N$PV(ptr addrspace(1) %29)
  call cc 1005 addrspace(1) void @N$PN()
  %31 = getelementptr i8, ptr @$str5, i16 6
  %32 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %31, i16 3, i16 4)
  %33 = getelementptr inbounds i8, ptr %32, i16 0
  %34 = getelementptr i8, ptr @$str6, i16 6
  store ptr %34, ptr %33
  %35 = getelementptr i8, ptr %33, i16 2
  store i16 12, ptr %35
  %36 = getelementptr inbounds i8, ptr %32, i16 4
  %37 = getelementptr i8, ptr @$str7, i16 6
  store ptr %37, ptr %36
  %38 = getelementptr i8, ptr %36, i16 2
  store i16 58, ptr %38
  %39 = getelementptr inbounds i8, ptr %32, i16 8
  %40 = getelementptr i8, ptr @$str8, i16 6
  store ptr %40, ptr %39
  %41 = getelementptr i8, ptr %39, i16 2
  store i16 40, ptr %41
  store ptr %32, ptr %12, !tbaa !2
  %42 = addrspacecast ptr %11 to ptr addrspace(1)
  %43 = load ptr, ptr %12, !tbaa !2
  %44 = getelementptr i8, ptr %43, i16 -4
  %45 = load i16, ptr %44
  %46 = addrspacecast ptr %43 to ptr addrspace(1)
  store i16 %45, ptr %10, !tbaa !2
  %47 = getelementptr inbounds i8, ptr %10, i16 2
  store i16 %45, ptr %47, !tbaa !2
  %48 = getelementptr inbounds i8, ptr %10, i16 4
  store ptr addrspace(1) %46, ptr %48, !tbaa !2
  %49 = addrspacecast ptr %10 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @best(ptr addrspace(1) %42, ptr addrspace(1) %49)
  %50 = load i8, ptr %11, !tbaa !2, !range !8
  %51 = icmp eq i8 %50, 0
  br i1 %51, label %b4, label %b3

b2:
  %52 = addrspacecast ptr %9 to ptr addrspace(1)
  %53 = load ptr, ptr %12, !tbaa !2
  %54 = getelementptr i8, ptr %53, i16 -4
  %55 = load i16, ptr %54
  %56 = addrspacecast ptr %53 to ptr addrspace(1)
  store i16 %55, ptr %8, !tbaa !2
  %57 = getelementptr inbounds i8, ptr %8, i16 2
  store i16 %55, ptr %57, !tbaa !2
  %58 = getelementptr inbounds i8, ptr %8, i16 4
  store ptr addrspace(1) %56, ptr %58, !tbaa !2
  %59 = addrspacecast ptr %8 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @first_name(ptr addrspace(1) %52, ptr addrspace(1) %59)
  %60 = getelementptr i8, ptr @$str12, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %60)
  call cc 1005 addrspace(1) void @N$PV(ptr addrspace(1) %52)
  call cc 1005 addrspace(1) void @N$PN()
  %61 = getelementptr i8, ptr @$str13, i16 6
  store ptr %61, ptr %7, !tbaa !2
  store i16 0, ptr %6, !tbaa !2
  %62 = addrspacecast ptr %5 to ptr addrspace(1)
  %63 = load ptr, ptr %7, !tbaa !2
  %64 = getelementptr i8, ptr %63, i16 -4
  %65 = load i16, ptr %64
  %66 = addrspacecast ptr %63 to ptr addrspace(1)
  store i16 %65, ptr %4, !tbaa !2
  %67 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 %65, ptr %67, !tbaa !2
  %68 = getelementptr inbounds i8, ptr %4, i16 4
  store ptr addrspace(1) %66, ptr %68, !tbaa !2
  %69 = addrspacecast ptr %4 to ptr addrspace(1)
  %70 = load i16, ptr %6, !tbaa !2
  call cc 1005 addrspace(1) void @word(ptr addrspace(1) %62, ptr addrspace(1) %69, i16 %70)
  %71 = addrspacecast ptr %3 to ptr addrspace(1)
  %72 = load i16, ptr addrspace(1) %62, !tbaa !2
  store i16 %72, ptr addrspace(1) %71, !tbaa !2
  %73 = getelementptr i8, ptr addrspace(1) %62, i16 2
  %74 = load i16, ptr addrspace(1) %73, !tbaa !2
  %75 = getelementptr i8, ptr addrspace(1) %71, i16 2
  store i16 %74, ptr addrspace(1) %75, !tbaa !2
  %76 = getelementptr i8, ptr addrspace(1) %62, i16 4
  %77 = load ptr addrspace(1), ptr addrspace(1) %76, !tbaa !2
  %78 = getelementptr i8, ptr addrspace(1) %71, i16 4
  store ptr addrspace(1) %77, ptr addrspace(1) %78, !tbaa !2
  br label %b6

b3:
  %79 = getelementptr i8, ptr @$str11, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %79)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b2

b4:
  %80 = getelementptr inbounds i8, ptr %11, i16 2
  %81 = load ptr addrspace(1), ptr %80, !tbaa !2
  %82 = getelementptr inbounds i8, ptr %11, i16 2
  %83 = load ptr addrspace(1), ptr %82, !tbaa !2
  %84 = getelementptr i8, ptr @$str9, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %84)
  %85 = load ptr, ptr addrspace(1) %83
  call cc 1005 addrspace(1) void @N$PS(ptr %85)
  %86 = getelementptr i8, ptr @$str10, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %86)
  %87 = getelementptr i8, ptr addrspace(1) %83, i16 2
  %88 = load i16, ptr addrspace(1) %87
  call cc 1005 addrspace(1) void @N$PI2(i16 %88)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b2

b6:
  %89 = load i16, ptr addrspace(1) %71
  %90 = icmp ugt i16 %89, 0
  br i1 %90, label %b7, label %b8

b7:
  %91 = getelementptr i8, ptr @$str14, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %91)
  call cc 1005 addrspace(1) void @N$PV(ptr addrspace(1) %71)
  %92 = getelementptr i8, ptr @$str15, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %92)
  call cc 1005 addrspace(1) void @N$PN()
  %93 = load i16, ptr %6, !tbaa !2
  %94 = load i16, ptr addrspace(1) %71
  %95 = add i16 %94, 1
  %96 = add i16 %93, %95
  store i16 %96, ptr %6, !tbaa !2
  %97 = load i16, ptr %6, !tbaa !2
  %98 = load ptr, ptr %7, !tbaa !2
  %99 = getelementptr i8, ptr %98, i16 -4
  %100 = load i16, ptr %99
  %101 = icmp uge i16 %97, %100
  br i1 %101, label %b9, label %b10

b8:
  %102 = load ptr, ptr %7, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %102)
  %103 = load ptr, ptr %12, !tbaa !2
  %104 = icmp ne ptr %103, null
  br i1 %104, label %b13, label %b12

b9:
  br label %b8

b10:
  br label %b11

b11:
  %105 = addrspacecast ptr %2 to ptr addrspace(1)
  %106 = load ptr, ptr %7, !tbaa !2
  %107 = getelementptr i8, ptr %106, i16 -4
  %108 = load i16, ptr %107
  %109 = addrspacecast ptr %106 to ptr addrspace(1)
  store i16 %108, ptr %1, !tbaa !2
  %110 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 %108, ptr %110, !tbaa !2
  %111 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %109, ptr %111, !tbaa !2
  %112 = addrspacecast ptr %1 to ptr addrspace(1)
  %113 = load i16, ptr %6, !tbaa !2
  call cc 1005 addrspace(1) void @word(ptr addrspace(1) %105, ptr addrspace(1) %112, i16 %113)
  %114 = load i16, ptr addrspace(1) %105, !tbaa !2
  store i16 %114, ptr addrspace(1) %71, !tbaa !2
  %115 = getelementptr i8, ptr addrspace(1) %105, i16 2
  %116 = load i16, ptr addrspace(1) %115, !tbaa !2
  %117 = getelementptr i8, ptr addrspace(1) %71, i16 2
  store i16 %116, ptr addrspace(1) %117, !tbaa !2
  %118 = getelementptr i8, ptr addrspace(1) %105, i16 4
  %119 = load ptr addrspace(1), ptr addrspace(1) %118, !tbaa !2
  %120 = getelementptr i8, ptr addrspace(1) %71, i16 4
  store ptr addrspace(1) %119, ptr addrspace(1) %120, !tbaa !2
  br label %b6

b12:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %103)
  %121 = load ptr, ptr %14, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %121)
  %122 = load ptr, ptr %15, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %122)
  ret i16 0

b13:
  %123 = getelementptr i8, ptr %103, i16 -4
  %124 = load i16, ptr %123
  store i16 0, ptr %0, !tbaa !2
  br label %b14

b14:
  %125 = load i16, ptr %0, !tbaa !2
  %126 = icmp ult i16 %125, %124
  br i1 %126, label %b16, label %b15

b15:
  br label %b12

b16:
  %127 = mul i16 %125, 4
  %128 = getelementptr inbounds i8, ptr %103, i16 %127
  %129 = load ptr, ptr %128
  call cc 1005 addrspace(1) void @N$BDRP(ptr %129)
  %130 = add i16 %125, 1
  store i16 %130, ptr %0, !tbaa !2
  br label %b14
}

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PV(ptr addrspace(1)) addrspace(1)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i16 0, i16 16384}
!6 = !{i16 0, i16 16384}
!7 = !{i16 0, i16 16384}
!8 = !{i8 0, i8 2}
