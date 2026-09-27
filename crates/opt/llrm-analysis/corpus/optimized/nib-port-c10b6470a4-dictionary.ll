target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca [8 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 8, i1 false)
  %1 = getelementptr inbounds i16, ptr %0, i16 0
  store i16 1, ptr %1, !tbaa !2
  %2 = getelementptr inbounds i16, ptr %0, i16 1
  store i16 2, ptr %2, !tbaa !2
  %3 = getelementptr inbounds i16, ptr %0, i16 2
  store i16 1, ptr %3, !tbaa !2
  %4 = getelementptr inbounds i16, ptr %0, i16 3
  store i16 3, ptr %4, !tbaa !2
  %5 = getelementptr i8, ptr @$str1, i16 6
  br label %b2

b2:
  %6 = phi ptr [ %5, %b1 ], [ %9, %b32 ]
  %7 = phi i16 [ 0, %b1 ], [ %63, %b32 ]
  %8 = icmp ult i16 %7, 4
  br i1 %8, label %b3, label %16

b3:
  %9 = call addrspace(1) ptr @N$DRES(ptr %6, i16 6)
  %10 = getelementptr inbounds i16, ptr %0, i16 %7
  %11 = load i16, ptr %10, !tbaa !2
  %12 = or i16 %11, 1
  %13 = getelementptr i8, ptr %9, i16 -4
  %14 = load i16, ptr %13
  %15 = icmp ne i16 %14, 0
  br i1 %15, label %b6, label %b8

16:
  %17 = getelementptr i8, ptr %6, i16 -4
  %18 = load i16, ptr %17
  %19 = icmp ne i16 %18, 0
  br i1 %19, label %b34, label %b36

b6:
  %20 = add i16 %14, -1
  %21 = and i16 %12, %20
  br label %b9

b8:
  %22 = phi i16 [ 0, %b3 ], [ %26, %b11 ]
  %23 = phi i8 [ 0, %b3 ], [ %32, %b11 ]
  %24 = xor i8 %23, -1
  %25 = icmp ne i8 %24, 0
  br i1 %25, label %b23, label %b25

b9:
  %26 = phi i16 [ %21, %b6 ], [ %44, %b21 ]
  %27 = load i16, ptr %13
  %28 = icmp ult i16 %26, %27
  br i1 %28, label %b12, label %b13

b10:
  %29 = load i16, ptr %34
  %30 = icmp eq i16 %29, %12
  %31 = sext i1 %30 to i8
  br i1 %30, label %b18, label %b17

b11:
  %32 = phi i8 [ 0, %b12 ], [ -1, %b20 ]
  br label %b8

b12:
  %33 = mul i16 %26, 6
  %34 = getelementptr i8, ptr %9, i16 %33
  %35 = load i16, ptr %34
  %36 = icmp ne i16 %35, 0
  br i1 %36, label %b10, label %b11

b13:
  call addrspace(1) void @N$EBND()
  unreachable

b17:
  %37 = phi i8 [ %31, %b10 ], [ %42, %b18 ]
  %38 = icmp ne i8 %37, 0
  br i1 %38, label %b20, label %b21

b18:
  %39 = getelementptr i8, ptr %34, i16 2
  %40 = load i16, ptr %39
  %41 = icmp eq i16 %40, %11
  %42 = sext i1 %41 to i8
  br label %b17

b20:
  br label %b11

b21:
  %43 = add i16 %26, 1
  %44 = and i16 %43, %20
  br label %b9

b23:
  %45 = load i16, ptr %13
  %46 = icmp ult i16 %22, %45
  br i1 %46, label %b26, label %b27

b25:
  %47 = icmp ne i8 %23, 0
  br i1 %47, label %b31, label %b30

b26:
  %48 = mul i16 %22, 6
  %49 = getelementptr i8, ptr %9, i16 %48
  store i16 %12, ptr %49
  %50 = load i16, ptr %13
  %51 = icmp ult i16 %22, %50
  br i1 %51, label %b28, label %b29

b27:
  call addrspace(1) void @N$EBND()
  unreachable

b28:
  %52 = getelementptr i8, ptr %49, i16 2
  store i16 %11, ptr %52
  br label %b25

b29:
  call addrspace(1) void @N$EBND()
  unreachable

b30:
  %53 = getelementptr i8, ptr %9, i16 -2
  %54 = load i16, ptr %53
  %55 = add i16 %54, 1
  store i16 %55, ptr %53
  br label %b31

b31:
  %56 = load i16, ptr %13
  %57 = icmp ult i16 %22, %56
  br i1 %57, label %b32, label %b33

b32:
  %58 = mul i16 %22, 6
  %59 = getelementptr i8, ptr %9, i16 %58
  %60 = load i16, ptr %10, !tbaa !2
  %61 = mul i16 %60, 10
  %62 = getelementptr i8, ptr %59, i16 4
  store i16 %61, ptr %62
  %63 = add i16 %7, 1
  br label %b2

b33:
  call addrspace(1) void @N$EBND()
  unreachable

b34:
  %64 = add i16 %18, -1
  %65 = and i16 %64, 1
  br label %b37

b36:
  %66 = phi i16 [ 0, %16 ], [ %69, %b39 ]
  %67 = phi i8 [ 0, %16 ], [ %75, %b39 ]
  %68 = icmp ne i8 %67, 0
  br i1 %68, label %b51, label %b53

b37:
  %69 = phi i16 [ %65, %b34 ], [ %87, %b49 ]
  %70 = load i16, ptr %17
  %71 = icmp ult i16 %69, %70
  br i1 %71, label %b40, label %b41

b38:
  %72 = load i16, ptr %77
  %73 = icmp eq i16 %72, 1
  %74 = sext i1 %73 to i8
  br i1 %73, label %b46, label %b45

b39:
  %75 = phi i8 [ 0, %b40 ], [ -1, %b48 ]
  br label %b36

b40:
  %76 = mul i16 %69, 6
  %77 = getelementptr i8, ptr %6, i16 %76
  %78 = load i16, ptr %77
  %79 = icmp ne i16 %78, 0
  br i1 %79, label %b38, label %b39

b41:
  call addrspace(1) void @N$EBND()
  unreachable

b45:
  %80 = phi i8 [ %74, %b38 ], [ %85, %b46 ]
  %81 = icmp ne i8 %80, 0
  br i1 %81, label %b48, label %b49

b46:
  %82 = getelementptr i8, ptr %77, i16 2
  %83 = load i16, ptr %82
  %84 = icmp eq i16 %83, 1
  %85 = sext i1 %84 to i8
  br label %b45

b48:
  br label %b39

b49:
  %86 = add i16 %69, 1
  %87 = and i16 %86, %64
  br label %b37

b51:
  %88 = load i16, ptr %17
  %89 = icmp ult i16 %66, %88
  br i1 %89, label %b54, label %b55

b53:
  %90 = phi i16 [ 0, %b36 ], [ %96, %b54 ]
  %91 = load i16, ptr %17
  %92 = icmp ne i16 %91, 0
  br i1 %92, label %b56, label %b58

b54:
  %93 = mul i16 %66, 6
  %94 = getelementptr i8, ptr %6, i16 %93
  %95 = getelementptr i8, ptr %94, i16 4
  %96 = load i16, ptr %95
  br label %b53

b55:
  call addrspace(1) void @N$EBND()
  unreachable

b56:
  %97 = add i16 %91, -1
  %98 = and i16 %97, 3
  br label %b59

b58:
  %99 = phi i16 [ 0, %b53 ], [ %102, %b61 ]
  %100 = phi i8 [ 0, %b53 ], [ %108, %b61 ]
  %101 = icmp ne i8 %100, 0
  br i1 %101, label %b73, label %b75

b59:
  %102 = phi i16 [ %98, %b56 ], [ %120, %b71 ]
  %103 = load i16, ptr %17
  %104 = icmp ult i16 %102, %103
  br i1 %104, label %b62, label %b63

b60:
  %105 = load i16, ptr %110
  %106 = icmp eq i16 %105, 3
  %107 = sext i1 %106 to i8
  br i1 %106, label %b68, label %b67

b61:
  %108 = phi i8 [ 0, %b62 ], [ -1, %b70 ]
  br label %b58

b62:
  %109 = mul i16 %102, 6
  %110 = getelementptr i8, ptr %6, i16 %109
  %111 = load i16, ptr %110
  %112 = icmp ne i16 %111, 0
  br i1 %112, label %b60, label %b61

b63:
  call addrspace(1) void @N$EBND()
  unreachable

b67:
  %113 = phi i8 [ %107, %b60 ], [ %118, %b68 ]
  %114 = icmp ne i8 %113, 0
  br i1 %114, label %b70, label %b71

b68:
  %115 = getelementptr i8, ptr %110, i16 2
  %116 = load i16, ptr %115
  %117 = icmp eq i16 %116, 3
  %118 = sext i1 %117 to i8
  br label %b67

b70:
  br label %b61

b71:
  %119 = add i16 %102, 1
  %120 = and i16 %119, %97
  br label %b59

b73:
  %121 = load i16, ptr %17
  %122 = icmp ult i16 %99, %121
  br i1 %122, label %b76, label %b77

b75:
  %123 = phi i16 [ 0, %b58 ], [ %130, %b76 ]
  %124 = add i16 %90, %123
  %125 = load i16, ptr %17
  %126 = icmp ne i16 %125, 0
  br i1 %126, label %b78, label %b80

b76:
  %127 = mul i16 %99, 6
  %128 = getelementptr i8, ptr %6, i16 %127
  %129 = getelementptr i8, ptr %128, i16 4
  %130 = load i16, ptr %129
  br label %b75

b77:
  call addrspace(1) void @N$EBND()
  unreachable

b78:
  %131 = add i16 %125, -1
  %132 = and i16 %131, 9
  br label %b81

b80:
  %133 = phi i16 [ 0, %b75 ], [ %136, %b83 ]
  %134 = phi i8 [ 0, %b75 ], [ %142, %b83 ]
  %135 = icmp ne i8 %134, 0
  br i1 %135, label %b95, label %b97

b81:
  %136 = phi i16 [ %132, %b78 ], [ %154, %b93 ]
  %137 = load i16, ptr %17
  %138 = icmp ult i16 %136, %137
  br i1 %138, label %b84, label %b85

b82:
  %139 = load i16, ptr %144
  %140 = icmp eq i16 %139, 9
  %141 = sext i1 %140 to i8
  br i1 %140, label %b90, label %b89

b83:
  %142 = phi i8 [ 0, %b84 ], [ -1, %b92 ]
  br label %b80

b84:
  %143 = mul i16 %136, 6
  %144 = getelementptr i8, ptr %6, i16 %143
  %145 = load i16, ptr %144
  %146 = icmp ne i16 %145, 0
  br i1 %146, label %b82, label %b83

b85:
  call addrspace(1) void @N$EBND()
  unreachable

b89:
  %147 = phi i8 [ %141, %b82 ], [ %152, %b90 ]
  %148 = icmp ne i8 %147, 0
  br i1 %148, label %b92, label %b93

b90:
  %149 = getelementptr i8, ptr %144, i16 2
  %150 = load i16, ptr %149
  %151 = icmp eq i16 %150, 9
  %152 = sext i1 %151 to i8
  br label %b89

b92:
  br label %b83

b93:
  %153 = add i16 %136, 1
  %154 = and i16 %153, %131
  br label %b81

b95:
  %155 = load i16, ptr %17
  %156 = icmp ult i16 %133, %155
  br i1 %156, label %b98, label %b99

b97:
  %157 = phi i16 [ 5, %b80 ], [ %162, %b98 ]
  %158 = add i16 %124, %157
  call addrspace(1) void @N$BDRP(ptr %6)
  call addrspace(1) void @N$BDRP(ptr null)
  ret i16 %158

b98:
  %159 = mul i16 %133, 6
  %160 = getelementptr i8, ptr %6, i16 %159
  %161 = getelementptr i8, ptr %160, i16 4
  %162 = load i16, ptr %161
  br label %b97

b99:
  call addrspace(1) void @N$EBND()
  unreachable
}

define internal i16 @count() addrspace(1) {
b1:
  %0 = alloca [8 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 8, i1 false)
  %1 = getelementptr inbounds i16, ptr %0, i16 0
  store i16 1, ptr %1, !tbaa !2
  %2 = getelementptr inbounds i16, ptr %0, i16 1
  store i16 2, ptr %2, !tbaa !2
  %3 = getelementptr inbounds i16, ptr %0, i16 2
  store i16 1, ptr %3, !tbaa !2
  %4 = getelementptr inbounds i16, ptr %0, i16 3
  store i16 3, ptr %4, !tbaa !2
  %5 = getelementptr i8, ptr @$str1, i16 6
  br label %b2

b2:
  %6 = phi ptr [ %5, %b1 ], [ %9, %b32 ]
  %7 = phi i16 [ 0, %b1 ], [ %61, %b32 ]
  %8 = icmp ult i16 %7, 4
  br i1 %8, label %b3, label %b5

b3:
  %9 = call addrspace(1) ptr @N$DRES(ptr %6, i16 6)
  %10 = getelementptr inbounds i16, ptr %0, i16 %7
  %11 = load i16, ptr %10, !tbaa !2
  %12 = or i16 %11, 1
  %13 = getelementptr i8, ptr %9, i16 -4
  %14 = load i16, ptr %13
  %15 = icmp ne i16 %14, 0
  br i1 %15, label %b6, label %b8

b5:
  %16 = getelementptr i8, ptr %6, i16 -2
  %17 = load i16, ptr %16
  call addrspace(1) void @N$BDRP(ptr %6)
  call addrspace(1) void @N$BDRP(ptr null)
  ret i16 %17

b6:
  %18 = add i16 %14, -1
  %19 = and i16 %12, %18
  br label %b9

b8:
  %20 = phi i16 [ 0, %b3 ], [ %24, %b11 ]
  %21 = phi i8 [ 0, %b3 ], [ %30, %b11 ]
  %22 = xor i8 %21, -1
  %23 = icmp ne i8 %22, 0
  br i1 %23, label %b23, label %b25

b9:
  %24 = phi i16 [ %19, %b6 ], [ %42, %b21 ]
  %25 = load i16, ptr %13
  %26 = icmp ult i16 %24, %25
  br i1 %26, label %b12, label %b13

b10:
  %27 = load i16, ptr %32
  %28 = icmp eq i16 %27, %12
  %29 = sext i1 %28 to i8
  br i1 %28, label %b18, label %b17

b11:
  %30 = phi i8 [ 0, %b12 ], [ -1, %b20 ]
  br label %b8

b12:
  %31 = mul i16 %24, 6
  %32 = getelementptr i8, ptr %9, i16 %31
  %33 = load i16, ptr %32
  %34 = icmp ne i16 %33, 0
  br i1 %34, label %b10, label %b11

b13:
  call addrspace(1) void @N$EBND()
  unreachable

b17:
  %35 = phi i8 [ %29, %b10 ], [ %40, %b18 ]
  %36 = icmp ne i8 %35, 0
  br i1 %36, label %b20, label %b21

b18:
  %37 = getelementptr i8, ptr %32, i16 2
  %38 = load i16, ptr %37
  %39 = icmp eq i16 %38, %11
  %40 = sext i1 %39 to i8
  br label %b17

b20:
  br label %b11

b21:
  %41 = add i16 %24, 1
  %42 = and i16 %41, %18
  br label %b9

b23:
  %43 = load i16, ptr %13
  %44 = icmp ult i16 %20, %43
  br i1 %44, label %b26, label %b27

b25:
  %45 = icmp ne i8 %21, 0
  br i1 %45, label %b31, label %b30

b26:
  %46 = mul i16 %20, 6
  %47 = getelementptr i8, ptr %9, i16 %46
  store i16 %12, ptr %47
  %48 = load i16, ptr %13
  %49 = icmp ult i16 %20, %48
  br i1 %49, label %b28, label %b29

b27:
  call addrspace(1) void @N$EBND()
  unreachable

b28:
  %50 = getelementptr i8, ptr %47, i16 2
  store i16 %11, ptr %50
  br label %b25

b29:
  call addrspace(1) void @N$EBND()
  unreachable

b30:
  %51 = getelementptr i8, ptr %9, i16 -2
  %52 = load i16, ptr %51
  %53 = add i16 %52, 1
  store i16 %53, ptr %51
  br label %b31

b31:
  %54 = load i16, ptr %13
  %55 = icmp ult i16 %20, %54
  br i1 %55, label %b32, label %b33

b32:
  %56 = mul i16 %20, 6
  %57 = getelementptr i8, ptr %9, i16 %56
  %58 = load i16, ptr %10, !tbaa !2
  %59 = mul i16 %58, 10
  %60 = getelementptr i8, ptr %57, i16 4
  store i16 %59, ptr %60
  %61 = add i16 %7, 1
  br label %b2

b33:
  call addrspace(1) void @N$EBND()
  unreachable
}

define internal i8 @i16.eq(i16 %0, i16 %1) addrspace(1) memory(none) willreturn {
b1:
  %2 = icmp eq i16 %0, %1
  %3 = sext i1 %2 to i8
  ret i8 %3
}

define internal i16 @i16.hash(i16 %0) addrspace(1) memory(none) willreturn {
b1:
  ret i16 %0
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare ptr @N$DRES(ptr, i16) addrspace(1)

declare void @N$EBND() addrspace(1)

declare void @N$BDRP(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
