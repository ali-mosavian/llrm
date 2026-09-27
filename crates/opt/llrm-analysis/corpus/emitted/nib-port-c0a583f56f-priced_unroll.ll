target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

define internal i32 @value(i16 %0) addrspace(1) {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca [256 x i8]
  %13 = alloca i32
  %14 = alloca i16
  %15 = alloca i16
  %16 = alloca i16
  %17 = alloca i16
  %18 = alloca i16
  %19 = alloca i16
  %20 = alloca i16
  %21 = alloca [256 x i8]
  %22 = alloca i32
  store i16 0, ptr %1
  store i16 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %4
  store i16 0, ptr %5
  store i16 0, ptr %6
  store i16 0, ptr %7
  store i16 0, ptr %8
  store i16 0, ptr %9
  store i16 0, ptr %10
  store i16 0, ptr %11
  call void @llvm.memset.p0.i16(ptr %12, i8 0, i16 256, i1 false)
  store i32 0, ptr %13
  store i16 0, ptr %14
  store i16 0, ptr %15
  store i16 0, ptr %16
  store i16 0, ptr %17
  store i16 0, ptr %18
  store i16 0, ptr %19
  store i16 0, ptr %20
  call void @llvm.memset.p0.i16(ptr %21, i8 0, i16 256, i1 false)
  store i32 0, ptr %22
  store i32 0, ptr %22, !tbaa !2
  store i16 8, ptr %18, !tbaa !2
  store i16 8, ptr %19, !tbaa !2
  store i16 64, ptr %20, !tbaa !2
  store i16 0, ptr %17, !tbaa !2
  store i16 8, ptr %16, !tbaa !2
  br label %b2

b2:
  %23 = load i16, ptr %17, !tbaa !2
  %24 = load i16, ptr %16, !tbaa !2
  %25 = icmp slt i16 %23, %24
  %26 = sext i1 %25 to i8
  %27 = icmp ne i8 %26, 0
  br i1 %27, label %b3, label %b5

b3:
  store i16 0, ptr %15, !tbaa !2
  store i16 8, ptr %14, !tbaa !2
  br label %b6

b4:
  %28 = load i16, ptr %17, !tbaa !2
  %29 = add i16 %28, 1
  store i16 %29, ptr %17, !tbaa !2
  br label %b2

b5:
  store i32 0, ptr %13, !tbaa !2
  store i16 8, ptr %9, !tbaa !2
  store i16 8, ptr %10, !tbaa !2
  store i16 64, ptr %11, !tbaa !2
  store i16 0, ptr %8, !tbaa !2
  store i16 8, ptr %7, !tbaa !2
  br label %b10

b6:
  %30 = load i16, ptr %15, !tbaa !2
  %31 = load i16, ptr %14, !tbaa !2
  %32 = icmp slt i16 %30, %31
  %33 = sext i1 %32 to i8
  %34 = icmp ne i8 %33, 0
  br i1 %34, label %b7, label %b9

b7:
  %35 = load i16, ptr %17, !tbaa !2
  %36 = load i16, ptr %15, !tbaa !2
  %37 = load i32, ptr %22, !tbaa !2
  %38 = sub i16 %35, 0
  %39 = sub i16 %36, 0
  %40 = mul i16 %38, 8
  %41 = add i16 %40, %39
  %42 = getelementptr inbounds i32, ptr %21, i16 %41
  store i32 %37, ptr %42, !tbaa !2
  br label %b8

b8:
  %43 = load i16, ptr %15, !tbaa !2
  %44 = add i16 %43, 1
  store i16 %44, ptr %15, !tbaa !2
  br label %b6

b9:
  br label %b4

b10:
  %45 = load i16, ptr %8, !tbaa !2
  %46 = load i16, ptr %7, !tbaa !2
  %47 = icmp slt i16 %45, %46
  %48 = sext i1 %47 to i8
  %49 = icmp ne i8 %48, 0
  br i1 %49, label %b11, label %b13

b11:
  store i16 0, ptr %6, !tbaa !2
  store i16 8, ptr %5, !tbaa !2
  br label %b14

b12:
  %50 = load i16, ptr %8, !tbaa !2
  %51 = add i16 %50, 1
  store i16 %51, ptr %8, !tbaa !2
  br label %b10

b13:
  store i16 0, ptr %4, !tbaa !2
  store i16 8, ptr %3, !tbaa !2
  br label %b18

b14:
  %52 = load i16, ptr %6, !tbaa !2
  %53 = load i16, ptr %5, !tbaa !2
  %54 = icmp slt i16 %52, %53
  %55 = sext i1 %54 to i8
  %56 = icmp ne i8 %55, 0
  br i1 %56, label %b15, label %b17

b15:
  %57 = load i16, ptr %8, !tbaa !2
  %58 = load i16, ptr %6, !tbaa !2
  %59 = load i32, ptr %13, !tbaa !2
  %60 = sub i16 %57, 0
  %61 = sub i16 %58, 0
  %62 = mul i16 %60, 8
  %63 = add i16 %62, %61
  %64 = getelementptr inbounds i32, ptr %12, i16 %63
  store i32 %59, ptr %64, !tbaa !2
  br label %b16

b16:
  %65 = load i16, ptr %6, !tbaa !2
  %66 = add i16 %65, 1
  store i16 %66, ptr %6, !tbaa !2
  br label %b14

b17:
  br label %b12

b18:
  %67 = load i16, ptr %4, !tbaa !2
  %68 = load i16, ptr %3, !tbaa !2
  %69 = icmp slt i16 %67, %68
  %70 = sext i1 %69 to i8
  %71 = icmp ne i8 %70, 0
  br i1 %71, label %b19, label %b21

b19:
  store i16 0, ptr %2, !tbaa !2
  store i16 8, ptr %1, !tbaa !2
  br label %b22

b20:
  %72 = load i16, ptr %4, !tbaa !2
  %73 = add i16 %72, 1
  store i16 %73, ptr %4, !tbaa !2
  br label %b18

b21:
  %74 = icmp ult i16 %0, 8
  %75 = sext i1 %74 to i8
  %76 = icmp ne i8 %75, 0
  br i1 %76, label %b41, label %b42

b22:
  %77 = load i16, ptr %2, !tbaa !2
  %78 = load i16, ptr %1, !tbaa !2
  %79 = icmp slt i16 %77, %78
  %80 = sext i1 %79 to i8
  %81 = icmp ne i8 %80, 0
  br i1 %81, label %b23, label %b25

b23:
  %82 = load i16, ptr %4, !tbaa !2
  %83 = load i16, ptr %2, !tbaa !2
  %84 = icmp ult i16 %82, 8
  %85 = sext i1 %84 to i8
  %86 = icmp ne i8 %85, 0
  br i1 %86, label %b26, label %b27

b24:
  %87 = load i16, ptr %2, !tbaa !2
  %88 = add i16 %87, 1
  store i16 %88, ptr %2, !tbaa !2
  br label %b22

b25:
  br label %b20

b26:
  %89 = icmp ult i16 %83, 8
  %90 = sext i1 %89 to i8
  %91 = icmp ne i8 %90, 0
  br i1 %91, label %b28, label %b29

b27:
  call addrspace(1) void @N$EBND()
  unreachable

b28:
  %92 = load i16, ptr %4, !tbaa !2
  %93 = mul i16 %92, 3
  %94 = load i16, ptr %2, !tbaa !2
  %95 = add i16 %93, %94
  %96 = add i16 %95, 1
  %97 = sext i16 %96 to i32
  %98 = zext i8 8 to i32
  %99 = shl i32 %97, %98
  %100 = sext i32 %99 to i64
  %101 = sext i32 1024 to i64
  %102 = shl i64 %100, 8
  %103 = sdiv i64 %102, %101
  %104 = trunc i64 %103 to i32
  %105 = sub i16 %82, 0
  %106 = sub i16 %83, 0
  %107 = mul i16 %105, 8
  %108 = add i16 %107, %106
  %109 = getelementptr inbounds i32, ptr %21, i16 %108
  store i32 %104, ptr %109, !tbaa !2
  %110 = load i16, ptr %4, !tbaa !2
  %111 = load i16, ptr %2, !tbaa !2
  %112 = icmp eq i16 %110, %111
  %113 = sext i1 %112 to i8
  %114 = icmp ne i8 %113, 0
  br i1 %114, label %b30, label %b31

b29:
  call addrspace(1) void @N$EBND()
  unreachable

b30:
  %115 = load i16, ptr %4, !tbaa !2
  %116 = load i16, ptr %2, !tbaa !2
  %117 = icmp ult i16 %115, 8
  %118 = sext i1 %117 to i8
  %119 = icmp ne i8 %118, 0
  br i1 %119, label %b33, label %b34

b31:
  %120 = load i16, ptr %4, !tbaa !2
  %121 = load i16, ptr %2, !tbaa !2
  %122 = icmp ult i16 %120, 8
  %123 = sext i1 %122 to i8
  %124 = icmp ne i8 %123, 0
  br i1 %124, label %b37, label %b38

b32:
  br label %b24

b33:
  %125 = icmp ult i16 %116, 8
  %126 = sext i1 %125 to i8
  %127 = icmp ne i8 %126, 0
  br i1 %127, label %b35, label %b36

b34:
  call addrspace(1) void @N$EBND()
  unreachable

b35:
  %128 = sub i16 %115, 0
  %129 = sub i16 %116, 0
  %130 = mul i16 %128, 8
  %131 = add i16 %130, %129
  %132 = getelementptr inbounds i32, ptr %12, i16 %131
  store i32 512, ptr %132, !tbaa !2
  br label %b32

b36:
  call addrspace(1) void @N$EBND()
  unreachable

b37:
  %133 = icmp ult i16 %121, 8
  %134 = sext i1 %133 to i8
  %135 = icmp ne i8 %134, 0
  br i1 %135, label %b39, label %b40

b38:
  call addrspace(1) void @N$EBND()
  unreachable

b39:
  %136 = load i16, ptr %4, !tbaa !2
  %137 = load i16, ptr %2, !tbaa !2
  %138 = add i16 %136, %137
  %139 = srem i16 %138, 3
  %140 = sext i16 %139 to i32
  %141 = zext i8 8 to i32
  %142 = shl i32 %140, %141
  %143 = sext i32 %142 to i64
  %144 = sext i32 512 to i64
  %145 = shl i64 %143, 8
  %146 = sdiv i64 %145, %144
  %147 = trunc i64 %146 to i32
  %148 = sub i16 %120, 0
  %149 = sub i16 %121, 0
  %150 = mul i16 %148, 8
  %151 = add i16 %150, %149
  %152 = getelementptr inbounds i32, ptr %12, i16 %151
  store i32 %147, ptr %152, !tbaa !2
  br label %b32

b40:
  call addrspace(1) void @N$EBND()
  unreachable

b41:
  %153 = sub i16 %0, 0
  %154 = sub i16 1, 0
  %155 = mul i16 %153, 8
  %156 = add i16 %155, %154
  %157 = getelementptr inbounds i32, ptr %21, i16 %156
  %158 = load i32, ptr %157, !tbaa !2
  %159 = icmp ult i16 %0, 8
  %160 = sext i1 %159 to i8
  %161 = icmp ne i8 %160, 0
  br i1 %161, label %b43, label %b44

b42:
  call addrspace(1) void @N$EBND()
  unreachable

b43:
  %162 = sub i16 %0, 0
  %163 = sub i16 2, 0
  %164 = mul i16 %162, 8
  %165 = add i16 %164, %163
  %166 = getelementptr inbounds i32, ptr %12, i16 %165
  %167 = load i32, ptr %166, !tbaa !2
  %168 = add i32 %158, %167
  ret i32 %168

b44:
  call addrspace(1) void @N$EBND()
  unreachable
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = call addrspace(1) i32 @value(i16 3)
  ret i16 0
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$EBND() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
