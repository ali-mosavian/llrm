target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [15 x i8] c"\08\00\08\00\08\00matmul: \00"

define internal cc 1005 void @multiply(ptr addrspace(1) noalias readonly dereferenceable(10) nocapture %0, ptr addrspace(1) noalias readonly dereferenceable(10) nocapture %1, ptr addrspace(1) noalias readonly dereferenceable(10) nocapture %2) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i32
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca i16
  %10 = load i16, ptr addrspace(1) %0
  store i16 0, ptr %9, !tbaa !2
  store i16 %10, ptr %8, !tbaa !2
  br label %b2

b2:
  %11 = load i16, ptr %9, !tbaa !2
  %12 = load i16, ptr %8, !tbaa !2
  %13 = icmp ult i16 %11, %12
  br i1 %13, label %b3, label %b5

b3:
  %14 = getelementptr i8, ptr addrspace(1) %1, i16 2
  %15 = load i16, ptr addrspace(1) %14
  store i16 0, ptr %7, !tbaa !2
  store i16 %15, ptr %6, !tbaa !2
  br label %b6

b4:
  %16 = load i16, ptr %9, !tbaa !2
  %17 = add nuw i16 %16, 1
  store i16 %17, ptr %9, !tbaa !2
  br label %b2

b5:
  ret void

b6:
  %18 = load i16, ptr %7, !tbaa !2
  %19 = load i16, ptr %6, !tbaa !2
  %20 = icmp ult i16 %18, %19
  br i1 %20, label %b7, label %b9

b7:
  store i32 0, ptr %5, !tbaa !2
  %21 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %22 = load i16, ptr addrspace(1) %21
  store i16 0, ptr %4, !tbaa !2
  store i16 %22, ptr %3, !tbaa !2
  br label %b10

b8:
  %23 = load i16, ptr %7, !tbaa !2
  %24 = add nuw i16 %23, 1
  store i16 %24, ptr %7, !tbaa !2
  br label %b6

b9:
  br label %b4

b10:
  %25 = load i16, ptr %4, !tbaa !2
  %26 = load i16, ptr %3, !tbaa !2
  %27 = icmp ult i16 %25, %26
  br i1 %27, label %b11, label %b13

b11:
  %28 = load i32, ptr %5, !tbaa !2
  %29 = load i16, ptr %9, !tbaa !2
  %30 = load i16, ptr %4, !tbaa !2
  %31 = load i16, ptr addrspace(1) %0, !range !5
  %32 = icmp ult i16 %29, %31
  br i1 %32, label %b14, label %b15

b12:
  %33 = load i16, ptr %4, !tbaa !2
  %34 = add nuw i16 %33, 1
  store i16 %34, ptr %4, !tbaa !2
  br label %b10

b13:
  %35 = load i16, ptr %9, !tbaa !2
  %36 = load i16, ptr %7, !tbaa !2
  %37 = load i16, ptr addrspace(1) %2, !range !9
  %38 = icmp ult i16 %35, %37
  br i1 %38, label %b22, label %b23

b14:
  %39 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %40 = load i16, ptr addrspace(1) %39, !range !6
  %41 = icmp ult i16 %30, %40
  br i1 %41, label %b16, label %b17

b15:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b16:
  %42 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %43 = load i16, ptr addrspace(1) %42
  %44 = mul i16 %43, 1
  %45 = mul i16 %29, %44
  %46 = mul i16 %30, 1
  %47 = add i16 %45, %46
  %48 = getelementptr i8, ptr addrspace(1) %0, i16 6
  %49 = load ptr addrspace(1), ptr addrspace(1) %48
  %50 = mul i16 %47, 4
  %51 = getelementptr inbounds i8, ptr addrspace(1) %49, i16 %50
  %52 = load i32, ptr addrspace(1) %51
  %53 = load i16, ptr %4, !tbaa !2
  %54 = load i16, ptr %7, !tbaa !2
  %55 = load i16, ptr addrspace(1) %1, !range !7
  %56 = icmp ult i16 %53, %55
  br i1 %56, label %b18, label %b19

b17:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b18:
  %57 = getelementptr i8, ptr addrspace(1) %1, i16 2
  %58 = load i16, ptr addrspace(1) %57, !range !8
  %59 = icmp ult i16 %54, %58
  br i1 %59, label %b20, label %b21

b19:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b20:
  %60 = getelementptr i8, ptr addrspace(1) %1, i16 2
  %61 = load i16, ptr addrspace(1) %60
  %62 = mul i16 %61, 1
  %63 = mul i16 %53, %62
  %64 = mul i16 %54, 1
  %65 = add i16 %63, %64
  %66 = getelementptr i8, ptr addrspace(1) %1, i16 6
  %67 = load ptr addrspace(1), ptr addrspace(1) %66
  %68 = mul i16 %65, 4
  %69 = getelementptr inbounds i8, ptr addrspace(1) %67, i16 %68
  %70 = load i32, ptr addrspace(1) %69
  %71 = call i32 @llvm.smul.fix.i32(i32 %52, i32 %70, i32 8)
  %72 = add i32 %28, %71
  store i32 %72, ptr %5, !tbaa !2
  br label %b12

b21:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b22:
  %73 = getelementptr i8, ptr addrspace(1) %2, i16 2
  %74 = load i16, ptr addrspace(1) %73, !range !10
  %75 = icmp ult i16 %36, %74
  br i1 %75, label %b24, label %b25

b23:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b24:
  %76 = getelementptr i8, ptr addrspace(1) %2, i16 2
  %77 = load i16, ptr addrspace(1) %76
  %78 = mul i16 %77, 1
  %79 = mul i16 %35, %78
  %80 = mul i16 %36, 1
  %81 = add i16 %79, %80
  %82 = getelementptr i8, ptr addrspace(1) %2, i16 6
  %83 = load ptr addrspace(1), ptr addrspace(1) %82
  %84 = mul i16 %81, 4
  %85 = getelementptr inbounds i8, ptr addrspace(1) %83, i16 %84
  %86 = load i32, ptr %5, !tbaa !2
  store i32 %86, ptr addrspace(1) %85
  br label %b8

b25:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i32
  %5 = alloca [10 x i8]
  %6 = alloca [10 x i8]
  %7 = alloca [10 x i8]
  %8 = alloca i16
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca i16
  %14 = alloca i16
  %15 = alloca i16
  %16 = alloca i16
  %17 = alloca i16
  %18 = alloca i16
  %19 = alloca [256 x i8]
  %20 = alloca i32
  %21 = alloca i16
  %22 = alloca i16
  %23 = alloca i16
  %24 = alloca i16
  %25 = alloca i16
  %26 = alloca i16
  %27 = alloca i16
  %28 = alloca [256 x i8]
  %29 = alloca i32
  %30 = alloca i16
  %31 = alloca i16
  %32 = alloca i16
  %33 = alloca i16
  %34 = alloca i16
  %35 = alloca i16
  %36 = alloca i16
  %37 = alloca [256 x i8]
  %38 = alloca i32
  %39 = alloca i16
  store i16 1, ptr %39, !tbaa !2
  store i32 0, ptr %38, !tbaa !2
  store i16 8, ptr %34, !tbaa !2
  store i16 8, ptr %35, !tbaa !2
  store i16 64, ptr %36, !tbaa !2
  store i16 0, ptr %33, !tbaa !2
  store i16 8, ptr %32, !tbaa !2
  br label %b2

b2:
  %40 = load i16, ptr %33, !tbaa !2
  %41 = load i16, ptr %32, !tbaa !2
  %42 = icmp slt i16 %40, %41
  br i1 %42, label %b3, label %b5

b3:
  store i16 0, ptr %31, !tbaa !2
  store i16 8, ptr %30, !tbaa !2
  br label %b6

b4:
  %43 = load i16, ptr %33, !tbaa !2
  %44 = add nsw i16 %43, 1
  store i16 %44, ptr %33, !tbaa !2
  br label %b2

b5:
  store i32 0, ptr %29, !tbaa !2
  store i16 8, ptr %25, !tbaa !2
  store i16 8, ptr %26, !tbaa !2
  store i16 64, ptr %27, !tbaa !2
  store i16 0, ptr %24, !tbaa !2
  store i16 8, ptr %23, !tbaa !2
  br label %b10

b6:
  %45 = load i16, ptr %31, !tbaa !2
  %46 = load i16, ptr %30, !tbaa !2
  %47 = icmp slt i16 %45, %46
  br i1 %47, label %b7, label %b9

b7:
  %48 = load i16, ptr %33, !tbaa !2
  %49 = load i16, ptr %31, !tbaa !2
  %50 = load i32, ptr %38, !tbaa !2
  %51 = sub i16 %48, 0
  %52 = sub i16 %49, 0
  %53 = mul i16 %51, 8
  %54 = add i16 %53, %52
  %55 = getelementptr inbounds i32, ptr %37, i16 %54
  store i32 %50, ptr %55, !tbaa !2
  br label %b8

b8:
  %56 = load i16, ptr %31, !tbaa !2
  %57 = add nsw i16 %56, 1
  store i16 %57, ptr %31, !tbaa !2
  br label %b6

b9:
  br label %b4

b10:
  %58 = load i16, ptr %24, !tbaa !2
  %59 = load i16, ptr %23, !tbaa !2
  %60 = icmp slt i16 %58, %59
  br i1 %60, label %b11, label %b13

b11:
  store i16 0, ptr %22, !tbaa !2
  store i16 8, ptr %21, !tbaa !2
  br label %b14

b12:
  %61 = load i16, ptr %24, !tbaa !2
  %62 = add nsw i16 %61, 1
  store i16 %62, ptr %24, !tbaa !2
  br label %b10

b13:
  store i32 0, ptr %20, !tbaa !2
  store i16 8, ptr %16, !tbaa !2
  store i16 8, ptr %17, !tbaa !2
  store i16 64, ptr %18, !tbaa !2
  store i16 0, ptr %15, !tbaa !2
  store i16 8, ptr %14, !tbaa !2
  br label %b18

b14:
  %63 = load i16, ptr %22, !tbaa !2
  %64 = load i16, ptr %21, !tbaa !2
  %65 = icmp slt i16 %63, %64
  br i1 %65, label %b15, label %b17

b15:
  %66 = load i16, ptr %24, !tbaa !2
  %67 = load i16, ptr %22, !tbaa !2
  %68 = load i32, ptr %29, !tbaa !2
  %69 = sub i16 %66, 0
  %70 = sub i16 %67, 0
  %71 = mul i16 %69, 8
  %72 = add i16 %71, %70
  %73 = getelementptr inbounds i32, ptr %28, i16 %72
  store i32 %68, ptr %73, !tbaa !2
  br label %b16

b16:
  %74 = load i16, ptr %22, !tbaa !2
  %75 = add nsw i16 %74, 1
  store i16 %75, ptr %22, !tbaa !2
  br label %b14

b17:
  br label %b12

b18:
  %76 = load i16, ptr %15, !tbaa !2
  %77 = load i16, ptr %14, !tbaa !2
  %78 = icmp slt i16 %76, %77
  br i1 %78, label %b19, label %b21

b19:
  store i16 0, ptr %13, !tbaa !2
  store i16 8, ptr %12, !tbaa !2
  br label %b22

b20:
  %79 = load i16, ptr %15, !tbaa !2
  %80 = add nsw i16 %79, 1
  store i16 %80, ptr %15, !tbaa !2
  br label %b18

b21:
  store i16 0, ptr %11, !tbaa !2
  store i16 8, ptr %10, !tbaa !2
  br label %b26

b22:
  %81 = load i16, ptr %13, !tbaa !2
  %82 = load i16, ptr %12, !tbaa !2
  %83 = icmp slt i16 %81, %82
  br i1 %83, label %b23, label %b25

b23:
  %84 = load i16, ptr %15, !tbaa !2
  %85 = load i16, ptr %13, !tbaa !2
  %86 = load i32, ptr %20, !tbaa !2
  %87 = sub i16 %84, 0
  %88 = sub i16 %85, 0
  %89 = mul i16 %87, 8
  %90 = add i16 %89, %88
  %91 = getelementptr inbounds i32, ptr %19, i16 %90
  store i32 %86, ptr %91, !tbaa !2
  br label %b24

b24:
  %92 = load i16, ptr %13, !tbaa !2
  %93 = add nsw i16 %92, 1
  store i16 %93, ptr %13, !tbaa !2
  br label %b22

b25:
  br label %b20

b26:
  %94 = load i16, ptr %11, !tbaa !2
  %95 = load i16, ptr %10, !tbaa !2
  %96 = icmp slt i16 %94, %95
  br i1 %96, label %b27, label %b29

b27:
  store i16 0, ptr %9, !tbaa !2
  store i16 8, ptr %8, !tbaa !2
  br label %b30

b28:
  %97 = load i16, ptr %11, !tbaa !2
  %98 = add nsw i16 %97, 1
  store i16 %98, ptr %11, !tbaa !2
  br label %b26

b29:
  %99 = addrspacecast ptr %37 to ptr addrspace(1)
  store i16 8, ptr %7, !tbaa !2
  %100 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 8, ptr %100, !tbaa !2
  %101 = getelementptr inbounds i8, ptr %7, i16 4
  store i16 64, ptr %101, !tbaa !2
  %102 = getelementptr inbounds i8, ptr %7, i16 6
  store ptr addrspace(1) %99, ptr %102, !tbaa !2
  %103 = addrspacecast ptr %7 to ptr addrspace(1)
  %104 = addrspacecast ptr %28 to ptr addrspace(1)
  store i16 8, ptr %6, !tbaa !2
  %105 = getelementptr inbounds i8, ptr %6, i16 2
  store i16 8, ptr %105, !tbaa !2
  %106 = getelementptr inbounds i8, ptr %6, i16 4
  store i16 64, ptr %106, !tbaa !2
  %107 = getelementptr inbounds i8, ptr %6, i16 6
  store ptr addrspace(1) %104, ptr %107, !tbaa !2
  %108 = addrspacecast ptr %6 to ptr addrspace(1)
  %109 = addrspacecast ptr %19 to ptr addrspace(1)
  store i16 8, ptr %5, !tbaa !2
  %110 = getelementptr inbounds i8, ptr %5, i16 2
  store i16 8, ptr %110, !tbaa !2
  %111 = getelementptr inbounds i8, ptr %5, i16 4
  store i16 64, ptr %111, !tbaa !2
  %112 = getelementptr inbounds i8, ptr %5, i16 6
  store ptr addrspace(1) %109, ptr %112, !tbaa !2
  %113 = addrspacecast ptr %5 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @multiply(ptr addrspace(1) %103, ptr addrspace(1) %108, ptr addrspace(1) %113)
  store i32 0, ptr %4, !tbaa !2
  store i16 0, ptr %3, !tbaa !2
  store i16 8, ptr %2, !tbaa !2
  br label %b49

b30:
  %114 = load i16, ptr %9, !tbaa !2
  %115 = load i16, ptr %8, !tbaa !2
  %116 = icmp slt i16 %114, %115
  br i1 %116, label %b31, label %b33

b31:
  %117 = load i16, ptr %11, !tbaa !2
  %118 = load i16, ptr %9, !tbaa !2
  %119 = icmp ult i16 %117, 8
  br i1 %119, label %b34, label %b35

b32:
  %120 = load i16, ptr %9, !tbaa !2
  %121 = add nsw i16 %120, 1
  store i16 %121, ptr %9, !tbaa !2
  br label %b30

b33:
  br label %b28

b34:
  %122 = icmp ult i16 %118, 8
  br i1 %122, label %b36, label %b37

b35:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b36:
  %123 = load i16, ptr %11, !tbaa !2
  %124 = mul i16 %123, 3
  %125 = load i16, ptr %9, !tbaa !2
  %126 = add i16 %124, %125
  %127 = add i16 %126, 1
  %128 = load i16, ptr %39, !tbaa !2
  %129 = add i16 %127, %128
  %130 = sext i16 %129 to i32
  %131 = shl i32 %130, 8
  %132 = sext i32 %131 to i64
  %133 = shl i64 %132, 8
  %134 = sdiv i64 %133, 1024
  %135 = trunc i64 %134 to i32
  %136 = sub i16 %117, 0
  %137 = sub i16 %118, 0
  %138 = mul i16 %136, 8
  %139 = add i16 %138, %137
  %140 = getelementptr inbounds i32, ptr %37, i16 %139
  store i32 %135, ptr %140, !tbaa !2
  %141 = load i16, ptr %11, !tbaa !2
  %142 = load i16, ptr %9, !tbaa !2
  %143 = icmp eq i16 %141, %142
  br i1 %143, label %b38, label %b39

b37:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b38:
  %144 = load i16, ptr %11, !tbaa !2
  %145 = load i16, ptr %9, !tbaa !2
  %146 = icmp ult i16 %144, 8
  br i1 %146, label %b41, label %b42

b39:
  %147 = load i16, ptr %11, !tbaa !2
  %148 = load i16, ptr %9, !tbaa !2
  %149 = icmp ult i16 %147, 8
  br i1 %149, label %b45, label %b46

b40:
  br label %b32

b41:
  %150 = icmp ult i16 %145, 8
  br i1 %150, label %b43, label %b44

b42:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b43:
  %151 = sub i16 %144, 0
  %152 = sub i16 %145, 0
  %153 = mul i16 %151, 8
  %154 = add i16 %153, %152
  %155 = getelementptr inbounds i32, ptr %28, i16 %154
  store i32 512, ptr %155, !tbaa !2
  br label %b40

b44:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b45:
  %156 = icmp ult i16 %148, 8
  br i1 %156, label %b47, label %b48

b46:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b47:
  %157 = load i16, ptr %11, !tbaa !2
  %158 = load i16, ptr %9, !tbaa !2
  %159 = add i16 %157, %158
  %160 = srem i16 %159, 3
  %161 = sext i16 %160 to i32
  %162 = shl i32 %161, 8
  %163 = sext i32 %162 to i64
  %164 = shl i64 %163, 8
  %165 = sdiv i64 %164, 512
  %166 = trunc i64 %165 to i32
  %167 = sub i16 %147, 0
  %168 = sub i16 %148, 0
  %169 = mul i16 %167, 8
  %170 = add i16 %169, %168
  %171 = getelementptr inbounds i32, ptr %28, i16 %170
  store i32 %166, ptr %171, !tbaa !2
  br label %b40

b48:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b49:
  %172 = load i16, ptr %3, !tbaa !2
  %173 = load i16, ptr %2, !tbaa !2
  %174 = icmp slt i16 %172, %173
  br i1 %174, label %b50, label %b52

b50:
  store i16 0, ptr %1, !tbaa !2
  store i16 8, ptr %0, !tbaa !2
  br label %b53

b51:
  %175 = load i16, ptr %3, !tbaa !2
  %176 = add nsw i16 %175, 1
  store i16 %176, ptr %3, !tbaa !2
  br label %b49

b52:
  %177 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %177)
  %178 = load i32, ptr %4, !tbaa !2
  call cc 1005 addrspace(1) void @N$PQ4(i32 %178, i8 zeroext 8)
  call cc 1005 addrspace(1) void @N$PN()
  ret i16 0

b53:
  %179 = load i16, ptr %1, !tbaa !2
  %180 = load i16, ptr %0, !tbaa !2
  %181 = icmp slt i16 %179, %180
  br i1 %181, label %b54, label %b56

b54:
  %182 = load i32, ptr %4, !tbaa !2
  %183 = load i16, ptr %3, !tbaa !2
  %184 = load i16, ptr %1, !tbaa !2
  %185 = icmp ult i16 %183, 8
  br i1 %185, label %b57, label %b58

b55:
  %186 = load i16, ptr %1, !tbaa !2
  %187 = add nsw i16 %186, 1
  store i16 %187, ptr %1, !tbaa !2
  br label %b53

b56:
  br label %b51

b57:
  %188 = icmp ult i16 %184, 8
  br i1 %188, label %b59, label %b60

b58:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b59:
  %189 = sub i16 %183, 0
  %190 = sub i16 %184, 0
  %191 = mul i16 %189, 8
  %192 = add i16 %191, %190
  %193 = getelementptr inbounds i32, ptr %19, i16 %192
  %194 = load i32, ptr %193, !tbaa !2
  %195 = load i16, ptr %3, !tbaa !2
  %196 = mul i16 %195, 8
  %197 = load i16, ptr %1, !tbaa !2
  %198 = add i16 %196, %197
  %199 = add i16 %198, 1
  %200 = sext i16 %199 to i32
  %201 = shl i32 %200, 8
  %202 = call i32 @llvm.smul.fix.i32(i32 %194, i32 %201, i32 8)
  %203 = add i32 %182, %202
  store i32 %203, ptr %4, !tbaa !2
  br label %b55

b60:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare i32 @llvm.smul.fix.i32(i32, i32, i32 immarg) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i32 @llvm.sdiv.fix.i32(i32, i32, i32 immarg) nocallback nofree nosync nounwind willreturn memory(none)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PQ4(i32, i8) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i16 0, i16 16384}
!6 = !{i16 0, i16 16384}
!7 = !{i16 0, i16 16384}
!8 = !{i16 0, i16 16384}
!9 = !{i16 0, i16 16384}
!10 = !{i16 0, i16 16384}
