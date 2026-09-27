target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

define internal i32 @total(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) {
b1:
  %1 = alloca i16
  %2 = alloca i32
  store i16 0, ptr %1
  store i32 0, ptr %2
  store i32 0, ptr %2, !tbaa !2
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
  %10 = mul i16 %4, 10
  %11 = getelementptr i8, ptr addrspace(1) %9, i16 %10
  %12 = load i32, ptr %2, !tbaa !2
  %13 = getelementptr i8, ptr addrspace(1) %11, i16 2
  %14 = load i32, ptr addrspace(1) %13
  %15 = add i32 %12, %14
  store i32 %15, ptr %2, !tbaa !2
  br label %b4

b4:
  %16 = load i16, ptr %1, !tbaa !2
  %17 = add i16 %16, 1
  store i16 %17, ptr %1, !tbaa !2
  br label %b2

b5:
  %18 = load i32, ptr %2, !tbaa !2
  ret i32 %18
}

define internal i32 @update(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) {
b1:
  %1 = alloca [8 x i8]
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca [50 x i8]
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 8, i1 false)
  store i16 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %4
  call void @llvm.memset.p0.i16(ptr %5, i8 0, i16 50, i1 false)
  store i16 5, ptr %3, !tbaa !2
  store i16 5, ptr %4, !tbaa !2
  %6 = load i16, ptr addrspace(1) %0
  %7 = icmp ult i16 0, %6
  %8 = sext i1 %7 to i8
  %9 = icmp ne i8 %8, 0
  br i1 %9, label %b2, label %b3

b2:
  %10 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %11 = load ptr addrspace(1), ptr addrspace(1) %10
  %12 = getelementptr i8, ptr addrspace(1) %11, i16 0
  %13 = load i32, ptr addrspace(1) %12
  %14 = load i16, ptr addrspace(1) %0
  %15 = icmp ult i16 1, %14
  %16 = sext i1 %15 to i8
  %17 = icmp ne i8 %16, 0
  br i1 %17, label %b4, label %b5

b3:
  call addrspace(1) void @N$EBND()
  unreachable

b4:
  %18 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %19 = load ptr addrspace(1), ptr addrspace(1) %18
  %20 = getelementptr i8, ptr addrspace(1) %19, i16 4
  %21 = load i32, ptr addrspace(1) %20
  %22 = sub i16 0, 0
  %23 = getelementptr inbounds [10 x i8], ptr %5, i16 %22
  store i16 0, ptr %23, !tbaa !2
  %24 = sub i16 0, 0
  %25 = getelementptr inbounds [10 x i8], ptr %5, i16 %24
  %26 = getelementptr inbounds i8, ptr %25, i16 2
  store i32 %13, ptr %26, !tbaa !2
  %27 = sub i16 0, 0
  %28 = getelementptr inbounds [10 x i8], ptr %5, i16 %27
  %29 = getelementptr inbounds i8, ptr %28, i16 6
  store i32 %21, ptr %29, !tbaa !2
  %30 = load i16, ptr addrspace(1) %0
  %31 = icmp ult i16 1, %30
  %32 = sext i1 %31 to i8
  %33 = icmp ne i8 %32, 0
  br i1 %33, label %b6, label %b7

b5:
  call addrspace(1) void @N$EBND()
  unreachable

b6:
  %34 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %35 = load ptr addrspace(1), ptr addrspace(1) %34
  %36 = getelementptr i8, ptr addrspace(1) %35, i16 4
  %37 = load i32, ptr addrspace(1) %36
  %38 = load i16, ptr addrspace(1) %0
  %39 = icmp ult i16 2, %38
  %40 = sext i1 %39 to i8
  %41 = icmp ne i8 %40, 0
  br i1 %41, label %b8, label %b9

b7:
  call addrspace(1) void @N$EBND()
  unreachable

b8:
  %42 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %43 = load ptr addrspace(1), ptr addrspace(1) %42
  %44 = getelementptr i8, ptr addrspace(1) %43, i16 8
  %45 = load i32, ptr addrspace(1) %44
  %46 = sub i16 1, 0
  %47 = getelementptr inbounds [10 x i8], ptr %5, i16 %46
  store i16 0, ptr %47, !tbaa !2
  %48 = sub i16 1, 0
  %49 = getelementptr inbounds [10 x i8], ptr %5, i16 %48
  %50 = getelementptr inbounds i8, ptr %49, i16 2
  store i32 %37, ptr %50, !tbaa !2
  %51 = sub i16 1, 0
  %52 = getelementptr inbounds [10 x i8], ptr %5, i16 %51
  %53 = getelementptr inbounds i8, ptr %52, i16 6
  store i32 %45, ptr %53, !tbaa !2
  %54 = load i16, ptr addrspace(1) %0
  %55 = icmp ult i16 2, %54
  %56 = sext i1 %55 to i8
  %57 = icmp ne i8 %56, 0
  br i1 %57, label %b10, label %b11

b9:
  call addrspace(1) void @N$EBND()
  unreachable

b10:
  %58 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %59 = load ptr addrspace(1), ptr addrspace(1) %58
  %60 = getelementptr i8, ptr addrspace(1) %59, i16 8
  %61 = load i32, ptr addrspace(1) %60
  %62 = load i16, ptr addrspace(1) %0
  %63 = icmp ult i16 3, %62
  %64 = sext i1 %63 to i8
  %65 = icmp ne i8 %64, 0
  br i1 %65, label %b12, label %b13

b11:
  call addrspace(1) void @N$EBND()
  unreachable

b12:
  %66 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %67 = load ptr addrspace(1), ptr addrspace(1) %66
  %68 = getelementptr i8, ptr addrspace(1) %67, i16 12
  %69 = load i32, ptr addrspace(1) %68
  %70 = sub i16 2, 0
  %71 = getelementptr inbounds [10 x i8], ptr %5, i16 %70
  store i16 0, ptr %71, !tbaa !2
  %72 = sub i16 2, 0
  %73 = getelementptr inbounds [10 x i8], ptr %5, i16 %72
  %74 = getelementptr inbounds i8, ptr %73, i16 2
  store i32 %61, ptr %74, !tbaa !2
  %75 = sub i16 2, 0
  %76 = getelementptr inbounds [10 x i8], ptr %5, i16 %75
  %77 = getelementptr inbounds i8, ptr %76, i16 6
  store i32 %69, ptr %77, !tbaa !2
  %78 = load i16, ptr addrspace(1) %0
  %79 = icmp ult i16 3, %78
  %80 = sext i1 %79 to i8
  %81 = icmp ne i8 %80, 0
  br i1 %81, label %b14, label %b15

b13:
  call addrspace(1) void @N$EBND()
  unreachable

b14:
  %82 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %83 = load ptr addrspace(1), ptr addrspace(1) %82
  %84 = getelementptr i8, ptr addrspace(1) %83, i16 12
  %85 = load i32, ptr addrspace(1) %84
  %86 = load i16, ptr addrspace(1) %0
  %87 = icmp ult i16 4, %86
  %88 = sext i1 %87 to i8
  %89 = icmp ne i8 %88, 0
  br i1 %89, label %b16, label %b17

b15:
  call addrspace(1) void @N$EBND()
  unreachable

b16:
  %90 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %91 = load ptr addrspace(1), ptr addrspace(1) %90
  %92 = getelementptr i8, ptr addrspace(1) %91, i16 16
  %93 = load i32, ptr addrspace(1) %92
  %94 = sub i16 3, 0
  %95 = getelementptr inbounds [10 x i8], ptr %5, i16 %94
  store i16 0, ptr %95, !tbaa !2
  %96 = sub i16 3, 0
  %97 = getelementptr inbounds [10 x i8], ptr %5, i16 %96
  %98 = getelementptr inbounds i8, ptr %97, i16 2
  store i32 %85, ptr %98, !tbaa !2
  %99 = sub i16 3, 0
  %100 = getelementptr inbounds [10 x i8], ptr %5, i16 %99
  %101 = getelementptr inbounds i8, ptr %100, i16 6
  store i32 %93, ptr %101, !tbaa !2
  %102 = load i16, ptr addrspace(1) %0
  %103 = icmp ult i16 4, %102
  %104 = sext i1 %103 to i8
  %105 = icmp ne i8 %104, 0
  br i1 %105, label %b18, label %b19

b17:
  call addrspace(1) void @N$EBND()
  unreachable

b18:
  %106 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %107 = load ptr addrspace(1), ptr addrspace(1) %106
  %108 = getelementptr i8, ptr addrspace(1) %107, i16 16
  %109 = load i32, ptr addrspace(1) %108
  %110 = load i16, ptr addrspace(1) %0
  %111 = icmp ult i16 5, %110
  %112 = sext i1 %111 to i8
  %113 = icmp ne i8 %112, 0
  br i1 %113, label %b20, label %b21

b19:
  call addrspace(1) void @N$EBND()
  unreachable

b20:
  %114 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %115 = load ptr addrspace(1), ptr addrspace(1) %114
  %116 = getelementptr i8, ptr addrspace(1) %115, i16 20
  %117 = load i32, ptr addrspace(1) %116
  %118 = sub i16 4, 0
  %119 = getelementptr inbounds [10 x i8], ptr %5, i16 %118
  store i16 0, ptr %119, !tbaa !2
  %120 = sub i16 4, 0
  %121 = getelementptr inbounds [10 x i8], ptr %5, i16 %120
  %122 = getelementptr inbounds i8, ptr %121, i16 2
  store i32 %109, ptr %122, !tbaa !2
  %123 = sub i16 4, 0
  %124 = getelementptr inbounds [10 x i8], ptr %5, i16 %123
  %125 = getelementptr inbounds i8, ptr %124, i16 6
  store i32 %117, ptr %125, !tbaa !2
  store i16 0, ptr %2, !tbaa !2
  br label %b22

b21:
  call addrspace(1) void @N$EBND()
  unreachable

b22:
  %126 = load i16, ptr %2, !tbaa !2
  %127 = icmp ult i16 %126, 5
  %128 = sext i1 %127 to i8
  %129 = icmp ne i8 %128, 0
  br i1 %129, label %b23, label %b25

b23:
  %130 = sub i16 %126, 0
  %131 = getelementptr inbounds [10 x i8], ptr %5, i16 %130
  %132 = getelementptr inbounds i8, ptr %131, i16 2
  %133 = load i32, ptr %132, !tbaa !2
  %134 = sub i16 %126, 0
  %135 = getelementptr inbounds [10 x i8], ptr %5, i16 %134
  %136 = getelementptr inbounds i8, ptr %135, i16 6
  %137 = load i32, ptr %136, !tbaa !2
  %138 = add i32 %133, %137
  %139 = sub i16 %126, 0
  %140 = getelementptr inbounds [10 x i8], ptr %5, i16 %139
  %141 = getelementptr inbounds i8, ptr %140, i16 2
  store i32 %138, ptr %141, !tbaa !2
  br label %b24

b24:
  %142 = load i16, ptr %2, !tbaa !2
  %143 = add i16 %142, 1
  store i16 %143, ptr %2, !tbaa !2
  br label %b22

b25:
  %144 = addrspacecast ptr %5 to ptr addrspace(1)
  store i16 5, ptr %1, !tbaa !2
  %145 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 5, ptr %145, !tbaa !2
  %146 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %144, ptr %146, !tbaa !2
  %147 = addrspacecast ptr %1 to ptr addrspace(1)
  %148 = call addrspace(1) i32 @total(ptr addrspace(1) %147)
  ret i32 %148
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca [8 x i8]
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca [24 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 8, i1 false)
  store i16 0, ptr %1
  store i16 0, ptr %2
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 24, i1 false)
  store i16 6, ptr %1, !tbaa !2
  store i16 6, ptr %2, !tbaa !2
  %4 = sub i16 0, 0
  %5 = getelementptr inbounds i32, ptr %3, i16 %4
  store i32 1, ptr %5, !tbaa !2
  %6 = sub i16 1, 0
  %7 = getelementptr inbounds i32, ptr %3, i16 %6
  store i32 2, ptr %7, !tbaa !2
  %8 = sub i16 2, 0
  %9 = getelementptr inbounds i32, ptr %3, i16 %8
  store i32 3, ptr %9, !tbaa !2
  %10 = sub i16 3, 0
  %11 = getelementptr inbounds i32, ptr %3, i16 %10
  store i32 4, ptr %11, !tbaa !2
  %12 = sub i16 4, 0
  %13 = getelementptr inbounds i32, ptr %3, i16 %12
  store i32 5, ptr %13, !tbaa !2
  %14 = sub i16 5, 0
  %15 = getelementptr inbounds i32, ptr %3, i16 %14
  store i32 6, ptr %15, !tbaa !2
  %16 = addrspacecast ptr %3 to ptr addrspace(1)
  store i16 6, ptr %0, !tbaa !2
  %17 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 6, ptr %17, !tbaa !2
  %18 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %16, ptr %18, !tbaa !2
  %19 = addrspacecast ptr %0 to ptr addrspace(1)
  %20 = call addrspace(1) i32 @update(ptr addrspace(1) %19)
  ret i16 0
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$EBND() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
