target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [15 x i8] c"\08\00\08\00\08\00matmul: \00"

define internal i32 @matmul(i32 %0) addrspace(1) {
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
  store i16 0, ptr %1
  store i16 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %4
  store i32 0, ptr %5
  store i16 0, ptr %6
  store i16 0, ptr %7
  store i32 0, ptr %8
  store i16 0, ptr %9
  store i16 0, ptr %10
  store i16 0, ptr %11
  store i16 0, ptr %12
  store i16 0, ptr %13
  store i16 0, ptr %14
  store i16 0, ptr %15
  store i16 0, ptr %16
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
  store i16 0, ptr %35
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
  %69 = load i16, ptr %35, !tbaa !2
  store i16 0, ptr %16, !tbaa !2
  store i16 %69, ptr %15, !tbaa !2
  br label %b14

b14:
  %70 = load i16, ptr %16, !tbaa !2
  %71 = load i16, ptr %15, !tbaa !2
  %72 = icmp slt i16 %70, %71
  %73 = sext i1 %72 to i8
  %74 = icmp ne i8 %73, 0
  br i1 %74, label %b15, label %b17

b15:
  %75 = load i16, ptr %35, !tbaa !2
  store i16 0, ptr %14, !tbaa !2
  store i16 %75, ptr %13, !tbaa !2
  br label %b18

b16:
  %76 = load i16, ptr %16, !tbaa !2
  %77 = add i16 %76, 1
  store i16 %77, ptr %16, !tbaa !2
  br label %b14

b17:
  %78 = load i16, ptr %35, !tbaa !2
  store i16 0, ptr %12, !tbaa !2
  store i16 %78, ptr %11, !tbaa !2
  br label %b31

b18:
  %79 = load i16, ptr %14, !tbaa !2
  %80 = load i16, ptr %13, !tbaa !2
  %81 = icmp slt i16 %79, %80
  %82 = sext i1 %81 to i8
  %83 = icmp ne i8 %82, 0
  br i1 %83, label %b19, label %b21

b19:
  %84 = load i16, ptr %16, !tbaa !2
  %85 = mul i16 %84, 8
  %86 = load i16, ptr %14, !tbaa !2
  %87 = add i16 %85, %86
  %88 = icmp ult i16 %87, 64
  %89 = sext i1 %88 to i8
  %90 = icmp ne i8 %89, 0
  br i1 %90, label %b22, label %b23

b20:
  %91 = load i16, ptr %14, !tbaa !2
  %92 = add i16 %91, 1
  store i16 %92, ptr %14, !tbaa !2
  br label %b18

b21:
  br label %b16

b22:
  %93 = load i16, ptr %16, !tbaa !2
  %94 = mul i16 %93, 3
  %95 = load i16, ptr %14, !tbaa !2
  %96 = add i16 %94, %95
  %97 = add i16 %96, 1
  %98 = sext i16 %97 to i32
  %99 = add i32 %98, %0
  %100 = zext i8 8 to i32
  %101 = shl i32 %99, %100
  %102 = sext i32 %101 to i64
  %103 = sext i32 1024 to i64
  %104 = shl i64 %102, 8
  %105 = sdiv i64 %104, %103
  %106 = trunc i64 %105 to i32
  %107 = sub i16 %87, 0
  %108 = getelementptr inbounds i32, ptr %33, i16 %107
  store i32 %106, ptr %108, !tbaa !2
  %109 = load i16, ptr %16, !tbaa !2
  %110 = load i16, ptr %14, !tbaa !2
  %111 = icmp eq i16 %109, %110
  %112 = sext i1 %111 to i8
  %113 = icmp ne i8 %112, 0
  br i1 %113, label %b24, label %b25

b23:
  call addrspace(1) void @N$EBND()
  unreachable

b24:
  %114 = load i16, ptr %16, !tbaa !2
  %115 = mul i16 %114, 8
  %116 = load i16, ptr %14, !tbaa !2
  %117 = add i16 %115, %116
  %118 = icmp ult i16 %117, 64
  %119 = sext i1 %118 to i8
  %120 = icmp ne i8 %119, 0
  br i1 %120, label %b27, label %b28

b25:
  %121 = load i16, ptr %16, !tbaa !2
  %122 = mul i16 %121, 8
  %123 = load i16, ptr %14, !tbaa !2
  %124 = add i16 %122, %123
  %125 = icmp ult i16 %124, 64
  %126 = sext i1 %125 to i8
  %127 = icmp ne i8 %126, 0
  br i1 %127, label %b29, label %b30

b26:
  br label %b20

b27:
  %128 = sub i16 %117, 0
  %129 = getelementptr inbounds i32, ptr %27, i16 %128
  store i32 512, ptr %129, !tbaa !2
  br label %b26

b28:
  call addrspace(1) void @N$EBND()
  unreachable

b29:
  %130 = load i16, ptr %16, !tbaa !2
  %131 = load i16, ptr %14, !tbaa !2
  %132 = add i16 %130, %131
  %133 = srem i16 %132, 3
  %134 = sext i16 %133 to i32
  %135 = zext i8 8 to i32
  %136 = shl i32 %134, %135
  %137 = sext i32 %136 to i64
  %138 = sext i32 512 to i64
  %139 = shl i64 %137, 8
  %140 = sdiv i64 %139, %138
  %141 = trunc i64 %140 to i32
  %142 = sub i16 %124, 0
  %143 = getelementptr inbounds i32, ptr %27, i16 %142
  store i32 %141, ptr %143, !tbaa !2
  br label %b26

b30:
  call addrspace(1) void @N$EBND()
  unreachable

b31:
  %144 = load i16, ptr %12, !tbaa !2
  %145 = load i16, ptr %11, !tbaa !2
  %146 = icmp slt i16 %144, %145
  %147 = sext i1 %146 to i8
  %148 = icmp ne i8 %147, 0
  br i1 %148, label %b32, label %b34

b32:
  %149 = load i16, ptr %35, !tbaa !2
  store i16 0, ptr %10, !tbaa !2
  store i16 %149, ptr %9, !tbaa !2
  br label %b35

b33:
  %150 = load i16, ptr %12, !tbaa !2
  %151 = add i16 %150, 1
  store i16 %151, ptr %12, !tbaa !2
  br label %b31

b34:
  store i32 0, ptr %5, !tbaa !2
  %152 = load i16, ptr %35, !tbaa !2
  store i16 0, ptr %4, !tbaa !2
  store i16 %152, ptr %3, !tbaa !2
  br label %b49

b35:
  %153 = load i16, ptr %10, !tbaa !2
  %154 = load i16, ptr %9, !tbaa !2
  %155 = icmp slt i16 %153, %154
  %156 = sext i1 %155 to i8
  %157 = icmp ne i8 %156, 0
  br i1 %157, label %b36, label %b38

b36:
  store i32 0, ptr %8, !tbaa !2
  %158 = load i16, ptr %35, !tbaa !2
  store i16 0, ptr %7, !tbaa !2
  store i16 %158, ptr %6, !tbaa !2
  br label %b39

b37:
  %159 = load i16, ptr %10, !tbaa !2
  %160 = add i16 %159, 1
  store i16 %160, ptr %10, !tbaa !2
  br label %b35

b38:
  br label %b33

b39:
  %161 = load i16, ptr %7, !tbaa !2
  %162 = load i16, ptr %6, !tbaa !2
  %163 = icmp slt i16 %161, %162
  %164 = sext i1 %163 to i8
  %165 = icmp ne i8 %164, 0
  br i1 %165, label %b40, label %b42

b40:
  %166 = load i32, ptr %8, !tbaa !2
  %167 = load i16, ptr %12, !tbaa !2
  %168 = mul i16 %167, 8
  %169 = load i16, ptr %7, !tbaa !2
  %170 = add i16 %168, %169
  %171 = icmp ult i16 %170, 64
  %172 = sext i1 %171 to i8
  %173 = icmp ne i8 %172, 0
  br i1 %173, label %b43, label %b44

b41:
  %174 = load i16, ptr %7, !tbaa !2
  %175 = add i16 %174, 1
  store i16 %175, ptr %7, !tbaa !2
  br label %b39

b42:
  %176 = load i16, ptr %12, !tbaa !2
  %177 = mul i16 %176, 8
  %178 = load i16, ptr %10, !tbaa !2
  %179 = add i16 %177, %178
  %180 = icmp ult i16 %179, 64
  %181 = sext i1 %180 to i8
  %182 = icmp ne i8 %181, 0
  br i1 %182, label %b47, label %b48

b43:
  %183 = sub i16 %170, 0
  %184 = getelementptr inbounds i32, ptr %33, i16 %183
  %185 = load i32, ptr %184, !tbaa !2
  %186 = load i16, ptr %7, !tbaa !2
  %187 = mul i16 %186, 8
  %188 = load i16, ptr %10, !tbaa !2
  %189 = add i16 %187, %188
  %190 = icmp ult i16 %189, 64
  %191 = sext i1 %190 to i8
  %192 = icmp ne i8 %191, 0
  br i1 %192, label %b45, label %b46

b44:
  call addrspace(1) void @N$EBND()
  unreachable

b45:
  %193 = sub i16 %189, 0
  %194 = getelementptr inbounds i32, ptr %27, i16 %193
  %195 = load i32, ptr %194, !tbaa !2
  %196 = sext i32 %185 to i64
  %197 = sext i32 %195 to i64
  %198 = mul i64 %196, %197
  %199 = ashr i64 %198, 8
  %200 = trunc i64 %199 to i32
  %201 = add i32 %166, %200
  store i32 %201, ptr %8, !tbaa !2
  br label %b41

b46:
  call addrspace(1) void @N$EBND()
  unreachable

b47:
  %202 = load i32, ptr %8, !tbaa !2
  %203 = sub i16 %179, 0
  %204 = getelementptr inbounds i32, ptr %21, i16 %203
  store i32 %202, ptr %204, !tbaa !2
  br label %b37

b48:
  call addrspace(1) void @N$EBND()
  unreachable

b49:
  %205 = load i16, ptr %4, !tbaa !2
  %206 = load i16, ptr %3, !tbaa !2
  %207 = icmp slt i16 %205, %206
  %208 = sext i1 %207 to i8
  %209 = icmp ne i8 %208, 0
  br i1 %209, label %b50, label %b52

b50:
  %210 = load i16, ptr %35, !tbaa !2
  store i16 0, ptr %2, !tbaa !2
  store i16 %210, ptr %1, !tbaa !2
  br label %b53

b51:
  %211 = load i16, ptr %4, !tbaa !2
  %212 = add i16 %211, 1
  store i16 %212, ptr %4, !tbaa !2
  br label %b49

b52:
  %213 = load i32, ptr %5, !tbaa !2
  ret i32 %213

b53:
  %214 = load i16, ptr %2, !tbaa !2
  %215 = load i16, ptr %1, !tbaa !2
  %216 = icmp slt i16 %214, %215
  %217 = sext i1 %216 to i8
  %218 = icmp ne i8 %217, 0
  br i1 %218, label %b54, label %b56

b54:
  %219 = load i32, ptr %5, !tbaa !2
  %220 = load i16, ptr %4, !tbaa !2
  %221 = mul i16 %220, 8
  %222 = load i16, ptr %2, !tbaa !2
  %223 = add i16 %221, %222
  %224 = icmp ult i16 %223, 64
  %225 = sext i1 %224 to i8
  %226 = icmp ne i8 %225, 0
  br i1 %226, label %b57, label %b58

b55:
  %227 = load i16, ptr %2, !tbaa !2
  %228 = add i16 %227, 1
  store i16 %228, ptr %2, !tbaa !2
  br label %b53

b56:
  br label %b51

b57:
  %229 = sub i16 %223, 0
  %230 = getelementptr inbounds i32, ptr %21, i16 %229
  %231 = load i32, ptr %230, !tbaa !2
  %232 = load i16, ptr %4, !tbaa !2
  %233 = mul i16 %232, 8
  %234 = load i16, ptr %2, !tbaa !2
  %235 = add i16 %233, %234
  %236 = add i16 %235, 1
  %237 = sext i16 %236 to i32
  %238 = zext i8 8 to i32
  %239 = shl i32 %237, %238
  %240 = sext i32 %231 to i64
  %241 = sext i32 %239 to i64
  %242 = mul i64 %240, %241
  %243 = ashr i64 %242, 8
  %244 = trunc i64 %243 to i32
  %245 = add i32 %219, %244
  store i32 %245, ptr %5, !tbaa !2
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
  %2 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %2)
  %3 = load i32, ptr %0, !tbaa !2
  call addrspace(1) void @N$PQ4(i32 %3, i8 8)
  call addrspace(1) void @N$PN()
  ret i16 0
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$EBND() addrspace(1)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PQ4(i32, i8) addrspace(1)

declare void @N$PN() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
