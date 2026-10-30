target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [10 x i8] c"\08\00\03\00\03\00ada\00"
@$str2 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str3 = internal constant [9 x i8] c"\08\00\02\00\02\00: \00"
@$str4 = internal constant [21 x i8] c"\08\00\0E\00\0E\00 marks, first \00"
@$str5 = internal constant [14 x i8] c"\08\00\07\00\07\00recent \00"
@$str6 = internal constant [17 x i8] c"\08\00\0A\00\0A\00, average \00"
@$str7 = internal constant [12 x i8] c"\08\00\05\00\05\00best \00"
@$str8 = internal constant [21 x i8] c"\08\00\0E\00\0E\00week 2 day 2: \00"
@$str9 = internal constant [20 x i8] c"\08\00\0D\00\0D\00, week 1 has \00"

define internal cc 1005 void @curve(ptr addrspace(1) nocapture %0, i16 %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca i16
  %3 = alloca i16
  %4 = load ptr, ptr addrspace(1) %0
  %5 = getelementptr i8, ptr %4, i16 -4
  %6 = load i16, ptr %5
  store i16 0, ptr %3, !tbaa !2
  store i16 %6, ptr %2, !tbaa !2
  br label %b2

b2:
  %7 = load i16, ptr %3, !tbaa !2
  %8 = load i16, ptr %2, !tbaa !2
  %9 = icmp ult i16 %7, %8
  br i1 %9, label %b3, label %b5

b3:
  %10 = load ptr, ptr addrspace(1) %0
  %11 = load i16, ptr %3, !tbaa !2
  %12 = getelementptr i8, ptr %10, i16 -4
  %13 = load i16, ptr %12
  %14 = icmp ult i16 %11, %13
  br i1 %14, label %b6, label %b7

b4:
  %15 = load i16, ptr %3, !tbaa !2
  %16 = add nuw i16 %15, 1
  store i16 %16, ptr %3, !tbaa !2
  br label %b2

b5:
  %17 = load ptr, ptr addrspace(1) %0
  %18 = getelementptr i8, ptr %17, i16 -4
  %19 = load i16, ptr %18
  %20 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %17, i16 1, i16 2)
  store ptr %20, ptr addrspace(1) %0
  %21 = mul i16 %19, 2
  %22 = getelementptr inbounds i8, ptr %20, i16 %21
  store i16 %1, ptr %22
  ret void

b6:
  %23 = mul i16 %11, 2
  %24 = getelementptr inbounds i8, ptr %10, i16 %23
  %25 = load i16, ptr %24
  %26 = add i16 %25, %1
  store i16 %26, ptr %24
  br label %b4

b7:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

define internal cc 1005 i16 @average(ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  store i16 0, ptr %2, !tbaa !2
  %3 = load i16, ptr addrspace(1) %0
  store i16 0, ptr %1, !tbaa !2
  br label %b2

b2:
  %4 = load i16, ptr %1, !tbaa !2
  %5 = icmp ult i16 %4, %3
  br i1 %5, label %b3, label %b5

b3:
  %6 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %7 = load ptr addrspace(1), ptr addrspace(1) %6
  %8 = mul i16 %4, 2
  %9 = getelementptr inbounds i8, ptr addrspace(1) %7, i16 %8
  %10 = load i16, ptr %2, !tbaa !2
  %11 = load i16, ptr addrspace(1) %9
  %12 = add i16 %10, %11
  store i16 %12, ptr %2, !tbaa !2
  br label %b4

b4:
  %13 = load i16, ptr %1, !tbaa !2
  %14 = add i16 %13, 1
  store i16 %14, ptr %1, !tbaa !2
  br label %b2

b5:
  %15 = load i16, ptr %2, !tbaa !2
  %16 = load i16, ptr addrspace(1) %0
  %17 = sdiv i16 %15, %16
  %18 = srem i16 %15, %16
  %19 = icmp ne i16 %18, 0
  %20 = zext i1 %19 to i8
  %21 = xor i16 %18, %16
  %22 = icmp slt i16 %21, 0
  %23 = zext i1 %22 to i8
  %24 = and i8 %20, %23
  %25 = zext i8 %24 to i16
  %26 = and i16 %25, 1
  %27 = sub i16 %17, %26
  ret i16 %27
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca ptr
  %2 = alloca i16
  %3 = alloca [8 x i8]
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca [8 x i8]
  %8 = alloca [4 x i8]
  %9 = getelementptr i8, ptr @$str1, i16 6
  %10 = getelementptr i8, ptr @$str2, i16 6
  store ptr %9, ptr %8, !tbaa !2
  %11 = getelementptr inbounds i8, ptr %8, i16 2
  store ptr %10, ptr %11, !tbaa !2
  %12 = getelementptr inbounds i8, ptr %8, i16 2
  %13 = load ptr, ptr %12, !tbaa !2
  %14 = getelementptr i8, ptr %13, i16 -4
  %15 = load i16, ptr %14
  %16 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %13, i16 1, i16 2)
  %17 = getelementptr inbounds i8, ptr %8, i16 2
  store ptr %16, ptr %17, !tbaa !2
  %18 = mul i16 %15, 2
  %19 = getelementptr inbounds i8, ptr %16, i16 %18
  store i16 71, ptr %19
  %20 = getelementptr inbounds i8, ptr %8, i16 2
  %21 = load ptr, ptr %20, !tbaa !2
  %22 = getelementptr i8, ptr %21, i16 -4
  %23 = load i16, ptr %22
  %24 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %21, i16 1, i16 2)
  %25 = getelementptr inbounds i8, ptr %8, i16 2
  store ptr %24, ptr %25, !tbaa !2
  %26 = mul i16 %23, 2
  %27 = getelementptr inbounds i8, ptr %24, i16 %26
  store i16 64, ptr %27
  %28 = getelementptr inbounds i8, ptr %8, i16 2
  %29 = load ptr, ptr %28, !tbaa !2
  %30 = getelementptr i8, ptr %29, i16 -4
  %31 = load i16, ptr %30
  %32 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %29, i16 1, i16 2)
  %33 = getelementptr inbounds i8, ptr %8, i16 2
  store ptr %32, ptr %33, !tbaa !2
  %34 = mul i16 %31, 2
  %35 = getelementptr inbounds i8, ptr %32, i16 %34
  store i16 80, ptr %35
  %36 = load ptr, ptr %8, !tbaa !2
  call cc 1005 addrspace(1) void @N$PS(ptr %36)
  %37 = getelementptr i8, ptr @$str3, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %37)
  %38 = getelementptr inbounds i8, ptr %8, i16 2
  %39 = load ptr, ptr %38, !tbaa !2
  %40 = getelementptr i8, ptr %39, i16 -4
  %41 = load i16, ptr %40
  call cc 1005 addrspace(1) void @N$PU2(i16 %41)
  %42 = getelementptr i8, ptr @$str4, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %42)
  %43 = getelementptr inbounds i8, ptr %8, i16 2
  %44 = load ptr, ptr %43, !tbaa !2
  %45 = getelementptr i8, ptr %44, i16 -4
  %46 = load i16, ptr %45
  %47 = icmp ult i16 0, %46
  br i1 %47, label %b2, label %b3

b2:
  %48 = getelementptr inbounds i8, ptr %44, i16 0
  %49 = load i16, ptr %48
  call cc 1005 addrspace(1) void @N$PI2(i16 %49)
  call cc 1005 addrspace(1) void @N$PN()
  %50 = getelementptr inbounds i8, ptr %8, i16 2
  %51 = addrspacecast ptr %50 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @curve(ptr addrspace(1) %51, i16 5)
  %52 = getelementptr inbounds i8, ptr %8, i16 2
  %53 = load ptr, ptr %52, !tbaa !2
  %54 = getelementptr i8, ptr %53, i16 -4
  %55 = load i16, ptr %54
  %56 = addrspacecast ptr %53 to ptr addrspace(1)
  %57 = icmp ule i16 1, %55
  br i1 %57, label %b4, label %b5

b3:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b4:
  %58 = getelementptr inbounds i8, ptr addrspace(1) %56, i16 2
  %59 = sub i16 %55, 1
  store i16 %59, ptr %7, !tbaa !2
  %60 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 %59, ptr %60, !tbaa !2
  %61 = getelementptr inbounds i8, ptr %7, i16 4
  store ptr addrspace(1) %58, ptr %61, !tbaa !2
  %62 = addrspacecast ptr %7 to ptr addrspace(1)
  %63 = load i16, ptr addrspace(1) %62
  store i16 %63, ptr %6, !tbaa !2
  %64 = call cc 1005 addrspace(1) i16 @average(ptr addrspace(1) %62)
  store i16 %64, ptr %5, !tbaa !2
  %65 = getelementptr i8, ptr @$str5, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %65)
  %66 = load i16, ptr %6, !tbaa !2
  call cc 1005 addrspace(1) void @N$PU2(i16 %66)
  %67 = getelementptr i8, ptr @$str6, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %67)
  %68 = load i16, ptr %5, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI2(i16 %68)
  call cc 1005 addrspace(1) void @N$PN()
  store i16 0, ptr %4, !tbaa !2
  %69 = getelementptr inbounds i8, ptr %8, i16 2
  %70 = load ptr, ptr %69, !tbaa !2
  %71 = getelementptr i8, ptr %70, i16 -4
  %72 = load i16, ptr %71
  %73 = addrspacecast ptr %70 to ptr addrspace(1)
  store i16 %72, ptr %3, !tbaa !2
  %74 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 %72, ptr %74, !tbaa !2
  %75 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %73, ptr %75, !tbaa !2
  %76 = addrspacecast ptr %3 to ptr addrspace(1)
  %77 = load i16, ptr addrspace(1) %76
  store i16 0, ptr %2, !tbaa !2
  br label %b6

b5:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b6:
  %78 = load i16, ptr %2, !tbaa !2
  %79 = icmp ult i16 %78, %77
  br i1 %79, label %b7, label %b9

b7:
  %80 = getelementptr i8, ptr addrspace(1) %76, i16 4
  %81 = load ptr addrspace(1), ptr addrspace(1) %80, !tbaa !2
  %82 = mul i16 %78, 2
  %83 = getelementptr inbounds i8, ptr addrspace(1) %81, i16 %82
  %84 = load i16, ptr addrspace(1) %83
  %85 = load i16, ptr %4, !tbaa !2
  %86 = icmp sgt i16 %84, %85
  br i1 %86, label %b10, label %b11

b8:
  %87 = load i16, ptr %2, !tbaa !2
  %88 = add i16 %87, 1
  store i16 %88, ptr %2, !tbaa !2
  br label %b6

b9:
  %89 = getelementptr i8, ptr @$str7, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %89)
  %90 = load i16, ptr %4, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI2(i16 %90)
  call cc 1005 addrspace(1) void @N$PN()
  %91 = getelementptr i8, ptr @$str2, i16 6
  store ptr %91, ptr %1, !tbaa !2
  %92 = load ptr, ptr %1, !tbaa !2
  %93 = getelementptr i8, ptr %92, i16 -4
  %94 = load i16, ptr %93
  %95 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %92, i16 1, i16 2)
  store ptr %95, ptr %1, !tbaa !2
  %96 = mul i16 %94, 2
  %97 = getelementptr inbounds i8, ptr %95, i16 %96
  %98 = getelementptr i8, ptr @$str2, i16 6
  %99 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %98, i16 3, i16 2)
  %100 = getelementptr inbounds i8, ptr %99, i16 0
  store i16 1, ptr %100
  %101 = getelementptr inbounds i8, ptr %99, i16 2
  store i16 2, ptr %101
  %102 = getelementptr inbounds i8, ptr %99, i16 4
  store i16 3, ptr %102
  store ptr %99, ptr %97
  %103 = load ptr, ptr %1, !tbaa !2
  %104 = getelementptr i8, ptr %103, i16 -4
  %105 = load i16, ptr %104
  %106 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %103, i16 1, i16 2)
  store ptr %106, ptr %1, !tbaa !2
  %107 = mul i16 %105, 2
  %108 = getelementptr inbounds i8, ptr %106, i16 %107
  %109 = getelementptr i8, ptr @$str2, i16 6
  %110 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %109, i16 2, i16 2)
  %111 = getelementptr inbounds i8, ptr %110, i16 0
  store i16 4, ptr %111
  %112 = getelementptr inbounds i8, ptr %110, i16 2
  store i16 5, ptr %112
  store ptr %110, ptr %108
  %113 = getelementptr i8, ptr @$str8, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %113)
  %114 = load ptr, ptr %1, !tbaa !2
  %115 = getelementptr i8, ptr %114, i16 -4
  %116 = load i16, ptr %115
  %117 = icmp ult i16 1, %116
  br i1 %117, label %b13, label %b14

b10:
  %118 = load i16, ptr addrspace(1) %83
  store i16 %118, ptr %4, !tbaa !2
  br label %b12

b11:
  br label %b12

b12:
  br label %b8

b13:
  %119 = getelementptr inbounds i8, ptr %114, i16 2
  %120 = load ptr, ptr %119
  %121 = getelementptr i8, ptr %120, i16 -4
  %122 = load i16, ptr %121
  %123 = icmp ult i16 1, %122
  br i1 %123, label %b15, label %b16

b14:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b15:
  %124 = getelementptr inbounds i8, ptr %120, i16 2
  %125 = load i16, ptr %124
  call cc 1005 addrspace(1) void @N$PI2(i16 %125)
  %126 = getelementptr i8, ptr @$str9, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %126)
  %127 = load ptr, ptr %1, !tbaa !2
  %128 = getelementptr i8, ptr %127, i16 -4
  %129 = load i16, ptr %128
  %130 = icmp ult i16 0, %129
  br i1 %130, label %b17, label %b18

b16:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b17:
  %131 = getelementptr inbounds i8, ptr %127, i16 0
  %132 = load ptr, ptr %131
  %133 = getelementptr i8, ptr %132, i16 -4
  %134 = load i16, ptr %133
  call cc 1005 addrspace(1) void @N$PU2(i16 %134)
  call cc 1005 addrspace(1) void @N$PN()
  %135 = load ptr, ptr %1, !tbaa !2
  %136 = icmp ne ptr %135, null
  br i1 %136, label %b20, label %b19

b18:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b19:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %135)
  %137 = getelementptr inbounds i8, ptr %8, i16 2
  %138 = load ptr, ptr %137, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %138)
  %139 = load ptr, ptr %8, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %139)
  ret i16 0

b20:
  %140 = getelementptr i8, ptr %135, i16 -4
  %141 = load i16, ptr %140
  store i16 0, ptr %0, !tbaa !2
  br label %b21

b21:
  %142 = load i16, ptr %0, !tbaa !2
  %143 = icmp ult i16 %142, %141
  br i1 %143, label %b23, label %b22

b22:
  br label %b19

b23:
  %144 = mul i16 %142, 2
  %145 = getelementptr inbounds i8, ptr %135, i16 %144
  %146 = load ptr, ptr %145
  call cc 1005 addrspace(1) void @N$BDRP(ptr %146)
  %147 = add i16 %142, 1
  store i16 %147, ptr %0, !tbaa !2
  br label %b21
}

declare cc 1005 ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PU2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
