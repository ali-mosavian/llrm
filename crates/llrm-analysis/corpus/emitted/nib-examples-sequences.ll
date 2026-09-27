target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

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

define internal i16 @describe(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) {
b1:
  %1 = alloca [8 x i8]
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 8, i1 false)
  %2 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %3 = load ptr addrspace(1), ptr addrspace(1) %2
  %4 = load i16, ptr addrspace(1) %0
  %5 = icmp eq i16 %4, 0
  %6 = sext i1 %5 to i8
  %7 = icmp ne i8 %6, 0
  br i1 %7, label %b4, label %b3

b2:
  ret i16 0

b3:
  %8 = icmp eq i16 %4, 1
  %9 = sext i1 %8 to i8
  %10 = icmp ne i8 %9, 0
  br i1 %10, label %b6, label %b5

b4:
  %11 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %11)
  call addrspace(1) void @N$PN()
  br label %b2

b5:
  %12 = getelementptr i8, ptr addrspace(1) %3, i16 0
  %13 = load i16, ptr addrspace(1) %12
  %14 = sub i16 %4, 1
  %15 = mul i16 %14, 2
  %16 = getelementptr i8, ptr addrspace(1) %3, i16 %15
  %17 = load i16, ptr addrspace(1) %16
  %18 = getelementptr i8, ptr addrspace(1) %3, i16 2
  %19 = sub i16 %4, 2
  store i16 %19, ptr %1, !tbaa !2
  %20 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 %19, ptr %20, !tbaa !2
  %21 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %18, ptr %21, !tbaa !2
  %22 = addrspacecast ptr %1 to ptr addrspace(1)
  %23 = load i16, ptr addrspace(1) %12
  call addrspace(1) void @N$PI2(i16 %23)
  %24 = getelementptr i8, ptr @$str3, i16 6
  call addrspace(1) void @N$PS(ptr %24)
  %25 = load i16, ptr addrspace(1) %16
  call addrspace(1) void @N$PI2(i16 %25)
  %26 = getelementptr i8, ptr @$str4, i16 6
  call addrspace(1) void @N$PS(ptr %26)
  %27 = load i16, ptr addrspace(1) %22
  call addrspace(1) void @N$PU2(i16 %27)
  call addrspace(1) void @N$PN()
  br label %b2

b6:
  %28 = getelementptr i8, ptr addrspace(1) %3, i16 0
  %29 = load i16, ptr addrspace(1) %28
  %30 = getelementptr i8, ptr addrspace(1) %3, i16 0
  %31 = load i16, ptr addrspace(1) %30
  %32 = getelementptr i8, ptr @$str2, i16 6
  call addrspace(1) void @N$PS(ptr %32)
  %33 = load i16, ptr addrspace(1) %30
  call addrspace(1) void @N$PI2(i16 %33)
  call addrspace(1) void @N$PN()
  br label %b2
}

define internal i16 @head(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) {
b1:
  %1 = alloca [8 x i8]
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 8, i1 false)
  %2 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %3 = load ptr addrspace(1), ptr addrspace(1) %2
  %4 = load i16, ptr addrspace(1) %0
  %5 = icmp sge i16 %4, 1
  %6 = sext i1 %5 to i8
  %7 = icmp ne i8 %6, 0
  br i1 %7, label %b4, label %b2

b2:
  ret i16 -1

b3:
  %8 = getelementptr i8, ptr addrspace(1) %3, i16 0
  %9 = load i16, ptr addrspace(1) %8
  %10 = getelementptr i8, ptr addrspace(1) %3, i16 2
  %11 = sub i16 %4, 1
  store i16 %11, ptr %1, !tbaa !2
  %12 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 %11, ptr %12, !tbaa !2
  %13 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %10, ptr %13, !tbaa !2
  %14 = addrspacecast ptr %1 to ptr addrspace(1)
  %15 = load i16, ptr addrspace(1) %8
  %16 = load i16, ptr addrspace(1) %14
  %17 = add i16 %15, %16
  ret i16 %17

b4:
  %18 = getelementptr i8, ptr addrspace(1) %3, i16 0
  %19 = load i16, ptr addrspace(1) %18
  br label %b3
}

define internal i16 @main() addrspace(1) {
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
  store i8 0, ptr %0
  store i8 0, ptr %1
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 8, i1 false)
  store ptr null, ptr %5
  store i16 0, ptr %6
  call void @llvm.memset.p0.i16(ptr %7, i8 0, i16 8, i1 false)
  store i16 0, ptr %8
  call void @llvm.memset.p0.i16(ptr %9, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %10, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %11, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %12, i8 0, i16 8, i1 false)
  store ptr null, ptr %13
  store i16 0, ptr %14
  store i16 0, ptr %15
  call void @llvm.memset.p0.i16(ptr %16, i8 0, i16 2, i1 false)
  store ptr null, ptr %17
  %18 = getelementptr i8, ptr @$str5, i16 6
  store ptr %18, ptr %17, !tbaa !2
  store i16 1, ptr %14, !tbaa !2
  store i16 1, ptr %15, !tbaa !2
  %19 = sub i16 0, 0
  %20 = getelementptr inbounds i16, ptr %16, i16 %19
  store i16 7, ptr %20, !tbaa !2
  %21 = getelementptr i8, ptr @$str5, i16 6
  %22 = call addrspace(1) ptr @N$BGRW(ptr %21, i16 4, i16 2)
  %23 = getelementptr i8, ptr %22, i16 0
  store i16 1, ptr %23
  %24 = getelementptr i8, ptr %22, i16 2
  store i16 2, ptr %24
  %25 = getelementptr i8, ptr %22, i16 4
  store i16 3, ptr %25
  %26 = getelementptr i8, ptr %22, i16 6
  store i16 4, ptr %26
  store ptr %22, ptr %13, !tbaa !2
  %27 = load ptr, ptr %17, !tbaa !2
  %28 = getelementptr i8, ptr %27, i16 -4
  %29 = load i16, ptr %28
  %30 = addrspacecast ptr %27 to ptr addrspace(1)
  store i16 %29, ptr %12, !tbaa !2
  %31 = getelementptr inbounds i8, ptr %12, i16 2
  store i16 %29, ptr %31, !tbaa !2
  %32 = getelementptr inbounds i8, ptr %12, i16 4
  store ptr addrspace(1) %30, ptr %32, !tbaa !2
  %33 = addrspacecast ptr %12 to ptr addrspace(1)
  %34 = call addrspace(1) i16 @describe(ptr addrspace(1) %33)
  %35 = addrspacecast ptr %16 to ptr addrspace(1)
  store i16 1, ptr %11, !tbaa !2
  %36 = getelementptr inbounds i8, ptr %11, i16 2
  store i16 1, ptr %36, !tbaa !2
  %37 = getelementptr inbounds i8, ptr %11, i16 4
  store ptr addrspace(1) %35, ptr %37, !tbaa !2
  %38 = addrspacecast ptr %11 to ptr addrspace(1)
  %39 = call addrspace(1) i16 @describe(ptr addrspace(1) %38)
  %40 = load ptr, ptr %13, !tbaa !2
  %41 = getelementptr i8, ptr %40, i16 -4
  %42 = load i16, ptr %41
  %43 = addrspacecast ptr %40 to ptr addrspace(1)
  store i16 %42, ptr %10, !tbaa !2
  %44 = getelementptr inbounds i8, ptr %10, i16 2
  store i16 %42, ptr %44, !tbaa !2
  %45 = getelementptr inbounds i8, ptr %10, i16 4
  store ptr addrspace(1) %43, ptr %45, !tbaa !2
  %46 = addrspacecast ptr %10 to ptr addrspace(1)
  %47 = call addrspace(1) i16 @describe(ptr addrspace(1) %46)
  %48 = load ptr, ptr %13, !tbaa !2
  %49 = getelementptr i8, ptr %48, i16 -4
  %50 = load i16, ptr %49
  %51 = addrspacecast ptr %48 to ptr addrspace(1)
  store i16 %50, ptr %9, !tbaa !2
  %52 = getelementptr inbounds i8, ptr %9, i16 2
  store i16 %50, ptr %52, !tbaa !2
  %53 = getelementptr inbounds i8, ptr %9, i16 4
  store ptr addrspace(1) %51, ptr %53, !tbaa !2
  %54 = addrspacecast ptr %9 to ptr addrspace(1)
  %55 = call addrspace(1) i16 @head(ptr addrspace(1) %54)
  store i16 %55, ptr %8, !tbaa !2
  %56 = load ptr, ptr %17, !tbaa !2
  %57 = getelementptr i8, ptr %56, i16 -4
  %58 = load i16, ptr %57
  %59 = addrspacecast ptr %56 to ptr addrspace(1)
  store i16 %58, ptr %7, !tbaa !2
  %60 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 %58, ptr %60, !tbaa !2
  %61 = getelementptr inbounds i8, ptr %7, i16 4
  store ptr addrspace(1) %59, ptr %61, !tbaa !2
  %62 = addrspacecast ptr %7 to ptr addrspace(1)
  %63 = call addrspace(1) i16 @head(ptr addrspace(1) %62)
  store i16 %63, ptr %6, !tbaa !2
  %64 = load i16, ptr %8, !tbaa !2
  call addrspace(1) void @N$PI2(i16 %64)
  %65 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @N$PS(ptr %65)
  %66 = load i16, ptr %6, !tbaa !2
  call addrspace(1) void @N$PI2(i16 %66)
  call addrspace(1) void @N$PN()
  %67 = getelementptr i8, ptr @$str5, i16 6
  %68 = call addrspace(1) ptr @N$BGRW(ptr %67, i16 2, i16 4)
  %69 = getelementptr i8, ptr %68, i16 0
  store i16 1, ptr %69
  %70 = getelementptr i8, ptr %69, i16 2
  store i16 2, ptr %70
  %71 = getelementptr i8, ptr %68, i16 4
  store i16 3, ptr %71
  %72 = getelementptr i8, ptr %71, i16 2
  store i16 4, ptr %72
  store ptr %68, ptr %5, !tbaa !2
  %73 = load ptr, ptr %5, !tbaa !2
  %74 = getelementptr i8, ptr %73, i16 -4
  %75 = load i16, ptr %74
  %76 = addrspacecast ptr %73 to ptr addrspace(1)
  store i16 %75, ptr %4, !tbaa !2
  %77 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 %75, ptr %77, !tbaa !2
  %78 = getelementptr inbounds i8, ptr %4, i16 4
  store ptr addrspace(1) %76, ptr %78, !tbaa !2
  %79 = addrspacecast ptr %4 to ptr addrspace(1)
  %80 = getelementptr i8, ptr addrspace(1) %79, i16 4
  %81 = load ptr addrspace(1), ptr addrspace(1) %80, !tbaa !2
  %82 = load i16, ptr addrspace(1) %79, !tbaa !2
  %83 = icmp sge i16 %82, 1
  %84 = sext i1 %83 to i8
  %85 = icmp ne i8 %84, 0
  br i1 %85, label %b4, label %b3

b2:
  %86 = load ptr, ptr %13, !tbaa !2
  %87 = getelementptr i8, ptr %86, i16 -4
  %88 = load i16, ptr %87
  %89 = addrspacecast ptr %86 to ptr addrspace(1)
  store i16 %88, ptr %3, !tbaa !2
  %90 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 %88, ptr %90, !tbaa !2
  %91 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %89, ptr %91, !tbaa !2
  %92 = addrspacecast ptr %3 to ptr addrspace(1)
  %93 = getelementptr i8, ptr addrspace(1) %92, i16 4
  %94 = load ptr addrspace(1), ptr addrspace(1) %93, !tbaa !2
  %95 = load i16, ptr addrspace(1) %92, !tbaa !2
  %96 = icmp sge i16 %95, 2
  %97 = sext i1 %96 to i8
  %98 = icmp ne i8 %97, 0
  br i1 %98, label %b8, label %b7

b3:
  %99 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %99)
  call addrspace(1) void @N$PN()
  br label %b2

b4:
  %100 = getelementptr i8, ptr addrspace(1) %81, i16 0
  %101 = load i16, ptr addrspace(1) %100
  %102 = getelementptr i8, ptr addrspace(1) %100, i16 2
  %103 = load i16, ptr addrspace(1) %102
  %104 = getelementptr i8, ptr addrspace(1) %81, i16 0
  %105 = load i16, ptr addrspace(1) %104
  %106 = getelementptr i8, ptr addrspace(1) %104, i16 2
  %107 = load i16, ptr addrspace(1) %106
  %108 = getelementptr i8, ptr addrspace(1) %104, i16 2
  %109 = getelementptr i8, ptr @$str7, i16 6
  call addrspace(1) void @N$PS(ptr %109)
  %110 = load i16, ptr addrspace(1) %104
  call addrspace(1) void @N$PI2(i16 %110)
  %111 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @N$PS(ptr %111)
  %112 = load i16, ptr addrspace(1) %108
  call addrspace(1) void @N$PI2(i16 %112)
  call addrspace(1) void @N$PN()
  br label %b2

b6:
  %113 = load ptr, ptr %17, !tbaa !2
  %114 = getelementptr i8, ptr %113, i16 -4
  %115 = load i16, ptr %114
  %116 = icmp eq i16 %115, 0
  %117 = sext i1 %116 to i8
  store i8 %117, ptr %1, !tbaa !2
  %118 = load ptr, ptr %13, !tbaa !2
  %119 = getelementptr i8, ptr %118, i16 -4
  %120 = load i16, ptr %119
  %121 = icmp eq i16 %120, 0
  %122 = sext i1 %121 to i8
  store i8 %122, ptr %0, !tbaa !2
  %123 = load i8, ptr %1, !tbaa !2
  call addrspace(1) void @N$PB(i8 %123)
  %124 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @N$PS(ptr %124)
  %125 = load i8, ptr %0, !tbaa !2
  call addrspace(1) void @N$PB(i8 %125)
  call addrspace(1) void @N$PN()
  %126 = load ptr, ptr %5, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %126)
  %127 = load ptr, ptr %13, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %127)
  %128 = load ptr, ptr %17, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %128)
  ret i16 0

b7:
  %129 = getelementptr i8, ptr @$str11, i16 6
  call addrspace(1) void @N$PS(ptr %129)
  call addrspace(1) void @N$PN()
  br label %b6

b8:
  %130 = getelementptr i8, ptr addrspace(1) %94, i16 0
  %131 = load i16, ptr addrspace(1) %130
  %132 = getelementptr i8, ptr addrspace(1) %94, i16 2
  %133 = load i16, ptr addrspace(1) %132
  %134 = icmp eq i16 %131, 1
  %135 = sext i1 %134 to i8
  %136 = icmp ne i8 %135, 0
  br i1 %136, label %b9, label %b7

b9:
  %137 = icmp eq i16 %133, 2
  %138 = sext i1 %137 to i8
  %139 = icmp ne i8 %138, 0
  br i1 %139, label %b10, label %b7

b10:
  %140 = getelementptr i8, ptr addrspace(1) %94, i16 0
  %141 = load i16, ptr addrspace(1) %140
  %142 = getelementptr i8, ptr addrspace(1) %94, i16 2
  %143 = load i16, ptr addrspace(1) %142
  %144 = getelementptr i8, ptr addrspace(1) %94, i16 4
  %145 = sub i16 %95, 2
  store i16 %145, ptr %2, !tbaa !2
  %146 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 %145, ptr %146, !tbaa !2
  %147 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %144, ptr %147, !tbaa !2
  %148 = addrspacecast ptr %2 to ptr addrspace(1)
  %149 = getelementptr i8, ptr @$str10, i16 6
  call addrspace(1) void @N$PS(ptr %149)
  %150 = load i16, ptr addrspace(1) %148
  call addrspace(1) void @N$PU2(i16 %150)
  call addrspace(1) void @N$PN()
  br label %b6
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PN() addrspace(1)

declare void @N$PI2(i16) addrspace(1)

declare void @N$PU2(i16) addrspace(1)

declare ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare void @N$PB(i8) addrspace(1)

declare void @N$BDRP(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
