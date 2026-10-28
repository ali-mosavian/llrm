target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

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

define internal cc 1005 i16 @stocked(ptr addrspace(1) nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  store i16 0, ptr %2, !tbaa !2
  %3 = load ptr, ptr addrspace(1) %0
  %4 = getelementptr i8, ptr %3, i16 -4
  %5 = load i16, ptr %4
  store i16 0, ptr %1, !tbaa !2
  br label %b2

b2:
  %6 = load i16, ptr %1, !tbaa !2
  %7 = icmp ult i16 %6, %5
  br i1 %7, label %b3, label %b5

b3:
  %8 = mul i16 %6, 4
  %9 = getelementptr inbounds i8, ptr %3, i16 %8
  %10 = load i16, ptr %2, !tbaa !2
  %11 = getelementptr i8, ptr %9, i16 2
  %12 = load i16, ptr %11
  %13 = add i16 %10, %12
  store i16 %13, ptr %2, !tbaa !2
  br label %b4

b4:
  %14 = load i16, ptr %1, !tbaa !2
  %15 = add i16 %14, 1
  store i16 %15, ptr %1, !tbaa !2
  br label %b2

b5:
  %16 = load i16, ptr %2, !tbaa !2
  ret i16 %16
}

define internal cc 1005 void @restock(ptr addrspace(1) nocapture %0, ptr %1, i16 %2) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %3 = alloca ptr
  store ptr %1, ptr %3, !tbaa !2
  %4 = load ptr, ptr addrspace(1) %0
  %5 = getelementptr i8, ptr %4, i16 -4
  %6 = load i16, ptr %5
  %7 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %4, i16 1, i16 4)
  store ptr %7, ptr addrspace(1) %0
  %8 = mul i16 %6, 4
  %9 = getelementptr inbounds i8, ptr %7, i16 %8
  %10 = load ptr, ptr %3, !tbaa !2
  store ptr null, ptr %3, !tbaa !2
  store ptr %10, ptr %9
  %11 = getelementptr i8, ptr %9, i16 2
  store i16 %2, ptr %11
  %12 = load ptr, ptr %3, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %12)
  ret void
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
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
  %19 = getelementptr i8, ptr @$str1, i16 6
  store ptr %19, ptr %18, !tbaa !2
  %20 = addrspacecast ptr %18 to ptr addrspace(1)
  %21 = getelementptr i8, ptr @$str2, i16 6
  call cc 1005 addrspace(1) void @restock(ptr addrspace(1) %20, ptr %21, i16 3)
  %22 = addrspacecast ptr %18 to ptr addrspace(1)
  %23 = getelementptr i8, ptr @$str3, i16 6
  %24 = getelementptr i8, ptr @$str4, i16 6
  %25 = call cc 1005 addrspace(1) ptr @N$TCAT(ptr %23, ptr %24)
  call cc 1005 addrspace(1) void @restock(ptr addrspace(1) %22, ptr %25, i16 2)
  %26 = addrspacecast ptr %18 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @N$PBEG()
  %27 = getelementptr i8, ptr @$str5, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %27)
  call cc 1005 addrspace(1) void @N$PI2(i16 4)
  %28 = call cc 1005 addrspace(1) ptr @N$PEND()
  call cc 1005 addrspace(1) void @restock(ptr addrspace(1) %26, ptr %28, i16 1)
  %29 = load ptr, ptr %18, !tbaa !2
  %30 = getelementptr i8, ptr %29, i16 -4
  %31 = load i16, ptr %30
  store i16 %31, ptr %17, !tbaa !2
  %32 = addrspacecast ptr %18 to ptr addrspace(1)
  %33 = call cc 1005 addrspace(1) i16 @stocked(ptr addrspace(1) %32)
  store i16 %33, ptr %16, !tbaa !2
  %34 = load i16, ptr %17, !tbaa !2
  call cc 1005 addrspace(1) void @N$PU2(i16 %34)
  %35 = getelementptr i8, ptr @$str6, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %35)
  %36 = load i16, ptr %16, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI2(i16 %36)
  %37 = getelementptr i8, ptr @$str7, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %37)
  call cc 1005 addrspace(1) void @N$PN()
  %38 = load ptr, ptr %18, !tbaa !2
  %39 = getelementptr i8, ptr %38, i16 -4
  %40 = load i16, ptr %39
  store i16 0, ptr %15, !tbaa !2
  br label %b2

b2:
  %41 = load i16, ptr %15, !tbaa !2
  %42 = icmp ult i16 %41, %40
  br i1 %42, label %b3, label %b5

b3:
  %43 = mul i16 %41, 4
  %44 = getelementptr inbounds i8, ptr %38, i16 %43
  %45 = getelementptr i8, ptr @$str8, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %45)
  %46 = load ptr, ptr %44
  call cc 1005 addrspace(1) void @N$PS(ptr %46)
  %47 = getelementptr i8, ptr @$str9, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %47)
  %48 = getelementptr i8, ptr %44, i16 2
  %49 = load i16, ptr %48
  call cc 1005 addrspace(1) void @N$PI2(i16 %49)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b4

b4:
  %50 = load i16, ptr %15, !tbaa !2
  %51 = add i16 %50, 1
  store i16 %51, ptr %15, !tbaa !2
  br label %b2

b5:
  %52 = getelementptr i8, ptr @$str1, i16 6
  store ptr %52, ptr %14, !tbaa !2
  %53 = load ptr, ptr %18, !tbaa !2
  %54 = getelementptr i8, ptr %53, i16 -4
  %55 = load i16, ptr %54
  store i16 0, ptr %13, !tbaa !2
  br label %b6

b6:
  %56 = load i16, ptr %13, !tbaa !2
  %57 = icmp ult i16 %56, %55
  br i1 %57, label %b7, label %b9

b7:
  %58 = mul i16 %56, 4
  %59 = getelementptr inbounds i8, ptr %53, i16 %58
  %60 = load ptr, ptr %14, !tbaa !2
  %61 = getelementptr i8, ptr %60, i16 -4
  %62 = load i16, ptr %61
  %63 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %60, i16 1, i16 2)
  store ptr %63, ptr %14, !tbaa !2
  %64 = mul i16 %62, 2
  %65 = getelementptr inbounds i8, ptr %63, i16 %64
  %66 = getelementptr i8, ptr %59, i16 2
  %67 = load i16, ptr %66
  store i16 %67, ptr %65
  br label %b8

b8:
  %68 = load i16, ptr %13, !tbaa !2
  %69 = add i16 %68, 1
  store i16 %69, ptr %13, !tbaa !2
  br label %b6

b9:
  %70 = load ptr, ptr %14, !tbaa !2
  store ptr null, ptr %14, !tbaa !2
  store ptr %70, ptr %12, !tbaa !2
  %71 = getelementptr i8, ptr @$str1, i16 6
  store ptr %71, ptr %11, !tbaa !2
  %72 = load ptr, ptr %12, !tbaa !2
  %73 = getelementptr i8, ptr %72, i16 -4
  %74 = load i16, ptr %73
  store i16 0, ptr %10, !tbaa !2
  br label %b10

b10:
  %75 = load i16, ptr %10, !tbaa !2
  %76 = icmp ult i16 %75, %74
  br i1 %76, label %b11, label %b13

b11:
  %77 = mul i16 %75, 2
  %78 = getelementptr inbounds i8, ptr %72, i16 %77
  %79 = load ptr, ptr %11, !tbaa !2
  %80 = getelementptr i8, ptr %79, i16 -4
  %81 = load i16, ptr %80
  %82 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %79, i16 1, i16 2)
  store ptr %82, ptr %11, !tbaa !2
  %83 = mul i16 %81, 2
  %84 = getelementptr inbounds i8, ptr %82, i16 %83
  %85 = load i16, ptr %78
  %86 = mul i16 %85, 2
  store i16 %86, ptr %84
  br label %b12

b12:
  %87 = load i16, ptr %10, !tbaa !2
  %88 = add i16 %87, 1
  store i16 %88, ptr %10, !tbaa !2
  br label %b10

b13:
  %89 = load ptr, ptr %11, !tbaa !2
  store ptr null, ptr %11, !tbaa !2
  store ptr %89, ptr %9, !tbaa !2
  %90 = getelementptr i8, ptr @$str1, i16 6
  %91 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %90, i16 2, i16 2)
  store i16 0, ptr %8, !tbaa !2
  br label %b14

b14:
  %92 = load i16, ptr %8, !tbaa !2
  %93 = icmp ult i16 %92, 2
  br i1 %93, label %b16, label %b15

b15:
  store ptr %91, ptr %7, !tbaa !2
  %94 = load ptr, ptr %7, !tbaa !2
  %95 = getelementptr i8, ptr %94, i16 -4
  %96 = load i16, ptr %95
  %97 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %94, i16 1, i16 2)
  store ptr %97, ptr %7, !tbaa !2
  %98 = mul i16 %96, 2
  %99 = getelementptr inbounds i8, ptr %97, i16 %98
  %100 = load ptr, ptr %9, !tbaa !2
  %101 = getelementptr i8, ptr %100, i16 -4
  %102 = load i16, ptr %101
  %103 = icmp ult i16 0, %102
  br i1 %103, label %b17, label %b18

b16:
  %104 = mul i16 %92, 2
  %105 = getelementptr inbounds i8, ptr %91, i16 %104
  store i16 7, ptr %105
  %106 = add i16 %92, 1
  store i16 %106, ptr %8, !tbaa !2
  br label %b14

b17:
  %107 = getelementptr inbounds i8, ptr %100, i16 0
  %108 = load i16, ptr %107
  store i16 %108, ptr %99
  %109 = load ptr, ptr %7, !tbaa !2
  %110 = call cc 1005 addrspace(1) i16 @N$BSHR(ptr %109, i16 1)
  %111 = mul i16 %110, 2
  %112 = getelementptr inbounds i8, ptr %109, i16 %111
  %113 = load i16, ptr %112
  store i16 %113, ptr %6, !tbaa !2
  %114 = getelementptr i8, ptr @$str10, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %114)
  %115 = load i16, ptr %6, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI2(i16 %115)
  %116 = getelementptr i8, ptr @$str11, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %116)
  %117 = load ptr, ptr %7, !tbaa !2
  %118 = getelementptr i8, ptr %117, i16 -4
  %119 = load i16, ptr %118
  call cc 1005 addrspace(1) void @N$PU2(i16 %119)
  %120 = getelementptr i8, ptr @$str12, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %120)
  call cc 1005 addrspace(1) void @N$PN()
  %121 = getelementptr i8, ptr @$str1, i16 6
  %122 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %121, i16 2, i16 2)
  %123 = getelementptr inbounds i8, ptr %122, i16 0
  %124 = getelementptr i8, ptr @$str13, i16 6
  store ptr %124, ptr %123
  %125 = getelementptr inbounds i8, ptr %122, i16 2
  %126 = getelementptr i8, ptr @$str14, i16 6
  store ptr %126, ptr %125
  store ptr %122, ptr %5, !tbaa !2
  %127 = load ptr, ptr %5, !tbaa !2
  %128 = call cc 1005 addrspace(1) ptr @N$BCLN(ptr %127, i16 2)
  %129 = getelementptr i8, ptr %128, i16 -4
  %130 = load i16, ptr %129
  store i16 0, ptr %4, !tbaa !2
  br label %b19

b18:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b19:
  %131 = load i16, ptr %4, !tbaa !2
  %132 = icmp ult i16 %131, %130
  br i1 %132, label %b21, label %b20

b20:
  store ptr %128, ptr %3, !tbaa !2
  %133 = load ptr, ptr %3, !tbaa !2
  %134 = getelementptr i8, ptr %133, i16 -4
  %135 = load i16, ptr %134
  %136 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %133, i16 1, i16 2)
  store ptr %136, ptr %3, !tbaa !2
  %137 = mul i16 %135, 2
  %138 = getelementptr inbounds i8, ptr %136, i16 %137
  %139 = getelementptr i8, ptr @$str15, i16 6
  store ptr %139, ptr %138
  %140 = load ptr, ptr %3, !tbaa !2
  %141 = getelementptr i8, ptr %140, i16 -4
  %142 = load i16, ptr %141
  %143 = icmp ult i16 0, %142
  br i1 %143, label %b22, label %b23

b21:
  %144 = mul i16 %131, 2
  %145 = getelementptr inbounds i8, ptr %128, i16 %144
  %146 = load ptr, ptr %145
  %147 = call cc 1005 addrspace(1) ptr @N$BCLN(ptr %146, i16 1)
  store ptr %147, ptr %145
  %148 = add i16 %131, 1
  store i16 %148, ptr %4, !tbaa !2
  br label %b19

b22:
  %149 = getelementptr inbounds i8, ptr %140, i16 0
  %150 = getelementptr i8, ptr @$str16, i16 6
  %151 = load ptr, ptr %149
  call cc 1005 addrspace(1) void @N$BDRP(ptr %151)
  store ptr %150, ptr %149
  %152 = load ptr, ptr %5, !tbaa !2
  %153 = getelementptr i8, ptr %152, i16 -4
  %154 = load i16, ptr %153
  %155 = icmp ult i16 0, %154
  br i1 %155, label %b24, label %b25

b23:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b24:
  %156 = getelementptr inbounds i8, ptr %152, i16 0
  %157 = load ptr, ptr %156
  call cc 1005 addrspace(1) void @N$PS(ptr %157)
  %158 = getelementptr i8, ptr @$str17, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %158)
  %159 = load ptr, ptr %3, !tbaa !2
  %160 = getelementptr i8, ptr %159, i16 -4
  %161 = load i16, ptr %160
  %162 = icmp ult i16 0, %161
  br i1 %162, label %b26, label %b27

b25:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b26:
  %163 = getelementptr inbounds i8, ptr %159, i16 0
  %164 = load ptr, ptr %163
  call cc 1005 addrspace(1) void @N$PS(ptr %164)
  %165 = getelementptr i8, ptr @$str17, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %165)
  %166 = load ptr, ptr %3, !tbaa !2
  %167 = getelementptr i8, ptr %166, i16 -4
  %168 = load i16, ptr %167
  %169 = icmp ult i16 2, %168
  br i1 %169, label %b28, label %b29

b27:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b28:
  %170 = getelementptr inbounds i8, ptr %166, i16 4
  %171 = load ptr, ptr %170
  call cc 1005 addrspace(1) void @N$PS(ptr %171)
  call cc 1005 addrspace(1) void @N$PN()
  %172 = load ptr, ptr %3, !tbaa !2
  %173 = icmp ne ptr %172, null
  br i1 %173, label %b31, label %b30

b29:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b30:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %172)
  %174 = load ptr, ptr %5, !tbaa !2
  %175 = icmp ne ptr %174, null
  br i1 %175, label %b36, label %b35

b31:
  %176 = getelementptr i8, ptr %172, i16 -4
  %177 = load i16, ptr %176
  store i16 0, ptr %2, !tbaa !2
  br label %b32

b32:
  %178 = load i16, ptr %2, !tbaa !2
  %179 = icmp ult i16 %178, %177
  br i1 %179, label %b34, label %b33

b33:
  br label %b30

b34:
  %180 = mul i16 %178, 2
  %181 = getelementptr inbounds i8, ptr %172, i16 %180
  %182 = load ptr, ptr %181
  call cc 1005 addrspace(1) void @N$BDRP(ptr %182)
  %183 = add i16 %178, 1
  store i16 %183, ptr %2, !tbaa !2
  br label %b32

b35:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %174)
  %184 = load ptr, ptr %7, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %184)
  %185 = load ptr, ptr %9, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %185)
  %186 = load ptr, ptr %11, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %186)
  %187 = load ptr, ptr %12, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %187)
  %188 = load ptr, ptr %14, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %188)
  %189 = load ptr, ptr %18, !tbaa !2
  %190 = icmp ne ptr %189, null
  br i1 %190, label %b41, label %b40

b36:
  %191 = getelementptr i8, ptr %174, i16 -4
  %192 = load i16, ptr %191
  store i16 0, ptr %1, !tbaa !2
  br label %b37

b37:
  %193 = load i16, ptr %1, !tbaa !2
  %194 = icmp ult i16 %193, %192
  br i1 %194, label %b39, label %b38

b38:
  br label %b35

b39:
  %195 = mul i16 %193, 2
  %196 = getelementptr inbounds i8, ptr %174, i16 %195
  %197 = load ptr, ptr %196
  call cc 1005 addrspace(1) void @N$BDRP(ptr %197)
  %198 = add i16 %193, 1
  store i16 %198, ptr %1, !tbaa !2
  br label %b37

b40:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %189)
  ret i16 0

b41:
  %199 = getelementptr i8, ptr %189, i16 -4
  %200 = load i16, ptr %199
  store i16 0, ptr %0, !tbaa !2
  br label %b42

b42:
  %201 = load i16, ptr %0, !tbaa !2
  %202 = icmp ult i16 %201, %200
  br i1 %202, label %b44, label %b43

b43:
  br label %b40

b44:
  %203 = mul i16 %201, 4
  %204 = getelementptr inbounds i8, ptr %189, i16 %203
  %205 = load ptr, ptr %204
  call cc 1005 addrspace(1) void @N$BDRP(ptr %205)
  %206 = add i16 %201, 1
  store i16 %206, ptr %0, !tbaa !2
  br label %b42
}

declare cc 1005 ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

declare cc 1005 ptr @N$TCAT(ptr, ptr) addrspace(1)

declare cc 1005 void @N$PBEG() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 ptr @N$PEND() addrspace(1)

declare cc 1005 void @N$PU2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 i16 @N$BSHR(ptr, i16) addrspace(1)

declare cc 1005 ptr @N$BCLN(ptr, i16) addrspace(1)

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
