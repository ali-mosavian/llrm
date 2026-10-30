target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [10 x i8] c"\08\00\03\00\03\00PX=\00"
@$str2 = internal constant [10 x i8] c"\08\00\03\00\03\00PY=\00"
@$str3 = internal constant [10 x i8] c"\08\00\03\00\03\00VX=\00"
@$str4 = internal constant [10 x i8] c"\08\00\03\00\03\00VY=\00"
@$str5 = internal constant [11 x i8] c"\08\00\04\00\04\00DONE\00"

define internal cc 1005 i32 @nbody(i32 %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i32
  %4 = alloca i32
  %5 = alloca [8 x i8]
  %6 = alloca i16
  %7 = alloca [8 x i8]
  %8 = alloca i16
  %9 = alloca i32
  %10 = alloca i32
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca [96 x i8]
  store i16 6, ptr %11, !tbaa !2
  store i16 6, ptr %12, !tbaa !2
  %14 = getelementptr inbounds [16 x i8], ptr %13, i16 0
  store i32 -7680, ptr %14, !tbaa !2
  %15 = getelementptr inbounds [16 x i8], ptr %13, i16 0
  %16 = getelementptr inbounds i8, ptr %15, i16 4
  store i32 -6144, ptr %16, !tbaa !2
  %17 = getelementptr inbounds [16 x i8], ptr %13, i16 0
  %18 = getelementptr inbounds i8, ptr %17, i16 8
  store i32 0, ptr %18, !tbaa !2
  %19 = getelementptr inbounds [16 x i8], ptr %13, i16 0
  %20 = getelementptr inbounds i8, ptr %19, i16 12
  store i32 0, ptr %20, !tbaa !2
  %21 = getelementptr inbounds [16 x i8], ptr %13, i16 1
  store i32 -4096, ptr %21, !tbaa !2
  %22 = getelementptr inbounds [16 x i8], ptr %13, i16 1
  %23 = getelementptr inbounds i8, ptr %22, i16 4
  store i32 -3584, ptr %23, !tbaa !2
  %24 = getelementptr inbounds [16 x i8], ptr %13, i16 1
  %25 = getelementptr inbounds i8, ptr %24, i16 8
  store i32 0, ptr %25, !tbaa !2
  %26 = getelementptr inbounds [16 x i8], ptr %13, i16 1
  %27 = getelementptr inbounds i8, ptr %26, i16 12
  store i32 0, ptr %27, !tbaa !2
  %28 = getelementptr inbounds [16 x i8], ptr %13, i16 2
  store i32 -512, ptr %28, !tbaa !2
  %29 = getelementptr inbounds [16 x i8], ptr %13, i16 2
  %30 = getelementptr inbounds i8, ptr %29, i16 4
  store i32 -1024, ptr %30, !tbaa !2
  %31 = getelementptr inbounds [16 x i8], ptr %13, i16 2
  %32 = getelementptr inbounds i8, ptr %31, i16 8
  store i32 0, ptr %32, !tbaa !2
  %33 = getelementptr inbounds [16 x i8], ptr %13, i16 2
  %34 = getelementptr inbounds i8, ptr %33, i16 12
  store i32 0, ptr %34, !tbaa !2
  %35 = getelementptr inbounds [16 x i8], ptr %13, i16 3
  store i32 3072, ptr %35, !tbaa !2
  %36 = getelementptr inbounds [16 x i8], ptr %13, i16 3
  %37 = getelementptr inbounds i8, ptr %36, i16 4
  store i32 1536, ptr %37, !tbaa !2
  %38 = getelementptr inbounds [16 x i8], ptr %13, i16 3
  %39 = getelementptr inbounds i8, ptr %38, i16 8
  store i32 0, ptr %39, !tbaa !2
  %40 = getelementptr inbounds [16 x i8], ptr %13, i16 3
  %41 = getelementptr inbounds i8, ptr %40, i16 12
  store i32 0, ptr %41, !tbaa !2
  %42 = getelementptr inbounds [16 x i8], ptr %13, i16 4
  store i32 6656, ptr %42, !tbaa !2
  %43 = getelementptr inbounds [16 x i8], ptr %13, i16 4
  %44 = getelementptr inbounds i8, ptr %43, i16 4
  store i32 4096, ptr %44, !tbaa !2
  %45 = getelementptr inbounds [16 x i8], ptr %13, i16 4
  %46 = getelementptr inbounds i8, ptr %45, i16 8
  store i32 0, ptr %46, !tbaa !2
  %47 = getelementptr inbounds [16 x i8], ptr %13, i16 4
  %48 = getelementptr inbounds i8, ptr %47, i16 12
  store i32 0, ptr %48, !tbaa !2
  %49 = getelementptr inbounds [16 x i8], ptr %13, i16 5
  store i32 10240, ptr %49, !tbaa !2
  %50 = getelementptr inbounds [16 x i8], ptr %13, i16 5
  %51 = getelementptr inbounds i8, ptr %50, i16 4
  store i32 6656, ptr %51, !tbaa !2
  %52 = getelementptr inbounds [16 x i8], ptr %13, i16 5
  %53 = getelementptr inbounds i8, ptr %52, i16 8
  store i32 0, ptr %53, !tbaa !2
  %54 = getelementptr inbounds [16 x i8], ptr %13, i16 5
  %55 = getelementptr inbounds i8, ptr %54, i16 12
  store i32 0, ptr %55, !tbaa !2
  store i32 0, ptr %10, !tbaa !2
  store i32 %0, ptr %9, !tbaa !2
  br label %b2

b2:
  %56 = load i32, ptr %10, !tbaa !2
  %57 = load i32, ptr %9, !tbaa !2
  %58 = icmp slt i32 %56, %57
  br i1 %58, label %b3, label %b5

b3:
  store i16 0, ptr %8, !tbaa !2
  br label %b6

b4:
  %59 = load i32, ptr %10, !tbaa !2
  %60 = add nsw i32 %59, 1
  store i32 %60, ptr %10, !tbaa !2
  br label %b2

b5:
  store i16 0, ptr %1, !tbaa !2
  br label %b21

b6:
  %61 = load i16, ptr %8, !tbaa !2
  %62 = icmp ult i16 %61, 6
  br i1 %62, label %b7, label %b9

b7:
  store i32 0, ptr %7, !tbaa !2
  %63 = getelementptr inbounds i8, ptr %7, i16 4
  store i32 0, ptr %63, !tbaa !2
  store i16 0, ptr %6, !tbaa !2
  br label %b10

b8:
  %64 = load i16, ptr %8, !tbaa !2
  %65 = add i16 %64, 1
  store i16 %65, ptr %8, !tbaa !2
  br label %b6

b9:
  store i16 0, ptr %2, !tbaa !2
  br label %b17

b10:
  %66 = load i16, ptr %6, !tbaa !2
  %67 = icmp ult i16 %66, 6
  br i1 %67, label %b11, label %b13

b11:
  %68 = icmp ne i16 %61, %66
  br i1 %68, label %b14, label %b15

b12:
  %69 = load i16, ptr %6, !tbaa !2
  %70 = add i16 %69, 1
  store i16 %70, ptr %6, !tbaa !2
  br label %b10

b13:
  %71 = sub i16 %61, 0
  %72 = getelementptr inbounds [16 x i8], ptr %13, i16 %71
  %73 = getelementptr inbounds i8, ptr %72, i16 8
  %74 = load i32, ptr %73, !tbaa !2
  %75 = load i32, ptr %7, !tbaa !2
  %76 = add i32 %74, %75
  %77 = sub i16 %61, 0
  %78 = getelementptr inbounds [16 x i8], ptr %13, i16 %77
  %79 = getelementptr inbounds i8, ptr %78, i16 8
  store i32 %76, ptr %79, !tbaa !2
  %80 = sub i16 %61, 0
  %81 = getelementptr inbounds [16 x i8], ptr %13, i16 %80
  %82 = getelementptr inbounds i8, ptr %81, i16 12
  %83 = load i32, ptr %82, !tbaa !2
  %84 = getelementptr inbounds i8, ptr %7, i16 4
  %85 = load i32, ptr %84, !tbaa !2
  %86 = add i32 %83, %85
  %87 = sub i16 %61, 0
  %88 = getelementptr inbounds [16 x i8], ptr %13, i16 %87
  %89 = getelementptr inbounds i8, ptr %88, i16 12
  store i32 %86, ptr %89, !tbaa !2
  %90 = sub i16 %61, 0
  %91 = getelementptr inbounds [16 x i8], ptr %13, i16 %90
  %92 = getelementptr inbounds i8, ptr %91, i16 8
  %93 = load i32, ptr %92, !tbaa !2
  %94 = sub i16 %61, 0
  %95 = getelementptr inbounds [16 x i8], ptr %13, i16 %94
  %96 = getelementptr inbounds i8, ptr %95, i16 8
  %97 = load i32, ptr %96, !tbaa !2
  %98 = sext i32 %97 to i64
  %99 = shl i64 %98, 9
  %100 = sdiv i64 %99, 8192
  %101 = trunc i64 %100 to i32
  %102 = sub i32 %93, %101
  %103 = sub i16 %61, 0
  %104 = getelementptr inbounds [16 x i8], ptr %13, i16 %103
  %105 = getelementptr inbounds i8, ptr %104, i16 8
  store i32 %102, ptr %105, !tbaa !2
  %106 = sub i16 %61, 0
  %107 = getelementptr inbounds [16 x i8], ptr %13, i16 %106
  %108 = getelementptr inbounds i8, ptr %107, i16 12
  %109 = load i32, ptr %108, !tbaa !2
  %110 = sub i16 %61, 0
  %111 = getelementptr inbounds [16 x i8], ptr %13, i16 %110
  %112 = getelementptr inbounds i8, ptr %111, i16 12
  %113 = load i32, ptr %112, !tbaa !2
  %114 = sext i32 %113 to i64
  %115 = shl i64 %114, 9
  %116 = sdiv i64 %115, 8192
  %117 = trunc i64 %116 to i32
  %118 = sub i32 %109, %117
  %119 = sub i16 %61, 0
  %120 = getelementptr inbounds [16 x i8], ptr %13, i16 %119
  %121 = getelementptr inbounds i8, ptr %120, i16 12
  store i32 %118, ptr %121, !tbaa !2
  br label %b8

b14:
  %122 = sub i16 %66, 0
  %123 = getelementptr inbounds [16 x i8], ptr %13, i16 %122
  %124 = load i32, ptr %123, !tbaa !2
  %125 = sub i16 %61, 0
  %126 = getelementptr inbounds [16 x i8], ptr %13, i16 %125
  %127 = load i32, ptr %126, !tbaa !2
  %128 = sub i32 %124, %127
  %129 = sub i16 %66, 0
  %130 = getelementptr inbounds [16 x i8], ptr %13, i16 %129
  %131 = getelementptr inbounds i8, ptr %130, i16 4
  %132 = load i32, ptr %131, !tbaa !2
  %133 = sub i16 %61, 0
  %134 = getelementptr inbounds [16 x i8], ptr %13, i16 %133
  %135 = getelementptr inbounds i8, ptr %134, i16 4
  %136 = load i32, ptr %135, !tbaa !2
  %137 = sub i32 %132, %136
  store i32 %128, ptr %5, !tbaa !2
  %138 = getelementptr inbounds i8, ptr %5, i16 4
  store i32 %137, ptr %138, !tbaa !2
  %139 = load i32, ptr %5, !tbaa !2
  %140 = load i32, ptr %5, !tbaa !2
  %141 = call i32 @llvm.smul.fix.i32(i32 %139, i32 %140, i32 9)
  %142 = getelementptr inbounds i8, ptr %5, i16 4
  %143 = load i32, ptr %142, !tbaa !2
  %144 = getelementptr inbounds i8, ptr %5, i16 4
  %145 = load i32, ptr %144, !tbaa !2
  %146 = call i32 @llvm.smul.fix.i32(i32 %143, i32 %145, i32 9)
  %147 = add i32 %141, %146
  %148 = add i32 %147, 512
  store i32 %148, ptr %4, !tbaa !2
  %149 = load i32, ptr %4, !tbaa !2
  %150 = sext i32 %149 to i64
  %151 = sdiv i64 262144, %150
  %152 = trunc i64 %151 to i32
  store i32 %152, ptr %3, !tbaa !2
  %153 = load i32, ptr %7, !tbaa !2
  %154 = load i32, ptr %5, !tbaa !2
  %155 = load i32, ptr %3, !tbaa !2
  %156 = call i32 @llvm.smul.fix.i32(i32 %154, i32 %155, i32 9)
  %157 = add i32 %153, %156
  store i32 %157, ptr %7, !tbaa !2
  %158 = getelementptr inbounds i8, ptr %7, i16 4
  %159 = load i32, ptr %158, !tbaa !2
  %160 = getelementptr inbounds i8, ptr %5, i16 4
  %161 = load i32, ptr %160, !tbaa !2
  %162 = load i32, ptr %3, !tbaa !2
  %163 = call i32 @llvm.smul.fix.i32(i32 %161, i32 %162, i32 9)
  %164 = add i32 %159, %163
  %165 = getelementptr inbounds i8, ptr %7, i16 4
  store i32 %164, ptr %165, !tbaa !2
  br label %b16

b15:
  br label %b16

b16:
  br label %b12

b17:
  %166 = load i16, ptr %2, !tbaa !2
  %167 = icmp ult i16 %166, 6
  br i1 %167, label %b18, label %b20

b18:
  %168 = sub i16 %166, 0
  %169 = getelementptr inbounds [16 x i8], ptr %13, i16 %168
  %170 = load i32, ptr %169, !tbaa !2
  %171 = sub i16 %166, 0
  %172 = getelementptr inbounds [16 x i8], ptr %13, i16 %171
  %173 = getelementptr inbounds i8, ptr %172, i16 8
  %174 = load i32, ptr %173, !tbaa !2
  %175 = add i32 %170, %174
  %176 = sub i16 %166, 0
  %177 = getelementptr inbounds [16 x i8], ptr %13, i16 %176
  store i32 %175, ptr %177, !tbaa !2
  %178 = sub i16 %166, 0
  %179 = getelementptr inbounds [16 x i8], ptr %13, i16 %178
  %180 = getelementptr inbounds i8, ptr %179, i16 4
  %181 = load i32, ptr %180, !tbaa !2
  %182 = sub i16 %166, 0
  %183 = getelementptr inbounds [16 x i8], ptr %13, i16 %182
  %184 = getelementptr inbounds i8, ptr %183, i16 12
  %185 = load i32, ptr %184, !tbaa !2
  %186 = add i32 %181, %185
  %187 = sub i16 %166, 0
  %188 = getelementptr inbounds [16 x i8], ptr %13, i16 %187
  %189 = getelementptr inbounds i8, ptr %188, i16 4
  store i32 %186, ptr %189, !tbaa !2
  br label %b19

b19:
  %190 = load i16, ptr %2, !tbaa !2
  %191 = add i16 %190, 1
  store i16 %191, ptr %2, !tbaa !2
  br label %b17

b20:
  br label %b4

b21:
  %192 = load i16, ptr %1, !tbaa !2
  %193 = icmp ult i16 %192, 6
  br i1 %193, label %b22, label %b24

b22:
  %194 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %194)
  %195 = sub i16 %192, 0
  %196 = getelementptr inbounds [16 x i8], ptr %13, i16 %195
  %197 = load i32, ptr %196, !tbaa !2
  call cc 1005 addrspace(1) void @N$PQ4(i32 %197, i8 zeroext 9)
  call cc 1005 addrspace(1) void @N$PN()
  %198 = getelementptr i8, ptr @$str2, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %198)
  %199 = sub i16 %192, 0
  %200 = getelementptr inbounds [16 x i8], ptr %13, i16 %199
  %201 = getelementptr inbounds i8, ptr %200, i16 4
  %202 = load i32, ptr %201, !tbaa !2
  call cc 1005 addrspace(1) void @N$PQ4(i32 %202, i8 zeroext 9)
  call cc 1005 addrspace(1) void @N$PN()
  %203 = getelementptr i8, ptr @$str3, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %203)
  %204 = sub i16 %192, 0
  %205 = getelementptr inbounds [16 x i8], ptr %13, i16 %204
  %206 = getelementptr inbounds i8, ptr %205, i16 8
  %207 = load i32, ptr %206, !tbaa !2
  call cc 1005 addrspace(1) void @N$PQ4(i32 %207, i8 zeroext 9)
  call cc 1005 addrspace(1) void @N$PN()
  %208 = getelementptr i8, ptr @$str4, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %208)
  %209 = sub i16 %192, 0
  %210 = getelementptr inbounds [16 x i8], ptr %13, i16 %209
  %211 = getelementptr inbounds i8, ptr %210, i16 12
  %212 = load i32, ptr %211, !tbaa !2
  call cc 1005 addrspace(1) void @N$PQ4(i32 %212, i8 zeroext 9)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b23

b23:
  %213 = load i16, ptr %1, !tbaa !2
  %214 = add i16 %213, 1
  store i16 %214, ptr %1, !tbaa !2
  br label %b21

b24:
  %215 = getelementptr i8, ptr @$str5, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %215)
  call cc 1005 addrspace(1) void @N$PN()
  %216 = getelementptr inbounds [16 x i8], ptr %13, i16 0
  %217 = load i32, ptr %216, !tbaa !2
  %218 = getelementptr inbounds [16 x i8], ptr %13, i16 1
  %219 = getelementptr inbounds i8, ptr %218, i16 4
  %220 = load i32, ptr %219, !tbaa !2
  %221 = add i32 %217, %220
  %222 = getelementptr inbounds [16 x i8], ptr %13, i16 2
  %223 = load i32, ptr %222, !tbaa !2
  %224 = add i32 %221, %223
  %225 = getelementptr inbounds [16 x i8], ptr %13, i16 3
  %226 = getelementptr inbounds i8, ptr %225, i16 4
  %227 = load i32, ptr %226, !tbaa !2
  %228 = add i32 %224, %227
  ret i32 %228
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = call cc 1005 addrspace(1) i32 @nbody(i32 1)
  ret i16 0
}

declare i32 @llvm.sdiv.fix.i32(i32, i32, i32 immarg) nocallback nofree nosync nounwind willreturn memory(none)

declare i32 @llvm.smul.fix.i32(i32, i32, i32 immarg) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PQ4(i32, i8) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
