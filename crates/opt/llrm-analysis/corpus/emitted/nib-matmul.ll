target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [15 x i8] c"\08\00\08\00\08\00matmul: \00"
@$str2 = internal constant [19 x i8] c"\08\00\0C\00\0C\00matmul: bad \00"

define internal i32 @matmul(i32 %0) addrspace(1) {
b1:
  %1 = alloca i32
  %2 = alloca i32
  %3 = alloca i32
  %4 = alloca i32
  %5 = alloca i32
  %6 = alloca i32
  %7 = alloca i32
  %8 = alloca i32
  %9 = alloca i32
  %10 = alloca i32
  %11 = alloca i32
  %12 = alloca i32
  %13 = alloca i32
  %14 = alloca i32
  %15 = alloca i32
  %16 = alloca i32
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
  %35 = alloca i32
  store i32 0, ptr %1
  store i32 0, ptr %2
  store i32 0, ptr %3
  store i32 0, ptr %4
  store i32 0, ptr %5
  store i32 0, ptr %6
  store i32 0, ptr %7
  store i32 0, ptr %8
  store i32 0, ptr %9
  store i32 0, ptr %10
  store i32 0, ptr %11
  store i32 0, ptr %12
  store i32 0, ptr %13
  store i32 0, ptr %14
  store i32 0, ptr %15
  store i32 0, ptr %16
  store i16 0, ptr %17
  store i16 0, ptr %18
  store i16 0, ptr %19
  store i16 0, ptr %20
  call void @llvm.memset.p0.i16(ptr %21, i8 0, i16 256, i1 false)
  store i32 0, ptr %22
  store i16 0, ptr %23
  store i16 0, ptr %24
  store i16 0, ptr %25
  store i16 0, ptr %26
  call void @llvm.memset.p0.i16(ptr %27, i8 0, i16 256, i1 false)
  store i32 0, ptr %28
  store i16 0, ptr %29
  store i16 0, ptr %30
  store i16 0, ptr %31
  store i16 0, ptr %32
  call void @llvm.memset.p0.i16(ptr %33, i8 0, i16 256, i1 false)
  store i32 0, ptr %34
  store i32 0, ptr %35
  store i32 8, ptr %35, !tbaa !2
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
  %39 = sext i1 %38 to i8
  %40 = icmp ne i8 %39, 0
  br i1 %40, label %b3, label %b5

b3:
  %41 = load i16, ptr %30, !tbaa !2
  %42 = load i32, ptr %34, !tbaa !2
  %43 = sub i16 %41, 0
  %44 = getelementptr inbounds i32, ptr %33, i16 %43
  store i32 %42, ptr %44, !tbaa !2
  br label %b4

b4:
  %45 = load i16, ptr %30, !tbaa !2
  %46 = add i16 %45, 1
  store i16 %46, ptr %30, !tbaa !2
  br label %b2

b5:
  store i32 0, ptr %28, !tbaa !2
  store i16 64, ptr %25, !tbaa !2
  store i16 64, ptr %26, !tbaa !2
  store i16 0, ptr %24, !tbaa !2
  store i16 64, ptr %23, !tbaa !2
  br label %b6

b6:
  %47 = load i16, ptr %24, !tbaa !2
  %48 = load i16, ptr %23, !tbaa !2
  %49 = icmp slt i16 %47, %48
  %50 = sext i1 %49 to i8
  %51 = icmp ne i8 %50, 0
  br i1 %51, label %b7, label %b9

b7:
  %52 = load i16, ptr %24, !tbaa !2
  %53 = load i32, ptr %28, !tbaa !2
  %54 = sub i16 %52, 0
  %55 = getelementptr inbounds i32, ptr %27, i16 %54
  store i32 %53, ptr %55, !tbaa !2
  br label %b8

b8:
  %56 = load i16, ptr %24, !tbaa !2
  %57 = add i16 %56, 1
  store i16 %57, ptr %24, !tbaa !2
  br label %b6

b9:
  store i32 0, ptr %22, !tbaa !2
  store i16 64, ptr %19, !tbaa !2
  store i16 64, ptr %20, !tbaa !2
  store i16 0, ptr %18, !tbaa !2
  store i16 64, ptr %17, !tbaa !2
  br label %b10

b10:
  %58 = load i16, ptr %18, !tbaa !2
  %59 = load i16, ptr %17, !tbaa !2
  %60 = icmp slt i16 %58, %59
  %61 = sext i1 %60 to i8
  %62 = icmp ne i8 %61, 0
  br i1 %62, label %b11, label %b13

b11:
  %63 = load i16, ptr %18, !tbaa !2
  %64 = load i32, ptr %22, !tbaa !2
  %65 = sub i16 %63, 0
  %66 = getelementptr inbounds i32, ptr %21, i16 %65
  store i32 %64, ptr %66, !tbaa !2
  br label %b12

b12:
  %67 = load i16, ptr %18, !tbaa !2
  %68 = add i16 %67, 1
  store i16 %68, ptr %18, !tbaa !2
  br label %b10

b13:
  %69 = load i32, ptr %35, !tbaa !2
  store i32 0, ptr %16, !tbaa !2
  store i32 %69, ptr %15, !tbaa !2
  br label %b14

b14:
  %70 = load i32, ptr %16, !tbaa !2
  %71 = load i32, ptr %15, !tbaa !2
  %72 = icmp slt i32 %70, %71
  %73 = sext i1 %72 to i8
  %74 = icmp ne i8 %73, 0
  br i1 %74, label %b15, label %b17

b15:
  %75 = load i32, ptr %35, !tbaa !2
  store i32 0, ptr %14, !tbaa !2
  store i32 %75, ptr %13, !tbaa !2
  br label %b18

b16:
  %76 = load i32, ptr %16, !tbaa !2
  %77 = add i32 %76, 1
  store i32 %77, ptr %16, !tbaa !2
  br label %b14

b17:
  %78 = load i32, ptr %35, !tbaa !2
  store i32 0, ptr %12, !tbaa !2
  store i32 %78, ptr %11, !tbaa !2
  br label %b31

b18:
  %79 = load i32, ptr %14, !tbaa !2
  %80 = load i32, ptr %13, !tbaa !2
  %81 = icmp slt i32 %79, %80
  %82 = sext i1 %81 to i8
  %83 = icmp ne i8 %82, 0
  br i1 %83, label %b19, label %b21

b19:
  %84 = load i32, ptr %16, !tbaa !2
  %85 = mul i32 %84, 8
  %86 = load i32, ptr %14, !tbaa !2
  %87 = add i32 %85, %86
  %88 = icmp ult i32 %87, 64
  %89 = sext i1 %88 to i8
  %90 = icmp ne i8 %89, 0
  br i1 %90, label %b22, label %b23

b20:
  %91 = load i32, ptr %14, !tbaa !2
  %92 = add i32 %91, 1
  store i32 %92, ptr %14, !tbaa !2
  br label %b18

b21:
  br label %b16

b22:
  %93 = load i32, ptr %16, !tbaa !2
  %94 = mul i32 %93, 3
  %95 = load i32, ptr %14, !tbaa !2
  %96 = add i32 %94, %95
  %97 = add i32 %96, 1
  %98 = add i32 %97, %0
  %99 = trunc i32 %87 to i16
  %100 = sub i16 %99, 0
  %101 = getelementptr inbounds i32, ptr %33, i16 %100
  store i32 %98, ptr %101, !tbaa !2
  %102 = load i32, ptr %16, !tbaa !2
  %103 = load i32, ptr %14, !tbaa !2
  %104 = icmp eq i32 %102, %103
  %105 = sext i1 %104 to i8
  %106 = icmp ne i8 %105, 0
  br i1 %106, label %b24, label %b25

b23:
  call addrspace(1) void @N$EBND()
  unreachable

b24:
  %107 = load i32, ptr %16, !tbaa !2
  %108 = mul i32 %107, 8
  %109 = load i32, ptr %14, !tbaa !2
  %110 = add i32 %108, %109
  %111 = icmp ult i32 %110, 64
  %112 = sext i1 %111 to i8
  %113 = icmp ne i8 %112, 0
  br i1 %113, label %b27, label %b28

b25:
  %114 = load i32, ptr %16, !tbaa !2
  %115 = mul i32 %114, 8
  %116 = load i32, ptr %14, !tbaa !2
  %117 = add i32 %115, %116
  %118 = icmp ult i32 %117, 64
  %119 = sext i1 %118 to i8
  %120 = icmp ne i8 %119, 0
  br i1 %120, label %b29, label %b30

b26:
  br label %b20

b27:
  %121 = trunc i32 %110 to i16
  %122 = sub i16 %121, 0
  %123 = getelementptr inbounds i32, ptr %27, i16 %122
  store i32 2, ptr %123, !tbaa !2
  br label %b26

b28:
  call addrspace(1) void @N$EBND()
  unreachable

b29:
  %124 = load i32, ptr %16, !tbaa !2
  %125 = load i32, ptr %14, !tbaa !2
  %126 = add i32 %124, %125
  %127 = srem i32 %126, 3
  %128 = trunc i32 %117 to i16
  %129 = sub i16 %128, 0
  %130 = getelementptr inbounds i32, ptr %27, i16 %129
  store i32 %127, ptr %130, !tbaa !2
  br label %b26

b30:
  call addrspace(1) void @N$EBND()
  unreachable

b31:
  %131 = load i32, ptr %12, !tbaa !2
  %132 = load i32, ptr %11, !tbaa !2
  %133 = icmp slt i32 %131, %132
  %134 = sext i1 %133 to i8
  %135 = icmp ne i8 %134, 0
  br i1 %135, label %b32, label %b34

b32:
  %136 = load i32, ptr %35, !tbaa !2
  store i32 0, ptr %10, !tbaa !2
  store i32 %136, ptr %9, !tbaa !2
  br label %b35

b33:
  %137 = load i32, ptr %12, !tbaa !2
  %138 = add i32 %137, 1
  store i32 %138, ptr %12, !tbaa !2
  br label %b31

b34:
  store i32 0, ptr %5, !tbaa !2
  %139 = load i32, ptr %35, !tbaa !2
  store i32 0, ptr %4, !tbaa !2
  store i32 %139, ptr %3, !tbaa !2
  br label %b49

b35:
  %140 = load i32, ptr %10, !tbaa !2
  %141 = load i32, ptr %9, !tbaa !2
  %142 = icmp slt i32 %140, %141
  %143 = sext i1 %142 to i8
  %144 = icmp ne i8 %143, 0
  br i1 %144, label %b36, label %b38

b36:
  store i32 0, ptr %8, !tbaa !2
  %145 = load i32, ptr %35, !tbaa !2
  store i32 0, ptr %7, !tbaa !2
  store i32 %145, ptr %6, !tbaa !2
  br label %b39

b37:
  %146 = load i32, ptr %10, !tbaa !2
  %147 = add i32 %146, 1
  store i32 %147, ptr %10, !tbaa !2
  br label %b35

b38:
  br label %b33

b39:
  %148 = load i32, ptr %7, !tbaa !2
  %149 = load i32, ptr %6, !tbaa !2
  %150 = icmp slt i32 %148, %149
  %151 = sext i1 %150 to i8
  %152 = icmp ne i8 %151, 0
  br i1 %152, label %b40, label %b42

b40:
  %153 = load i32, ptr %8, !tbaa !2
  %154 = load i32, ptr %12, !tbaa !2
  %155 = mul i32 %154, 8
  %156 = load i32, ptr %7, !tbaa !2
  %157 = add i32 %155, %156
  %158 = icmp ult i32 %157, 64
  %159 = sext i1 %158 to i8
  %160 = icmp ne i8 %159, 0
  br i1 %160, label %b43, label %b44

b41:
  %161 = load i32, ptr %7, !tbaa !2
  %162 = add i32 %161, 1
  store i32 %162, ptr %7, !tbaa !2
  br label %b39

b42:
  %163 = load i32, ptr %12, !tbaa !2
  %164 = mul i32 %163, 8
  %165 = load i32, ptr %10, !tbaa !2
  %166 = add i32 %164, %165
  %167 = icmp ult i32 %166, 64
  %168 = sext i1 %167 to i8
  %169 = icmp ne i8 %168, 0
  br i1 %169, label %b47, label %b48

b43:
  %170 = trunc i32 %157 to i16
  %171 = sub i16 %170, 0
  %172 = getelementptr inbounds i32, ptr %33, i16 %171
  %173 = load i32, ptr %172, !tbaa !2
  %174 = load i32, ptr %7, !tbaa !2
  %175 = mul i32 %174, 8
  %176 = load i32, ptr %10, !tbaa !2
  %177 = add i32 %175, %176
  %178 = icmp ult i32 %177, 64
  %179 = sext i1 %178 to i8
  %180 = icmp ne i8 %179, 0
  br i1 %180, label %b45, label %b46

b44:
  call addrspace(1) void @N$EBND()
  unreachable

b45:
  %181 = trunc i32 %177 to i16
  %182 = sub i16 %181, 0
  %183 = getelementptr inbounds i32, ptr %27, i16 %182
  %184 = load i32, ptr %183, !tbaa !2
  %185 = mul i32 %173, %184
  %186 = add i32 %153, %185
  store i32 %186, ptr %8, !tbaa !2
  br label %b41

b46:
  call addrspace(1) void @N$EBND()
  unreachable

b47:
  %187 = load i32, ptr %8, !tbaa !2
  %188 = trunc i32 %166 to i16
  %189 = sub i16 %188, 0
  %190 = getelementptr inbounds i32, ptr %21, i16 %189
  store i32 %187, ptr %190, !tbaa !2
  br label %b37

b48:
  call addrspace(1) void @N$EBND()
  unreachable

b49:
  %191 = load i32, ptr %4, !tbaa !2
  %192 = load i32, ptr %3, !tbaa !2
  %193 = icmp slt i32 %191, %192
  %194 = sext i1 %193 to i8
  %195 = icmp ne i8 %194, 0
  br i1 %195, label %b50, label %b52

b50:
  %196 = load i32, ptr %35, !tbaa !2
  store i32 0, ptr %2, !tbaa !2
  store i32 %196, ptr %1, !tbaa !2
  br label %b53

b51:
  %197 = load i32, ptr %4, !tbaa !2
  %198 = add i32 %197, 1
  store i32 %198, ptr %4, !tbaa !2
  br label %b49

b52:
  %199 = load i32, ptr %5, !tbaa !2
  ret i32 %199

b53:
  %200 = load i32, ptr %2, !tbaa !2
  %201 = load i32, ptr %1, !tbaa !2
  %202 = icmp slt i32 %200, %201
  %203 = sext i1 %202 to i8
  %204 = icmp ne i8 %203, 0
  br i1 %204, label %b54, label %b56

b54:
  %205 = load i32, ptr %5, !tbaa !2
  %206 = load i32, ptr %4, !tbaa !2
  %207 = mul i32 %206, 8
  %208 = load i32, ptr %2, !tbaa !2
  %209 = add i32 %207, %208
  %210 = icmp ult i32 %209, 64
  %211 = sext i1 %210 to i8
  %212 = icmp ne i8 %211, 0
  br i1 %212, label %b57, label %b58

b55:
  %213 = load i32, ptr %2, !tbaa !2
  %214 = add i32 %213, 1
  store i32 %214, ptr %2, !tbaa !2
  br label %b53

b56:
  br label %b51

b57:
  %215 = trunc i32 %209 to i16
  %216 = sub i16 %215, 0
  %217 = getelementptr inbounds i32, ptr %21, i16 %216
  %218 = load i32, ptr %217, !tbaa !2
  %219 = load i32, ptr %4, !tbaa !2
  %220 = mul i32 %219, 8
  %221 = load i32, ptr %2, !tbaa !2
  %222 = add i32 %220, %221
  %223 = add i32 %222, 1
  %224 = mul i32 %218, %223
  %225 = add i32 %205, %224
  store i32 %225, ptr %5, !tbaa !2
  br label %b55

b58:
  call addrspace(1) void @N$EBND()
  unreachable
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca i32
  store i32 0, ptr %0
  %1 = call addrspace(1) i32 @matmul(i32 1)
  store i32 %1, ptr %0, !tbaa !2
  %2 = load i32, ptr %0, !tbaa !2
  %3 = icmp eq i32 %2, 372432
  %4 = sext i1 %3 to i8
  %5 = icmp ne i8 %4, 0
  br i1 %5, label %b2, label %b3

b2:
  %6 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %6)
  %7 = load i32, ptr %0, !tbaa !2
  call addrspace(1) void @N$PI4(i32 %7)
  call addrspace(1) void @N$PN()
  ret i16 0

b3:
  br label %b4

b4:
  %8 = getelementptr i8, ptr @$str2, i16 6
  call addrspace(1) void @N$PS(ptr %8)
  %9 = load i32, ptr %0, !tbaa !2
  call addrspace(1) void @N$PI4(i32 %9)
  call addrspace(1) void @N$PN()
  ret i16 1
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$EBND() addrspace(1)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PI4(i32) addrspace(1)

declare void @N$PN() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
