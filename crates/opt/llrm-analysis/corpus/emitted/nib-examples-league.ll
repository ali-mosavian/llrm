target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str2 = internal constant [13 x i8] c"\08\00\06\00\06\00Rovers\00"
@$str3 = internal constant [13 x i8] c"\08\00\06\00\06\00United\00"
@$str4 = internal constant [15 x i8] c"\08\00\08\00\08\00Athletic\00"
@$str5 = internal constant [16 x i8] c"\08\00\09\00\09\00 lead on \00"
@$str6 = internal constant [9 x i8] c"\08\00\02\00\02\00: \00"
@$str7 = internal constant [13 x i8] c"\08\00\06\00\06\00 from \00"

define internal i16 @Team.points(ptr addrspace(1) %0) addrspace(1) {
b1:
  %1 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %2 = load i16, ptr addrspace(1) %1
  %3 = mul i16 %2, 3
  %4 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %5 = load i16, ptr addrspace(1) %4
  %6 = add i16 %3, %5
  ret i16 %6
}

define internal i16 @Team.played(ptr addrspace(1) %0) addrspace(1) {
b1:
  %1 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %2 = load i16, ptr addrspace(1) %1
  %3 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %4 = load i16, ptr addrspace(1) %3
  %5 = add i16 %2, %4
  %6 = getelementptr i8, ptr addrspace(1) %0, i16 6
  %7 = load i16, ptr addrspace(1) %6
  %8 = add i16 %5, %7
  ret i16 %8
}

define internal void @Team.record(ptr addrspace(1) %0, i16 %1, i16 %2) addrspace(1) {
b1:
  %3 = icmp sgt i16 %1, %2
  %4 = sext i1 %3 to i8
  %5 = icmp ne i8 %4, 0
  br i1 %5, label %b2, label %b3

b2:
  %6 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %7 = load i16, ptr addrspace(1) %6
  %8 = add i16 %7, 1
  %9 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %8, ptr addrspace(1) %9
  br label %b4

b3:
  %10 = icmp eq i16 %1, %2
  %11 = sext i1 %10 to i8
  %12 = icmp ne i8 %11, 0
  br i1 %12, label %b5, label %b6

b4:
  ret void

b5:
  %13 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %14 = load i16, ptr addrspace(1) %13
  %15 = add i16 %14, 1
  %16 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %15, ptr addrspace(1) %16
  br label %b7

b6:
  %17 = getelementptr i8, ptr addrspace(1) %0, i16 6
  %18 = load i16, ptr addrspace(1) %17
  %19 = add i16 %18, 1
  %20 = getelementptr i8, ptr addrspace(1) %0, i16 6
  store i16 %19, ptr addrspace(1) %20
  br label %b7

b7:
  br label %b4
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca [8 x i8]
  %4 = alloca [8 x i8]
  %5 = alloca [8 x i8]
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca [8 x i8]
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca [8 x i8]
  %14 = alloca [4 x i8]
  %15 = alloca ptr
  %16 = alloca i16
  %17 = alloca ptr
  %18 = alloca i16
  %19 = alloca i16
  %20 = alloca ptr
  %21 = alloca ptr
  store i16 0, ptr %0
  store i16 0, ptr %1
  store i16 0, ptr %2
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %5, i8 0, i16 8, i1 false)
  store i16 0, ptr %6
  store i16 0, ptr %7
  store i16 0, ptr %8
  call void @llvm.memset.p0.i16(ptr %9, i8 0, i16 8, i1 false)
  store i16 0, ptr %10
  store i16 0, ptr %11
  store i16 0, ptr %12
  call void @llvm.memset.p0.i16(ptr %13, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %14, i8 0, i16 4, i1 false)
  store ptr null, ptr %15
  store i16 0, ptr %16
  store ptr null, ptr %17
  store i16 0, ptr %18
  store i16 0, ptr %19
  store ptr null, ptr %20
  store ptr null, ptr %21
  %22 = getelementptr i8, ptr @$str1, i16 6
  store ptr %22, ptr %21, !tbaa !2
  %23 = getelementptr i8, ptr @$str1, i16 6
  %24 = call addrspace(1) ptr @N$BGRW(ptr %23, i16 3, i16 2)
  %25 = getelementptr i8, ptr %24, i16 0
  %26 = getelementptr i8, ptr @$str2, i16 6
  store ptr %26, ptr %25
  %27 = getelementptr i8, ptr %24, i16 2
  %28 = getelementptr i8, ptr @$str3, i16 6
  store ptr %28, ptr %27
  %29 = getelementptr i8, ptr %24, i16 4
  %30 = getelementptr i8, ptr @$str4, i16 6
  store ptr %30, ptr %29
  store ptr %24, ptr %20, !tbaa !2
  %31 = load ptr, ptr %20, !tbaa !2
  %32 = getelementptr i8, ptr %31, i16 -4
  %33 = load i16, ptr %32
  store i16 0, ptr %19, !tbaa !2
  br label %b2

b2:
  %34 = load i16, ptr %19, !tbaa !2
  %35 = icmp ult i16 %34, %33
  %36 = sext i1 %35 to i8
  %37 = icmp ne i8 %36, 0
  br i1 %37, label %b3, label %b5

b3:
  %38 = mul i16 %34, 2
  %39 = getelementptr i8, ptr %31, i16 %38
  %40 = load ptr, ptr %21, !tbaa !2
  %41 = getelementptr i8, ptr %40, i16 -4
  %42 = load i16, ptr %41
  %43 = call addrspace(1) ptr @N$BGRW(ptr %40, i16 1, i16 8)
  store ptr %43, ptr %21, !tbaa !2
  %44 = mul i16 %42, 8
  %45 = getelementptr i8, ptr %43, i16 %44
  %46 = load ptr, ptr %39
  %47 = call addrspace(1) ptr @N$BCLN(ptr %46, i16 1)
  store ptr %47, ptr %45
  %48 = getelementptr i8, ptr %45, i16 2
  store i16 0, ptr %48
  %49 = getelementptr i8, ptr %45, i16 4
  store i16 0, ptr %49
  %50 = getelementptr i8, ptr %45, i16 6
  store i16 0, ptr %50
  br label %b4

b4:
  %51 = load i16, ptr %19, !tbaa !2
  %52 = add i16 %51, 1
  store i16 %52, ptr %19, !tbaa !2
  br label %b2

b5:
  %53 = load ptr, ptr %20, !tbaa !2
  %54 = icmp ne ptr %53, null
  %55 = sext i1 %54 to i8
  %56 = icmp ne i8 %55, 0
  br i1 %56, label %b7, label %b6

b6:
  call addrspace(1) void @N$BDRP(ptr %53)
  %57 = load ptr, ptr %21, !tbaa !2
  %58 = getelementptr i8, ptr %57, i16 -4
  %59 = load i16, ptr %58
  %60 = icmp ult i16 0, %59
  %61 = sext i1 %60 to i8
  %62 = icmp ne i8 %61, 0
  br i1 %62, label %b11, label %b12

b7:
  %63 = getelementptr i8, ptr %53, i16 -4
  %64 = load i16, ptr %63
  store i16 0, ptr %18, !tbaa !2
  br label %b8

b8:
  %65 = load i16, ptr %18, !tbaa !2
  %66 = icmp ult i16 %65, %64
  %67 = sext i1 %66 to i8
  %68 = icmp ne i8 %67, 0
  br i1 %68, label %b10, label %b9

b9:
  br label %b6

b10:
  %69 = mul i16 %65, 2
  %70 = getelementptr i8, ptr %53, i16 %69
  %71 = load ptr, ptr %70
  call addrspace(1) void @N$BDRP(ptr %71)
  %72 = add i16 %65, 1
  store i16 %72, ptr %18, !tbaa !2
  br label %b8

b11:
  %73 = getelementptr i8, ptr %57, i16 0
  %74 = addrspacecast ptr %73 to ptr addrspace(1)
  call addrspace(1) void @Team.record(ptr addrspace(1) %74, i16 2, i16 1)
  %75 = load ptr, ptr %21, !tbaa !2
  %76 = getelementptr i8, ptr %75, i16 -4
  %77 = load i16, ptr %76
  %78 = icmp ult i16 0, %77
  %79 = sext i1 %78 to i8
  %80 = icmp ne i8 %79, 0
  br i1 %80, label %b13, label %b14

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b13:
  %81 = getelementptr i8, ptr %75, i16 0
  %82 = addrspacecast ptr %81 to ptr addrspace(1)
  call addrspace(1) void @Team.record(ptr addrspace(1) %82, i16 0, i16 0)
  %83 = load ptr, ptr %21, !tbaa !2
  %84 = getelementptr i8, ptr %83, i16 -4
  %85 = load i16, ptr %84
  %86 = icmp ult i16 1, %85
  %87 = sext i1 %86 to i8
  %88 = icmp ne i8 %87, 0
  br i1 %88, label %b15, label %b16

b14:
  call addrspace(1) void @N$EBND()
  unreachable

b15:
  %89 = getelementptr i8, ptr %83, i16 8
  %90 = addrspacecast ptr %89 to ptr addrspace(1)
  call addrspace(1) void @Team.record(ptr addrspace(1) %90, i16 3, i16 0)
  %91 = load ptr, ptr %21, !tbaa !2
  %92 = getelementptr i8, ptr %91, i16 -4
  %93 = load i16, ptr %92
  %94 = icmp ult i16 1, %93
  %95 = sext i1 %94 to i8
  %96 = icmp ne i8 %95, 0
  br i1 %96, label %b17, label %b18

b16:
  call addrspace(1) void @N$EBND()
  unreachable

b17:
  %97 = getelementptr i8, ptr %91, i16 8
  %98 = addrspacecast ptr %97 to ptr addrspace(1)
  call addrspace(1) void @Team.record(ptr addrspace(1) %98, i16 1, i16 2)
  %99 = load ptr, ptr %21, !tbaa !2
  %100 = getelementptr i8, ptr %99, i16 -4
  %101 = load i16, ptr %100
  %102 = icmp ult i16 2, %101
  %103 = sext i1 %102 to i8
  %104 = icmp ne i8 %103, 0
  br i1 %104, label %b19, label %b20

b18:
  call addrspace(1) void @N$EBND()
  unreachable

b19:
  %105 = getelementptr i8, ptr %99, i16 16
  %106 = addrspacecast ptr %105 to ptr addrspace(1)
  call addrspace(1) void @Team.record(ptr addrspace(1) %106, i16 1, i16 1)
  %107 = load ptr, ptr %21, !tbaa !2
  %108 = getelementptr i8, ptr %107, i16 -4
  %109 = load i16, ptr %108
  %110 = icmp ult i16 2, %109
  %111 = sext i1 %110 to i8
  %112 = icmp ne i8 %111, 0
  br i1 %112, label %b21, label %b22

b20:
  call addrspace(1) void @N$EBND()
  unreachable

b21:
  %113 = getelementptr i8, ptr %107, i16 16
  %114 = addrspacecast ptr %113 to ptr addrspace(1)
  call addrspace(1) void @Team.record(ptr addrspace(1) %114, i16 0, i16 4)
  %115 = getelementptr i8, ptr @$str1, i16 6
  store ptr %115, ptr %17, !tbaa !2
  %116 = load ptr, ptr %21, !tbaa !2
  %117 = getelementptr i8, ptr %116, i16 -4
  %118 = load i16, ptr %117
  store i16 0, ptr %16, !tbaa !2
  br label %b23

b22:
  call addrspace(1) void @N$EBND()
  unreachable

b23:
  %119 = load i16, ptr %16, !tbaa !2
  %120 = icmp ult i16 %119, %118
  %121 = sext i1 %120 to i8
  %122 = icmp ne i8 %121, 0
  br i1 %122, label %b24, label %b26

b24:
  %123 = mul i16 %119, 8
  %124 = getelementptr i8, ptr %116, i16 %123
  %125 = load ptr, ptr %17, !tbaa !2
  %126 = getelementptr i8, ptr %125, i16 -4
  %127 = load i16, ptr %126
  %128 = call addrspace(1) ptr @N$BGRW(ptr %125, i16 1, i16 2)
  store ptr %128, ptr %17, !tbaa !2
  %129 = mul i16 %127, 2
  %130 = getelementptr i8, ptr %128, i16 %129
  %131 = addrspacecast ptr %124 to ptr addrspace(1)
  %132 = call addrspace(1) i16 @Team.points(ptr addrspace(1) %131)
  store i16 %132, ptr %130
  br label %b25

b25:
  %133 = load i16, ptr %16, !tbaa !2
  %134 = add i16 %133, 1
  store i16 %134, ptr %16, !tbaa !2
  br label %b23

b26:
  %135 = load ptr, ptr %17, !tbaa !2
  store ptr null, ptr %17, !tbaa !2
  store ptr %135, ptr %15, !tbaa !2
  %136 = load ptr, ptr %15, !tbaa !2
  %137 = getelementptr i8, ptr %136, i16 -4
  %138 = load i16, ptr %137
  %139 = addrspacecast ptr %136 to ptr addrspace(1)
  store i16 %138, ptr %13, !tbaa !2
  %140 = getelementptr inbounds i8, ptr %13, i16 2
  store i16 %138, ptr %140, !tbaa !2
  %141 = getelementptr inbounds i8, ptr %13, i16 4
  store ptr addrspace(1) %139, ptr %141, !tbaa !2
  %142 = addrspacecast ptr %13 to ptr addrspace(1)
  %143 = call addrspace(1) i32 @"best[i16]"(ptr addrspace(1) %142)
  %144 = addrspacecast ptr %14 to ptr addrspace(1)
  store i32 %143, ptr addrspace(1) %144, !tbaa !2
  %145 = load i16, ptr %14, !tbaa !2
  %146 = getelementptr inbounds i8, ptr %14, i16 2
  %147 = load i16, ptr %146, !tbaa !2
  store i16 %145, ptr %12, !tbaa !2
  store i16 %147, ptr %11, !tbaa !2
  %148 = load ptr, ptr %21, !tbaa !2
  %149 = load i16, ptr %12, !tbaa !2
  %150 = getelementptr i8, ptr %148, i16 -4
  %151 = load i16, ptr %150
  %152 = icmp ult i16 %149, %151
  %153 = sext i1 %152 to i8
  %154 = icmp ne i8 %153, 0
  br i1 %154, label %b27, label %b28

b27:
  %155 = mul i16 %149, 8
  %156 = getelementptr i8, ptr %148, i16 %155
  %157 = load ptr, ptr %156
  call addrspace(1) void @N$PS(ptr %157)
  %158 = getelementptr i8, ptr @$str5, i16 6
  call addrspace(1) void @N$PS(ptr %158)
  %159 = load i16, ptr %11, !tbaa !2
  call addrspace(1) void @N$PI2(i16 %159)
  call addrspace(1) void @N$PN()
  store i16 2, ptr %10, !tbaa !2
  %160 = load ptr, ptr %15, !tbaa !2
  %161 = getelementptr i8, ptr %160, i16 -4
  %162 = load i16, ptr %161
  %163 = addrspacecast ptr %160 to ptr addrspace(1)
  store i16 %162, ptr %9, !tbaa !2
  %164 = getelementptr inbounds i8, ptr %9, i16 2
  store i16 %162, ptr %164, !tbaa !2
  %165 = getelementptr inbounds i8, ptr %9, i16 4
  store ptr addrspace(1) %163, ptr %165, !tbaa !2
  %166 = addrspacecast ptr %9 to ptr addrspace(1)
  store i16 0, ptr %8, !tbaa !2
  %167 = load i16, ptr addrspace(1) %166
  store i16 0, ptr %7, !tbaa !2
  br label %b30

b28:
  call addrspace(1) void @N$EBND()
  unreachable

b29:
  %168 = load ptr, ptr %15, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %168)
  %169 = load ptr, ptr %17, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %169)
  %170 = load ptr, ptr %21, !tbaa !2
  %171 = icmp ne ptr %170, null
  %172 = sext i1 %171 to i8
  %173 = icmp ne i8 %172, 0
  br i1 %173, label %b41, label %b40

b30:
  %174 = load i16, ptr %7, !tbaa !2
  %175 = icmp ult i16 %174, %167
  %176 = sext i1 %175 to i8
  %177 = icmp ne i8 %176, 0
  br i1 %177, label %b31, label %b33

b31:
  %178 = getelementptr i8, ptr addrspace(1) %166, i16 4
  %179 = load ptr addrspace(1), ptr addrspace(1) %178, !tbaa !2
  %180 = mul i16 %174, 2
  %181 = getelementptr i8, ptr addrspace(1) %179, i16 %180
  %182 = load i16, ptr addrspace(1) %181
  %183 = load i16, ptr %10, !tbaa !2
  %184 = icmp sge i16 %182, %183
  %185 = sext i1 %184 to i8
  %186 = icmp ne i8 %185, 0
  br i1 %186, label %b34, label %b35

b32:
  %187 = load i16, ptr %7, !tbaa !2
  %188 = add i16 %187, 1
  store i16 %188, ptr %7, !tbaa !2
  br label %b30

b33:
  br label %b29

b34:
  %189 = load i16, ptr %8, !tbaa !2
  store i16 %189, ptr %6, !tbaa !2
  %190 = load ptr, ptr %21, !tbaa !2
  %191 = load i16, ptr %6, !tbaa !2
  %192 = getelementptr i8, ptr %190, i16 -4
  %193 = load i16, ptr %192
  %194 = icmp ult i16 %191, %193
  %195 = sext i1 %194 to i8
  %196 = icmp ne i8 %195, 0
  br i1 %196, label %b38, label %b39

b35:
  br label %b36

b36:
  %197 = load i16, ptr %8, !tbaa !2
  %198 = add i16 %197, 1
  store i16 %198, ptr %8, !tbaa !2
  br label %b32

b37:
  br label %b36

b38:
  %199 = mul i16 %191, 8
  %200 = getelementptr i8, ptr %190, i16 %199
  %201 = load ptr, ptr %200
  %202 = getelementptr i8, ptr %200, i16 2
  %203 = load i16, ptr %202
  %204 = getelementptr i8, ptr %200, i16 4
  %205 = load i16, ptr %204
  %206 = getelementptr i8, ptr %200, i16 6
  %207 = load i16, ptr %206
  store ptr %201, ptr %4, !tbaa !2
  %208 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 %203, ptr %208, !tbaa !2
  %209 = getelementptr inbounds i8, ptr %4, i16 4
  store i16 %205, ptr %209, !tbaa !2
  %210 = getelementptr inbounds i8, ptr %4, i16 6
  store i16 %207, ptr %210, !tbaa !2
  %211 = load ptr, ptr %4, !tbaa !2
  %212 = call addrspace(1) ptr @N$BCLN(ptr %211, i16 1)
  store ptr %212, ptr %4, !tbaa !2
  %213 = load ptr, ptr %4, !tbaa !2
  %214 = getelementptr inbounds i8, ptr %4, i16 2
  %215 = load i16, ptr %214, !tbaa !2
  %216 = getelementptr inbounds i8, ptr %4, i16 4
  %217 = load i16, ptr %216, !tbaa !2
  %218 = getelementptr inbounds i8, ptr %4, i16 6
  %219 = load i16, ptr %218, !tbaa !2
  store ptr %213, ptr %5, !tbaa !2
  %220 = getelementptr inbounds i8, ptr %5, i16 2
  store i16 %215, ptr %220, !tbaa !2
  %221 = getelementptr inbounds i8, ptr %5, i16 4
  store i16 %217, ptr %221, !tbaa !2
  %222 = getelementptr inbounds i8, ptr %5, i16 6
  store i16 %219, ptr %222, !tbaa !2
  %223 = load ptr, ptr %5, !tbaa !2
  %224 = getelementptr i8, ptr %223, i16 -4
  %225 = load i16, ptr %224
  %226 = addrspacecast ptr %223 to ptr addrspace(1)
  store i16 %225, ptr %3, !tbaa !2
  %227 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 %225, ptr %227, !tbaa !2
  %228 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %226, ptr %228, !tbaa !2
  %229 = addrspacecast ptr %3 to ptr addrspace(1)
  %230 = addrspacecast ptr %5 to ptr addrspace(1)
  %231 = call addrspace(1) i16 @Team.points(ptr addrspace(1) %230)
  store i16 %231, ptr %2, !tbaa !2
  %232 = addrspacecast ptr %5 to ptr addrspace(1)
  %233 = call addrspace(1) i16 @Team.played(ptr addrspace(1) %232)
  store i16 %233, ptr %1, !tbaa !2
  call addrspace(1) void @N$PV(ptr addrspace(1) %229)
  %234 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @N$PS(ptr %234)
  %235 = load i16, ptr %2, !tbaa !2
  call addrspace(1) void @N$PI2(i16 %235)
  %236 = getelementptr i8, ptr @$str7, i16 6
  call addrspace(1) void @N$PS(ptr %236)
  %237 = load i16, ptr %1, !tbaa !2
  call addrspace(1) void @N$PI2(i16 %237)
  call addrspace(1) void @N$PN()
  %238 = load ptr, ptr %5, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %238)
  br label %b37

b39:
  call addrspace(1) void @N$EBND()
  unreachable

b40:
  call addrspace(1) void @N$BDRP(ptr %170)
  ret i16 0

b41:
  %239 = getelementptr i8, ptr %170, i16 -4
  %240 = load i16, ptr %239
  store i16 0, ptr %0, !tbaa !2
  br label %b42

b42:
  %241 = load i16, ptr %0, !tbaa !2
  %242 = icmp ult i16 %241, %240
  %243 = sext i1 %242 to i8
  %244 = icmp ne i8 %243, 0
  br i1 %244, label %b44, label %b43

b43:
  br label %b40

b44:
  %245 = mul i16 %241, 8
  %246 = getelementptr i8, ptr %170, i16 %245
  %247 = load ptr, ptr %246
  call addrspace(1) void @N$BDRP(ptr %247)
  %248 = add i16 %241, 1
  store i16 %248, ptr %0, !tbaa !2
  br label %b42
}

define internal i32 @"best[i16]"(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca [4 x i8]
  store i16 0, ptr %1
  store i16 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %4
  call void @llvm.memset.p0.i16(ptr %5, i8 0, i16 4, i1 false)
  store i16 0, ptr %4, !tbaa !2
  %6 = load i16, ptr addrspace(1) %0
  %7 = icmp ult i16 0, %6
  %8 = sext i1 %7 to i8
  %9 = icmp ne i8 %8, 0
  br i1 %9, label %b2, label %b3

b2:
  %10 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %11 = load ptr addrspace(1), ptr addrspace(1) %10
  %12 = getelementptr i8, ptr addrspace(1) %11, i16 0
  %13 = load i16, ptr addrspace(1) %12
  store i16 %13, ptr %3, !tbaa !2
  store i16 0, ptr %2, !tbaa !2
  %14 = load i16, ptr addrspace(1) %0
  store i16 0, ptr %1, !tbaa !2
  br label %b4

b3:
  call addrspace(1) void @N$EBND()
  unreachable

b4:
  %15 = load i16, ptr %1, !tbaa !2
  %16 = icmp ult i16 %15, %14
  %17 = sext i1 %16 to i8
  %18 = icmp ne i8 %17, 0
  br i1 %18, label %b5, label %b7

b5:
  %19 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %20 = load ptr addrspace(1), ptr addrspace(1) %19
  %21 = mul i16 %15, 2
  %22 = getelementptr i8, ptr addrspace(1) %20, i16 %21
  %23 = load i16, ptr addrspace(1) %22
  %24 = load i16, ptr %3, !tbaa !2
  %25 = icmp sgt i16 %23, %24
  %26 = sext i1 %25 to i8
  %27 = icmp ne i8 %26, 0
  br i1 %27, label %b8, label %b9

b6:
  %28 = load i16, ptr %1, !tbaa !2
  %29 = add i16 %28, 1
  store i16 %29, ptr %1, !tbaa !2
  br label %b4

b7:
  %30 = load i16, ptr %4, !tbaa !2
  %31 = load i16, ptr %3, !tbaa !2
  store i16 %30, ptr %5, !tbaa !2
  %32 = getelementptr inbounds i8, ptr %5, i16 2
  store i16 %31, ptr %32, !tbaa !2
  %33 = addrspacecast ptr %5 to ptr addrspace(1)
  %34 = load i32, ptr addrspace(1) %33, !tbaa !2
  ret i32 %34

b8:
  %35 = load i16, ptr addrspace(1) %22
  store i16 %35, ptr %3, !tbaa !2
  %36 = load i16, ptr %2, !tbaa !2
  store i16 %36, ptr %4, !tbaa !2
  br label %b10

b9:
  br label %b10

b10:
  %37 = load i16, ptr %2, !tbaa !2
  %38 = add i16 %37, 1
  store i16 %38, ptr %2, !tbaa !2
  br label %b6
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare ptr @N$BCLN(ptr, i16) addrspace(1)

declare void @N$BDRP(ptr) addrspace(1)

declare void @N$EBND() addrspace(1)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PI2(i16) addrspace(1)

declare void @N$PN() addrspace(1)

declare void @N$PV(ptr addrspace(1)) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
