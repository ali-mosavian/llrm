target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [15 x i8] c"\08\00\08\00\08\00matmul: \00"

define internal cc 1005 i32 @matmul(i32 %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i32
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i32
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
  %19 = alloca i16
  %20 = alloca i16
  %21 = alloca [256 x i8]
  %22 = alloca i32
  %23 = alloca i16
  %24 = alloca i16
  %25 = alloca i16
  %26 = alloca i16
  %27 = alloca [256 x i8]
  %28 = alloca i32
  %29 = alloca i16
  %30 = alloca i16
  %31 = alloca i16
  %32 = alloca i16
  %33 = alloca [256 x i8]
  %34 = alloca i32
  %35 = alloca i16
  store i16 8, ptr %35, !tbaa !2
  store i32 0, ptr %34, !tbaa !2
  store i16 64, ptr %31, !tbaa !2
  store i16 64, ptr %32, !tbaa !2
  store i16 0, ptr %30, !tbaa !2
  store i16 64, ptr %29, !tbaa !2
  br label %b2

b2:
  %36 = load i16, ptr %30, !tbaa !2
  %37 = load i16, ptr %29, !tbaa !2
  %38 = icmp slt i16 %36, %37
  br i1 %38, label %b3, label %b5

b3:
  %39 = load i16, ptr %30, !tbaa !2
  %40 = load i32, ptr %34, !tbaa !2
  %41 = sub i16 %39, 0
  %42 = getelementptr inbounds i32, ptr %33, i16 %41
  store i32 %40, ptr %42, !tbaa !2
  br label %b4

b4:
  %43 = load i16, ptr %30, !tbaa !2
  %44 = add nsw i16 %43, 1
  store i16 %44, ptr %30, !tbaa !2
  br label %b2

b5:
  store i32 0, ptr %28, !tbaa !2
  store i16 64, ptr %25, !tbaa !2
  store i16 64, ptr %26, !tbaa !2
  store i16 0, ptr %24, !tbaa !2
  store i16 64, ptr %23, !tbaa !2
  br label %b6

b6:
  %45 = load i16, ptr %24, !tbaa !2
  %46 = load i16, ptr %23, !tbaa !2
  %47 = icmp slt i16 %45, %46
  br i1 %47, label %b7, label %b9

b7:
  %48 = load i16, ptr %24, !tbaa !2
  %49 = load i32, ptr %28, !tbaa !2
  %50 = sub i16 %48, 0
  %51 = getelementptr inbounds i32, ptr %27, i16 %50
  store i32 %49, ptr %51, !tbaa !2
  br label %b8

b8:
  %52 = load i16, ptr %24, !tbaa !2
  %53 = add nsw i16 %52, 1
  store i16 %53, ptr %24, !tbaa !2
  br label %b6

b9:
  store i32 0, ptr %22, !tbaa !2
  store i16 64, ptr %19, !tbaa !2
  store i16 64, ptr %20, !tbaa !2
  store i16 0, ptr %18, !tbaa !2
  store i16 64, ptr %17, !tbaa !2
  br label %b10

b10:
  %54 = load i16, ptr %18, !tbaa !2
  %55 = load i16, ptr %17, !tbaa !2
  %56 = icmp slt i16 %54, %55
  br i1 %56, label %b11, label %b13

b11:
  %57 = load i16, ptr %18, !tbaa !2
  %58 = load i32, ptr %22, !tbaa !2
  %59 = sub i16 %57, 0
  %60 = getelementptr inbounds i32, ptr %21, i16 %59
  store i32 %58, ptr %60, !tbaa !2
  br label %b12

b12:
  %61 = load i16, ptr %18, !tbaa !2
  %62 = add nsw i16 %61, 1
  store i16 %62, ptr %18, !tbaa !2
  br label %b10

b13:
  %63 = load i16, ptr %35, !tbaa !2
  store i16 0, ptr %16, !tbaa !2
  store i16 %63, ptr %15, !tbaa !2
  br label %b14

b14:
  %64 = load i16, ptr %16, !tbaa !2
  %65 = load i16, ptr %15, !tbaa !2
  %66 = icmp slt i16 %64, %65
  br i1 %66, label %b15, label %b17

b15:
  %67 = load i16, ptr %35, !tbaa !2
  store i16 0, ptr %14, !tbaa !2
  store i16 %67, ptr %13, !tbaa !2
  br label %b18

b16:
  %68 = load i16, ptr %16, !tbaa !2
  %69 = add nsw i16 %68, 1
  store i16 %69, ptr %16, !tbaa !2
  br label %b14

b17:
  %70 = load i16, ptr %35, !tbaa !2
  store i16 0, ptr %12, !tbaa !2
  store i16 %70, ptr %11, !tbaa !2
  br label %b31

b18:
  %71 = load i16, ptr %14, !tbaa !2
  %72 = load i16, ptr %13, !tbaa !2
  %73 = icmp slt i16 %71, %72
  br i1 %73, label %b19, label %b21

b19:
  %74 = load i16, ptr %16, !tbaa !2
  %75 = mul i16 %74, 8
  %76 = load i16, ptr %14, !tbaa !2
  %77 = add i16 %75, %76
  %78 = icmp ult i16 %77, 64
  br i1 %78, label %b22, label %b23

b20:
  %79 = load i16, ptr %14, !tbaa !2
  %80 = add nsw i16 %79, 1
  store i16 %80, ptr %14, !tbaa !2
  br label %b18

b21:
  br label %b16

b22:
  %81 = load i16, ptr %16, !tbaa !2
  %82 = mul i16 %81, 3
  %83 = load i16, ptr %14, !tbaa !2
  %84 = add i16 %82, %83
  %85 = add i16 %84, 1
  %86 = sext i16 %85 to i32
  %87 = add i32 %86, %0
  %88 = shl i32 %87, 8
  %89 = sext i32 %88 to i64
  %90 = shl i64 %89, 8
  %91 = sdiv i64 %90, 1024
  %92 = trunc i64 %91 to i32
  %93 = sub i16 %77, 0
  %94 = getelementptr inbounds i32, ptr %33, i16 %93
  store i32 %92, ptr %94, !tbaa !2
  %95 = load i16, ptr %16, !tbaa !2
  %96 = load i16, ptr %14, !tbaa !2
  %97 = icmp eq i16 %95, %96
  br i1 %97, label %b24, label %b25

b23:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b24:
  %98 = load i16, ptr %16, !tbaa !2
  %99 = mul i16 %98, 8
  %100 = load i16, ptr %14, !tbaa !2
  %101 = add i16 %99, %100
  %102 = icmp ult i16 %101, 64
  br i1 %102, label %b27, label %b28

b25:
  %103 = load i16, ptr %16, !tbaa !2
  %104 = mul i16 %103, 8
  %105 = load i16, ptr %14, !tbaa !2
  %106 = add i16 %104, %105
  %107 = icmp ult i16 %106, 64
  br i1 %107, label %b29, label %b30

b26:
  br label %b20

b27:
  %108 = sub i16 %101, 0
  %109 = getelementptr inbounds i32, ptr %27, i16 %108
  store i32 512, ptr %109, !tbaa !2
  br label %b26

b28:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b29:
  %110 = load i16, ptr %16, !tbaa !2
  %111 = load i16, ptr %14, !tbaa !2
  %112 = add i16 %110, %111
  %113 = srem i16 %112, 3
  %114 = sext i16 %113 to i32
  %115 = shl i32 %114, 8
  %116 = sext i32 %115 to i64
  %117 = shl i64 %116, 8
  %118 = sdiv i64 %117, 512
  %119 = trunc i64 %118 to i32
  %120 = sub i16 %106, 0
  %121 = getelementptr inbounds i32, ptr %27, i16 %120
  store i32 %119, ptr %121, !tbaa !2
  br label %b26

b30:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b31:
  %122 = load i16, ptr %12, !tbaa !2
  %123 = load i16, ptr %11, !tbaa !2
  %124 = icmp slt i16 %122, %123
  br i1 %124, label %b32, label %b34

b32:
  %125 = load i16, ptr %35, !tbaa !2
  store i16 0, ptr %10, !tbaa !2
  store i16 %125, ptr %9, !tbaa !2
  br label %b35

b33:
  %126 = load i16, ptr %12, !tbaa !2
  %127 = add nsw i16 %126, 1
  store i16 %127, ptr %12, !tbaa !2
  br label %b31

b34:
  store i32 0, ptr %5, !tbaa !2
  %128 = load i16, ptr %35, !tbaa !2
  store i16 0, ptr %4, !tbaa !2
  store i16 %128, ptr %3, !tbaa !2
  br label %b49

b35:
  %129 = load i16, ptr %10, !tbaa !2
  %130 = load i16, ptr %9, !tbaa !2
  %131 = icmp slt i16 %129, %130
  br i1 %131, label %b36, label %b38

b36:
  store i32 0, ptr %8, !tbaa !2
  %132 = load i16, ptr %35, !tbaa !2
  store i16 0, ptr %7, !tbaa !2
  store i16 %132, ptr %6, !tbaa !2
  br label %b39

b37:
  %133 = load i16, ptr %10, !tbaa !2
  %134 = add nsw i16 %133, 1
  store i16 %134, ptr %10, !tbaa !2
  br label %b35

b38:
  br label %b33

b39:
  %135 = load i16, ptr %7, !tbaa !2
  %136 = load i16, ptr %6, !tbaa !2
  %137 = icmp slt i16 %135, %136
  br i1 %137, label %b40, label %b42

b40:
  %138 = load i32, ptr %8, !tbaa !2
  %139 = load i16, ptr %12, !tbaa !2
  %140 = mul i16 %139, 8
  %141 = load i16, ptr %7, !tbaa !2
  %142 = add i16 %140, %141
  %143 = icmp ult i16 %142, 64
  br i1 %143, label %b43, label %b44

b41:
  %144 = load i16, ptr %7, !tbaa !2
  %145 = add nsw i16 %144, 1
  store i16 %145, ptr %7, !tbaa !2
  br label %b39

b42:
  %146 = load i16, ptr %12, !tbaa !2
  %147 = mul i16 %146, 8
  %148 = load i16, ptr %10, !tbaa !2
  %149 = add i16 %147, %148
  %150 = icmp ult i16 %149, 64
  br i1 %150, label %b47, label %b48

b43:
  %151 = sub i16 %142, 0
  %152 = getelementptr inbounds i32, ptr %33, i16 %151
  %153 = load i32, ptr %152, !tbaa !2
  %154 = load i16, ptr %7, !tbaa !2
  %155 = mul i16 %154, 8
  %156 = load i16, ptr %10, !tbaa !2
  %157 = add i16 %155, %156
  %158 = icmp ult i16 %157, 64
  br i1 %158, label %b45, label %b46

b44:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b45:
  %159 = sub i16 %157, 0
  %160 = getelementptr inbounds i32, ptr %27, i16 %159
  %161 = load i32, ptr %160, !tbaa !2
  %162 = call i32 @llvm.smul.fix.i32(i32 %153, i32 %161, i32 8)
  %163 = add i32 %138, %162
  store i32 %163, ptr %8, !tbaa !2
  br label %b41

b46:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b47:
  %164 = load i32, ptr %8, !tbaa !2
  %165 = sub i16 %149, 0
  %166 = getelementptr inbounds i32, ptr %21, i16 %165
  store i32 %164, ptr %166, !tbaa !2
  br label %b37

b48:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b49:
  %167 = load i16, ptr %4, !tbaa !2
  %168 = load i16, ptr %3, !tbaa !2
  %169 = icmp slt i16 %167, %168
  br i1 %169, label %b50, label %b52

b50:
  %170 = load i16, ptr %35, !tbaa !2
  store i16 0, ptr %2, !tbaa !2
  store i16 %170, ptr %1, !tbaa !2
  br label %b53

b51:
  %171 = load i16, ptr %4, !tbaa !2
  %172 = add nsw i16 %171, 1
  store i16 %172, ptr %4, !tbaa !2
  br label %b49

b52:
  %173 = load i32, ptr %5, !tbaa !2
  ret i32 %173

b53:
  %174 = load i16, ptr %2, !tbaa !2
  %175 = load i16, ptr %1, !tbaa !2
  %176 = icmp slt i16 %174, %175
  br i1 %176, label %b54, label %b56

b54:
  %177 = load i32, ptr %5, !tbaa !2
  %178 = load i16, ptr %4, !tbaa !2
  %179 = mul i16 %178, 8
  %180 = load i16, ptr %2, !tbaa !2
  %181 = add i16 %179, %180
  %182 = icmp ult i16 %181, 64
  br i1 %182, label %b57, label %b58

b55:
  %183 = load i16, ptr %2, !tbaa !2
  %184 = add nsw i16 %183, 1
  store i16 %184, ptr %2, !tbaa !2
  br label %b53

b56:
  br label %b51

b57:
  %185 = sub i16 %181, 0
  %186 = getelementptr inbounds i32, ptr %21, i16 %185
  %187 = load i32, ptr %186, !tbaa !2
  %188 = load i16, ptr %4, !tbaa !2
  %189 = mul i16 %188, 8
  %190 = load i16, ptr %2, !tbaa !2
  %191 = add i16 %189, %190
  %192 = add i16 %191, 1
  %193 = sext i16 %192 to i32
  %194 = shl i32 %193, 8
  %195 = call i32 @llvm.smul.fix.i32(i32 %187, i32 %194, i32 8)
  %196 = add i32 %177, %195
  store i32 %196, ptr %5, !tbaa !2
  br label %b55

b58:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i32
  %1 = call cc 1005 addrspace(1) i32 @matmul(i32 1)
  store i32 %1, ptr %0, !tbaa !2
  %2 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %2)
  %3 = load i32, ptr %0, !tbaa !2
  call cc 1005 addrspace(1) void @N$PQ4(i32 %3, i8 zeroext 8)
  call cc 1005 addrspace(1) void @N$PN()
  ret i16 0
}

declare i32 @llvm.sdiv.fix.i32(i32, i32, i32 immarg) nocallback nofree nosync nounwind willreturn memory(none)

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare i32 @llvm.smul.fix.i32(i32, i32, i32 immarg) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PQ4(i32, i8) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
