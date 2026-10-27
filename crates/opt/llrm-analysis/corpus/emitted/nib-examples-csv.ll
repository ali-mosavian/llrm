target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str2 = internal constant [20 x i8] c"\08\00\0D\00\0D\00Ada, pilot, 7\00"
@$str3 = internal constant [26 x i8] c"\08\00\13\00\13\00Grace,  admiral , 9\00"
@$str4 = internal constant [19 x i8] c"\08\00\0C\00\0C\00Linus ,  , 3\00"
@$str5 = internal constant [18 x i8] c"\08\00\0B\00\0B\00| (no role)\00"
@$str6 = internal constant [9 x i8] c"\08\00\02\00\02\00| \00"
@$str7 = internal constant [17 x i8] c"\08\00\0A\00\0A\00 at level \00"
@$str8 = internal constant [12 x i8] c"\08\00\05\00\05\00Grace\00"
@$str9 = internal constant [8 x i8] c"\08\00\01\00\01\00 \00"

define internal cc 1005 void @trimmed(ptr addrspace(1) %0, ptr addrspace(1) noalias readonly dereferenceable(8) %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca [8 x i8]
  %3 = alloca i16
  %4 = alloca i16
  store i16 0, ptr %4, !tbaa !2
  %5 = load i16, ptr addrspace(1) %1
  store i16 %5, ptr %3, !tbaa !2
  br label %b2

b2:
  %6 = load i16, ptr %4, !tbaa !2
  %7 = load i16, ptr %3, !tbaa !2
  %8 = icmp ult i16 %6, %7
  br i1 %8, label %b5, label %b4

b3:
  %9 = load i16, ptr %4, !tbaa !2
  %10 = add i16 %9, 1
  store i16 %10, ptr %4, !tbaa !2
  br label %b2

b4:
  br label %b8

b5:
  %11 = load i16, ptr %4, !tbaa !2
  %12 = load i16, ptr addrspace(1) %1
  %13 = icmp ult i16 %11, %12
  br i1 %13, label %b6, label %b7

b6:
  %14 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %15 = load ptr addrspace(1), ptr addrspace(1) %14
  %16 = getelementptr inbounds i8, ptr addrspace(1) %15, i16 %11
  %17 = load i8, ptr addrspace(1) %16
  %18 = icmp eq i8 %17, 32
  br i1 %18, label %b3, label %b4

b7:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b8:
  %19 = load i16, ptr %3, !tbaa !2
  %20 = load i16, ptr %4, !tbaa !2
  %21 = icmp ugt i16 %19, %20
  br i1 %21, label %b11, label %b10

b9:
  %22 = load i16, ptr %3, !tbaa !2
  %23 = sub i16 %22, 1
  store i16 %23, ptr %3, !tbaa !2
  br label %b8

b10:
  %24 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %25 = load ptr addrspace(1), ptr addrspace(1) %24
  %26 = load i16, ptr addrspace(1) %1
  %27 = load i16, ptr %4, !tbaa !2
  %28 = load i16, ptr %3, !tbaa !2
  %29 = icmp ule i16 %28, %26
  br i1 %29, label %b14, label %b15

b11:
  %30 = load i16, ptr %3, !tbaa !2
  %31 = sub i16 %30, 1
  %32 = load i16, ptr addrspace(1) %1
  %33 = icmp ult i16 %31, %32
  br i1 %33, label %b12, label %b13

b12:
  %34 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %35 = load ptr addrspace(1), ptr addrspace(1) %34
  %36 = getelementptr inbounds i8, ptr addrspace(1) %35, i16 %31
  %37 = load i8, ptr addrspace(1) %36
  %38 = icmp eq i8 %37, 32
  br i1 %38, label %b9, label %b10

b13:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b14:
  %39 = icmp ule i16 %27, %28
  br i1 %39, label %b16, label %b17

b15:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b16:
  %40 = getelementptr inbounds i8, ptr addrspace(1) %25, i16 %27
  %41 = sub i16 %28, %27
  store i16 %41, ptr %2, !tbaa !2
  %42 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 %41, ptr %42, !tbaa !2
  %43 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %40, ptr %43, !tbaa !2
  %44 = addrspacecast ptr %2 to ptr addrspace(1)
  %45 = load i16, ptr addrspace(1) %44, !tbaa !2
  store i16 %45, ptr addrspace(1) %0
  %46 = getelementptr i8, ptr addrspace(1) %44, i16 2
  %47 = load i16, ptr addrspace(1) %46, !tbaa !2
  %48 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %47, ptr addrspace(1) %48
  %49 = getelementptr i8, ptr addrspace(1) %44, i16 4
  %50 = load ptr addrspace(1), ptr addrspace(1) %49, !tbaa !2
  %51 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store ptr addrspace(1) %50, ptr addrspace(1) %51
  ret void

b17:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

define internal cc 1005 void @field(ptr addrspace(1) %0, ptr addrspace(1) noalias readonly dereferenceable(8) %1, i16 %2) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %3 = alloca [8 x i8]
  %4 = alloca [8 x i8]
  %5 = alloca [8 x i8]
  %6 = alloca [8 x i8]
  %7 = alloca [8 x i8]
  %8 = alloca i16
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca i16
  store i16 0, ptr %11, !tbaa !2
  store i16 0, ptr %10, !tbaa !2
  %12 = load i16, ptr addrspace(1) %1
  store i16 0, ptr %9, !tbaa !2
  store i16 %12, ptr %8, !tbaa !2
  br label %b2

b2:
  %13 = load i16, ptr %9, !tbaa !2
  %14 = load i16, ptr %8, !tbaa !2
  %15 = icmp ult i16 %13, %14
  br i1 %15, label %b3, label %b5

b3:
  %16 = load i16, ptr %9, !tbaa !2
  %17 = load i16, ptr addrspace(1) %1
  %18 = icmp ult i16 %16, %17
  br i1 %18, label %b9, label %b10

b4:
  %19 = load i16, ptr %9, !tbaa !2
  %20 = add nuw i16 %19, 1
  store i16 %20, ptr %9, !tbaa !2
  br label %b2

b5:
  %21 = load i16, ptr %11, !tbaa !2
  %22 = icmp eq i16 %21, %2
  br i1 %22, label %b18, label %b19

b6:
  %23 = load i16, ptr %11, !tbaa !2
  %24 = icmp eq i16 %23, %2
  br i1 %24, label %b11, label %b12

b7:
  br label %b8

b8:
  br label %b4

b9:
  %25 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %26 = load ptr addrspace(1), ptr addrspace(1) %25
  %27 = getelementptr inbounds i8, ptr addrspace(1) %26, i16 %16
  %28 = load i8, ptr addrspace(1) %27
  %29 = icmp eq i8 %28, 44
  br i1 %29, label %b6, label %b7

b10:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b11:
  %30 = addrspacecast ptr %7 to ptr addrspace(1)
  %31 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %32 = load ptr addrspace(1), ptr addrspace(1) %31
  %33 = load i16, ptr addrspace(1) %1
  %34 = load i16, ptr %10, !tbaa !2
  %35 = load i16, ptr %9, !tbaa !2
  %36 = icmp ule i16 %35, %33
  br i1 %36, label %b14, label %b15

b12:
  br label %b13

b13:
  %37 = load i16, ptr %11, !tbaa !2
  %38 = add i16 %37, 1
  store i16 %38, ptr %11, !tbaa !2
  %39 = load i16, ptr %9, !tbaa !2
  %40 = add i16 %39, 1
  store i16 %40, ptr %10, !tbaa !2
  br label %b8

b14:
  %41 = icmp ule i16 %34, %35
  br i1 %41, label %b16, label %b17

b15:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b16:
  %42 = getelementptr inbounds i8, ptr addrspace(1) %32, i16 %34
  %43 = sub i16 %35, %34
  store i16 %43, ptr %6, !tbaa !2
  %44 = getelementptr inbounds i8, ptr %6, i16 2
  store i16 %43, ptr %44, !tbaa !2
  %45 = getelementptr inbounds i8, ptr %6, i16 4
  store ptr addrspace(1) %42, ptr %45, !tbaa !2
  %46 = addrspacecast ptr %6 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @trimmed(ptr addrspace(1) %30, ptr addrspace(1) %46)
  %47 = load i16, ptr addrspace(1) %30, !tbaa !2
  store i16 %47, ptr addrspace(1) %0
  %48 = getelementptr i8, ptr addrspace(1) %30, i16 2
  %49 = load i16, ptr addrspace(1) %48, !tbaa !2
  %50 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %49, ptr addrspace(1) %50
  %51 = getelementptr i8, ptr addrspace(1) %30, i16 4
  %52 = load ptr addrspace(1), ptr addrspace(1) %51, !tbaa !2
  %53 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store ptr addrspace(1) %52, ptr addrspace(1) %53
  ret void

b17:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b18:
  %54 = addrspacecast ptr %5 to ptr addrspace(1)
  %55 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %56 = load ptr addrspace(1), ptr addrspace(1) %55
  %57 = load i16, ptr addrspace(1) %1
  %58 = load i16, ptr %10, !tbaa !2
  %59 = load i16, ptr addrspace(1) %1
  %60 = icmp ule i16 %59, %57
  br i1 %60, label %b21, label %b22

b19:
  br label %b20

b20:
  %61 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %62 = load ptr addrspace(1), ptr addrspace(1) %61
  %63 = load i16, ptr addrspace(1) %1
  %64 = icmp ule i16 0, %63
  br i1 %64, label %b25, label %b26

b21:
  %65 = icmp ule i16 %58, %59
  br i1 %65, label %b23, label %b24

b22:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b23:
  %66 = getelementptr inbounds i8, ptr addrspace(1) %56, i16 %58
  %67 = sub i16 %59, %58
  store i16 %67, ptr %4, !tbaa !2
  %68 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 %67, ptr %68, !tbaa !2
  %69 = getelementptr inbounds i8, ptr %4, i16 4
  store ptr addrspace(1) %66, ptr %69, !tbaa !2
  %70 = addrspacecast ptr %4 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @trimmed(ptr addrspace(1) %54, ptr addrspace(1) %70)
  %71 = load i16, ptr addrspace(1) %54, !tbaa !2
  store i16 %71, ptr addrspace(1) %0
  %72 = getelementptr i8, ptr addrspace(1) %54, i16 2
  %73 = load i16, ptr addrspace(1) %72, !tbaa !2
  %74 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %73, ptr addrspace(1) %74
  %75 = getelementptr i8, ptr addrspace(1) %54, i16 4
  %76 = load ptr addrspace(1), ptr addrspace(1) %75, !tbaa !2
  %77 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store ptr addrspace(1) %76, ptr addrspace(1) %77
  ret void

b24:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b25:
  store i16 0, ptr %3, !tbaa !2
  %78 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 0, ptr %78, !tbaa !2
  %79 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %62, ptr %79, !tbaa !2
  %80 = addrspacecast ptr %3 to ptr addrspace(1)
  %81 = load i16, ptr addrspace(1) %80, !tbaa !2
  store i16 %81, ptr addrspace(1) %0
  %82 = getelementptr i8, ptr addrspace(1) %80, i16 2
  %83 = load i16, ptr addrspace(1) %82, !tbaa !2
  %84 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %83, ptr addrspace(1) %84
  %85 = getelementptr i8, ptr addrspace(1) %80, i16 4
  %86 = load ptr addrspace(1), ptr addrspace(1) %85, !tbaa !2
  %87 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store ptr addrspace(1) %86, ptr addrspace(1) %87
  ret void

b26:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca ptr
  %2 = alloca i8
  %3 = alloca [8 x i8]
  %4 = alloca [8 x i8]
  %5 = alloca [8 x i8]
  %6 = alloca [8 x i8]
  %7 = alloca [8 x i8]
  %8 = alloca [8 x i8]
  %9 = alloca [8 x i8]
  %10 = alloca [8 x i8]
  %11 = alloca [8 x i8]
  %12 = alloca i16
  %13 = alloca ptr
  %14 = getelementptr i8, ptr @$str1, i16 6
  %15 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %14, i16 3, i16 2)
  %16 = getelementptr inbounds i8, ptr %15, i16 0
  %17 = getelementptr i8, ptr @$str2, i16 6
  store ptr %17, ptr %16
  %18 = getelementptr inbounds i8, ptr %15, i16 2
  %19 = getelementptr i8, ptr @$str3, i16 6
  store ptr %19, ptr %18
  %20 = getelementptr inbounds i8, ptr %15, i16 4
  %21 = getelementptr i8, ptr @$str4, i16 6
  store ptr %21, ptr %20
  store ptr %15, ptr %13, !tbaa !2
  %22 = load ptr, ptr %13, !tbaa !2
  %23 = getelementptr i8, ptr %22, i16 -4
  %24 = load i16, ptr %23
  store i16 0, ptr %12, !tbaa !2
  br label %b2

b2:
  %25 = load i16, ptr %12, !tbaa !2
  %26 = icmp ult i16 %25, %24
  br i1 %26, label %b3, label %b5

b3:
  %27 = mul i16 %25, 2
  %28 = getelementptr inbounds i8, ptr %22, i16 %27
  %29 = addrspacecast ptr %11 to ptr addrspace(1)
  %30 = load ptr, ptr %28
  %31 = getelementptr i8, ptr %30, i16 -4
  %32 = load i16, ptr %31
  %33 = addrspacecast ptr %30 to ptr addrspace(1)
  store i16 %32, ptr %10, !tbaa !2
  %34 = getelementptr inbounds i8, ptr %10, i16 2
  store i16 %32, ptr %34, !tbaa !2
  %35 = getelementptr inbounds i8, ptr %10, i16 4
  store ptr addrspace(1) %33, ptr %35, !tbaa !2
  %36 = addrspacecast ptr %10 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @field(ptr addrspace(1) %29, ptr addrspace(1) %36, i16 0)
  %37 = addrspacecast ptr %9 to ptr addrspace(1)
  %38 = load ptr, ptr %28
  %39 = getelementptr i8, ptr %38, i16 -4
  %40 = load i16, ptr %39
  %41 = addrspacecast ptr %38 to ptr addrspace(1)
  store i16 %40, ptr %8, !tbaa !2
  %42 = getelementptr inbounds i8, ptr %8, i16 2
  store i16 %40, ptr %42, !tbaa !2
  %43 = getelementptr inbounds i8, ptr %8, i16 4
  store ptr addrspace(1) %41, ptr %43, !tbaa !2
  %44 = addrspacecast ptr %8 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @field(ptr addrspace(1) %37, ptr addrspace(1) %44, i16 1)
  %45 = load i16, ptr addrspace(1) %37
  %46 = icmp eq i16 %45, 0
  br i1 %46, label %b6, label %b7

b4:
  %47 = load i16, ptr %12, !tbaa !2
  %48 = add i16 %47, 1
  store i16 %48, ptr %12, !tbaa !2
  br label %b2

b5:
  %49 = addrspacecast ptr %5 to ptr addrspace(1)
  %50 = load ptr, ptr %13, !tbaa !2
  %51 = getelementptr i8, ptr %50, i16 -4
  %52 = load i16, ptr %51
  %53 = icmp ult i16 1, %52
  br i1 %53, label %b9, label %b10

b6:
  call cc 1005 addrspace(1) void @N$PFLD(i8 zeroext 6, i8 zeroext 10, i8 zeroext 32, i8 zeroext 1)
  call cc 1005 addrspace(1) void @N$PV(ptr addrspace(1) %29)
  %54 = getelementptr i8, ptr @$str5, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %54)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b8

b7:
  %55 = addrspacecast ptr %7 to ptr addrspace(1)
  %56 = load ptr, ptr %28
  %57 = getelementptr i8, ptr %56, i16 -4
  %58 = load i16, ptr %57
  %59 = addrspacecast ptr %56 to ptr addrspace(1)
  store i16 %58, ptr %6, !tbaa !2
  %60 = getelementptr inbounds i8, ptr %6, i16 2
  store i16 %58, ptr %60, !tbaa !2
  %61 = getelementptr inbounds i8, ptr %6, i16 4
  store ptr addrspace(1) %59, ptr %61, !tbaa !2
  %62 = addrspacecast ptr %6 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @field(ptr addrspace(1) %55, ptr addrspace(1) %62, i16 2)
  call cc 1005 addrspace(1) void @N$PFLD(i8 zeroext 6, i8 zeroext 10, i8 zeroext 32, i8 zeroext 1)
  call cc 1005 addrspace(1) void @N$PV(ptr addrspace(1) %29)
  %63 = getelementptr i8, ptr @$str6, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %63)
  call cc 1005 addrspace(1) void @N$PV(ptr addrspace(1) %37)
  %64 = getelementptr i8, ptr @$str7, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %64)
  call cc 1005 addrspace(1) void @N$PV(ptr addrspace(1) %55)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b8

b8:
  br label %b4

b9:
  %65 = getelementptr inbounds i8, ptr %50, i16 2
  %66 = load ptr, ptr %65
  %67 = getelementptr i8, ptr %66, i16 -4
  %68 = load i16, ptr %67
  %69 = addrspacecast ptr %66 to ptr addrspace(1)
  store i16 %68, ptr %4, !tbaa !2
  %70 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 %68, ptr %70, !tbaa !2
  %71 = getelementptr inbounds i8, ptr %4, i16 4
  store ptr addrspace(1) %69, ptr %71, !tbaa !2
  %72 = addrspacecast ptr %4 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @field(ptr addrspace(1) %49, ptr addrspace(1) %72, i16 0)
  %73 = getelementptr i8, ptr @$str8, i16 6
  %74 = getelementptr i8, ptr %73, i16 -4
  %75 = load i16, ptr %74
  %76 = addrspacecast ptr %73 to ptr addrspace(1)
  store i16 %75, ptr %3, !tbaa !2
  %77 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 %75, ptr %77, !tbaa !2
  %78 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %76, ptr %78, !tbaa !2
  %79 = addrspacecast ptr %3 to ptr addrspace(1)
  %80 = call cc 1005 addrspace(1) i8 @N$VCMP(ptr addrspace(1) %49, ptr addrspace(1) %79)
  %81 = icmp eq i8 %80, 0
  %82 = zext i1 %81 to i8
  store i8 %82, ptr %2, !tbaa !2
  %83 = call cc 1005 addrspace(1) ptr @N$VCPY(ptr addrspace(1) %49)
  store ptr %83, ptr %1, !tbaa !2
  %84 = load i8, ptr %2, !tbaa !2, !range !5
  call cc 1005 addrspace(1) void @N$PB(i8 %84)
  %85 = getelementptr i8, ptr @$str9, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %85)
  %86 = load ptr, ptr %1, !tbaa !2
  call cc 1005 addrspace(1) void @N$PS(ptr %86)
  call cc 1005 addrspace(1) void @N$PN()
  %87 = load ptr, ptr %1, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %87)
  %88 = load ptr, ptr %13, !tbaa !2
  %89 = icmp ne ptr %88, null
  br i1 %89, label %b12, label %b11

b10:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b11:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %88)
  ret i16 0

b12:
  %90 = getelementptr i8, ptr %88, i16 -4
  %91 = load i16, ptr %90
  store i16 0, ptr %0, !tbaa !2
  br label %b13

b13:
  %92 = load i16, ptr %0, !tbaa !2
  %93 = icmp ult i16 %92, %91
  br i1 %93, label %b15, label %b14

b14:
  br label %b11

b15:
  %94 = mul i16 %92, 2
  %95 = getelementptr inbounds i8, ptr %88, i16 %94
  %96 = load ptr, ptr %95
  call cc 1005 addrspace(1) void @N$BDRP(ptr %96)
  %97 = add i16 %92, 1
  store i16 %97, ptr %0, !tbaa !2
  br label %b13
}

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare cc 1005 ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare cc 1005 void @N$PFLD(i8, i8, i8, i8) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PV(ptr addrspace(1)) addrspace(1)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 i8 @N$VCMP(ptr addrspace(1), ptr addrspace(1)) addrspace(1) memory(read)

declare cc 1005 ptr @N$VCPY(ptr addrspace(1)) addrspace(1)

declare cc 1005 void @N$PB(i8) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i8 0, i8 2}
