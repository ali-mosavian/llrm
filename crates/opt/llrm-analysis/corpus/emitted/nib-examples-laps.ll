target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [11 x i8] c"\08\00\04\00\04\00lap \00"
@$str2 = internal constant [9 x i8] c"\08\00\02\00\02\00: \00"
@$str3 = internal constant [8 x i8] c"\08\00\01\00\01\00s\00"
@$str4 = internal constant [19 x i8] c"\08\00\0C\00\0C\00fastest lap \00"
@$str5 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str6 = internal constant [10 x i8] c"\08\00\03\00\03\00ada\00"
@$str7 = internal constant [10 x i8] c"\08\00\03\00\03\00bob\00"
@$str8 = internal constant [13 x i8] c"\08\00\06\00\06\00 made \00"
@$str9 = internal constant [11 x i8] c"\08\00\04\00\04\00 is \00"
@$str10 = internal constant [13 x i8] c"\08\00\06\00\06\00 short\00"
@$str11 = internal constant [15 x i8] c"\08\00\08\00\08\00s pace: \00"
@$str12 = internal constant [20 x i8] c"\08\00\0D\00\0D\00s for 12 laps\00"

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca [8 x i8]
  %4 = alloca i16
  %5 = alloca [8 x i8]
  %6 = alloca [8 x i8]
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca [4 x i8]
  %12 = alloca i16
  %13 = alloca ptr
  %14 = alloca i16
  %15 = alloca [6 x i8]
  %16 = alloca i16
  %17 = alloca i16
  %18 = alloca [8 x i8]
  %19 = alloca i16
  %20 = alloca i16
  %21 = alloca i16
  %22 = alloca [8 x i8]
  store i16 4, ptr %20, !tbaa !2
  store i16 4, ptr %21, !tbaa !2
  %23 = getelementptr inbounds i16, ptr %22, i16 0
  store i16 62, ptr %23, !tbaa !2
  %24 = getelementptr inbounds i16, ptr %22, i16 1
  store i16 58, ptr %24, !tbaa !2
  %25 = getelementptr inbounds i16, ptr %22, i16 2
  store i16 60, ptr %25, !tbaa !2
  %26 = getelementptr inbounds i16, ptr %22, i16 3
  store i16 57, ptr %26, !tbaa !2
  store i16 0, ptr %19, !tbaa !2
  %27 = addrspacecast ptr %22 to ptr addrspace(1)
  store i16 4, ptr %18, !tbaa !2
  %28 = getelementptr inbounds i8, ptr %18, i16 2
  store i16 4, ptr %28, !tbaa !2
  %29 = getelementptr inbounds i8, ptr %18, i16 4
  store ptr addrspace(1) %27, ptr %29, !tbaa !2
  %30 = addrspacecast ptr %18 to ptr addrspace(1)
  store i16 0, ptr %17, !tbaa !2
  %31 = load i16, ptr addrspace(1) %30
  store i16 0, ptr %16, !tbaa !2
  br label %b3

b2:
  %32 = getelementptr i8, ptr @$str4, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %32)
  %33 = load i16, ptr %19, !tbaa !2
  %34 = add i16 %33, 1
  call cc 1005 addrspace(1) void @N$PU2(i16 %34)
  call cc 1005 addrspace(1) void @N$PN()
  %35 = getelementptr i8, ptr @$str5, i16 6
  %36 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %35, i16 2, i16 4)
  %37 = getelementptr inbounds i8, ptr %36, i16 0
  %38 = getelementptr i8, ptr @$str6, i16 6
  store ptr %38, ptr %37
  %39 = getelementptr i8, ptr %37, i16 2
  store i16 12, ptr %39
  %40 = getelementptr inbounds i8, ptr %36, i16 4
  %41 = getelementptr i8, ptr @$str7, i16 6
  store ptr %41, ptr %40
  %42 = getelementptr i8, ptr %40, i16 2
  store i16 9, ptr %42
  store ptr %36, ptr %13, !tbaa !2
  store i16 10, ptr %12, !tbaa !2
  store i16 2, ptr %9, !tbaa !2
  store i16 2, ptr %10, !tbaa !2
  store i16 0, ptr %8, !tbaa !2
  store i16 2, ptr %7, !tbaa !2
  br label %b13

b3:
  %43 = load i16, ptr %16, !tbaa !2
  %44 = icmp ult i16 %43, %31
  br i1 %44, label %b4, label %b6

b4:
  %45 = getelementptr i8, ptr addrspace(1) %30, i16 4
  %46 = load ptr addrspace(1), ptr addrspace(1) %45, !tbaa !2
  %47 = mul i16 %43, 2
  %48 = getelementptr inbounds i8, ptr addrspace(1) %46, i16 %47
  %49 = load i16, ptr %17, !tbaa !2
  store i16 %49, ptr %15, !tbaa !2
  %50 = getelementptr inbounds i8, ptr %15, i16 2
  store ptr addrspace(1) %48, ptr %50, !tbaa !2
  %51 = load i16, ptr %15, !tbaa !2
  %52 = getelementptr inbounds i8, ptr %15, i16 2
  %53 = load ptr addrspace(1), ptr %52, !tbaa !2
  store i16 %51, ptr %14, !tbaa !2
  %54 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %54)
  %55 = load i16, ptr %14, !tbaa !2
  %56 = add i16 %55, 1
  call cc 1005 addrspace(1) void @N$PU2(i16 %56)
  %57 = getelementptr i8, ptr @$str2, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %57)
  %58 = load i16, ptr addrspace(1) %53
  call cc 1005 addrspace(1) void @N$PI2(i16 %58)
  %59 = getelementptr i8, ptr @$str3, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %59)
  call cc 1005 addrspace(1) void @N$PN()
  %60 = load i16, ptr addrspace(1) %53
  %61 = load i16, ptr %19, !tbaa !2
  %62 = icmp ult i16 %61, 4
  br i1 %62, label %b11, label %b12

b5:
  %63 = load i16, ptr %16, !tbaa !2
  %64 = add i16 %63, 1
  store i16 %64, ptr %16, !tbaa !2
  br label %b3

b6:
  br label %b2

b7:
  %65 = load i16, ptr %17, !tbaa !2
  %66 = add i16 %65, 1
  store i16 %66, ptr %17, !tbaa !2
  br label %b5

b8:
  %67 = load i16, ptr %14, !tbaa !2
  store i16 %67, ptr %19, !tbaa !2
  br label %b10

b9:
  br label %b10

b10:
  br label %b7

b11:
  %68 = sub i16 %61, 0
  %69 = getelementptr inbounds i16, ptr %22, i16 %68
  %70 = load i16, ptr %69, !tbaa !2
  %71 = icmp slt i16 %60, %70
  br i1 %71, label %b8, label %b9

b12:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b13:
  %72 = load i16, ptr %8, !tbaa !2
  %73 = load i16, ptr %7, !tbaa !2
  %74 = icmp slt i16 %72, %73
  br i1 %74, label %b14, label %b16

b14:
  %75 = load i16, ptr %8, !tbaa !2
  %76 = load i16, ptr %12, !tbaa !2
  %77 = sub i16 %75, 0
  %78 = getelementptr inbounds i16, ptr %11, i16 %77
  store i16 %76, ptr %78, !tbaa !2
  br label %b15

b15:
  %79 = load i16, ptr %8, !tbaa !2
  %80 = add nsw i16 %79, 1
  store i16 %80, ptr %8, !tbaa !2
  br label %b13

b16:
  %81 = load ptr, ptr %13, !tbaa !2
  %82 = getelementptr i8, ptr %81, i16 -4
  %83 = load i16, ptr %82
  %84 = addrspacecast ptr %81 to ptr addrspace(1)
  store i16 %83, ptr %6, !tbaa !2
  %85 = getelementptr inbounds i8, ptr %6, i16 2
  store i16 %83, ptr %85, !tbaa !2
  %86 = getelementptr inbounds i8, ptr %6, i16 4
  store ptr addrspace(1) %84, ptr %86, !tbaa !2
  %87 = addrspacecast ptr %6 to ptr addrspace(1)
  %88 = addrspacecast ptr %11 to ptr addrspace(1)
  store i16 2, ptr %5, !tbaa !2
  %89 = getelementptr inbounds i8, ptr %5, i16 2
  store i16 2, ptr %89, !tbaa !2
  %90 = getelementptr inbounds i8, ptr %5, i16 4
  store ptr addrspace(1) %88, ptr %90, !tbaa !2
  %91 = addrspacecast ptr %5 to ptr addrspace(1)
  store i16 0, ptr %4, !tbaa !2
  br label %b18

b17:
  store i16 58, ptr %2, !tbaa !2
  store i16 61, ptr %1, !tbaa !2
  br label %b30

b18:
  %92 = load i16, ptr %4, !tbaa !2
  %93 = load i16, ptr addrspace(1) %87
  %94 = icmp ult i16 %92, %93
  br i1 %94, label %b21, label %b20

b19:
  %95 = load i16, ptr %4, !tbaa !2
  %96 = load i16, ptr addrspace(1) %87, !tbaa !2, !range !5
  %97 = icmp ult i16 %95, %96
  br i1 %97, label %b23, label %b24

b20:
  br label %b17

b21:
  %98 = load i16, ptr %4, !tbaa !2
  %99 = load i16, ptr addrspace(1) %91
  %100 = icmp ult i16 %98, %99
  br i1 %100, label %b19, label %b20

b22:
  %101 = load i16, ptr %4, !tbaa !2
  %102 = add i16 %101, 1
  store i16 %102, ptr %4, !tbaa !2
  br label %b18

b23:
  %103 = getelementptr i8, ptr addrspace(1) %87, i16 4
  %104 = load ptr addrspace(1), ptr addrspace(1) %103, !tbaa !2
  %105 = mul i16 %95, 4
  %106 = getelementptr inbounds i8, ptr addrspace(1) %104, i16 %105
  %107 = load i16, ptr %4, !tbaa !2
  %108 = load i16, ptr addrspace(1) %91, !tbaa !2, !range !6
  %109 = icmp ult i16 %107, %108
  br i1 %109, label %b25, label %b26

b24:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b25:
  %110 = getelementptr i8, ptr addrspace(1) %91, i16 4
  %111 = load ptr addrspace(1), ptr addrspace(1) %110, !tbaa !2
  %112 = mul i16 %107, 2
  %113 = getelementptr inbounds i8, ptr addrspace(1) %111, i16 %112
  store ptr addrspace(1) %106, ptr %3, !tbaa !2
  %114 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %113, ptr %114, !tbaa !2
  %115 = load ptr addrspace(1), ptr %3, !tbaa !2
  %116 = getelementptr inbounds i8, ptr %3, i16 4
  %117 = load ptr addrspace(1), ptr %116, !tbaa !2
  %118 = getelementptr i8, ptr addrspace(1) %115, i16 2
  %119 = load i16, ptr addrspace(1) %118
  %120 = load i16, ptr addrspace(1) %117
  %121 = icmp sge i16 %119, %120
  br i1 %121, label %b27, label %b28

b26:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b27:
  %122 = load ptr, ptr addrspace(1) %115
  call cc 1005 addrspace(1) void @N$PS(ptr %122)
  %123 = getelementptr i8, ptr @$str8, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %123)
  %124 = load i16, ptr addrspace(1) %117
  call cc 1005 addrspace(1) void @N$PI2(i16 %124)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b29

b28:
  %125 = load ptr, ptr addrspace(1) %115
  call cc 1005 addrspace(1) void @N$PS(ptr %125)
  %126 = getelementptr i8, ptr @$str9, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %126)
  %127 = load i16, ptr addrspace(1) %117
  %128 = getelementptr i8, ptr addrspace(1) %115, i16 2
  %129 = load i16, ptr addrspace(1) %128
  %130 = sub i16 %127, %129
  call cc 1005 addrspace(1) void @N$PI2(i16 %130)
  %131 = getelementptr i8, ptr @$str10, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %131)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b29

b29:
  br label %b22

b30:
  %132 = load i16, ptr %2, !tbaa !2
  %133 = load i16, ptr %1, !tbaa !2
  %134 = icmp slt i16 %132, %133
  br i1 %134, label %b31, label %b33

b31:
  %135 = load i16, ptr %2, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI2(i16 %135)
  %136 = getelementptr i8, ptr @$str11, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %136)
  %137 = load i16, ptr %2, !tbaa !2
  %138 = mul i16 %137, 12
  call cc 1005 addrspace(1) void @N$PI2(i16 %138)
  %139 = getelementptr i8, ptr @$str12, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %139)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b32

b32:
  %140 = load i16, ptr %2, !tbaa !2
  %141 = add nsw i16 %140, 1
  store i16 %141, ptr %2, !tbaa !2
  br label %b30

b33:
  %142 = load ptr, ptr %13, !tbaa !2
  %143 = icmp ne ptr %142, null
  br i1 %143, label %b35, label %b34

b34:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %142)
  ret i16 0

b35:
  %144 = getelementptr i8, ptr %142, i16 -4
  %145 = load i16, ptr %144
  store i16 0, ptr %0, !tbaa !2
  br label %b36

b36:
  %146 = load i16, ptr %0, !tbaa !2
  %147 = icmp ult i16 %146, %145
  br i1 %147, label %b38, label %b37

b37:
  br label %b34

b38:
  %148 = mul i16 %146, 4
  %149 = getelementptr inbounds i8, ptr %142, i16 %148
  %150 = load ptr, ptr %149
  call cc 1005 addrspace(1) void @N$BDRP(ptr %150)
  %151 = add i16 %146, 1
  store i16 %151, ptr %0, !tbaa !2
  br label %b36
}

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PU2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i16 0, i16 16384}
!6 = !{i16 0, i16 -32768}
