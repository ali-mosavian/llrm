target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str2 = internal constant [14 x i8] c"\08\00\07\00\07\00closed \00"
@$str3 = internal constant [14 x i8] c"\08\00\07\00\07\00 after \00"
@$str4 = internal constant [13 x i8] c"\08\00\06\00\06\00 steps\00"
@$str5 = internal constant [14 x i8] c"\08\00\07\00\07\00printer\00"
@$str6 = internal constant [13 x i8] c"\08\00\06\00\06\00unplug\00"
@$str7 = internal constant [13 x i8] c"\08\00\06\00\06\00replug\00"
@$str8 = internal constant [12 x i8] c"\08\00\05\00\05\00login\00"
@$str9 = internal constant [12 x i8] c"\08\00\05\00\05\00reset\00"
@$str10 = internal constant [12 x i8] c"\08\00\05\00\05\00retry\00"
@$str11 = internal constant [11 x i8] c"\08\00\04\00\04\00wifi\00"
@$str12 = internal constant [13 x i8] c"\08\00\06\00\06\00reboot\00"
@$str13 = internal constant [15 x i8] c"\08\00\08\00\08\00call isp\00"
@$str14 = internal constant [22 x i8] c"\08\00\0F\00\0F\00no login ticket\00"
@$str15 = internal constant [12 x i8] c"\08\00\05\00\05\00email\00"
@$str16 = internal constant [22 x i8] c"\08\00\0F\00\0F\00no email ticket\00"
@$str17 = internal constant [17 x i8] c"\08\00\0A\00\0A\00 waiting, \00"
@$str18 = internal constant [14 x i8] c"\08\00\07\00\07\00 closed\00"
@$str19 = internal constant [9 x i8] c"\08\00\02\00\02\00  \00"

define internal cc 1005 void @Ticket.new(ptr addrspace(1) %0, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %1, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %2, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %3) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %4 = alloca i16
  %5 = alloca ptr
  %6 = alloca ptr
  %7 = call cc 1005 addrspace(1) ptr @N$VCPY(ptr addrspace(1) %1)
  store ptr %7, ptr %6, !tbaa !2
  %8 = getelementptr i8, ptr @$str1, i16 6
  store ptr %8, ptr %5, !tbaa !2
  %9 = load ptr, ptr %5, !tbaa !2
  %10 = getelementptr i8, ptr %9, i16 -4
  %11 = load i16, ptr %10
  %12 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %9, i16 1, i16 2)
  store ptr %12, ptr %5, !tbaa !2
  %13 = mul i16 %11, 2
  %14 = getelementptr inbounds i8, ptr %12, i16 %13
  %15 = call cc 1005 addrspace(1) ptr @N$VCPY(ptr addrspace(1) %2)
  store ptr %15, ptr %14
  %16 = load ptr, ptr %5, !tbaa !2
  %17 = getelementptr i8, ptr %16, i16 -4
  %18 = load i16, ptr %17
  %19 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %16, i16 1, i16 2)
  store ptr %19, ptr %5, !tbaa !2
  %20 = mul i16 %18, 2
  %21 = getelementptr inbounds i8, ptr %19, i16 %20
  %22 = call cc 1005 addrspace(1) ptr @N$VCPY(ptr addrspace(1) %3)
  store ptr %22, ptr %21
  %23 = load ptr, ptr %6, !tbaa !2
  store ptr null, ptr %6, !tbaa !2
  %24 = load ptr, ptr %5, !tbaa !2
  store ptr null, ptr %5, !tbaa !2
  store ptr %23, ptr addrspace(1) %0
  %25 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr %24, ptr addrspace(1) %25
  %26 = load ptr, ptr %5, !tbaa !2
  %27 = icmp ne ptr %26, null
  br i1 %27, label %b3, label %b2

b2:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %26)
  %28 = load ptr, ptr %6, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %28)
  ret void

b3:
  %29 = getelementptr i8, ptr %26, i16 -4
  %30 = load i16, ptr %29
  store i16 0, ptr %4, !tbaa !2
  br label %b4

b4:
  %31 = load i16, ptr %4, !tbaa !2
  %32 = icmp ult i16 %31, %30
  br i1 %32, label %b6, label %b5

b5:
  br label %b2

b6:
  %33 = mul i16 %31, 2
  %34 = getelementptr inbounds i8, ptr %26, i16 %33
  %35 = load ptr, ptr %34
  call cc 1005 addrspace(1) void @N$BDRP(ptr %35)
  %36 = add i16 %31, 1
  store i16 %36, ptr %4, !tbaa !2
  br label %b4
}

define internal cc 1005 void @Desk.open(ptr addrspace(1) nonnull dereferenceable(4) noalias nocapture %0, ptr addrspace(1) noalias %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca i16
  %3 = load ptr, ptr addrspace(1) %0
  %4 = getelementptr i8, ptr %3, i16 -4
  %5 = load i16, ptr %4
  %6 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %3, i16 1, i16 4)
  store ptr %6, ptr addrspace(1) %0
  %7 = mul i16 %5, 4
  %8 = getelementptr inbounds i8, ptr %6, i16 %7
  %9 = load ptr, ptr addrspace(1) %1
  %10 = getelementptr i8, ptr addrspace(1) %1, i16 2
  %11 = load ptr, ptr addrspace(1) %10
  store ptr %9, ptr %8
  %12 = getelementptr i8, ptr %8, i16 2
  store ptr %11, ptr %12
  store ptr null, ptr addrspace(1) %1
  %13 = getelementptr i8, ptr addrspace(1) %1, i16 2
  store ptr null, ptr addrspace(1) %13
  %14 = getelementptr i8, ptr addrspace(1) %1, i16 2
  %15 = load ptr, ptr addrspace(1) %14
  %16 = icmp ne ptr %15, null
  br i1 %16, label %b3, label %b2

b2:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %15)
  %17 = load ptr, ptr addrspace(1) %1
  call cc 1005 addrspace(1) void @N$BDRP(ptr %17)
  ret void

b3:
  %18 = getelementptr i8, ptr %15, i16 -4
  %19 = load i16, ptr %18
  store i16 0, ptr %2, !tbaa !2
  br label %b4

b4:
  %20 = load i16, ptr %2, !tbaa !2
  %21 = icmp ult i16 %20, %19
  br i1 %21, label %b6, label %b5

b5:
  br label %b2

b6:
  %22 = mul i16 %20, 2
  %23 = getelementptr inbounds i8, ptr %15, i16 %22
  %24 = load ptr, ptr %23
  call cc 1005 addrspace(1) void @N$BDRP(ptr %24)
  %25 = add i16 %20, 1
  store i16 %25, ptr %2, !tbaa !2
  br label %b4
}

define internal cc 1005 void @Desk.next(ptr addrspace(1) %0, ptr addrspace(1) nonnull dereferenceable(4) noalias nocapture %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca [4 x i8]
  %3 = load ptr, ptr addrspace(1) %1
  %4 = getelementptr i8, ptr %3, i16 -4
  %5 = load i16, ptr %4
  %6 = icmp eq i16 %5, 0
  br i1 %6, label %b2, label %b3

b2:
  store i8 1, ptr addrspace(1) %0
  ret void

b3:
  br label %b4

b4:
  %7 = load ptr, ptr addrspace(1) %1
  %8 = call cc 1005 addrspace(1) i16 @N$BSHR(ptr %7, i16 1)
  %9 = mul i16 %8, 4
  %10 = getelementptr inbounds i8, ptr %7, i16 %9
  %11 = load ptr, ptr %10
  %12 = getelementptr i8, ptr %10, i16 2
  %13 = load ptr, ptr %12
  store ptr %11, ptr %2, !tbaa !2
  %14 = getelementptr inbounds i8, ptr %2, i16 2
  store ptr %13, ptr %14, !tbaa !2
  %15 = load ptr, ptr %2, !tbaa !2
  %16 = getelementptr inbounds i8, ptr %2, i16 2
  %17 = load ptr, ptr %16, !tbaa !2
  store i8 0, ptr addrspace(1) %0
  %18 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr %15, ptr addrspace(1) %18
  %19 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store ptr %17, ptr addrspace(1) %19
  ret void
}

define internal cc 1005 void @Desk.claim(ptr addrspace(1) %0, ptr addrspace(1) nonnull dereferenceable(4) noalias nocapture %1, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %2) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca [4 x i8]
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca [8 x i8]
  %10 = alloca [4 x i8]
  %11 = alloca [4 x i8]
  %12 = alloca ptr
  %13 = alloca [6 x i8]
  store i8 1, ptr %13, !tbaa !2
  %14 = getelementptr i8, ptr @$str1, i16 6
  store ptr %14, ptr %12, !tbaa !2
  br label %b2

b2:
  %15 = load ptr, ptr addrspace(1) %1
  %16 = getelementptr i8, ptr %15, i16 -4
  %17 = load i16, ptr %16
  %18 = icmp ugt i16 %17, 0
  br i1 %18, label %b3, label %b4

b3:
  %19 = load ptr, ptr addrspace(1) %1
  %20 = call cc 1005 addrspace(1) i16 @N$BSHR(ptr %19, i16 1)
  %21 = mul i16 %20, 4
  %22 = getelementptr inbounds i8, ptr %19, i16 %21
  %23 = load ptr, ptr %22
  %24 = getelementptr i8, ptr %22, i16 2
  %25 = load ptr, ptr %24
  store ptr %23, ptr %10, !tbaa !2
  %26 = getelementptr inbounds i8, ptr %10, i16 2
  store ptr %25, ptr %26, !tbaa !2
  %27 = load ptr, ptr %10, !tbaa !2
  %28 = getelementptr inbounds i8, ptr %10, i16 2
  %29 = load ptr, ptr %28, !tbaa !2
  store ptr %27, ptr %11, !tbaa !2
  %30 = getelementptr inbounds i8, ptr %11, i16 2
  store ptr %29, ptr %30, !tbaa !2
  %31 = load ptr, ptr %11, !tbaa !2
  %32 = getelementptr i8, ptr %31, i16 -4
  %33 = load i16, ptr %32
  %34 = addrspacecast ptr %31 to ptr addrspace(1)
  store i16 %33, ptr %9, !tbaa !2
  %35 = getelementptr inbounds i8, ptr %9, i16 2
  store i16 %33, ptr %35, !tbaa !2
  %36 = getelementptr inbounds i8, ptr %9, i16 4
  store ptr addrspace(1) %34, ptr %36, !tbaa !2
  %37 = addrspacecast ptr %9 to ptr addrspace(1)
  %38 = call cc 1005 addrspace(1) i8 @N$VCMP(ptr addrspace(1) %37, ptr addrspace(1) %2)
  %39 = icmp eq i8 %38, 0
  br i1 %39, label %b5, label %b6

b4:
  br label %b20

b5:
  %40 = load ptr, ptr %11, !tbaa !2
  %41 = getelementptr inbounds i8, ptr %11, i16 2
  %42 = load ptr, ptr %41, !tbaa !2
  %43 = load i8, ptr %13, !tbaa !2, !range !5
  %44 = icmp eq i8 %43, 0
  br i1 %44, label %b9, label %b8

b6:
  %45 = load ptr, ptr %12, !tbaa !2
  %46 = getelementptr i8, ptr %45, i16 -4
  %47 = load i16, ptr %46
  %48 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %45, i16 1, i16 4)
  store ptr %48, ptr %12, !tbaa !2
  %49 = mul i16 %47, 4
  %50 = getelementptr inbounds i8, ptr %48, i16 %49
  %51 = load ptr, ptr %11, !tbaa !2
  %52 = getelementptr inbounds i8, ptr %11, i16 2
  %53 = load ptr, ptr %52, !tbaa !2
  store ptr %51, ptr %50
  %54 = getelementptr i8, ptr %50, i16 2
  store ptr %53, ptr %54
  store ptr null, ptr %11, !tbaa !2
  %55 = getelementptr inbounds i8, ptr %11, i16 2
  store ptr null, ptr %55, !tbaa !2
  br label %b7

b7:
  %56 = getelementptr inbounds i8, ptr %11, i16 2
  %57 = load ptr, ptr %56, !tbaa !2
  %58 = icmp ne ptr %57, null
  br i1 %58, label %b16, label %b15

b8:
  store i8 0, ptr %13, !tbaa !2
  %59 = getelementptr inbounds i8, ptr %13, i16 2
  store ptr %40, ptr %59, !tbaa !2
  %60 = getelementptr inbounds i8, ptr %13, i16 4
  store ptr %42, ptr %60, !tbaa !2
  store ptr null, ptr %11, !tbaa !2
  %61 = getelementptr inbounds i8, ptr %11, i16 2
  store ptr null, ptr %61, !tbaa !2
  br label %b7

b9:
  %62 = getelementptr inbounds i8, ptr %13, i16 4
  %63 = load ptr, ptr %62, !tbaa !2
  %64 = icmp ne ptr %63, null
  br i1 %64, label %b11, label %b10

b10:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %63)
  %65 = getelementptr inbounds i8, ptr %13, i16 2
  %66 = load ptr, ptr %65, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %66)
  br label %b8

b11:
  %67 = getelementptr i8, ptr %63, i16 -4
  %68 = load i16, ptr %67
  store i16 0, ptr %8, !tbaa !2
  br label %b12

b12:
  %69 = load i16, ptr %8, !tbaa !2
  %70 = icmp ult i16 %69, %68
  br i1 %70, label %b14, label %b13

b13:
  br label %b10

b14:
  %71 = mul i16 %69, 2
  %72 = getelementptr inbounds i8, ptr %63, i16 %71
  %73 = load ptr, ptr %72
  call cc 1005 addrspace(1) void @N$BDRP(ptr %73)
  %74 = add i16 %69, 1
  store i16 %74, ptr %8, !tbaa !2
  br label %b12

b15:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %57)
  %75 = load ptr, ptr %11, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %75)
  br label %b2

b16:
  %76 = getelementptr i8, ptr %57, i16 -4
  %77 = load i16, ptr %76
  store i16 0, ptr %7, !tbaa !2
  br label %b17

b17:
  %78 = load i16, ptr %7, !tbaa !2
  %79 = icmp ult i16 %78, %77
  br i1 %79, label %b19, label %b18

b18:
  br label %b15

b19:
  %80 = mul i16 %78, 2
  %81 = getelementptr inbounds i8, ptr %57, i16 %80
  %82 = load ptr, ptr %81
  call cc 1005 addrspace(1) void @N$BDRP(ptr %82)
  %83 = add i16 %78, 1
  store i16 %83, ptr %7, !tbaa !2
  br label %b17

b20:
  %84 = load ptr, ptr %12, !tbaa !2
  %85 = getelementptr i8, ptr %84, i16 -4
  %86 = load i16, ptr %85
  %87 = icmp ugt i16 %86, 0
  br i1 %87, label %b21, label %b22

b21:
  %88 = load ptr, ptr addrspace(1) %1
  %89 = getelementptr i8, ptr %88, i16 -4
  %90 = load i16, ptr %89
  %91 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %88, i16 1, i16 4)
  store ptr %91, ptr addrspace(1) %1
  %92 = mul i16 %90, 4
  %93 = getelementptr inbounds i8, ptr %91, i16 %92
  %94 = load ptr, ptr %12, !tbaa !2
  %95 = call cc 1005 addrspace(1) i16 @N$BSHR(ptr %94, i16 1)
  %96 = mul i16 %95, 4
  %97 = getelementptr inbounds i8, ptr %94, i16 %96
  %98 = load ptr, ptr %97
  %99 = getelementptr i8, ptr %97, i16 2
  %100 = load ptr, ptr %99
  store ptr %98, ptr %6, !tbaa !2
  %101 = getelementptr inbounds i8, ptr %6, i16 2
  store ptr %100, ptr %101, !tbaa !2
  %102 = load ptr, ptr %6, !tbaa !2
  %103 = getelementptr inbounds i8, ptr %6, i16 2
  %104 = load ptr, ptr %103, !tbaa !2
  store ptr %102, ptr %93
  %105 = getelementptr i8, ptr %93, i16 2
  store ptr %104, ptr %105
  br label %b20

b22:
  call void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) %0, ptr %13, i16 6, i1 false)
  store i16 0, ptr %13, !tbaa !2
  %106 = getelementptr inbounds i8, ptr %13, i16 2
  store i16 0, ptr %106, !tbaa !2
  %107 = getelementptr inbounds i8, ptr %13, i16 4
  store i16 0, ptr %107, !tbaa !2
  %108 = load ptr, ptr %12, !tbaa !2
  %109 = icmp ne ptr %108, null
  br i1 %109, label %b24, label %b23

b23:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %108)
  %110 = load i8, ptr %13, !tbaa !2, !range !5
  %111 = icmp eq i8 %110, 0
  br i1 %111, label %b34, label %b33

b24:
  %112 = getelementptr i8, ptr %108, i16 -4
  %113 = load i16, ptr %112
  store i16 0, ptr %5, !tbaa !2
  br label %b25

b25:
  %114 = load i16, ptr %5, !tbaa !2
  %115 = icmp ult i16 %114, %113
  br i1 %115, label %b27, label %b26

b26:
  br label %b23

b27:
  %116 = mul i16 %114, 4
  %117 = getelementptr inbounds i8, ptr %108, i16 %116
  %118 = getelementptr i8, ptr %117, i16 2
  %119 = load ptr, ptr %118
  %120 = icmp ne ptr %119, null
  br i1 %120, label %b29, label %b28

b28:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %119)
  %121 = load ptr, ptr %117
  call cc 1005 addrspace(1) void @N$BDRP(ptr %121)
  %122 = add i16 %114, 1
  store i16 %122, ptr %5, !tbaa !2
  br label %b25

b29:
  %123 = getelementptr i8, ptr %119, i16 -4
  %124 = load i16, ptr %123
  store i16 0, ptr %4, !tbaa !2
  br label %b30

b30:
  %125 = load i16, ptr %4, !tbaa !2
  %126 = icmp ult i16 %125, %124
  br i1 %126, label %b32, label %b31

b31:
  br label %b28

b32:
  %127 = mul i16 %125, 2
  %128 = getelementptr inbounds i8, ptr %119, i16 %127
  %129 = load ptr, ptr %128
  call cc 1005 addrspace(1) void @N$BDRP(ptr %129)
  %130 = add i16 %125, 1
  store i16 %130, ptr %4, !tbaa !2
  br label %b30

b33:
  ret void

b34:
  %131 = getelementptr inbounds i8, ptr %13, i16 4
  %132 = load ptr, ptr %131, !tbaa !2
  %133 = icmp ne ptr %132, null
  br i1 %133, label %b36, label %b35

b35:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %132)
  %134 = getelementptr inbounds i8, ptr %13, i16 2
  %135 = load ptr, ptr %134, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %135)
  br label %b33

b36:
  %136 = getelementptr i8, ptr %132, i16 -4
  %137 = load i16, ptr %136
  store i16 0, ptr %3, !tbaa !2
  br label %b37

b37:
  %138 = load i16, ptr %3, !tbaa !2
  %139 = icmp ult i16 %138, %137
  br i1 %139, label %b39, label %b38

b38:
  br label %b35

b39:
  %140 = mul i16 %138, 2
  %141 = getelementptr inbounds i8, ptr %132, i16 %140
  %142 = load ptr, ptr %141
  call cc 1005 addrspace(1) void @N$BDRP(ptr %142)
  %143 = add i16 %138, 1
  store i16 %143, ptr %3, !tbaa !2
  br label %b37
}

define internal cc 1005 void @Desk.close(ptr addrspace(1) nonnull dereferenceable(4) noalias nocapture %0, ptr addrspace(1) noalias %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca i16
  %3 = getelementptr i8, ptr @$str2, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %3)
  %4 = load ptr, ptr addrspace(1) %1
  call cc 1005 addrspace(1) void @N$PS(ptr %4)
  %5 = getelementptr i8, ptr @$str3, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %5)
  %6 = getelementptr i8, ptr addrspace(1) %1, i16 2
  %7 = load ptr, ptr addrspace(1) %6
  %8 = getelementptr i8, ptr %7, i16 -4
  %9 = load i16, ptr %8
  call cc 1005 addrspace(1) void @N$PU2(i16 %9)
  %10 = getelementptr i8, ptr @$str4, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %10)
  call cc 1005 addrspace(1) void @N$PN()
  %11 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %12 = load ptr, ptr addrspace(1) %11
  %13 = getelementptr i8, ptr %12, i16 -4
  %14 = load i16, ptr %13
  %15 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %12, i16 1, i16 4)
  %16 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr %15, ptr addrspace(1) %16
  %17 = mul i16 %14, 4
  %18 = getelementptr inbounds i8, ptr %15, i16 %17
  %19 = load ptr, ptr addrspace(1) %1
  %20 = getelementptr i8, ptr addrspace(1) %1, i16 2
  %21 = load ptr, ptr addrspace(1) %20
  store ptr %19, ptr %18
  %22 = getelementptr i8, ptr %18, i16 2
  store ptr %21, ptr %22
  store ptr null, ptr addrspace(1) %1
  %23 = getelementptr i8, ptr addrspace(1) %1, i16 2
  store ptr null, ptr addrspace(1) %23
  %24 = getelementptr i8, ptr addrspace(1) %1, i16 2
  %25 = load ptr, ptr addrspace(1) %24
  %26 = icmp ne ptr %25, null
  br i1 %26, label %b3, label %b2

b2:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %25)
  %27 = load ptr, ptr addrspace(1) %1
  call cc 1005 addrspace(1) void @N$BDRP(ptr %27)
  ret void

b3:
  %28 = getelementptr i8, ptr %25, i16 -4
  %29 = load i16, ptr %28
  store i16 0, ptr %2, !tbaa !2
  br label %b4

b4:
  %30 = load i16, ptr %2, !tbaa !2
  %31 = icmp ult i16 %30, %29
  br i1 %31, label %b6, label %b5

b5:
  br label %b2

b6:
  %32 = mul i16 %30, 2
  %33 = getelementptr inbounds i8, ptr %25, i16 %32
  %34 = load ptr, ptr %33
  call cc 1005 addrspace(1) void @N$BDRP(ptr %34)
  %35 = add i16 %30, 1
  store i16 %35, ptr %2, !tbaa !2
  br label %b4
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca [8 x i8]
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca [4 x i8]
  %9 = alloca [4 x i8]
  %10 = alloca [6 x i8]
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca [4 x i8]
  %14 = alloca [4 x i8]
  %15 = alloca [8 x i8]
  %16 = alloca [6 x i8]
  %17 = alloca i16
  %18 = alloca i16
  %19 = alloca [4 x i8]
  %20 = alloca [4 x i8]
  %21 = alloca [8 x i8]
  %22 = alloca [6 x i8]
  %23 = alloca [8 x i8]
  %24 = alloca [8 x i8]
  %25 = alloca [8 x i8]
  %26 = alloca [4 x i8]
  %27 = alloca [4 x i8]
  %28 = alloca [8 x i8]
  %29 = alloca [8 x i8]
  %30 = alloca [8 x i8]
  %31 = alloca [4 x i8]
  %32 = alloca [4 x i8]
  %33 = alloca [8 x i8]
  %34 = alloca [8 x i8]
  %35 = alloca [8 x i8]
  %36 = alloca [4 x i8]
  %37 = alloca [4 x i8]
  %38 = alloca [4 x i8]
  %39 = getelementptr i8, ptr @$str1, i16 6
  %40 = getelementptr i8, ptr @$str1, i16 6
  store ptr %39, ptr %38, !tbaa !2
  %41 = getelementptr inbounds i8, ptr %38, i16 2
  store ptr %40, ptr %41, !tbaa !2
  %42 = addrspacecast ptr %38 to ptr addrspace(1)
  %43 = addrspacecast ptr %36 to ptr addrspace(1)
  %44 = getelementptr i8, ptr @$str5, i16 6
  %45 = getelementptr i8, ptr %44, i16 -4
  %46 = load i16, ptr %45
  %47 = addrspacecast ptr %44 to ptr addrspace(1)
  store i16 %46, ptr %35, !tbaa !2
  %48 = getelementptr inbounds i8, ptr %35, i16 2
  store i16 %46, ptr %48, !tbaa !2
  %49 = getelementptr inbounds i8, ptr %35, i16 4
  store ptr addrspace(1) %47, ptr %49, !tbaa !2
  %50 = addrspacecast ptr %35 to ptr addrspace(1)
  %51 = getelementptr i8, ptr @$str6, i16 6
  %52 = getelementptr i8, ptr %51, i16 -4
  %53 = load i16, ptr %52
  %54 = addrspacecast ptr %51 to ptr addrspace(1)
  store i16 %53, ptr %34, !tbaa !2
  %55 = getelementptr inbounds i8, ptr %34, i16 2
  store i16 %53, ptr %55, !tbaa !2
  %56 = getelementptr inbounds i8, ptr %34, i16 4
  store ptr addrspace(1) %54, ptr %56, !tbaa !2
  %57 = addrspacecast ptr %34 to ptr addrspace(1)
  %58 = getelementptr i8, ptr @$str7, i16 6
  %59 = getelementptr i8, ptr %58, i16 -4
  %60 = load i16, ptr %59
  %61 = addrspacecast ptr %58 to ptr addrspace(1)
  store i16 %60, ptr %33, !tbaa !2
  %62 = getelementptr inbounds i8, ptr %33, i16 2
  store i16 %60, ptr %62, !tbaa !2
  %63 = getelementptr inbounds i8, ptr %33, i16 4
  store ptr addrspace(1) %61, ptr %63, !tbaa !2
  %64 = addrspacecast ptr %33 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Ticket.new(ptr addrspace(1) %43, ptr addrspace(1) %50, ptr addrspace(1) %57, ptr addrspace(1) %64)
  %65 = load ptr, ptr %36, !tbaa !2
  %66 = getelementptr inbounds i8, ptr %36, i16 2
  %67 = load ptr, ptr %66, !tbaa !2
  store ptr %65, ptr %37, !tbaa !2
  %68 = getelementptr inbounds i8, ptr %37, i16 2
  store ptr %67, ptr %68, !tbaa !2
  %69 = addrspacecast ptr %37 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Desk.open(ptr addrspace(1) %42, ptr addrspace(1) %69)
  %70 = addrspacecast ptr %38 to ptr addrspace(1)
  %71 = addrspacecast ptr %31 to ptr addrspace(1)
  %72 = getelementptr i8, ptr @$str8, i16 6
  %73 = getelementptr i8, ptr %72, i16 -4
  %74 = load i16, ptr %73
  %75 = addrspacecast ptr %72 to ptr addrspace(1)
  store i16 %74, ptr %30, !tbaa !2
  %76 = getelementptr inbounds i8, ptr %30, i16 2
  store i16 %74, ptr %76, !tbaa !2
  %77 = getelementptr inbounds i8, ptr %30, i16 4
  store ptr addrspace(1) %75, ptr %77, !tbaa !2
  %78 = addrspacecast ptr %30 to ptr addrspace(1)
  %79 = getelementptr i8, ptr @$str9, i16 6
  %80 = getelementptr i8, ptr %79, i16 -4
  %81 = load i16, ptr %80
  %82 = addrspacecast ptr %79 to ptr addrspace(1)
  store i16 %81, ptr %29, !tbaa !2
  %83 = getelementptr inbounds i8, ptr %29, i16 2
  store i16 %81, ptr %83, !tbaa !2
  %84 = getelementptr inbounds i8, ptr %29, i16 4
  store ptr addrspace(1) %82, ptr %84, !tbaa !2
  %85 = addrspacecast ptr %29 to ptr addrspace(1)
  %86 = getelementptr i8, ptr @$str10, i16 6
  %87 = getelementptr i8, ptr %86, i16 -4
  %88 = load i16, ptr %87
  %89 = addrspacecast ptr %86 to ptr addrspace(1)
  store i16 %88, ptr %28, !tbaa !2
  %90 = getelementptr inbounds i8, ptr %28, i16 2
  store i16 %88, ptr %90, !tbaa !2
  %91 = getelementptr inbounds i8, ptr %28, i16 4
  store ptr addrspace(1) %89, ptr %91, !tbaa !2
  %92 = addrspacecast ptr %28 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Ticket.new(ptr addrspace(1) %71, ptr addrspace(1) %78, ptr addrspace(1) %85, ptr addrspace(1) %92)
  %93 = load ptr, ptr %31, !tbaa !2
  %94 = getelementptr inbounds i8, ptr %31, i16 2
  %95 = load ptr, ptr %94, !tbaa !2
  store ptr %93, ptr %32, !tbaa !2
  %96 = getelementptr inbounds i8, ptr %32, i16 2
  store ptr %95, ptr %96, !tbaa !2
  %97 = addrspacecast ptr %32 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Desk.open(ptr addrspace(1) %70, ptr addrspace(1) %97)
  %98 = addrspacecast ptr %38 to ptr addrspace(1)
  %99 = addrspacecast ptr %26 to ptr addrspace(1)
  %100 = getelementptr i8, ptr @$str11, i16 6
  %101 = getelementptr i8, ptr %100, i16 -4
  %102 = load i16, ptr %101
  %103 = addrspacecast ptr %100 to ptr addrspace(1)
  store i16 %102, ptr %25, !tbaa !2
  %104 = getelementptr inbounds i8, ptr %25, i16 2
  store i16 %102, ptr %104, !tbaa !2
  %105 = getelementptr inbounds i8, ptr %25, i16 4
  store ptr addrspace(1) %103, ptr %105, !tbaa !2
  %106 = addrspacecast ptr %25 to ptr addrspace(1)
  %107 = getelementptr i8, ptr @$str12, i16 6
  %108 = getelementptr i8, ptr %107, i16 -4
  %109 = load i16, ptr %108
  %110 = addrspacecast ptr %107 to ptr addrspace(1)
  store i16 %109, ptr %24, !tbaa !2
  %111 = getelementptr inbounds i8, ptr %24, i16 2
  store i16 %109, ptr %111, !tbaa !2
  %112 = getelementptr inbounds i8, ptr %24, i16 4
  store ptr addrspace(1) %110, ptr %112, !tbaa !2
  %113 = addrspacecast ptr %24 to ptr addrspace(1)
  %114 = getelementptr i8, ptr @$str13, i16 6
  %115 = getelementptr i8, ptr %114, i16 -4
  %116 = load i16, ptr %115
  %117 = addrspacecast ptr %114 to ptr addrspace(1)
  store i16 %116, ptr %23, !tbaa !2
  %118 = getelementptr inbounds i8, ptr %23, i16 2
  store i16 %116, ptr %118, !tbaa !2
  %119 = getelementptr inbounds i8, ptr %23, i16 4
  store ptr addrspace(1) %117, ptr %119, !tbaa !2
  %120 = addrspacecast ptr %23 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Ticket.new(ptr addrspace(1) %99, ptr addrspace(1) %106, ptr addrspace(1) %113, ptr addrspace(1) %120)
  %121 = load ptr, ptr %26, !tbaa !2
  %122 = getelementptr inbounds i8, ptr %26, i16 2
  %123 = load ptr, ptr %122, !tbaa !2
  store ptr %121, ptr %27, !tbaa !2
  %124 = getelementptr inbounds i8, ptr %27, i16 2
  store ptr %123, ptr %124, !tbaa !2
  %125 = addrspacecast ptr %27 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Desk.open(ptr addrspace(1) %98, ptr addrspace(1) %125)
  %126 = addrspacecast ptr %22 to ptr addrspace(1)
  %127 = addrspacecast ptr %38 to ptr addrspace(1)
  %128 = getelementptr i8, ptr @$str8, i16 6
  %129 = getelementptr i8, ptr %128, i16 -4
  %130 = load i16, ptr %129
  %131 = addrspacecast ptr %128 to ptr addrspace(1)
  store i16 %130, ptr %21, !tbaa !2
  %132 = getelementptr inbounds i8, ptr %21, i16 2
  store i16 %130, ptr %132, !tbaa !2
  %133 = getelementptr inbounds i8, ptr %21, i16 4
  store ptr addrspace(1) %131, ptr %133, !tbaa !2
  %134 = addrspacecast ptr %21 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Desk.claim(ptr addrspace(1) %126, ptr addrspace(1) %127, ptr addrspace(1) %134)
  %135 = load i8, ptr %22, !tbaa !2, !range !6
  %136 = icmp eq i8 %135, 0
  br i1 %136, label %b4, label %b3

b2:
  %137 = addrspacecast ptr %16 to ptr addrspace(1)
  %138 = addrspacecast ptr %38 to ptr addrspace(1)
  %139 = getelementptr i8, ptr @$str15, i16 6
  %140 = getelementptr i8, ptr %139, i16 -4
  %141 = load i16, ptr %140
  %142 = addrspacecast ptr %139 to ptr addrspace(1)
  store i16 %141, ptr %15, !tbaa !2
  %143 = getelementptr inbounds i8, ptr %15, i16 2
  store i16 %141, ptr %143, !tbaa !2
  %144 = getelementptr inbounds i8, ptr %15, i16 4
  store ptr addrspace(1) %142, ptr %144, !tbaa !2
  %145 = addrspacecast ptr %15 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Desk.claim(ptr addrspace(1) %137, ptr addrspace(1) %138, ptr addrspace(1) %145)
  %146 = load i8, ptr %16, !tbaa !2, !range !6
  %147 = icmp eq i8 %146, 0
  br i1 %147, label %b20, label %b19

b3:
  %148 = getelementptr i8, ptr @$str14, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %148)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b2

b4:
  %149 = getelementptr inbounds i8, ptr %22, i16 2
  %150 = load ptr, ptr %149, !tbaa !2
  %151 = getelementptr inbounds i8, ptr %22, i16 4
  %152 = load ptr, ptr %151, !tbaa !2
  store ptr %150, ptr %20, !tbaa !2
  %153 = getelementptr inbounds i8, ptr %20, i16 2
  store ptr %152, ptr %153, !tbaa !2
  %154 = addrspacecast ptr %38 to ptr addrspace(1)
  %155 = load ptr, ptr %20, !tbaa !2
  %156 = getelementptr inbounds i8, ptr %20, i16 2
  %157 = load ptr, ptr %156, !tbaa !2
  store ptr %155, ptr %19, !tbaa !2
  %158 = getelementptr inbounds i8, ptr %19, i16 2
  store ptr %157, ptr %158, !tbaa !2
  store ptr null, ptr %20, !tbaa !2
  %159 = getelementptr inbounds i8, ptr %20, i16 2
  store ptr null, ptr %159, !tbaa !2
  %160 = addrspacecast ptr %19 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Desk.close(ptr addrspace(1) %154, ptr addrspace(1) %160)
  %161 = getelementptr inbounds i8, ptr %20, i16 2
  %162 = load ptr, ptr %161, !tbaa !2
  %163 = icmp ne ptr %162, null
  br i1 %163, label %b6, label %b5

b5:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %162)
  %164 = load ptr, ptr %20, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %164)
  br label %b2

b6:
  %165 = getelementptr i8, ptr %162, i16 -4
  %166 = load i16, ptr %165
  store i16 0, ptr %18, !tbaa !2
  br label %b7

b7:
  %167 = load i16, ptr %18, !tbaa !2
  %168 = icmp ult i16 %167, %166
  br i1 %168, label %b9, label %b8

b8:
  br label %b5

b9:
  %169 = mul i16 %167, 2
  %170 = getelementptr inbounds i8, ptr %162, i16 %169
  %171 = load ptr, ptr %170
  call cc 1005 addrspace(1) void @N$BDRP(ptr %171)
  %172 = add i16 %167, 1
  store i16 %172, ptr %18, !tbaa !2
  br label %b7

b18:
  br label %b34

b19:
  %173 = getelementptr i8, ptr @$str16, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %173)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b18

b20:
  %174 = getelementptr inbounds i8, ptr %16, i16 2
  %175 = load ptr, ptr %174, !tbaa !2
  %176 = getelementptr inbounds i8, ptr %16, i16 4
  %177 = load ptr, ptr %176, !tbaa !2
  store ptr %175, ptr %14, !tbaa !2
  %178 = getelementptr inbounds i8, ptr %14, i16 2
  store ptr %177, ptr %178, !tbaa !2
  %179 = addrspacecast ptr %38 to ptr addrspace(1)
  %180 = load ptr, ptr %14, !tbaa !2
  %181 = getelementptr inbounds i8, ptr %14, i16 2
  %182 = load ptr, ptr %181, !tbaa !2
  store ptr %180, ptr %13, !tbaa !2
  %183 = getelementptr inbounds i8, ptr %13, i16 2
  store ptr %182, ptr %183, !tbaa !2
  store ptr null, ptr %14, !tbaa !2
  %184 = getelementptr inbounds i8, ptr %14, i16 2
  store ptr null, ptr %184, !tbaa !2
  %185 = addrspacecast ptr %13 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Desk.close(ptr addrspace(1) %179, ptr addrspace(1) %185)
  %186 = getelementptr inbounds i8, ptr %14, i16 2
  %187 = load ptr, ptr %186, !tbaa !2
  %188 = icmp ne ptr %187, null
  br i1 %188, label %b22, label %b21

b21:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %187)
  %189 = load ptr, ptr %14, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %189)
  br label %b18

b22:
  %190 = getelementptr i8, ptr %187, i16 -4
  %191 = load i16, ptr %190
  store i16 0, ptr %12, !tbaa !2
  br label %b23

b23:
  %192 = load i16, ptr %12, !tbaa !2
  %193 = icmp ult i16 %192, %191
  br i1 %193, label %b25, label %b24

b24:
  br label %b21

b25:
  %194 = mul i16 %192, 2
  %195 = getelementptr inbounds i8, ptr %187, i16 %194
  %196 = load ptr, ptr %195
  call cc 1005 addrspace(1) void @N$BDRP(ptr %196)
  %197 = add i16 %192, 1
  store i16 %197, ptr %12, !tbaa !2
  br label %b23

b34:
  br label %b35

b35:
  %198 = addrspacecast ptr %10 to ptr addrspace(1)
  %199 = addrspacecast ptr %38 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Desk.next(ptr addrspace(1) %198, ptr addrspace(1) %199)
  %200 = load i8, ptr %10, !tbaa !2, !range !6
  %201 = icmp eq i8 %200, 0
  br i1 %201, label %b39, label %b38

b36:
  %202 = load ptr, ptr %38, !tbaa !2
  %203 = getelementptr i8, ptr %202, i16 -4
  %204 = load i16, ptr %203
  call cc 1005 addrspace(1) void @N$PU2(i16 %204)
  %205 = getelementptr i8, ptr @$str17, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %205)
  %206 = getelementptr inbounds i8, ptr %38, i16 2
  %207 = load ptr, ptr %206, !tbaa !2
  %208 = getelementptr i8, ptr %207, i16 -4
  %209 = load i16, ptr %208
  call cc 1005 addrspace(1) void @N$PU2(i16 %209)
  %210 = getelementptr i8, ptr @$str18, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %210)
  call cc 1005 addrspace(1) void @N$PN()
  %211 = getelementptr inbounds i8, ptr %38, i16 2
  %212 = load ptr, ptr %211, !tbaa !2
  %213 = getelementptr i8, ptr %212, i16 -4
  %214 = load i16, ptr %213
  %215 = addrspacecast ptr %212 to ptr addrspace(1)
  store i16 %214, ptr %5, !tbaa !2
  %216 = getelementptr inbounds i8, ptr %5, i16 2
  store i16 %214, ptr %216, !tbaa !2
  %217 = getelementptr inbounds i8, ptr %5, i16 4
  store ptr addrspace(1) %215, ptr %217, !tbaa !2
  %218 = addrspacecast ptr %5 to ptr addrspace(1)
  %219 = load i16, ptr addrspace(1) %218
  store i16 0, ptr %4, !tbaa !2
  br label %b53

b37:
  br label %b34

b38:
  br label %b36

b39:
  %220 = getelementptr inbounds i8, ptr %10, i16 2
  %221 = load ptr, ptr %220, !tbaa !2
  %222 = getelementptr inbounds i8, ptr %10, i16 4
  %223 = load ptr, ptr %222, !tbaa !2
  store ptr %221, ptr %9, !tbaa !2
  %224 = getelementptr inbounds i8, ptr %9, i16 2
  store ptr %223, ptr %224, !tbaa !2
  %225 = addrspacecast ptr %38 to ptr addrspace(1)
  %226 = load ptr, ptr %9, !tbaa !2
  %227 = getelementptr inbounds i8, ptr %9, i16 2
  %228 = load ptr, ptr %227, !tbaa !2
  store ptr %226, ptr %8, !tbaa !2
  %229 = getelementptr inbounds i8, ptr %8, i16 2
  store ptr %228, ptr %229, !tbaa !2
  store ptr null, ptr %9, !tbaa !2
  %230 = getelementptr inbounds i8, ptr %9, i16 2
  store ptr null, ptr %230, !tbaa !2
  %231 = addrspacecast ptr %8 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Desk.close(ptr addrspace(1) %225, ptr addrspace(1) %231)
  %232 = getelementptr inbounds i8, ptr %9, i16 2
  %233 = load ptr, ptr %232, !tbaa !2
  %234 = icmp ne ptr %233, null
  br i1 %234, label %b41, label %b40

b40:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %233)
  %235 = load ptr, ptr %9, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %235)
  br label %b37

b41:
  %236 = getelementptr i8, ptr %233, i16 -4
  %237 = load i16, ptr %236
  store i16 0, ptr %7, !tbaa !2
  br label %b42

b42:
  %238 = load i16, ptr %7, !tbaa !2
  %239 = icmp ult i16 %238, %237
  br i1 %239, label %b44, label %b43

b43:
  br label %b40

b44:
  %240 = mul i16 %238, 2
  %241 = getelementptr inbounds i8, ptr %233, i16 %240
  %242 = load ptr, ptr %241
  call cc 1005 addrspace(1) void @N$BDRP(ptr %242)
  %243 = add i16 %238, 1
  store i16 %243, ptr %7, !tbaa !2
  br label %b42

b53:
  %244 = load i16, ptr %4, !tbaa !2
  %245 = icmp ult i16 %244, %219
  br i1 %245, label %b54, label %b56

b54:
  %246 = getelementptr i8, ptr addrspace(1) %218, i16 4
  %247 = load ptr addrspace(1), ptr addrspace(1) %246, !tbaa !2
  %248 = mul i16 %244, 4
  %249 = getelementptr inbounds i8, ptr addrspace(1) %247, i16 %248
  %250 = getelementptr i8, ptr @$str19, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %250)
  %251 = load ptr, ptr addrspace(1) %249
  call cc 1005 addrspace(1) void @N$PS(ptr %251)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b55

b55:
  %252 = load i16, ptr %4, !tbaa !2
  %253 = add i16 %252, 1
  store i16 %253, ptr %4, !tbaa !2
  br label %b53

b56:
  %254 = getelementptr inbounds i8, ptr %38, i16 2
  %255 = load ptr, ptr %254, !tbaa !2
  %256 = icmp ne ptr %255, null
  br i1 %256, label %b58, label %b57

b57:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %255)
  %257 = load ptr, ptr %38, !tbaa !2
  %258 = icmp ne ptr %257, null
  br i1 %258, label %b68, label %b67

b58:
  %259 = getelementptr i8, ptr %255, i16 -4
  %260 = load i16, ptr %259
  store i16 0, ptr %3, !tbaa !2
  br label %b59

b59:
  %261 = load i16, ptr %3, !tbaa !2
  %262 = icmp ult i16 %261, %260
  br i1 %262, label %b61, label %b60

b60:
  br label %b57

b61:
  %263 = mul i16 %261, 4
  %264 = getelementptr inbounds i8, ptr %255, i16 %263
  %265 = getelementptr i8, ptr %264, i16 2
  %266 = load ptr, ptr %265
  %267 = icmp ne ptr %266, null
  br i1 %267, label %b63, label %b62

b62:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %266)
  %268 = load ptr, ptr %264
  call cc 1005 addrspace(1) void @N$BDRP(ptr %268)
  %269 = add i16 %261, 1
  store i16 %269, ptr %3, !tbaa !2
  br label %b59

b63:
  %270 = getelementptr i8, ptr %266, i16 -4
  %271 = load i16, ptr %270
  store i16 0, ptr %2, !tbaa !2
  br label %b64

b64:
  %272 = load i16, ptr %2, !tbaa !2
  %273 = icmp ult i16 %272, %271
  br i1 %273, label %b66, label %b65

b65:
  br label %b62

b66:
  %274 = mul i16 %272, 2
  %275 = getelementptr inbounds i8, ptr %266, i16 %274
  %276 = load ptr, ptr %275
  call cc 1005 addrspace(1) void @N$BDRP(ptr %276)
  %277 = add i16 %272, 1
  store i16 %277, ptr %2, !tbaa !2
  br label %b64

b67:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %257)
  ret i16 0

b68:
  %278 = getelementptr i8, ptr %257, i16 -4
  %279 = load i16, ptr %278
  store i16 0, ptr %1, !tbaa !2
  br label %b69

b69:
  %280 = load i16, ptr %1, !tbaa !2
  %281 = icmp ult i16 %280, %279
  br i1 %281, label %b71, label %b70

b70:
  br label %b67

b71:
  %282 = mul i16 %280, 4
  %283 = getelementptr inbounds i8, ptr %257, i16 %282
  %284 = getelementptr i8, ptr %283, i16 2
  %285 = load ptr, ptr %284
  %286 = icmp ne ptr %285, null
  br i1 %286, label %b73, label %b72

b72:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %285)
  %287 = load ptr, ptr %283
  call cc 1005 addrspace(1) void @N$BDRP(ptr %287)
  %288 = add i16 %280, 1
  store i16 %288, ptr %1, !tbaa !2
  br label %b69

b73:
  %289 = getelementptr i8, ptr %285, i16 -4
  %290 = load i16, ptr %289
  store i16 0, ptr %0, !tbaa !2
  br label %b74

b74:
  %291 = load i16, ptr %0, !tbaa !2
  %292 = icmp ult i16 %291, %290
  br i1 %292, label %b76, label %b75

b75:
  br label %b72

b76:
  %293 = mul i16 %291, 2
  %294 = getelementptr inbounds i8, ptr %285, i16 %293
  %295 = load ptr, ptr %294
  call cc 1005 addrspace(1) void @N$BDRP(ptr %295)
  %296 = add i16 %291, 1
  store i16 %296, ptr %0, !tbaa !2
  br label %b74
}

declare cc 1005 ptr @N$VCPY(ptr addrspace(1)) addrspace(1)

declare cc 1005 ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

declare cc 1005 i16 @N$BSHR(ptr, i16) addrspace(1)

declare void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) noalias nocapture writeonly, ptr noalias nocapture readonly, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)

declare cc 1005 i8 @N$VCMP(ptr addrspace(1), ptr addrspace(1)) addrspace(1) memory(read)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PU2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i8 0, i8 2}
!6 = !{i8 0, i8 2}
