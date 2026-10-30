target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [12 x i8] c"\08\00\05\00\05\00empty\00"
@$str2 = internal constant [11 x i8] c"\08\00\04\00\04\00one \00"
@$str3 = internal constant [9 x i8] c"\08\00\02\00\02\00..\00"
@$str4 = internal constant [15 x i8] c"\08\00\08\00\08\00 around \00"
@$str5 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str6 = internal constant [8 x i8] c"\08\00\01\00\01\00 \00"
@$str7 = internal constant [16 x i8] c"\08\00\09\00\09\00first at \00"
@$str8 = internal constant [8 x i8] c"\08\00\01\00\01\00,\00"
@$str9 = internal constant [16 x i8] c"\08\00\09\00\09\00no points\00"
@$str10 = internal constant [24 x i8] c"\08\00\11\00\11\00starts 1, 2 then \00"
@$str11 = internal constant [12 x i8] c"\08\00\05\00\05\00other\00"

define internal cc 1005 i16 @describe(ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca [8 x i8]
  %2 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %3 = load ptr addrspace(1), ptr addrspace(1) %2
  %4 = load i16, ptr addrspace(1) %0
  %5 = icmp eq i16 %4, 0
  br i1 %5, label %b4, label %b3

b2:
  ret i16 0

b3:
  %6 = icmp eq i16 %4, 1
  br i1 %6, label %b6, label %b5

b4:
  %7 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %7)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b2

b5:
  %8 = getelementptr inbounds i8, ptr addrspace(1) %3, i16 0
  %9 = load i16, ptr addrspace(1) %8
  %10 = sub i16 %4, 1
  %11 = mul i16 %10, 2
  %12 = getelementptr inbounds i8, ptr addrspace(1) %3, i16 %11
  %13 = load i16, ptr addrspace(1) %12
  %14 = getelementptr inbounds i8, ptr addrspace(1) %3, i16 2
  %15 = sub i16 %4, 2
  store i16 %15, ptr %1, !tbaa !2
  %16 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 %15, ptr %16, !tbaa !2
  %17 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %14, ptr %17, !tbaa !2
  %18 = addrspacecast ptr %1 to ptr addrspace(1)
  %19 = load i16, ptr addrspace(1) %8
  call cc 1005 addrspace(1) void @N$PI2(i16 %19)
  %20 = getelementptr i8, ptr @$str3, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %20)
  %21 = load i16, ptr addrspace(1) %12
  call cc 1005 addrspace(1) void @N$PI2(i16 %21)
  %22 = getelementptr i8, ptr @$str4, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %22)
  %23 = load i16, ptr addrspace(1) %18
  call cc 1005 addrspace(1) void @N$PU2(i16 %23)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b2

b6:
  %24 = getelementptr inbounds i8, ptr addrspace(1) %3, i16 0
  %25 = load i16, ptr addrspace(1) %24
  %26 = getelementptr inbounds i8, ptr addrspace(1) %3, i16 0
  %27 = load i16, ptr addrspace(1) %26
  %28 = getelementptr i8, ptr @$str2, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %28)
  %29 = load i16, ptr addrspace(1) %26
  call cc 1005 addrspace(1) void @N$PI2(i16 %29)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b2
}

define internal cc 1005 i16 @head(ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca [8 x i8]
  %2 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %3 = load ptr addrspace(1), ptr addrspace(1) %2
  %4 = load i16, ptr addrspace(1) %0
  %5 = icmp sge i16 %4, 1
  br i1 %5, label %b4, label %b2

b2:
  ret i16 -1

b3:
  %6 = getelementptr inbounds i8, ptr addrspace(1) %3, i16 0
  %7 = load i16, ptr addrspace(1) %6
  %8 = getelementptr inbounds i8, ptr addrspace(1) %3, i16 2
  %9 = sub i16 %4, 1
  store i16 %9, ptr %1, !tbaa !2
  %10 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 %9, ptr %10, !tbaa !2
  %11 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %8, ptr %11, !tbaa !2
  %12 = addrspacecast ptr %1 to ptr addrspace(1)
  %13 = load i16, ptr addrspace(1) %6
  %14 = load i16, ptr addrspace(1) %12
  %15 = add i16 %13, %14
  ret i16 %15

b4:
  %16 = getelementptr inbounds i8, ptr addrspace(1) %3, i16 0
  %17 = load i16, ptr addrspace(1) %16
  br label %b3
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i8
  %1 = alloca i8
  %2 = alloca [8 x i8]
  %3 = alloca [8 x i8]
  %4 = alloca [8 x i8]
  %5 = alloca ptr
  %6 = alloca i16
  %7 = alloca [8 x i8]
  %8 = alloca i16
  %9 = alloca [8 x i8]
  %10 = alloca [8 x i8]
  %11 = alloca [8 x i8]
  %12 = alloca [8 x i8]
  %13 = alloca ptr
  %14 = alloca i16
  %15 = alloca i16
  %16 = alloca [2 x i8]
  %17 = alloca ptr
  %18 = getelementptr i8, ptr @$str5, i16 6
  store ptr %18, ptr %17, !tbaa !2
  store i16 1, ptr %14, !tbaa !2
  store i16 1, ptr %15, !tbaa !2
  %19 = getelementptr inbounds i16, ptr %16, i16 0
  store i16 7, ptr %19, !tbaa !2
  %20 = getelementptr i8, ptr @$str5, i16 6
  %21 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %20, i16 4, i16 2)
  %22 = getelementptr inbounds i8, ptr %21, i16 0
  store i16 1, ptr %22
  %23 = getelementptr inbounds i8, ptr %21, i16 2
  store i16 2, ptr %23
  %24 = getelementptr inbounds i8, ptr %21, i16 4
  store i16 3, ptr %24
  %25 = getelementptr inbounds i8, ptr %21, i16 6
  store i16 4, ptr %25
  store ptr %21, ptr %13, !tbaa !2
  %26 = load ptr, ptr %17, !tbaa !2
  %27 = getelementptr i8, ptr %26, i16 -4
  %28 = load i16, ptr %27
  %29 = addrspacecast ptr %26 to ptr addrspace(1)
  store i16 %28, ptr %12, !tbaa !2
  %30 = getelementptr inbounds i8, ptr %12, i16 2
  store i16 %28, ptr %30, !tbaa !2
  %31 = getelementptr inbounds i8, ptr %12, i16 4
  store ptr addrspace(1) %29, ptr %31, !tbaa !2
  %32 = addrspacecast ptr %12 to ptr addrspace(1)
  %33 = call cc 1005 addrspace(1) i16 @describe(ptr addrspace(1) %32)
  %34 = addrspacecast ptr %16 to ptr addrspace(1)
  store i16 1, ptr %11, !tbaa !2
  %35 = getelementptr inbounds i8, ptr %11, i16 2
  store i16 1, ptr %35, !tbaa !2
  %36 = getelementptr inbounds i8, ptr %11, i16 4
  store ptr addrspace(1) %34, ptr %36, !tbaa !2
  %37 = addrspacecast ptr %11 to ptr addrspace(1)
  %38 = call cc 1005 addrspace(1) i16 @describe(ptr addrspace(1) %37)
  %39 = load ptr, ptr %13, !tbaa !2
  %40 = getelementptr i8, ptr %39, i16 -4
  %41 = load i16, ptr %40
  %42 = addrspacecast ptr %39 to ptr addrspace(1)
  store i16 %41, ptr %10, !tbaa !2
  %43 = getelementptr inbounds i8, ptr %10, i16 2
  store i16 %41, ptr %43, !tbaa !2
  %44 = getelementptr inbounds i8, ptr %10, i16 4
  store ptr addrspace(1) %42, ptr %44, !tbaa !2
  %45 = addrspacecast ptr %10 to ptr addrspace(1)
  %46 = call cc 1005 addrspace(1) i16 @describe(ptr addrspace(1) %45)
  %47 = load ptr, ptr %13, !tbaa !2
  %48 = getelementptr i8, ptr %47, i16 -4
  %49 = load i16, ptr %48
  %50 = addrspacecast ptr %47 to ptr addrspace(1)
  store i16 %49, ptr %9, !tbaa !2
  %51 = getelementptr inbounds i8, ptr %9, i16 2
  store i16 %49, ptr %51, !tbaa !2
  %52 = getelementptr inbounds i8, ptr %9, i16 4
  store ptr addrspace(1) %50, ptr %52, !tbaa !2
  %53 = addrspacecast ptr %9 to ptr addrspace(1)
  %54 = call cc 1005 addrspace(1) i16 @head(ptr addrspace(1) %53)
  store i16 %54, ptr %8, !tbaa !2
  %55 = load ptr, ptr %17, !tbaa !2
  %56 = getelementptr i8, ptr %55, i16 -4
  %57 = load i16, ptr %56
  %58 = addrspacecast ptr %55 to ptr addrspace(1)
  store i16 %57, ptr %7, !tbaa !2
  %59 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 %57, ptr %59, !tbaa !2
  %60 = getelementptr inbounds i8, ptr %7, i16 4
  store ptr addrspace(1) %58, ptr %60, !tbaa !2
  %61 = addrspacecast ptr %7 to ptr addrspace(1)
  %62 = call cc 1005 addrspace(1) i16 @head(ptr addrspace(1) %61)
  store i16 %62, ptr %6, !tbaa !2
  %63 = load i16, ptr %8, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI2(i16 %63)
  %64 = getelementptr i8, ptr @$str6, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %64)
  %65 = load i16, ptr %6, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI2(i16 %65)
  call cc 1005 addrspace(1) void @N$PN()
  %66 = getelementptr i8, ptr @$str5, i16 6
  %67 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %66, i16 2, i16 4)
  %68 = getelementptr inbounds i8, ptr %67, i16 0
  store i16 1, ptr %68
  %69 = getelementptr i8, ptr %68, i16 2
  store i16 2, ptr %69
  %70 = getelementptr inbounds i8, ptr %67, i16 4
  store i16 3, ptr %70
  %71 = getelementptr i8, ptr %70, i16 2
  store i16 4, ptr %71
  store ptr %67, ptr %5, !tbaa !2
  %72 = load ptr, ptr %5, !tbaa !2
  %73 = getelementptr i8, ptr %72, i16 -4
  %74 = load i16, ptr %73
  %75 = addrspacecast ptr %72 to ptr addrspace(1)
  store i16 %74, ptr %4, !tbaa !2
  %76 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 %74, ptr %76, !tbaa !2
  %77 = getelementptr inbounds i8, ptr %4, i16 4
  store ptr addrspace(1) %75, ptr %77, !tbaa !2
  %78 = addrspacecast ptr %4 to ptr addrspace(1)
  %79 = getelementptr i8, ptr addrspace(1) %78, i16 4
  %80 = load ptr addrspace(1), ptr addrspace(1) %79, !tbaa !2
  %81 = load i16, ptr addrspace(1) %78, !tbaa !2
  %82 = icmp sge i16 %81, 1
  br i1 %82, label %b4, label %b3

b2:
  %83 = load ptr, ptr %13, !tbaa !2
  %84 = getelementptr i8, ptr %83, i16 -4
  %85 = load i16, ptr %84
  %86 = addrspacecast ptr %83 to ptr addrspace(1)
  store i16 %85, ptr %3, !tbaa !2
  %87 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 %85, ptr %87, !tbaa !2
  %88 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %86, ptr %88, !tbaa !2
  %89 = addrspacecast ptr %3 to ptr addrspace(1)
  %90 = getelementptr i8, ptr addrspace(1) %89, i16 4
  %91 = load ptr addrspace(1), ptr addrspace(1) %90, !tbaa !2
  %92 = load i16, ptr addrspace(1) %89, !tbaa !2
  %93 = icmp sge i16 %92, 2
  br i1 %93, label %b8, label %b7

b3:
  %94 = getelementptr i8, ptr @$str9, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %94)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b2

b4:
  %95 = getelementptr inbounds i8, ptr addrspace(1) %80, i16 0
  %96 = load i16, ptr addrspace(1) %95
  %97 = getelementptr i8, ptr addrspace(1) %95, i16 2
  %98 = load i16, ptr addrspace(1) %97
  %99 = getelementptr inbounds i8, ptr addrspace(1) %80, i16 0
  %100 = load i16, ptr addrspace(1) %99
  %101 = getelementptr i8, ptr addrspace(1) %99, i16 2
  %102 = load i16, ptr addrspace(1) %101
  %103 = getelementptr i8, ptr addrspace(1) %99, i16 2
  %104 = getelementptr i8, ptr @$str7, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %104)
  %105 = load i16, ptr addrspace(1) %99
  call cc 1005 addrspace(1) void @N$PI2(i16 %105)
  %106 = getelementptr i8, ptr @$str8, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %106)
  %107 = load i16, ptr addrspace(1) %103
  call cc 1005 addrspace(1) void @N$PI2(i16 %107)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b2

b6:
  %108 = load ptr, ptr %17, !tbaa !2
  %109 = getelementptr i8, ptr %108, i16 -4
  %110 = load i16, ptr %109
  %111 = icmp eq i16 %110, 0
  %112 = zext i1 %111 to i8
  store i8 %112, ptr %1, !tbaa !2
  %113 = load ptr, ptr %13, !tbaa !2
  %114 = getelementptr i8, ptr %113, i16 -4
  %115 = load i16, ptr %114
  %116 = icmp eq i16 %115, 0
  %117 = zext i1 %116 to i8
  store i8 %117, ptr %0, !tbaa !2
  %118 = load i8, ptr %1, !tbaa !2, !range !5
  call cc 1005 addrspace(1) void @N$PB(i8 %118)
  %119 = getelementptr i8, ptr @$str6, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %119)
  %120 = load i8, ptr %0, !tbaa !2, !range !6
  call cc 1005 addrspace(1) void @N$PB(i8 %120)
  call cc 1005 addrspace(1) void @N$PN()
  %121 = load ptr, ptr %5, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %121)
  %122 = load ptr, ptr %13, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %122)
  %123 = load ptr, ptr %17, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %123)
  ret i16 0

b7:
  %124 = getelementptr i8, ptr @$str11, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %124)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b6

b8:
  %125 = getelementptr inbounds i8, ptr addrspace(1) %91, i16 0
  %126 = load i16, ptr addrspace(1) %125
  %127 = getelementptr inbounds i8, ptr addrspace(1) %91, i16 2
  %128 = load i16, ptr addrspace(1) %127
  %129 = icmp eq i16 %126, 1
  br i1 %129, label %b9, label %b7

b9:
  %130 = icmp eq i16 %128, 2
  br i1 %130, label %b10, label %b7

b10:
  %131 = getelementptr inbounds i8, ptr addrspace(1) %91, i16 0
  %132 = load i16, ptr addrspace(1) %131
  %133 = getelementptr inbounds i8, ptr addrspace(1) %91, i16 2
  %134 = load i16, ptr addrspace(1) %133
  %135 = getelementptr inbounds i8, ptr addrspace(1) %91, i16 4
  %136 = sub i16 %92, 2
  store i16 %136, ptr %2, !tbaa !2
  %137 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 %136, ptr %137, !tbaa !2
  %138 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %135, ptr %138, !tbaa !2
  %139 = addrspacecast ptr %2 to ptr addrspace(1)
  %140 = getelementptr i8, ptr @$str10, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %140)
  %141 = load i16, ptr addrspace(1) %139
  call cc 1005 addrspace(1) void @N$PU2(i16 %141)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b6
}

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PU2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare cc 1005 void @N$PB(i8) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i8 0, i8 2}
!6 = !{i8 0, i8 2}
