target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [10 x i8] c"\08\00\03\00\03\00ada\00"
@$str2 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str3 = internal constant [9 x i8] c"\08\00\02\00\02\00: \00"
@$str4 = internal constant [21 x i8] c"\08\00\0E\00\0E\00 marks, first \00"
@$str5 = internal constant [14 x i8] c"\08\00\07\00\07\00recent \00"
@$str6 = internal constant [17 x i8] c"\08\00\0A\00\0A\00, average \00"
@$str7 = internal constant [12 x i8] c"\08\00\05\00\05\00best \00"
@$str8 = internal constant [21 x i8] c"\08\00\0E\00\0E\00week 2 day 2: \00"
@$str9 = internal constant [20 x i8] c"\08\00\0D\00\0D\00, week 1 has \00"

define internal void @curve(ptr addrspace(1) %0, i16 %1) addrspace(1) {
b1:
  %2 = alloca i16
  %3 = alloca i16
  store i16 0, ptr %2
  store i16 0, ptr %3
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
  %10 = sext i1 %9 to i8
  %11 = icmp ne i8 %10, 0
  br i1 %11, label %b3, label %b5

b3:
  %12 = load ptr, ptr addrspace(1) %0
  %13 = load i16, ptr %3, !tbaa !2
  %14 = getelementptr i8, ptr %12, i16 -4
  %15 = load i16, ptr %14
  %16 = icmp ult i16 %13, %15
  %17 = sext i1 %16 to i8
  %18 = icmp ne i8 %17, 0
  br i1 %18, label %b6, label %b7

b4:
  %19 = load i16, ptr %3, !tbaa !2
  %20 = add i16 %19, 1
  store i16 %20, ptr %3, !tbaa !2
  br label %b2

b5:
  %21 = load ptr, ptr addrspace(1) %0
  %22 = getelementptr i8, ptr %21, i16 -4
  %23 = load i16, ptr %22
  %24 = call addrspace(1) ptr @N$BGRW(ptr %21, i16 1, i16 2)
  store ptr %24, ptr addrspace(1) %0
  %25 = mul i16 %23, 2
  %26 = getelementptr i8, ptr %24, i16 %25
  store i16 %1, ptr %26
  ret void

b6:
  %27 = mul i16 %13, 2
  %28 = getelementptr i8, ptr %12, i16 %27
  %29 = load i16, ptr %28
  %30 = add i16 %29, %1
  store i16 %30, ptr %28
  br label %b4

b7:
  call addrspace(1) void @N$EBND()
  unreachable
}

define internal i16 @average(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) {
b1:
  %1 = alloca i16
  %2 = alloca i16
  store i16 0, ptr %1
  store i16 0, ptr %2
  store i16 0, ptr %2, !tbaa !2
  %3 = load i16, ptr addrspace(1) %0
  store i16 0, ptr %1, !tbaa !2
  br label %b2

b2:
  %4 = load i16, ptr %1, !tbaa !2
  %5 = icmp ult i16 %4, %3
  %6 = sext i1 %5 to i8
  %7 = icmp ne i8 %6, 0
  br i1 %7, label %b3, label %b5

b3:
  %8 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %9 = load ptr addrspace(1), ptr addrspace(1) %8
  %10 = mul i16 %4, 2
  %11 = getelementptr i8, ptr addrspace(1) %9, i16 %10
  %12 = load i16, ptr %2, !tbaa !2
  %13 = load i16, ptr addrspace(1) %11
  %14 = add i16 %12, %13
  store i16 %14, ptr %2, !tbaa !2
  br label %b4

b4:
  %15 = load i16, ptr %1, !tbaa !2
  %16 = add i16 %15, 1
  store i16 %16, ptr %1, !tbaa !2
  br label %b2

b5:
  %17 = load i16, ptr %2, !tbaa !2
  %18 = load i16, ptr addrspace(1) %0
  %19 = sdiv i16 %17, %18
  %20 = srem i16 %17, %18
  %21 = icmp ne i16 %20, 0
  %22 = sext i1 %21 to i8
  %23 = xor i16 %20, %18
  %24 = icmp slt i16 %23, 0
  %25 = sext i1 %24 to i8
  %26 = and i8 %22, %25
  %27 = sext i8 %26 to i16
  %28 = and i16 %27, 1
  %29 = sub i16 %19, %28
  ret i16 %29
}

define internal i16 @main() addrspace(1) {
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
  store i16 0, ptr %0
  store ptr null, ptr %1
  store i16 0, ptr %2
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 8, i1 false)
  store i16 0, ptr %4
  store i16 0, ptr %5
  store i16 0, ptr %6
  call void @llvm.memset.p0.i16(ptr %7, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %8, i8 0, i16 4, i1 false)
  %9 = getelementptr i8, ptr @$str1, i16 6
  %10 = getelementptr i8, ptr @$str2, i16 6
  store ptr %9, ptr %8, !tbaa !2
  %11 = getelementptr inbounds i8, ptr %8, i16 2
  store ptr %10, ptr %11, !tbaa !2
  %12 = getelementptr inbounds i8, ptr %8, i16 2
  %13 = load ptr, ptr %12, !tbaa !2
  %14 = getelementptr i8, ptr %13, i16 -4
  %15 = load i16, ptr %14
  %16 = call addrspace(1) ptr @N$BGRW(ptr %13, i16 1, i16 2)
  %17 = getelementptr inbounds i8, ptr %8, i16 2
  store ptr %16, ptr %17, !tbaa !2
  %18 = mul i16 %15, 2
  %19 = getelementptr i8, ptr %16, i16 %18
  store i16 71, ptr %19
  %20 = getelementptr inbounds i8, ptr %8, i16 2
  %21 = load ptr, ptr %20, !tbaa !2
  %22 = getelementptr i8, ptr %21, i16 -4
  %23 = load i16, ptr %22
  %24 = call addrspace(1) ptr @N$BGRW(ptr %21, i16 1, i16 2)
  %25 = getelementptr inbounds i8, ptr %8, i16 2
  store ptr %24, ptr %25, !tbaa !2
  %26 = mul i16 %23, 2
  %27 = getelementptr i8, ptr %24, i16 %26
  store i16 64, ptr %27
  %28 = getelementptr inbounds i8, ptr %8, i16 2
  %29 = load ptr, ptr %28, !tbaa !2
  %30 = getelementptr i8, ptr %29, i16 -4
  %31 = load i16, ptr %30
  %32 = call addrspace(1) ptr @N$BGRW(ptr %29, i16 1, i16 2)
  %33 = getelementptr inbounds i8, ptr %8, i16 2
  store ptr %32, ptr %33, !tbaa !2
  %34 = mul i16 %31, 2
  %35 = getelementptr i8, ptr %32, i16 %34
  store i16 80, ptr %35
  %36 = load ptr, ptr %8, !tbaa !2
  call addrspace(1) void @N$PS(ptr %36)
  %37 = getelementptr i8, ptr @$str3, i16 6
  call addrspace(1) void @N$PS(ptr %37)
  %38 = getelementptr inbounds i8, ptr %8, i16 2
  %39 = load ptr, ptr %38, !tbaa !2
  %40 = getelementptr i8, ptr %39, i16 -4
  %41 = load i16, ptr %40
  call addrspace(1) void @N$PU2(i16 %41)
  %42 = getelementptr i8, ptr @$str4, i16 6
  call addrspace(1) void @N$PS(ptr %42)
  %43 = getelementptr inbounds i8, ptr %8, i16 2
  %44 = load ptr, ptr %43, !tbaa !2
  %45 = getelementptr i8, ptr %44, i16 -4
  %46 = load i16, ptr %45
  %47 = icmp ult i16 0, %46
  %48 = sext i1 %47 to i8
  %49 = icmp ne i8 %48, 0
  br i1 %49, label %b2, label %b3

b2:
  %50 = getelementptr i8, ptr %44, i16 0
  %51 = load i16, ptr %50
  call addrspace(1) void @N$PI2(i16 %51)
  call addrspace(1) void @N$PN()
  %52 = getelementptr inbounds i8, ptr %8, i16 2
  %53 = addrspacecast ptr %52 to ptr addrspace(1)
  call addrspace(1) void @curve(ptr addrspace(1) %53, i16 5)
  %54 = getelementptr inbounds i8, ptr %8, i16 2
  %55 = load ptr, ptr %54, !tbaa !2
  %56 = getelementptr i8, ptr %55, i16 -4
  %57 = load i16, ptr %56
  %58 = addrspacecast ptr %55 to ptr addrspace(1)
  %59 = icmp ule i16 1, %57
  %60 = sext i1 %59 to i8
  %61 = icmp ne i8 %60, 0
  br i1 %61, label %b4, label %b5

b3:
  call addrspace(1) void @N$EBND()
  unreachable

b4:
  %62 = getelementptr i8, ptr addrspace(1) %58, i16 2
  %63 = sub i16 %57, 1
  store i16 %63, ptr %7, !tbaa !2
  %64 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 %63, ptr %64, !tbaa !2
  %65 = getelementptr inbounds i8, ptr %7, i16 4
  store ptr addrspace(1) %62, ptr %65, !tbaa !2
  %66 = addrspacecast ptr %7 to ptr addrspace(1)
  %67 = load i16, ptr addrspace(1) %66
  store i16 %67, ptr %6, !tbaa !2
  %68 = call addrspace(1) i16 @average(ptr addrspace(1) %66)
  store i16 %68, ptr %5, !tbaa !2
  %69 = getelementptr i8, ptr @$str5, i16 6
  call addrspace(1) void @N$PS(ptr %69)
  %70 = load i16, ptr %6, !tbaa !2
  call addrspace(1) void @N$PU2(i16 %70)
  %71 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @N$PS(ptr %71)
  %72 = load i16, ptr %5, !tbaa !2
  call addrspace(1) void @N$PI2(i16 %72)
  call addrspace(1) void @N$PN()
  store i16 0, ptr %4, !tbaa !2
  %73 = getelementptr inbounds i8, ptr %8, i16 2
  %74 = load ptr, ptr %73, !tbaa !2
  %75 = getelementptr i8, ptr %74, i16 -4
  %76 = load i16, ptr %75
  %77 = addrspacecast ptr %74 to ptr addrspace(1)
  store i16 %76, ptr %3, !tbaa !2
  %78 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 %76, ptr %78, !tbaa !2
  %79 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %77, ptr %79, !tbaa !2
  %80 = addrspacecast ptr %3 to ptr addrspace(1)
  %81 = load i16, ptr addrspace(1) %80
  store i16 0, ptr %2, !tbaa !2
  br label %b6

b5:
  call addrspace(1) void @N$EBND()
  unreachable

b6:
  %82 = load i16, ptr %2, !tbaa !2
  %83 = icmp ult i16 %82, %81
  %84 = sext i1 %83 to i8
  %85 = icmp ne i8 %84, 0
  br i1 %85, label %b7, label %b9

b7:
  %86 = getelementptr i8, ptr addrspace(1) %80, i16 4
  %87 = load ptr addrspace(1), ptr addrspace(1) %86, !tbaa !2
  %88 = mul i16 %82, 2
  %89 = getelementptr i8, ptr addrspace(1) %87, i16 %88
  %90 = load i16, ptr addrspace(1) %89
  %91 = load i16, ptr %4, !tbaa !2
  %92 = icmp sgt i16 %90, %91
  %93 = sext i1 %92 to i8
  %94 = icmp ne i8 %93, 0
  br i1 %94, label %b10, label %b11

b8:
  %95 = load i16, ptr %2, !tbaa !2
  %96 = add i16 %95, 1
  store i16 %96, ptr %2, !tbaa !2
  br label %b6

b9:
  %97 = getelementptr i8, ptr @$str7, i16 6
  call addrspace(1) void @N$PS(ptr %97)
  %98 = load i16, ptr %4, !tbaa !2
  call addrspace(1) void @N$PI2(i16 %98)
  call addrspace(1) void @N$PN()
  %99 = getelementptr i8, ptr @$str2, i16 6
  store ptr %99, ptr %1, !tbaa !2
  %100 = load ptr, ptr %1, !tbaa !2
  %101 = getelementptr i8, ptr %100, i16 -4
  %102 = load i16, ptr %101
  %103 = call addrspace(1) ptr @N$BGRW(ptr %100, i16 1, i16 2)
  store ptr %103, ptr %1, !tbaa !2
  %104 = mul i16 %102, 2
  %105 = getelementptr i8, ptr %103, i16 %104
  %106 = getelementptr i8, ptr @$str2, i16 6
  %107 = call addrspace(1) ptr @N$BGRW(ptr %106, i16 3, i16 2)
  %108 = getelementptr i8, ptr %107, i16 0
  store i16 1, ptr %108
  %109 = getelementptr i8, ptr %107, i16 2
  store i16 2, ptr %109
  %110 = getelementptr i8, ptr %107, i16 4
  store i16 3, ptr %110
  store ptr %107, ptr %105
  %111 = load ptr, ptr %1, !tbaa !2
  %112 = getelementptr i8, ptr %111, i16 -4
  %113 = load i16, ptr %112
  %114 = call addrspace(1) ptr @N$BGRW(ptr %111, i16 1, i16 2)
  store ptr %114, ptr %1, !tbaa !2
  %115 = mul i16 %113, 2
  %116 = getelementptr i8, ptr %114, i16 %115
  %117 = getelementptr i8, ptr @$str2, i16 6
  %118 = call addrspace(1) ptr @N$BGRW(ptr %117, i16 2, i16 2)
  %119 = getelementptr i8, ptr %118, i16 0
  store i16 4, ptr %119
  %120 = getelementptr i8, ptr %118, i16 2
  store i16 5, ptr %120
  store ptr %118, ptr %116
  %121 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @N$PS(ptr %121)
  %122 = load ptr, ptr %1, !tbaa !2
  %123 = getelementptr i8, ptr %122, i16 -4
  %124 = load i16, ptr %123
  %125 = icmp ult i16 1, %124
  %126 = sext i1 %125 to i8
  %127 = icmp ne i8 %126, 0
  br i1 %127, label %b13, label %b14

b10:
  %128 = load i16, ptr addrspace(1) %89
  store i16 %128, ptr %4, !tbaa !2
  br label %b12

b11:
  br label %b12

b12:
  br label %b8

b13:
  %129 = getelementptr i8, ptr %122, i16 2
  %130 = load ptr, ptr %129
  %131 = getelementptr i8, ptr %130, i16 -4
  %132 = load i16, ptr %131
  %133 = icmp ult i16 1, %132
  %134 = sext i1 %133 to i8
  %135 = icmp ne i8 %134, 0
  br i1 %135, label %b15, label %b16

b14:
  call addrspace(1) void @N$EBND()
  unreachable

b15:
  %136 = getelementptr i8, ptr %130, i16 2
  %137 = load i16, ptr %136
  call addrspace(1) void @N$PI2(i16 %137)
  %138 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %138)
  %139 = load ptr, ptr %1, !tbaa !2
  %140 = getelementptr i8, ptr %139, i16 -4
  %141 = load i16, ptr %140
  %142 = icmp ult i16 0, %141
  %143 = sext i1 %142 to i8
  %144 = icmp ne i8 %143, 0
  br i1 %144, label %b17, label %b18

b16:
  call addrspace(1) void @N$EBND()
  unreachable

b17:
  %145 = getelementptr i8, ptr %139, i16 0
  %146 = load ptr, ptr %145
  %147 = getelementptr i8, ptr %146, i16 -4
  %148 = load i16, ptr %147
  call addrspace(1) void @N$PU2(i16 %148)
  call addrspace(1) void @N$PN()
  %149 = load ptr, ptr %1, !tbaa !2
  %150 = icmp ne ptr %149, null
  %151 = sext i1 %150 to i8
  %152 = icmp ne i8 %151, 0
  br i1 %152, label %b20, label %b19

b18:
  call addrspace(1) void @N$EBND()
  unreachable

b19:
  call addrspace(1) void @N$BDRP(ptr %149)
  %153 = getelementptr inbounds i8, ptr %8, i16 2
  %154 = load ptr, ptr %153, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %154)
  %155 = load ptr, ptr %8, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %155)
  ret i16 0

b20:
  %156 = getelementptr i8, ptr %149, i16 -4
  %157 = load i16, ptr %156
  store i16 0, ptr %0, !tbaa !2
  br label %b21

b21:
  %158 = load i16, ptr %0, !tbaa !2
  %159 = icmp ult i16 %158, %157
  %160 = sext i1 %159 to i8
  %161 = icmp ne i8 %160, 0
  br i1 %161, label %b23, label %b22

b22:
  br label %b19

b23:
  %162 = mul i16 %158, 2
  %163 = getelementptr i8, ptr %149, i16 %162
  %164 = load ptr, ptr %163
  call addrspace(1) void @N$BDRP(ptr %164)
  %165 = add i16 %158, 1
  store i16 %165, ptr %0, !tbaa !2
  br label %b21
}

declare ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare void @N$EBND() addrspace(1)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PU2(i16) addrspace(1)

declare void @N$PI2(i16) addrspace(1)

declare void @N$PN() addrspace(1)

declare void @N$BDRP(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
