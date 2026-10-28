target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [15 x i8] c"\08\00\08\00\08\00 is busy\00"
@$str2 = internal constant [12 x i8] c"\08\00\05\00\05\00open \00"
@$str3 = internal constant [13 x i8] c"\08\00\06\00\06\00close \00"
@$str4 = internal constant [12 x i8] c"\08\00\05\00\05\00link \00"
@$str5 = internal constant [8 x i8] c"\08\00\01\00\01\00-\00"
@$str6 = internal constant [12 x i8] c"\08\00\05\00\05\00 down\00"
@$str7 = internal constant [10 x i8] c"\08\00\03\00\03\00log\00"
@$str8 = internal constant [11 x i8] c"\08\00\04\00\04\00stop\00"
@$str9 = internal constant [12 x i8] c"\08\00\05\00\05\00 saw \00"
@$str10 = internal constant [15 x i8] c"\08\00\08\00\08\00visited \00"
@$str11 = internal constant [19 x i8] c"\08\00\0C\00\0C\00visit failed\00"
@$str12 = internal constant [8 x i8] c"\08\00\01\00\01\00a\00"
@$str13 = internal constant [8 x i8] c"\08\00\01\00\01\00b\00"
@$str14 = internal constant [11 x i8] c"\08\00\04\00\04\00 to \00"
@$str15 = internal constant [14 x i8] c"\08\00\07\00\07\00no link\00"
@$str16 = internal constant [9 x i8] c"\08\00\02\00\02\00--\00"
@$str17 = internal constant [8 x i8] c"\08\00\01\00\01\00c\00"
@$str18 = internal constant [8 x i8] c"\08\00\01\00\01\00d\00"
@$str19 = internal constant [13 x i8] c"\08\00\06\00\06\00linked\00"
@$str20 = internal constant [8 x i8] c"\08\00\01\00\01\00e\00"
@$str21 = internal constant [12 x i8] c"\08\00\05\00\05\00kept \00"
@$str22 = internal constant [13 x i8] c"\08\00\06\00\06\00closed\00"
@$str23 = internal constant [8 x i8] c"\08\00\01\00\01\00f\00"
@$str24 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str25 = internal constant [8 x i8] c"\08\00\01\00\01\00p\00"
@$str26 = internal constant [8 x i8] c"\08\00\01\00\01\00q\00"
@$str27 = internal constant [8 x i8] c"\08\00\01\00\01\00r\00"
@$str28 = internal constant [8 x i8] c"\08\00\01\00\01\00z\00"

define internal cc 1005 void @Channel.open(ptr addrspace(1) %0, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %1, i8 %2) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %3 = icmp ne i8 %2, 0
  br i1 %3, label %b3, label %b2

b2:
  call cc 1005 addrspace(1) void @N$PV(ptr addrspace(1) %1)
  %4 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %4)
  call cc 1005 addrspace(1) void @N$PN()
  %5 = getelementptr i8, ptr addrspace(1) %0, i16 3
  store i8 0, ptr addrspace(1) %5
  store i16 1, ptr addrspace(1) %0
  %6 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 0, ptr addrspace(1) %6
  ret void

b3:
  br label %b4

b4:
  %7 = getelementptr i8, ptr @$str2, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %7)
  call cc 1005 addrspace(1) void @N$PV(ptr addrspace(1) %1)
  call cc 1005 addrspace(1) void @N$PN()
  %8 = call cc 1005 addrspace(1) ptr @N$VCPY(ptr addrspace(1) %1)
  store i16 0, ptr addrspace(1) %0
  %9 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr %8, ptr addrspace(1) %9
  ret void
}

define internal cc 1005 void @Channel.drop(ptr addrspace(1) nonnull dereferenceable(2) noalias nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = getelementptr i8, ptr @$str3, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %1)
  %2 = load ptr, ptr addrspace(1) %0
  call cc 1005 addrspace(1) void @N$PS(ptr %2)
  call cc 1005 addrspace(1) void @N$PN()
  ret void
}

define internal cc 1005 void @Link.drop(ptr addrspace(1) nonnull dereferenceable(4) noalias nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = getelementptr i8, ptr @$str4, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %1)
  %2 = load ptr, ptr addrspace(1) %0
  call cc 1005 addrspace(1) void @N$PS(ptr %2)
  %3 = getelementptr i8, ptr @$str5, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %3)
  %4 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %5 = load ptr, ptr addrspace(1) %4
  call cc 1005 addrspace(1) void @N$PS(ptr %5)
  %6 = getelementptr i8, ptr @$str6, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %6)
  call cc 1005 addrspace(1) void @N$PN()
  ret void
}

define internal cc 1005 void @connect(ptr addrspace(1) %0, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %1, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %2, i8 %3) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %4 = alloca i8
  %5 = alloca [4 x i8]
  %6 = alloca [2 x i8]
  %7 = alloca i8
  %8 = alloca [4 x i8]
  %9 = alloca [2 x i8]
  %10 = addrspacecast ptr %8 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Channel.open(ptr addrspace(1) %10, ptr addrspace(1) %1, i8 1)
  %11 = load i8, ptr %8, !tbaa !2, !range !17
  %12 = icmp eq i8 %11, 1
  br i1 %12, label %b2, label %b3

b2:
  %13 = getelementptr inbounds i8, ptr %8, i16 2
  %14 = load i8, ptr %13, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %15 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 %14, ptr addrspace(1) %15
  ret void

b3:
  %16 = getelementptr inbounds i8, ptr %8, i16 2
  %17 = load ptr, ptr %16, !tbaa !2
  store ptr %17, ptr %9, !tbaa !2
  store i8 1, ptr %7, !tbaa !2
  %18 = addrspacecast ptr %5 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Channel.open(ptr addrspace(1) %18, ptr addrspace(1) %2, i8 %3)
  %19 = load i8, ptr %5, !tbaa !2, !range !17
  %20 = icmp eq i8 %19, 1
  br i1 %20, label %b4, label %b5

b4:
  %21 = getelementptr inbounds i8, ptr %5, i16 2
  %22 = load i8, ptr %21, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %23 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 %22, ptr addrspace(1) %23
  %24 = load i8, ptr %7, !tbaa !2, !range !5
  %25 = icmp ne i8 %24, 0
  br i1 %25, label %b7, label %b6

b5:
  %26 = getelementptr inbounds i8, ptr %5, i16 2
  %27 = load ptr, ptr %26, !tbaa !2
  store ptr %27, ptr %6, !tbaa !2
  store i8 1, ptr %4, !tbaa !2
  %28 = load ptr, ptr %9, !tbaa !2
  store i8 0, ptr %7, !tbaa !2
  %29 = load ptr, ptr %6, !tbaa !2
  store i8 0, ptr %4, !tbaa !2
  store i8 0, ptr addrspace(1) %0
  %30 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr %28, ptr addrspace(1) %30
  store ptr null, ptr %9, !tbaa !2
  %31 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store ptr %29, ptr addrspace(1) %31
  store ptr null, ptr %6, !tbaa !2
  %32 = load i8, ptr %4, !tbaa !2, !range !6
  %33 = icmp ne i8 %32, 0
  br i1 %33, label %b9, label %b8

b6:
  ret void

b7:
  %34 = addrspacecast ptr %9 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Channel.drop(ptr addrspace(1) %34)
  %35 = load ptr, ptr %9, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %35)
  br label %b6

b8:
  %36 = load i8, ptr %7, !tbaa !2, !range !7
  %37 = icmp ne i8 %36, 0
  br i1 %37, label %b11, label %b10

b9:
  %38 = addrspacecast ptr %6 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Channel.drop(ptr addrspace(1) %38)
  %39 = load ptr, ptr %6, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %39)
  br label %b8

b10:
  ret void

b11:
  %40 = addrspacecast ptr %9 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Channel.drop(ptr addrspace(1) %40)
  %41 = load ptr, ptr %9, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %41)
  br label %b10
}

define internal cc 1005 void @pass_on(ptr addrspace(1) %0, ptr addrspace(1) noalias %1, i8 %2) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %3 = alloca i8
  store i8 1, ptr %3, !tbaa !2
  %4 = icmp ne i8 %2, 0
  br i1 %4, label %b2, label %b3

b2:
  %5 = load ptr, ptr addrspace(1) %1
  store i8 0, ptr %3, !tbaa !2
  store i16 0, ptr addrspace(1) %0
  %6 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr %5, ptr addrspace(1) %6
  store ptr null, ptr addrspace(1) %1
  %7 = load i8, ptr %3, !tbaa !2, !range !8
  %8 = icmp ne i8 %7, 0
  br i1 %8, label %b6, label %b5

b3:
  br label %b4

b4:
  store i32 1, ptr addrspace(1) %0
  %9 = load i8, ptr %3, !tbaa !2, !range !9
  %10 = icmp ne i8 %9, 0
  br i1 %10, label %b8, label %b7

b5:
  ret void

b6:
  call cc 1005 addrspace(1) void @Channel.drop(ptr addrspace(1) %1)
  %11 = load ptr, ptr addrspace(1) %1
  call cc 1005 addrspace(1) void @N$BDRP(ptr %11)
  br label %b5

b7:
  ret void

b8:
  call cc 1005 addrspace(1) void @Channel.drop(ptr addrspace(1) %1)
  %12 = load ptr, ptr addrspace(1) %1
  call cc 1005 addrspace(1) void @N$BDRP(ptr %12)
  br label %b7
}

define internal cc 1005 i32 @visit(ptr addrspace(1) nocapture %0, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca [8 x i8]
  %3 = alloca [8 x i8]
  %4 = alloca i8
  %5 = alloca [8 x i8]
  %6 = alloca [8 x i8]
  %7 = alloca [4 x i8]
  %8 = alloca [2 x i8]
  %9 = alloca i16
  %10 = alloca i8
  %11 = alloca [8 x i8]
  %12 = alloca [4 x i8]
  %13 = alloca [2 x i8]
  %14 = alloca i16
  %15 = alloca [4 x i8]
  store i16 0, ptr %14, !tbaa !2
  %16 = addrspacecast ptr %12 to ptr addrspace(1)
  %17 = getelementptr i8, ptr @$str7, i16 6
  %18 = getelementptr i8, ptr %17, i16 -4
  %19 = load i16, ptr %18
  %20 = addrspacecast ptr %17 to ptr addrspace(1)
  store i16 %19, ptr %11, !tbaa !2
  %21 = getelementptr inbounds i8, ptr %11, i16 2
  store i16 %19, ptr %21, !tbaa !2
  %22 = getelementptr inbounds i8, ptr %11, i16 4
  store ptr addrspace(1) %20, ptr %22, !tbaa !2
  %23 = addrspacecast ptr %11 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Channel.open(ptr addrspace(1) %16, ptr addrspace(1) %23, i8 1)
  %24 = load i8, ptr %12, !tbaa !2, !range !18
  %25 = icmp eq i8 %24, 1
  br i1 %25, label %b2, label %b3

b2:
  %26 = getelementptr inbounds i8, ptr %12, i16 2
  %27 = load i8, ptr %26, !tbaa !2
  store i8 1, ptr %15, !tbaa !2
  %28 = getelementptr inbounds i8, ptr %15, i16 2
  store i8 %27, ptr %28, !tbaa !2
  %29 = addrspacecast ptr %15 to ptr addrspace(1)
  %30 = load i32, ptr addrspace(1) %29, !tbaa !2
  ret i32 %30

b3:
  %31 = getelementptr inbounds i8, ptr %12, i16 2
  %32 = load ptr, ptr %31, !tbaa !2
  store ptr %32, ptr %13, !tbaa !2
  store i8 1, ptr %10, !tbaa !2
  %33 = load ptr, ptr addrspace(1) %0
  %34 = getelementptr i8, ptr %33, i16 -4
  %35 = load i16, ptr %34
  store i16 0, ptr %9, !tbaa !2
  br label %b4

b4:
  %36 = load i16, ptr %9, !tbaa !2
  %37 = icmp ult i16 %36, %35
  br i1 %37, label %b5, label %b7

b5:
  %38 = mul i16 %36, 2
  %39 = getelementptr inbounds i8, ptr %33, i16 %38
  %40 = load ptr, ptr %39
  %41 = getelementptr i8, ptr %40, i16 -4
  %42 = load i16, ptr %41
  %43 = icmp eq i16 %42, 0
  br i1 %43, label %b8, label %b9

b6:
  %44 = load i16, ptr %9, !tbaa !2
  %45 = add i16 %44, 1
  store i16 %45, ptr %9, !tbaa !2
  br label %b4

b7:
  %46 = load ptr, ptr %13, !tbaa !2
  call cc 1005 addrspace(1) void @N$PS(ptr %46)
  %47 = getelementptr i8, ptr @$str9, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %47)
  %48 = load i16, ptr %14, !tbaa !2
  call cc 1005 addrspace(1) void @N$PU2(i16 %48)
  call cc 1005 addrspace(1) void @N$PN()
  %49 = load i8, ptr %10, !tbaa !2, !range !10
  %50 = icmp ne i8 %49, 0
  br i1 %50, label %b23, label %b22

b8:
  br label %b6

b9:
  br label %b10

b10:
  %51 = addrspacecast ptr %7 to ptr addrspace(1)
  %52 = load ptr, ptr %39
  %53 = getelementptr i8, ptr %52, i16 -4
  %54 = load i16, ptr %53
  %55 = addrspacecast ptr %52 to ptr addrspace(1)
  store i16 %54, ptr %6, !tbaa !2
  %56 = getelementptr inbounds i8, ptr %6, i16 2
  store i16 %54, ptr %56, !tbaa !2
  %57 = getelementptr inbounds i8, ptr %6, i16 4
  store ptr addrspace(1) %55, ptr %57, !tbaa !2
  %58 = addrspacecast ptr %6 to ptr addrspace(1)
  %59 = load ptr, ptr %39
  %60 = getelementptr i8, ptr %59, i16 -4
  %61 = load i16, ptr %60
  %62 = addrspacecast ptr %59 to ptr addrspace(1)
  store i16 %61, ptr %5, !tbaa !2
  %63 = getelementptr inbounds i8, ptr %5, i16 2
  store i16 %61, ptr %63, !tbaa !2
  %64 = getelementptr inbounds i8, ptr %5, i16 4
  store ptr addrspace(1) %62, ptr %64, !tbaa !2
  %65 = addrspacecast ptr %5 to ptr addrspace(1)
  %66 = call cc 1005 addrspace(1) i8 @N$VCMP(ptr addrspace(1) %65, ptr addrspace(1) %1)
  %67 = icmp ne i8 %66, 0
  %68 = zext i1 %67 to i8
  call cc 1005 addrspace(1) void @Channel.open(ptr addrspace(1) %51, ptr addrspace(1) %58, i8 %68)
  %69 = load i8, ptr %7, !tbaa !2, !range !18
  %70 = icmp eq i8 %69, 1
  br i1 %70, label %b11, label %b12

b11:
  %71 = getelementptr inbounds i8, ptr %7, i16 2
  %72 = load i8, ptr %71, !tbaa !2
  store i8 1, ptr %15, !tbaa !2
  %73 = getelementptr inbounds i8, ptr %15, i16 2
  store i8 %72, ptr %73, !tbaa !2
  %74 = load i8, ptr %10, !tbaa !2, !range !11
  %75 = icmp ne i8 %74, 0
  br i1 %75, label %b14, label %b13

b12:
  %76 = getelementptr inbounds i8, ptr %7, i16 2
  %77 = load ptr, ptr %76, !tbaa !2
  store ptr %77, ptr %8, !tbaa !2
  store i8 1, ptr %4, !tbaa !2
  %78 = load ptr, ptr %8, !tbaa !2
  %79 = getelementptr i8, ptr @$str8, i16 6
  %80 = getelementptr i8, ptr %78, i16 -4
  %81 = load i16, ptr %80
  %82 = addrspacecast ptr %78 to ptr addrspace(1)
  store i16 %81, ptr %3, !tbaa !2
  %83 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 %81, ptr %83, !tbaa !2
  %84 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %82, ptr %84, !tbaa !2
  %85 = addrspacecast ptr %3 to ptr addrspace(1)
  %86 = getelementptr i8, ptr %79, i16 -4
  %87 = load i16, ptr %86
  %88 = addrspacecast ptr %79 to ptr addrspace(1)
  store i16 %87, ptr %2, !tbaa !2
  %89 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 %87, ptr %89, !tbaa !2
  %90 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %88, ptr %90, !tbaa !2
  %91 = addrspacecast ptr %2 to ptr addrspace(1)
  %92 = call cc 1005 addrspace(1) i8 @N$VCMP(ptr addrspace(1) %85, ptr addrspace(1) %91)
  %93 = icmp eq i8 %92, 0
  br i1 %93, label %b15, label %b16

b13:
  %94 = addrspacecast ptr %15 to ptr addrspace(1)
  %95 = load i32, ptr addrspace(1) %94, !tbaa !2
  ret i32 %95

b14:
  %96 = addrspacecast ptr %13 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Channel.drop(ptr addrspace(1) %96)
  %97 = load ptr, ptr %13, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %97)
  br label %b13

b15:
  %98 = load i8, ptr %4, !tbaa !2, !range !12
  %99 = icmp ne i8 %98, 0
  br i1 %99, label %b19, label %b18

b16:
  br label %b17

b17:
  %100 = load i16, ptr %14, !tbaa !2
  %101 = add i16 %100, 1
  store i16 %101, ptr %14, !tbaa !2
  %102 = load i8, ptr %4, !tbaa !2, !range !13
  %103 = icmp ne i8 %102, 0
  br i1 %103, label %b21, label %b20

b18:
  br label %b7

b19:
  %104 = addrspacecast ptr %8 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Channel.drop(ptr addrspace(1) %104)
  %105 = load ptr, ptr %8, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %105)
  br label %b18

b20:
  br label %b6

b21:
  %106 = addrspacecast ptr %8 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Channel.drop(ptr addrspace(1) %106)
  %107 = load ptr, ptr %8, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %107)
  br label %b20

b22:
  %108 = load i16, ptr %14, !tbaa !2
  store i16 0, ptr %15, !tbaa !2
  %109 = getelementptr inbounds i8, ptr %15, i16 2
  store i16 %108, ptr %109, !tbaa !2
  %110 = addrspacecast ptr %15 to ptr addrspace(1)
  %111 = load i32, ptr addrspace(1) %110, !tbaa !2
  ret i32 %111

b23:
  %112 = addrspacecast ptr %13 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Channel.drop(ptr addrspace(1) %112)
  %113 = load ptr, ptr %13, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %113)
  br label %b22
}

define internal cc 1005 void @show(ptr addrspace(1) noalias %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = load i8, ptr addrspace(1) %0, !range !19
  %2 = icmp eq i8 %1, 0
  br i1 %2, label %b4, label %b3

b2:
  ret void

b3:
  %3 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %4 = load i8, ptr addrspace(1) %3
  %5 = getelementptr i8, ptr @$str11, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %5)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b2

b4:
  %6 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %7 = load i16, ptr addrspace(1) %6
  %8 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %9 = load i16, ptr addrspace(1) %8
  %10 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %11 = getelementptr i8, ptr @$str10, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %11)
  %12 = load i16, ptr addrspace(1) %10
  call cc 1005 addrspace(1) void @N$PU2(i16 %12)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b2
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca [8 x i8]
  %3 = alloca [4 x i8]
  %4 = alloca [4 x i8]
  %5 = alloca ptr
  %6 = alloca [8 x i8]
  %7 = alloca [4 x i8]
  %8 = alloca [4 x i8]
  %9 = alloca ptr
  %10 = alloca i8
  %11 = alloca [2 x i8]
  %12 = alloca [2 x i8]
  %13 = alloca [4 x i8]
  %14 = alloca i8
  %15 = alloca [2 x i8]
  %16 = alloca [2 x i8]
  %17 = alloca [4 x i8]
  %18 = alloca [8 x i8]
  %19 = alloca [8 x i8]
  %20 = alloca [6 x i8]
  %21 = alloca i8
  %22 = alloca [4 x i8]
  %23 = alloca [8 x i8]
  %24 = alloca [8 x i8]
  %25 = alloca [6 x i8]
  %26 = addrspacecast ptr %25 to ptr addrspace(1)
  %27 = getelementptr i8, ptr @$str12, i16 6
  %28 = getelementptr i8, ptr %27, i16 -4
  %29 = load i16, ptr %28
  %30 = addrspacecast ptr %27 to ptr addrspace(1)
  store i16 %29, ptr %24, !tbaa !2
  %31 = getelementptr inbounds i8, ptr %24, i16 2
  store i16 %29, ptr %31, !tbaa !2
  %32 = getelementptr inbounds i8, ptr %24, i16 4
  store ptr addrspace(1) %30, ptr %32, !tbaa !2
  %33 = addrspacecast ptr %24 to ptr addrspace(1)
  %34 = getelementptr i8, ptr @$str13, i16 6
  %35 = getelementptr i8, ptr %34, i16 -4
  %36 = load i16, ptr %35
  %37 = addrspacecast ptr %34 to ptr addrspace(1)
  store i16 %36, ptr %23, !tbaa !2
  %38 = getelementptr inbounds i8, ptr %23, i16 2
  store i16 %36, ptr %38, !tbaa !2
  %39 = getelementptr inbounds i8, ptr %23, i16 4
  store ptr addrspace(1) %37, ptr %39, !tbaa !2
  %40 = addrspacecast ptr %23 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @connect(ptr addrspace(1) %26, ptr addrspace(1) %33, ptr addrspace(1) %40, i8 1)
  %41 = load i8, ptr %25, !tbaa !2, !range !20
  %42 = icmp eq i8 %41, 0
  br i1 %42, label %b4, label %b3

b2:
  %43 = getelementptr i8, ptr @$str16, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %43)
  call cc 1005 addrspace(1) void @N$PN()
  %44 = addrspacecast ptr %20 to ptr addrspace(1)
  %45 = getelementptr i8, ptr @$str17, i16 6
  %46 = getelementptr i8, ptr %45, i16 -4
  %47 = load i16, ptr %46
  %48 = addrspacecast ptr %45 to ptr addrspace(1)
  store i16 %47, ptr %19, !tbaa !2
  %49 = getelementptr inbounds i8, ptr %19, i16 2
  store i16 %47, ptr %49, !tbaa !2
  %50 = getelementptr inbounds i8, ptr %19, i16 4
  store ptr addrspace(1) %48, ptr %50, !tbaa !2
  %51 = addrspacecast ptr %19 to ptr addrspace(1)
  %52 = getelementptr i8, ptr @$str18, i16 6
  %53 = getelementptr i8, ptr %52, i16 -4
  %54 = load i16, ptr %53
  %55 = addrspacecast ptr %52 to ptr addrspace(1)
  store i16 %54, ptr %18, !tbaa !2
  %56 = getelementptr inbounds i8, ptr %18, i16 2
  store i16 %54, ptr %56, !tbaa !2
  %57 = getelementptr inbounds i8, ptr %18, i16 4
  store ptr addrspace(1) %55, ptr %57, !tbaa !2
  %58 = addrspacecast ptr %18 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @connect(ptr addrspace(1) %44, ptr addrspace(1) %51, ptr addrspace(1) %58, i8 0)
  %59 = load i8, ptr %20, !tbaa !2, !range !20
  %60 = icmp eq i8 %59, 0
  br i1 %60, label %b12, label %b11

b3:
  %61 = getelementptr inbounds i8, ptr %25, i16 2
  %62 = load i8, ptr %61, !tbaa !2
  %63 = getelementptr i8, ptr @$str15, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %63)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b2

b4:
  %64 = getelementptr inbounds i8, ptr %25, i16 2
  %65 = load ptr, ptr %64, !tbaa !2
  %66 = getelementptr inbounds i8, ptr %25, i16 4
  %67 = load ptr, ptr %66, !tbaa !2
  store ptr %65, ptr %22, !tbaa !2
  %68 = getelementptr inbounds i8, ptr %22, i16 2
  store ptr %67, ptr %68, !tbaa !2
  store i8 1, ptr %21, !tbaa !2
  %69 = load ptr, ptr %22, !tbaa !2
  call cc 1005 addrspace(1) void @N$PS(ptr %69)
  %70 = getelementptr i8, ptr @$str14, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %70)
  %71 = getelementptr inbounds i8, ptr %22, i16 2
  %72 = load ptr, ptr %71, !tbaa !2
  call cc 1005 addrspace(1) void @N$PS(ptr %72)
  call cc 1005 addrspace(1) void @N$PN()
  %73 = load i8, ptr %21, !tbaa !2, !range !14
  %74 = icmp ne i8 %73, 0
  br i1 %74, label %b6, label %b5

b5:
  br label %b2

b6:
  %75 = addrspacecast ptr %22 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Link.drop(ptr addrspace(1) %75)
  %76 = getelementptr inbounds i8, ptr %22, i16 2
  %77 = addrspacecast ptr %76 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Channel.drop(ptr addrspace(1) %77)
  %78 = getelementptr inbounds i8, ptr %22, i16 2
  %79 = load ptr, ptr %78, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %79)
  %80 = addrspacecast ptr %22 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Channel.drop(ptr addrspace(1) %80)
  %81 = load ptr, ptr %22, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %81)
  br label %b5

b10:
  %82 = getelementptr i8, ptr @$str16, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %82)
  call cc 1005 addrspace(1) void @N$PN()
  %83 = addrspacecast ptr %17 to ptr addrspace(1)
  %84 = getelementptr i8, ptr @$str20, i16 6
  store ptr %84, ptr %16, !tbaa !2
  %85 = addrspacecast ptr %16 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @pass_on(ptr addrspace(1) %83, ptr addrspace(1) %85, i8 1)
  %86 = load i8, ptr %17, !tbaa !2, !range !20
  %87 = icmp eq i8 %86, 0
  br i1 %87, label %b18, label %b17

b11:
  %88 = getelementptr inbounds i8, ptr %20, i16 2
  %89 = load i8, ptr %88, !tbaa !2
  %90 = getelementptr i8, ptr @$str15, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %90)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b10

b12:
  %91 = getelementptr inbounds i8, ptr %20, i16 2
  %92 = addrspacecast ptr %91 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Link.drop(ptr addrspace(1) %92)
  %93 = getelementptr inbounds i8, ptr %20, i16 4
  %94 = addrspacecast ptr %93 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Channel.drop(ptr addrspace(1) %94)
  %95 = getelementptr inbounds i8, ptr %20, i16 4
  %96 = load ptr, ptr %95, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %96)
  %97 = getelementptr inbounds i8, ptr %20, i16 2
  %98 = addrspacecast ptr %97 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Channel.drop(ptr addrspace(1) %98)
  %99 = getelementptr inbounds i8, ptr %20, i16 2
  %100 = load ptr, ptr %99, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %100)
  %101 = getelementptr i8, ptr @$str19, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %101)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b10

b16:
  %102 = addrspacecast ptr %13 to ptr addrspace(1)
  %103 = getelementptr i8, ptr @$str23, i16 6
  store ptr %103, ptr %12, !tbaa !2
  %104 = addrspacecast ptr %12 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @pass_on(ptr addrspace(1) %102, ptr addrspace(1) %104, i8 0)
  %105 = load i8, ptr %13, !tbaa !2, !range !20
  %106 = icmp eq i8 %105, 0
  br i1 %106, label %b26, label %b25

b17:
  %107 = getelementptr i8, ptr @$str22, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %107)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b16

b18:
  %108 = getelementptr inbounds i8, ptr %17, i16 2
  %109 = load ptr, ptr %108, !tbaa !2
  store ptr %109, ptr %15, !tbaa !2
  store i8 1, ptr %14, !tbaa !2
  %110 = getelementptr i8, ptr @$str21, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %110)
  %111 = load ptr, ptr %15, !tbaa !2
  call cc 1005 addrspace(1) void @N$PS(ptr %111)
  call cc 1005 addrspace(1) void @N$PN()
  %112 = load i8, ptr %14, !tbaa !2, !range !15
  %113 = icmp ne i8 %112, 0
  br i1 %113, label %b20, label %b19

b19:
  br label %b16

b20:
  %114 = addrspacecast ptr %15 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Channel.drop(ptr addrspace(1) %114)
  %115 = load ptr, ptr %15, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %115)
  br label %b19

b24:
  %116 = getelementptr i8, ptr @$str16, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %116)
  call cc 1005 addrspace(1) void @N$PN()
  %117 = getelementptr i8, ptr @$str24, i16 6
  %118 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %117, i16 5, i16 2)
  %119 = getelementptr inbounds i8, ptr %118, i16 0
  %120 = getelementptr i8, ptr @$str25, i16 6
  store ptr %120, ptr %119
  %121 = getelementptr inbounds i8, ptr %118, i16 2
  %122 = getelementptr i8, ptr @$str24, i16 6
  store ptr %122, ptr %121
  %123 = getelementptr inbounds i8, ptr %118, i16 4
  %124 = getelementptr i8, ptr @$str26, i16 6
  store ptr %124, ptr %123
  %125 = getelementptr inbounds i8, ptr %118, i16 6
  %126 = getelementptr i8, ptr @$str8, i16 6
  store ptr %126, ptr %125
  %127 = getelementptr inbounds i8, ptr %118, i16 8
  %128 = getelementptr i8, ptr @$str27, i16 6
  store ptr %128, ptr %127
  store ptr %118, ptr %9, !tbaa !2
  %129 = addrspacecast ptr %9 to ptr addrspace(1)
  %130 = getelementptr i8, ptr @$str28, i16 6
  %131 = getelementptr i8, ptr %130, i16 -4
  %132 = load i16, ptr %131
  %133 = addrspacecast ptr %130 to ptr addrspace(1)
  store i16 %132, ptr %6, !tbaa !2
  %134 = getelementptr inbounds i8, ptr %6, i16 2
  store i16 %132, ptr %134, !tbaa !2
  %135 = getelementptr inbounds i8, ptr %6, i16 4
  store ptr addrspace(1) %133, ptr %135, !tbaa !2
  %136 = addrspacecast ptr %6 to ptr addrspace(1)
  %137 = call cc 1005 addrspace(1) i32 @visit(ptr addrspace(1) %129, ptr addrspace(1) %136)
  %138 = addrspacecast ptr %7 to ptr addrspace(1)
  store i32 %137, ptr addrspace(1) %138, !tbaa !2
  call void @llvm.memcpy.p0.p0.i16(ptr %8, ptr %7, i16 4, i1 false)
  %139 = addrspacecast ptr %8 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @show(ptr addrspace(1) %139)
  %140 = getelementptr i8, ptr @$str24, i16 6
  %141 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %140, i16 3, i16 2)
  %142 = getelementptr inbounds i8, ptr %141, i16 0
  %143 = getelementptr i8, ptr @$str25, i16 6
  store ptr %143, ptr %142
  %144 = getelementptr inbounds i8, ptr %141, i16 2
  %145 = getelementptr i8, ptr @$str26, i16 6
  store ptr %145, ptr %144
  %146 = getelementptr inbounds i8, ptr %141, i16 4
  %147 = getelementptr i8, ptr @$str27, i16 6
  store ptr %147, ptr %146
  store ptr %141, ptr %5, !tbaa !2
  %148 = addrspacecast ptr %5 to ptr addrspace(1)
  %149 = getelementptr i8, ptr @$str26, i16 6
  %150 = getelementptr i8, ptr %149, i16 -4
  %151 = load i16, ptr %150
  %152 = addrspacecast ptr %149 to ptr addrspace(1)
  store i16 %151, ptr %2, !tbaa !2
  %153 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 %151, ptr %153, !tbaa !2
  %154 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %152, ptr %154, !tbaa !2
  %155 = addrspacecast ptr %2 to ptr addrspace(1)
  %156 = call cc 1005 addrspace(1) i32 @visit(ptr addrspace(1) %148, ptr addrspace(1) %155)
  %157 = addrspacecast ptr %3 to ptr addrspace(1)
  store i32 %156, ptr addrspace(1) %157, !tbaa !2
  call void @llvm.memcpy.p0.p0.i16(ptr %4, ptr %3, i16 4, i1 false)
  %158 = addrspacecast ptr %4 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @show(ptr addrspace(1) %158)
  %159 = load ptr, ptr %5, !tbaa !2
  %160 = icmp ne ptr %159, null
  br i1 %160, label %b33, label %b32

b25:
  %161 = getelementptr i8, ptr @$str22, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %161)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b24

b26:
  %162 = getelementptr inbounds i8, ptr %13, i16 2
  %163 = load ptr, ptr %162, !tbaa !2
  store ptr %163, ptr %11, !tbaa !2
  store i8 1, ptr %10, !tbaa !2
  %164 = getelementptr i8, ptr @$str21, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %164)
  %165 = load ptr, ptr %11, !tbaa !2
  call cc 1005 addrspace(1) void @N$PS(ptr %165)
  call cc 1005 addrspace(1) void @N$PN()
  %166 = load i8, ptr %10, !tbaa !2, !range !16
  %167 = icmp ne i8 %166, 0
  br i1 %167, label %b28, label %b27

b27:
  br label %b24

b28:
  %168 = addrspacecast ptr %11 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Channel.drop(ptr addrspace(1) %168)
  %169 = load ptr, ptr %11, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %169)
  br label %b27

b32:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %159)
  %170 = load ptr, ptr %9, !tbaa !2
  %171 = icmp ne ptr %170, null
  br i1 %171, label %b38, label %b37

b33:
  %172 = getelementptr i8, ptr %159, i16 -4
  %173 = load i16, ptr %172
  store i16 0, ptr %1, !tbaa !2
  br label %b34

b34:
  %174 = load i16, ptr %1, !tbaa !2
  %175 = icmp ult i16 %174, %173
  br i1 %175, label %b36, label %b35

b35:
  br label %b32

b36:
  %176 = mul i16 %174, 2
  %177 = getelementptr inbounds i8, ptr %159, i16 %176
  %178 = load ptr, ptr %177
  call cc 1005 addrspace(1) void @N$BDRP(ptr %178)
  %179 = add i16 %174, 1
  store i16 %179, ptr %1, !tbaa !2
  br label %b34

b37:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %170)
  ret i16 0

b38:
  %180 = getelementptr i8, ptr %170, i16 -4
  %181 = load i16, ptr %180
  store i16 0, ptr %0, !tbaa !2
  br label %b39

b39:
  %182 = load i16, ptr %0, !tbaa !2
  %183 = icmp ult i16 %182, %181
  br i1 %183, label %b41, label %b40

b40:
  br label %b37

b41:
  %184 = mul i16 %182, 2
  %185 = getelementptr inbounds i8, ptr %170, i16 %184
  %186 = load ptr, ptr %185
  call cc 1005 addrspace(1) void @N$BDRP(ptr %186)
  %187 = add i16 %182, 1
  store i16 %187, ptr %0, !tbaa !2
  br label %b39
}

declare cc 1005 void @N$PV(ptr addrspace(1)) addrspace(1)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 ptr @N$VCPY(ptr addrspace(1)) addrspace(1)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

declare cc 1005 void @N$PU2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 i8 @N$VCMP(ptr addrspace(1), ptr addrspace(1)) addrspace(1) memory(read)

declare void @llvm.memcpy.p0.p0.i16(ptr noalias nocapture writeonly, ptr noalias nocapture readonly, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)

declare cc 1005 ptr @N$BGRW(ptr, i16, i16) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i8 0, i8 2}
!6 = !{i8 0, i8 2}
!7 = !{i8 0, i8 2}
!8 = !{i8 0, i8 2}
!9 = !{i8 0, i8 2}
!10 = !{i8 0, i8 2}
!11 = !{i8 0, i8 2}
!12 = !{i8 0, i8 2}
!13 = !{i8 0, i8 2}
!14 = !{i8 0, i8 2}
!15 = !{i8 0, i8 2}
!16 = !{i8 0, i8 2}
!17 = !{i8 0, i8 2}
!18 = !{i8 0, i8 2}
!19 = !{i8 0, i8 2}
!20 = !{i8 0, i8 2}
