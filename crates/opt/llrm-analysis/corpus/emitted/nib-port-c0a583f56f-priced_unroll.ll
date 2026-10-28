target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

define internal cc 1005 i32 @value(i16 %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
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
  br i1 %25, label %b3, label %b5

b3:
  store i16 0, ptr %15, !tbaa !2
  store i16 8, ptr %14, !tbaa !2
  br label %b6

b4:
  %26 = load i16, ptr %17, !tbaa !2
  %27 = add nsw i16 %26, 1
  store i16 %27, ptr %17, !tbaa !2
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
  %28 = load i16, ptr %15, !tbaa !2
  %29 = load i16, ptr %14, !tbaa !2
  %30 = icmp slt i16 %28, %29
  br i1 %30, label %b7, label %b9

b7:
  %31 = load i16, ptr %17, !tbaa !2
  %32 = load i16, ptr %15, !tbaa !2
  %33 = load i32, ptr %22, !tbaa !2
  %34 = sub i16 %31, 0
  %35 = sub i16 %32, 0
  %36 = mul i16 %34, 8
  %37 = add i16 %36, %35
  %38 = getelementptr inbounds i32, ptr %21, i16 %37
  store i32 %33, ptr %38, !tbaa !2
  br label %b8

b8:
  %39 = load i16, ptr %15, !tbaa !2
  %40 = add nsw i16 %39, 1
  store i16 %40, ptr %15, !tbaa !2
  br label %b6

b9:
  br label %b4

b10:
  %41 = load i16, ptr %8, !tbaa !2
  %42 = load i16, ptr %7, !tbaa !2
  %43 = icmp slt i16 %41, %42
  br i1 %43, label %b11, label %b13

b11:
  store i16 0, ptr %6, !tbaa !2
  store i16 8, ptr %5, !tbaa !2
  br label %b14

b12:
  %44 = load i16, ptr %8, !tbaa !2
  %45 = add nsw i16 %44, 1
  store i16 %45, ptr %8, !tbaa !2
  br label %b10

b13:
  store i16 0, ptr %4, !tbaa !2
  store i16 8, ptr %3, !tbaa !2
  br label %b18

b14:
  %46 = load i16, ptr %6, !tbaa !2
  %47 = load i16, ptr %5, !tbaa !2
  %48 = icmp slt i16 %46, %47
  br i1 %48, label %b15, label %b17

b15:
  %49 = load i16, ptr %8, !tbaa !2
  %50 = load i16, ptr %6, !tbaa !2
  %51 = load i32, ptr %13, !tbaa !2
  %52 = sub i16 %49, 0
  %53 = sub i16 %50, 0
  %54 = mul i16 %52, 8
  %55 = add i16 %54, %53
  %56 = getelementptr inbounds i32, ptr %12, i16 %55
  store i32 %51, ptr %56, !tbaa !2
  br label %b16

b16:
  %57 = load i16, ptr %6, !tbaa !2
  %58 = add nsw i16 %57, 1
  store i16 %58, ptr %6, !tbaa !2
  br label %b14

b17:
  br label %b12

b18:
  %59 = load i16, ptr %4, !tbaa !2
  %60 = load i16, ptr %3, !tbaa !2
  %61 = icmp slt i16 %59, %60
  br i1 %61, label %b19, label %b21

b19:
  store i16 0, ptr %2, !tbaa !2
  store i16 8, ptr %1, !tbaa !2
  br label %b22

b20:
  %62 = load i16, ptr %4, !tbaa !2
  %63 = add nsw i16 %62, 1
  store i16 %63, ptr %4, !tbaa !2
  br label %b18

b21:
  %64 = icmp ult i16 %0, 8
  br i1 %64, label %b41, label %b42

b22:
  %65 = load i16, ptr %2, !tbaa !2
  %66 = load i16, ptr %1, !tbaa !2
  %67 = icmp slt i16 %65, %66
  br i1 %67, label %b23, label %b25

b23:
  %68 = load i16, ptr %4, !tbaa !2
  %69 = load i16, ptr %2, !tbaa !2
  %70 = icmp ult i16 %68, 8
  br i1 %70, label %b26, label %b27

b24:
  %71 = load i16, ptr %2, !tbaa !2
  %72 = add nsw i16 %71, 1
  store i16 %72, ptr %2, !tbaa !2
  br label %b22

b25:
  br label %b20

b26:
  %73 = icmp ult i16 %69, 8
  br i1 %73, label %b28, label %b29

b27:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b28:
  %74 = load i16, ptr %4, !tbaa !2
  %75 = mul i16 %74, 3
  %76 = load i16, ptr %2, !tbaa !2
  %77 = add i16 %75, %76
  %78 = add i16 %77, 1
  %79 = sext i16 %78 to i32
  %80 = shl i32 %79, 8
  %81 = sext i32 %80 to i64
  %82 = shl i64 %81, 8
  %83 = sdiv i64 %82, 1024
  %84 = trunc i64 %83 to i32
  %85 = sub i16 %68, 0
  %86 = sub i16 %69, 0
  %87 = mul i16 %85, 8
  %88 = add i16 %87, %86
  %89 = getelementptr inbounds i32, ptr %21, i16 %88
  store i32 %84, ptr %89, !tbaa !2
  %90 = load i16, ptr %4, !tbaa !2
  %91 = load i16, ptr %2, !tbaa !2
  %92 = icmp eq i16 %90, %91
  br i1 %92, label %b30, label %b31

b29:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b30:
  %93 = load i16, ptr %4, !tbaa !2
  %94 = load i16, ptr %2, !tbaa !2
  %95 = icmp ult i16 %93, 8
  br i1 %95, label %b33, label %b34

b31:
  %96 = load i16, ptr %4, !tbaa !2
  %97 = load i16, ptr %2, !tbaa !2
  %98 = icmp ult i16 %96, 8
  br i1 %98, label %b37, label %b38

b32:
  br label %b24

b33:
  %99 = icmp ult i16 %94, 8
  br i1 %99, label %b35, label %b36

b34:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b35:
  %100 = sub i16 %93, 0
  %101 = sub i16 %94, 0
  %102 = mul i16 %100, 8
  %103 = add i16 %102, %101
  %104 = getelementptr inbounds i32, ptr %12, i16 %103
  store i32 512, ptr %104, !tbaa !2
  br label %b32

b36:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b37:
  %105 = icmp ult i16 %97, 8
  br i1 %105, label %b39, label %b40

b38:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b39:
  %106 = load i16, ptr %4, !tbaa !2
  %107 = load i16, ptr %2, !tbaa !2
  %108 = add i16 %106, %107
  %109 = srem i16 %108, 3
  %110 = sext i16 %109 to i32
  %111 = shl i32 %110, 8
  %112 = sext i32 %111 to i64
  %113 = shl i64 %112, 8
  %114 = sdiv i64 %113, 512
  %115 = trunc i64 %114 to i32
  %116 = sub i16 %96, 0
  %117 = sub i16 %97, 0
  %118 = mul i16 %116, 8
  %119 = add i16 %118, %117
  %120 = getelementptr inbounds i32, ptr %12, i16 %119
  store i32 %115, ptr %120, !tbaa !2
  br label %b32

b40:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b41:
  %121 = sub i16 %0, 0
  %122 = mul i16 %121, 8
  %123 = add i16 %122, 1
  %124 = getelementptr inbounds i32, ptr %21, i16 %123
  %125 = load i32, ptr %124, !tbaa !2
  %126 = icmp ult i16 %0, 8
  br i1 %126, label %b43, label %b44

b42:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b43:
  %127 = sub i16 %0, 0
  %128 = mul i16 %127, 8
  %129 = add i16 %128, 2
  %130 = getelementptr inbounds i32, ptr %12, i16 %129
  %131 = load i32, ptr %130, !tbaa !2
  %132 = add i32 %125, %131
  ret i32 %132

b44:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = call cc 1005 addrspace(1) i32 @value(i16 3)
  ret i16 0
}

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare i32 @llvm.sdiv.fix.i32(i32, i32, i32 immarg) nocallback nofree nosync nounwind willreturn memory(none)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
