target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [10 x i8] c"\08\00\03\00\03\00PX=\00"
@$str2 = internal constant [10 x i8] c"\08\00\03\00\03\00PY=\00"
@$str3 = internal constant [10 x i8] c"\08\00\03\00\03\00VX=\00"
@$str4 = internal constant [10 x i8] c"\08\00\03\00\03\00VY=\00"
@$str5 = internal constant [11 x i8] c"\08\00\04\00\04\00DONE\00"

define internal i32 @nbody(i32 %0) addrspace(1) {
b1:
  %1 = alloca [8 x i8]
  %2 = alloca [96 x i8]
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 96, i1 false)
  %3 = getelementptr inbounds [16 x i8], ptr %2, i16 0
  store i32 -7680, ptr %3, !tbaa !2
  %4 = getelementptr inbounds i8, ptr %3, i16 4
  store i32 -6144, ptr %4, !tbaa !2
  %5 = getelementptr inbounds i8, ptr %3, i16 8
  store i32 0, ptr %5, !tbaa !2
  %6 = getelementptr inbounds i8, ptr %3, i16 12
  store i32 0, ptr %6, !tbaa !2
  %7 = getelementptr inbounds [16 x i8], ptr %2, i16 1
  store i32 -4096, ptr %7, !tbaa !2
  %8 = getelementptr inbounds i8, ptr %7, i16 4
  store i32 -3584, ptr %8, !tbaa !2
  %9 = getelementptr inbounds i8, ptr %7, i16 8
  store i32 0, ptr %9, !tbaa !2
  %10 = getelementptr inbounds i8, ptr %7, i16 12
  store i32 0, ptr %10, !tbaa !2
  %11 = getelementptr inbounds [16 x i8], ptr %2, i16 2
  store i32 -512, ptr %11, !tbaa !2
  %12 = getelementptr inbounds i8, ptr %11, i16 4
  store i32 -1024, ptr %12, !tbaa !2
  %13 = getelementptr inbounds i8, ptr %11, i16 8
  store i32 0, ptr %13, !tbaa !2
  %14 = getelementptr inbounds i8, ptr %11, i16 12
  store i32 0, ptr %14, !tbaa !2
  %15 = getelementptr inbounds [16 x i8], ptr %2, i16 3
  store i32 3072, ptr %15, !tbaa !2
  %16 = getelementptr inbounds i8, ptr %15, i16 4
  store i32 1536, ptr %16, !tbaa !2
  %17 = getelementptr inbounds i8, ptr %15, i16 8
  store i32 0, ptr %17, !tbaa !2
  %18 = getelementptr inbounds i8, ptr %15, i16 12
  store i32 0, ptr %18, !tbaa !2
  %19 = getelementptr inbounds [16 x i8], ptr %2, i16 4
  store i32 6656, ptr %19, !tbaa !2
  %20 = getelementptr inbounds i8, ptr %19, i16 4
  store i32 4096, ptr %20, !tbaa !2
  %21 = getelementptr inbounds i8, ptr %19, i16 8
  store i32 0, ptr %21, !tbaa !2
  %22 = getelementptr inbounds i8, ptr %19, i16 12
  store i32 0, ptr %22, !tbaa !2
  %23 = getelementptr inbounds [16 x i8], ptr %2, i16 5
  store i32 10240, ptr %23, !tbaa !2
  %24 = getelementptr inbounds i8, ptr %23, i16 4
  store i32 6656, ptr %24, !tbaa !2
  %25 = getelementptr inbounds i8, ptr %23, i16 8
  store i32 0, ptr %25, !tbaa !2
  %26 = getelementptr inbounds i8, ptr %23, i16 12
  store i32 0, ptr %26, !tbaa !2
  %27 = getelementptr inbounds i8, ptr %1, i16 4
  br label %b2

b2:
  %28 = phi i32 [ 0, %b1 ], [ %110, %b20 ]
  %29 = icmp slt i32 %28, 1
  br i1 %29, label %b3, label %b5

b3:
  br label %b6

b5:
  %30 = getelementptr i8, ptr @$str1, i16 6
  %31 = getelementptr i8, ptr @$str2, i16 6
  %32 = getelementptr i8, ptr @$str3, i16 6
  %33 = getelementptr i8, ptr @$str4, i16 6
  br label %b21

b6:
  %34 = phi i16 [ 0, %b3 ], [ %64, %b13 ]
  %35 = icmp ult i16 %34, 6
  br i1 %35, label %b7, label %b9

b7:
  store i32 0, ptr %1, !tbaa !2
  store i32 0, ptr %27, !tbaa !2
  %36 = getelementptr inbounds [16 x i8], ptr %2, i16 %34
  %37 = getelementptr inbounds i8, ptr %36, i16 4
  br label %b10

b9:
  br label %b17

b10:
  %38 = phi i16 [ 0, %b7 ], [ %41, %b12 ]
  %39 = icmp ult i16 %38, 6
  br i1 %39, label %b11, label %b13

b11:
  %40 = icmp ne i16 %34, %38
  br i1 %40, label %b14, label %b12

b12:
  %41 = add i16 %38, 1
  br label %b10

b13:
  %42 = getelementptr inbounds i8, ptr %36, i16 8
  %43 = load i32, ptr %42, !tbaa !2
  %44 = load i32, ptr %1, !tbaa !2
  %45 = add i32 %43, %44
  store i32 %45, ptr %42, !tbaa !2
  %46 = getelementptr inbounds i8, ptr %36, i16 12
  %47 = load i32, ptr %46, !tbaa !2
  %48 = load i32, ptr %27, !tbaa !2
  %49 = add i32 %47, %48
  store i32 %49, ptr %46, !tbaa !2
  %50 = load i32, ptr %42, !tbaa !2
  %51 = load i32, ptr %42, !tbaa !2
  %52 = sext i32 %51 to i64
  %53 = shl i64 %52, 9
  %54 = sdiv i64 %53, 8192
  %55 = trunc i64 %54 to i32
  %56 = sub i32 %50, %55
  store i32 %56, ptr %42, !tbaa !2
  %57 = load i32, ptr %46, !tbaa !2
  %58 = load i32, ptr %46, !tbaa !2
  %59 = sext i32 %58 to i64
  %60 = shl i64 %59, 9
  %61 = sdiv i64 %60, 8192
  %62 = trunc i64 %61 to i32
  %63 = sub i32 %57, %62
  store i32 %63, ptr %46, !tbaa !2
  %64 = add i16 %34, 1
  br label %b6

b14:
  %65 = getelementptr inbounds [16 x i8], ptr %2, i16 %38
  %66 = load i32, ptr %65, !tbaa !2
  %67 = load i32, ptr %36, !tbaa !2
  %68 = sub i32 %66, %67
  %69 = getelementptr inbounds i8, ptr %65, i16 4
  %70 = load i32, ptr %69, !tbaa !2
  %71 = load i32, ptr %37, !tbaa !2
  %72 = sub i32 %70, %71
  %73 = sext i32 %68 to i64
  %74 = mul i64 %73, %73
  %75 = ashr i64 %74, 9
  %76 = trunc i64 %75 to i32
  %77 = sext i32 %72 to i64
  %78 = mul i64 %77, %77
  %79 = ashr i64 %78, 9
  %80 = trunc i64 %79 to i32
  %81 = add i32 %76, %80
  %82 = add i32 %81, 512
  %83 = sext i32 %82 to i64
  %84 = sdiv i64 262144, %83
  %85 = trunc i64 %84 to i32
  %86 = load i32, ptr %1, !tbaa !2
  %87 = sext i32 %85 to i64
  %88 = mul i64 %73, %87
  %89 = ashr i64 %88, 9
  %90 = trunc i64 %89 to i32
  %91 = add i32 %86, %90
  store i32 %91, ptr %1, !tbaa !2
  %92 = load i32, ptr %27, !tbaa !2
  %93 = mul i64 %77, %87
  %94 = ashr i64 %93, 9
  %95 = trunc i64 %94 to i32
  %96 = add i32 %92, %95
  store i32 %96, ptr %27, !tbaa !2
  br label %b12

b17:
  %97 = phi i16 [ 0, %b9 ], [ %109, %b18 ]
  %98 = icmp ult i16 %97, 6
  br i1 %98, label %b18, label %b20

b18:
  %99 = getelementptr inbounds [16 x i8], ptr %2, i16 %97
  %100 = load i32, ptr %99, !tbaa !2
  %101 = getelementptr inbounds i8, ptr %99, i16 8
  %102 = load i32, ptr %101, !tbaa !2
  %103 = add i32 %100, %102
  store i32 %103, ptr %99, !tbaa !2
  %104 = getelementptr inbounds i8, ptr %99, i16 4
  %105 = load i32, ptr %104, !tbaa !2
  %106 = getelementptr inbounds i8, ptr %99, i16 12
  %107 = load i32, ptr %106, !tbaa !2
  %108 = add i32 %105, %107
  store i32 %108, ptr %104, !tbaa !2
  %109 = add i16 %97, 1
  br label %b17

b20:
  %110 = add i32 %28, 1
  br label %b2

b21:
  %111 = phi i16 [ 0, %b5 ], [ %121, %b22 ]
  %112 = icmp ult i16 %111, 6
  br i1 %112, label %b22, label %b24

b22:
  call addrspace(1) void @N$PS(ptr %30)
  %113 = getelementptr inbounds [16 x i8], ptr %2, i16 %111
  %114 = load i32, ptr %113, !tbaa !2
  call addrspace(1) void @N$PQ4(i32 %114, i8 9)
  call addrspace(1) void @N$PN()
  call addrspace(1) void @N$PS(ptr %31)
  %115 = getelementptr inbounds i8, ptr %113, i16 4
  %116 = load i32, ptr %115, !tbaa !2
  call addrspace(1) void @N$PQ4(i32 %116, i8 9)
  call addrspace(1) void @N$PN()
  call addrspace(1) void @N$PS(ptr %32)
  %117 = getelementptr inbounds i8, ptr %113, i16 8
  %118 = load i32, ptr %117, !tbaa !2
  call addrspace(1) void @N$PQ4(i32 %118, i8 9)
  call addrspace(1) void @N$PN()
  call addrspace(1) void @N$PS(ptr %33)
  %119 = getelementptr inbounds i8, ptr %113, i16 12
  %120 = load i32, ptr %119, !tbaa !2
  call addrspace(1) void @N$PQ4(i32 %120, i8 9)
  call addrspace(1) void @N$PN()
  %121 = add i16 %111, 1
  br label %b21

b24:
  %122 = getelementptr i8, ptr @$str5, i16 6
  call addrspace(1) void @N$PS(ptr %122)
  call addrspace(1) void @N$PN()
  %123 = load i32, ptr %3, !tbaa !2
  %124 = load i32, ptr %8, !tbaa !2
  %125 = add i32 %123, %124
  %126 = load i32, ptr %11, !tbaa !2
  %127 = add i32 %125, %126
  %128 = load i32, ptr %16, !tbaa !2
  %129 = add i32 %127, %128
  ret i32 %129
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = call addrspace(1) i32 @nbody(i32 1)
  ret i16 0
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PQ4(i32, i8) addrspace(1)

declare void @N$PN() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
