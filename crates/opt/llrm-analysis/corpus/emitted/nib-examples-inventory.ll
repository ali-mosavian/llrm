target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str2 = internal constant [11 x i8] c"\08\00\04\00\04\00rope\00"
@$str3 = internal constant [11 x i8] c"\08\00\04\00\04\00lamp\00"
@$str4 = internal constant [8 x i8] c"\08\00\01\00\01\00s\00"
@$str5 = internal constant [14 x i8] c"\08\00\07\00\07\00torch x\00"
@$str6 = internal constant [15 x i8] c"\08\00\08\00\08\00 kinds, \00"
@$str7 = internal constant [16 x i8] c"\08\00\09\00\09\00 in stock\00"
@$str8 = internal constant [9 x i8] c"\08\00\02\00\02\00  \00"
@$str9 = internal constant [9 x i8] c"\08\00\02\00\02\00: \00"
@$str10 = internal constant [14 x i8] c"\08\00\07\00\07\00popped \00"
@$str11 = internal constant [9 x i8] c"\08\00\02\00\02\00, \00"
@$str12 = internal constant [12 x i8] c"\08\00\05\00\05\00 left\00"
@$str13 = internal constant [12 x i8] c"\08\00\05\00\05\00north\00"
@$str14 = internal constant [11 x i8] c"\08\00\04\00\04\00east\00"
@$str15 = internal constant [12 x i8] c"\08\00\05\00\05\00south\00"
@$str16 = internal constant [9 x i8] c"\08\00\02\00\02\00up\00"
@$str17 = internal constant [8 x i8] c"\08\00\01\00\01\00 \00"

define internal i16 @stocked(ptr addrspace(1) %0) addrspace(1) {
b1:
  %1 = alloca i16
  %2 = alloca i16
  store i16 0, ptr %1
  store i16 0, ptr %2
  store i16 0, ptr %2, !tbaa !2
  %3 = load ptr, ptr addrspace(1) %0
  %4 = getelementptr i8, ptr %3, i16 -4
  %5 = load i16, ptr %4
  store i16 0, ptr %1, !tbaa !2
  br label %b2

b2:
  %6 = load i16, ptr %1, !tbaa !2
  %7 = icmp ult i16 %6, %5
  %8 = sext i1 %7 to i8
  %9 = icmp ne i8 %8, 0
  br i1 %9, label %b3, label %b5

b3:
  %10 = mul i16 %6, 4
  %11 = getelementptr i8, ptr %3, i16 %10
  %12 = load i16, ptr %2, !tbaa !2
  %13 = getelementptr i8, ptr %11, i16 2
  %14 = load i16, ptr %13
  %15 = add i16 %12, %14
  store i16 %15, ptr %2, !tbaa !2
  br label %b4

b4:
  %16 = load i16, ptr %1, !tbaa !2
  %17 = add i16 %16, 1
  store i16 %17, ptr %1, !tbaa !2
  br label %b2

b5:
  %18 = load i16, ptr %2, !tbaa !2
  ret i16 %18
}

define internal void @restock(ptr addrspace(1) %0, ptr %1, i16 %2) addrspace(1) {
b1:
  %3 = alloca ptr
  store ptr null, ptr %3
  store ptr %1, ptr %3, !tbaa !2
  %4 = load ptr, ptr addrspace(1) %0
  %5 = getelementptr i8, ptr %4, i16 -4
  %6 = load i16, ptr %5
  %7 = call addrspace(1) ptr @N$BGRW(ptr %4, i16 1, i16 4)
  store ptr %7, ptr addrspace(1) %0
  %8 = mul i16 %6, 4
  %9 = getelementptr i8, ptr %7, i16 %8
  %10 = load ptr, ptr %3, !tbaa !2
  store ptr null, ptr %3, !tbaa !2
  store ptr %10, ptr %9
  %11 = getelementptr i8, ptr %9, i16 2
  store i16 %2, ptr %11
  %12 = load ptr, ptr %3, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %12)
  ret void
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca ptr
  %4 = alloca i16
  %5 = alloca ptr
  %6 = alloca i16
  %7 = alloca ptr
  %8 = alloca i16
  %9 = alloca ptr
  %10 = alloca i16
  %11 = alloca ptr
  %12 = alloca ptr
  %13 = alloca i16
  %14 = alloca ptr
  %15 = alloca i16
  %16 = alloca i16
  %17 = alloca i16
  %18 = alloca ptr
  store i16 0, ptr %0
  store i16 0, ptr %1
  store i16 0, ptr %2
  store ptr null, ptr %3
  store i16 0, ptr %4
  store ptr null, ptr %5
  store i16 0, ptr %6
  store ptr null, ptr %7
  store i16 0, ptr %8
  store ptr null, ptr %9
  store i16 0, ptr %10
  store ptr null, ptr %11
  store ptr null, ptr %12
  store i16 0, ptr %13
  store ptr null, ptr %14
  store i16 0, ptr %15
  store i16 0, ptr %16
  store i16 0, ptr %17
  store ptr null, ptr %18
  %19 = getelementptr i8, ptr @$str1, i16 6
  store ptr %19, ptr %18, !tbaa !2
  %20 = addrspacecast ptr %18 to ptr addrspace(1)
  %21 = getelementptr i8, ptr @$str2, i16 6
  call addrspace(1) void @restock(ptr addrspace(1) %20, ptr %21, i16 3)
  %22 = addrspacecast ptr %18 to ptr addrspace(1)
  %23 = getelementptr i8, ptr @$str3, i16 6
  %24 = getelementptr i8, ptr @$str4, i16 6
  %25 = call addrspace(1) ptr @N$TCAT(ptr %23, ptr %24)
  call addrspace(1) void @restock(ptr addrspace(1) %22, ptr %25, i16 2)
  %26 = addrspacecast ptr %18 to ptr addrspace(1)
  call addrspace(1) void @N$PBEG()
  %27 = getelementptr i8, ptr @$str5, i16 6
  call addrspace(1) void @N$PS(ptr %27)
  call addrspace(1) void @N$PI2(i16 4)
  %28 = call addrspace(1) ptr @N$PEND()
  call addrspace(1) void @restock(ptr addrspace(1) %26, ptr %28, i16 1)
  %29 = load ptr, ptr %18, !tbaa !2
  %30 = getelementptr i8, ptr %29, i16 -4
  %31 = load i16, ptr %30
  store i16 %31, ptr %17, !tbaa !2
  %32 = addrspacecast ptr %18 to ptr addrspace(1)
  %33 = call addrspace(1) i16 @stocked(ptr addrspace(1) %32)
  store i16 %33, ptr %16, !tbaa !2
  %34 = load i16, ptr %17, !tbaa !2
  call addrspace(1) void @N$PU2(i16 %34)
  %35 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @N$PS(ptr %35)
  %36 = load i16, ptr %16, !tbaa !2
  call addrspace(1) void @N$PI2(i16 %36)
  %37 = getelementptr i8, ptr @$str7, i16 6
  call addrspace(1) void @N$PS(ptr %37)
  call addrspace(1) void @N$PN()
  %38 = load ptr, ptr %18, !tbaa !2
  %39 = getelementptr i8, ptr %38, i16 -4
  %40 = load i16, ptr %39
  store i16 0, ptr %15, !tbaa !2
  br label %b2

b2:
  %41 = load i16, ptr %15, !tbaa !2
  %42 = icmp ult i16 %41, %40
  %43 = sext i1 %42 to i8
  %44 = icmp ne i8 %43, 0
  br i1 %44, label %b3, label %b5

b3:
  %45 = mul i16 %41, 4
  %46 = getelementptr i8, ptr %38, i16 %45
  %47 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @N$PS(ptr %47)
  %48 = load ptr, ptr %46
  call addrspace(1) void @N$PS(ptr %48)
  %49 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %49)
  %50 = getelementptr i8, ptr %46, i16 2
  %51 = load i16, ptr %50
  call addrspace(1) void @N$PI2(i16 %51)
  call addrspace(1) void @N$PN()
  br label %b4

b4:
  %52 = load i16, ptr %15, !tbaa !2
  %53 = add i16 %52, 1
  store i16 %53, ptr %15, !tbaa !2
  br label %b2

b5:
  %54 = getelementptr i8, ptr @$str1, i16 6
  store ptr %54, ptr %14, !tbaa !2
  %55 = load ptr, ptr %18, !tbaa !2
  %56 = getelementptr i8, ptr %55, i16 -4
  %57 = load i16, ptr %56
  store i16 0, ptr %13, !tbaa !2
  br label %b6

b6:
  %58 = load i16, ptr %13, !tbaa !2
  %59 = icmp ult i16 %58, %57
  %60 = sext i1 %59 to i8
  %61 = icmp ne i8 %60, 0
  br i1 %61, label %b7, label %b9

b7:
  %62 = mul i16 %58, 4
  %63 = getelementptr i8, ptr %55, i16 %62
  %64 = load ptr, ptr %14, !tbaa !2
  %65 = getelementptr i8, ptr %64, i16 -4
  %66 = load i16, ptr %65
  %67 = call addrspace(1) ptr @N$BGRW(ptr %64, i16 1, i16 2)
  store ptr %67, ptr %14, !tbaa !2
  %68 = mul i16 %66, 2
  %69 = getelementptr i8, ptr %67, i16 %68
  %70 = getelementptr i8, ptr %63, i16 2
  %71 = load i16, ptr %70
  store i16 %71, ptr %69
  br label %b8

b8:
  %72 = load i16, ptr %13, !tbaa !2
  %73 = add i16 %72, 1
  store i16 %73, ptr %13, !tbaa !2
  br label %b6

b9:
  %74 = load ptr, ptr %14, !tbaa !2
  store ptr null, ptr %14, !tbaa !2
  store ptr %74, ptr %12, !tbaa !2
  %75 = getelementptr i8, ptr @$str1, i16 6
  store ptr %75, ptr %11, !tbaa !2
  %76 = load ptr, ptr %12, !tbaa !2
  %77 = getelementptr i8, ptr %76, i16 -4
  %78 = load i16, ptr %77
  store i16 0, ptr %10, !tbaa !2
  br label %b10

b10:
  %79 = load i16, ptr %10, !tbaa !2
  %80 = icmp ult i16 %79, %78
  %81 = sext i1 %80 to i8
  %82 = icmp ne i8 %81, 0
  br i1 %82, label %b11, label %b13

b11:
  %83 = mul i16 %79, 2
  %84 = getelementptr i8, ptr %76, i16 %83
  %85 = load ptr, ptr %11, !tbaa !2
  %86 = getelementptr i8, ptr %85, i16 -4
  %87 = load i16, ptr %86
  %88 = call addrspace(1) ptr @N$BGRW(ptr %85, i16 1, i16 2)
  store ptr %88, ptr %11, !tbaa !2
  %89 = mul i16 %87, 2
  %90 = getelementptr i8, ptr %88, i16 %89
  %91 = load i16, ptr %84
  %92 = mul i16 %91, 2
  store i16 %92, ptr %90
  br label %b12

b12:
  %93 = load i16, ptr %10, !tbaa !2
  %94 = add i16 %93, 1
  store i16 %94, ptr %10, !tbaa !2
  br label %b10

b13:
  %95 = load ptr, ptr %11, !tbaa !2
  store ptr null, ptr %11, !tbaa !2
  store ptr %95, ptr %9, !tbaa !2
  %96 = getelementptr i8, ptr @$str1, i16 6
  %97 = call addrspace(1) ptr @N$BGRW(ptr %96, i16 2, i16 2)
  store i16 0, ptr %8, !tbaa !2
  br label %b14

b14:
  %98 = load i16, ptr %8, !tbaa !2
  %99 = icmp ult i16 %98, 2
  %100 = sext i1 %99 to i8
  %101 = icmp ne i8 %100, 0
  br i1 %101, label %b16, label %b15

b15:
  store ptr %97, ptr %7, !tbaa !2
  %102 = load ptr, ptr %7, !tbaa !2
  %103 = getelementptr i8, ptr %102, i16 -4
  %104 = load i16, ptr %103
  %105 = call addrspace(1) ptr @N$BGRW(ptr %102, i16 1, i16 2)
  store ptr %105, ptr %7, !tbaa !2
  %106 = mul i16 %104, 2
  %107 = getelementptr i8, ptr %105, i16 %106
  %108 = load ptr, ptr %9, !tbaa !2
  %109 = getelementptr i8, ptr %108, i16 -4
  %110 = load i16, ptr %109
  %111 = icmp ult i16 0, %110
  %112 = sext i1 %111 to i8
  %113 = icmp ne i8 %112, 0
  br i1 %113, label %b17, label %b18

b16:
  %114 = mul i16 %98, 2
  %115 = getelementptr i8, ptr %97, i16 %114
  store i16 7, ptr %115
  %116 = add i16 %98, 1
  store i16 %116, ptr %8, !tbaa !2
  br label %b14

b17:
  %117 = getelementptr i8, ptr %108, i16 0
  %118 = load i16, ptr %117
  store i16 %118, ptr %107
  %119 = load ptr, ptr %7, !tbaa !2
  %120 = call addrspace(1) i16 @N$BSHR(ptr %119, i16 1)
  %121 = mul i16 %120, 2
  %122 = getelementptr i8, ptr %119, i16 %121
  %123 = load i16, ptr %122
  store i16 %123, ptr %6, !tbaa !2
  %124 = getelementptr i8, ptr @$str10, i16 6
  call addrspace(1) void @N$PS(ptr %124)
  %125 = load i16, ptr %6, !tbaa !2
  call addrspace(1) void @N$PI2(i16 %125)
  %126 = getelementptr i8, ptr @$str11, i16 6
  call addrspace(1) void @N$PS(ptr %126)
  %127 = load ptr, ptr %7, !tbaa !2
  %128 = getelementptr i8, ptr %127, i16 -4
  %129 = load i16, ptr %128
  call addrspace(1) void @N$PU2(i16 %129)
  %130 = getelementptr i8, ptr @$str12, i16 6
  call addrspace(1) void @N$PS(ptr %130)
  call addrspace(1) void @N$PN()
  %131 = getelementptr i8, ptr @$str1, i16 6
  %132 = call addrspace(1) ptr @N$BGRW(ptr %131, i16 2, i16 2)
  %133 = getelementptr i8, ptr %132, i16 0
  %134 = getelementptr i8, ptr @$str13, i16 6
  store ptr %134, ptr %133
  %135 = getelementptr i8, ptr %132, i16 2
  %136 = getelementptr i8, ptr @$str14, i16 6
  store ptr %136, ptr %135
  store ptr %132, ptr %5, !tbaa !2
  %137 = load ptr, ptr %5, !tbaa !2
  %138 = call addrspace(1) ptr @N$BCLN(ptr %137, i16 2)
  %139 = getelementptr i8, ptr %138, i16 -4
  %140 = load i16, ptr %139
  store i16 0, ptr %4, !tbaa !2
  br label %b19

b18:
  call addrspace(1) void @N$EBND()
  unreachable

b19:
  %141 = load i16, ptr %4, !tbaa !2
  %142 = icmp ult i16 %141, %140
  %143 = sext i1 %142 to i8
  %144 = icmp ne i8 %143, 0
  br i1 %144, label %b21, label %b20

b20:
  store ptr %138, ptr %3, !tbaa !2
  %145 = load ptr, ptr %3, !tbaa !2
  %146 = getelementptr i8, ptr %145, i16 -4
  %147 = load i16, ptr %146
  %148 = call addrspace(1) ptr @N$BGRW(ptr %145, i16 1, i16 2)
  store ptr %148, ptr %3, !tbaa !2
  %149 = mul i16 %147, 2
  %150 = getelementptr i8, ptr %148, i16 %149
  %151 = getelementptr i8, ptr @$str15, i16 6
  store ptr %151, ptr %150
  %152 = load ptr, ptr %3, !tbaa !2
  %153 = getelementptr i8, ptr %152, i16 -4
  %154 = load i16, ptr %153
  %155 = icmp ult i16 0, %154
  %156 = sext i1 %155 to i8
  %157 = icmp ne i8 %156, 0
  br i1 %157, label %b22, label %b23

b21:
  %158 = mul i16 %141, 2
  %159 = getelementptr i8, ptr %138, i16 %158
  %160 = load ptr, ptr %159
  %161 = call addrspace(1) ptr @N$BCLN(ptr %160, i16 1)
  store ptr %161, ptr %159
  %162 = add i16 %141, 1
  store i16 %162, ptr %4, !tbaa !2
  br label %b19

b22:
  %163 = getelementptr i8, ptr %152, i16 0
  %164 = getelementptr i8, ptr @$str16, i16 6
  %165 = load ptr, ptr %163
  call addrspace(1) void @N$BDRP(ptr %165)
  store ptr %164, ptr %163
  %166 = load ptr, ptr %5, !tbaa !2
  %167 = getelementptr i8, ptr %166, i16 -4
  %168 = load i16, ptr %167
  %169 = icmp ult i16 0, %168
  %170 = sext i1 %169 to i8
  %171 = icmp ne i8 %170, 0
  br i1 %171, label %b24, label %b25

b23:
  call addrspace(1) void @N$EBND()
  unreachable

b24:
  %172 = getelementptr i8, ptr %166, i16 0
  %173 = load ptr, ptr %172
  call addrspace(1) void @N$PS(ptr %173)
  %174 = getelementptr i8, ptr @$str17, i16 6
  call addrspace(1) void @N$PS(ptr %174)
  %175 = load ptr, ptr %3, !tbaa !2
  %176 = getelementptr i8, ptr %175, i16 -4
  %177 = load i16, ptr %176
  %178 = icmp ult i16 0, %177
  %179 = sext i1 %178 to i8
  %180 = icmp ne i8 %179, 0
  br i1 %180, label %b26, label %b27

b25:
  call addrspace(1) void @N$EBND()
  unreachable

b26:
  %181 = getelementptr i8, ptr %175, i16 0
  %182 = load ptr, ptr %181
  call addrspace(1) void @N$PS(ptr %182)
  %183 = getelementptr i8, ptr @$str17, i16 6
  call addrspace(1) void @N$PS(ptr %183)
  %184 = load ptr, ptr %3, !tbaa !2
  %185 = getelementptr i8, ptr %184, i16 -4
  %186 = load i16, ptr %185
  %187 = icmp ult i16 2, %186
  %188 = sext i1 %187 to i8
  %189 = icmp ne i8 %188, 0
  br i1 %189, label %b28, label %b29

b27:
  call addrspace(1) void @N$EBND()
  unreachable

b28:
  %190 = getelementptr i8, ptr %184, i16 4
  %191 = load ptr, ptr %190
  call addrspace(1) void @N$PS(ptr %191)
  call addrspace(1) void @N$PN()
  %192 = load ptr, ptr %3, !tbaa !2
  %193 = icmp ne ptr %192, null
  %194 = sext i1 %193 to i8
  %195 = icmp ne i8 %194, 0
  br i1 %195, label %b31, label %b30

b29:
  call addrspace(1) void @N$EBND()
  unreachable

b30:
  call addrspace(1) void @N$BDRP(ptr %192)
  %196 = load ptr, ptr %5, !tbaa !2
  %197 = icmp ne ptr %196, null
  %198 = sext i1 %197 to i8
  %199 = icmp ne i8 %198, 0
  br i1 %199, label %b36, label %b35

b31:
  %200 = getelementptr i8, ptr %192, i16 -4
  %201 = load i16, ptr %200
  store i16 0, ptr %2, !tbaa !2
  br label %b32

b32:
  %202 = load i16, ptr %2, !tbaa !2
  %203 = icmp ult i16 %202, %201
  %204 = sext i1 %203 to i8
  %205 = icmp ne i8 %204, 0
  br i1 %205, label %b34, label %b33

b33:
  br label %b30

b34:
  %206 = mul i16 %202, 2
  %207 = getelementptr i8, ptr %192, i16 %206
  %208 = load ptr, ptr %207
  call addrspace(1) void @N$BDRP(ptr %208)
  %209 = add i16 %202, 1
  store i16 %209, ptr %2, !tbaa !2
  br label %b32

b35:
  call addrspace(1) void @N$BDRP(ptr %196)
  %210 = load ptr, ptr %7, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %210)
  %211 = load ptr, ptr %9, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %211)
  %212 = load ptr, ptr %11, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %212)
  %213 = load ptr, ptr %12, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %213)
  %214 = load ptr, ptr %14, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %214)
  %215 = load ptr, ptr %18, !tbaa !2
  %216 = icmp ne ptr %215, null
  %217 = sext i1 %216 to i8
  %218 = icmp ne i8 %217, 0
  br i1 %218, label %b41, label %b40

b36:
  %219 = getelementptr i8, ptr %196, i16 -4
  %220 = load i16, ptr %219
  store i16 0, ptr %1, !tbaa !2
  br label %b37

b37:
  %221 = load i16, ptr %1, !tbaa !2
  %222 = icmp ult i16 %221, %220
  %223 = sext i1 %222 to i8
  %224 = icmp ne i8 %223, 0
  br i1 %224, label %b39, label %b38

b38:
  br label %b35

b39:
  %225 = mul i16 %221, 2
  %226 = getelementptr i8, ptr %196, i16 %225
  %227 = load ptr, ptr %226
  call addrspace(1) void @N$BDRP(ptr %227)
  %228 = add i16 %221, 1
  store i16 %228, ptr %1, !tbaa !2
  br label %b37

b40:
  call addrspace(1) void @N$BDRP(ptr %215)
  ret i16 0

b41:
  %229 = getelementptr i8, ptr %215, i16 -4
  %230 = load i16, ptr %229
  store i16 0, ptr %0, !tbaa !2
  br label %b42

b42:
  %231 = load i16, ptr %0, !tbaa !2
  %232 = icmp ult i16 %231, %230
  %233 = sext i1 %232 to i8
  %234 = icmp ne i8 %233, 0
  br i1 %234, label %b44, label %b43

b43:
  br label %b40

b44:
  %235 = mul i16 %231, 4
  %236 = getelementptr i8, ptr %215, i16 %235
  %237 = load ptr, ptr %236
  call addrspace(1) void @N$BDRP(ptr %237)
  %238 = add i16 %231, 1
  store i16 %238, ptr %0, !tbaa !2
  br label %b42
}

declare ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare void @N$BDRP(ptr) addrspace(1)

declare ptr @N$TCAT(ptr, ptr) addrspace(1)

declare void @N$PBEG() addrspace(1)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PI2(i16) addrspace(1)

declare ptr @N$PEND() addrspace(1)

declare void @N$PU2(i16) addrspace(1)

declare void @N$PN() addrspace(1)

declare i16 @N$BSHR(ptr, i16) addrspace(1)

declare ptr @N$BCLN(ptr, i16) addrspace(1)

declare void @N$EBND() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
