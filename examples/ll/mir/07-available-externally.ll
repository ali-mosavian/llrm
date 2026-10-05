target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-n8:16:32"

@$str1 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str2 = internal constant [11 x i8] c"\08\00\04\00\04\00bolt\00"
@$str3 = internal constant [11 x i8] c"\08\00\04\00\04\00gear\00"
@$str4 = internal constant [10 x i8] c"\08\00\03\00\03\00cog\00"
@$str5 = internal constant [10 x i8] c"\08\00\03\00\03\00pin\00"
@$str6 = internal constant [9 x i8] c"\08\00\02\00\02\00: \00"
@$str7 = internal constant [11 x i8] c"\08\00\04\00\04\00 at \00"
@$str8 = internal constant [13 x i8] c"\08\00\06\00\06\00no pin\00"
@$str9 = internal constant [21 x i8] c"\08\00\0E\00\0E\00cheapest gear \00"
@$str10 = internal constant [14 x i8] c"\08\00\07\00\07\00no gear\00"
@$str11 = internal constant [24 x i8] c"\08\00\11\00\11\00 under 20, first \00"
@$str12 = internal constant [12 x i8] c"\08\00\05\00\05\00low: \00"
@$str13 = internal constant [9 x i8] c"\08\00\02\00\02\00 (\00"
@$str14 = internal constant [8 x i8] c"\08\00\01\00\01\00)\00"
@$str15 = internal constant [10 x i8] c"\08\00\03\00\03\00nut\00"
@$str16 = internal constant [13 x i8] c"\08\00\06\00\06\00 parts\00"

define internal void @Catalog.add(ptr addrspace(5) nocapture %0, ptr addrspace(5) %1, i16 range(i16 1, 31) %2, i16 range(i16 0, 501) %3) addrspace(1) nearcode {
b1:
  %4 = addrspacecast ptr addrspace(5) %1 to ptr addrspace(1)
  %5 = load ptr, ptr addrspace(5) %0
  %6 = getelementptr i8, ptr %5, i16 -4
  %7 = load i16, ptr %6
  %8 = call addrspace(1) ptr @N$BGRW(ptr %5, i16 1, i16 6)
  store ptr %8, ptr addrspace(5) %0
  %9 = mul i16 %7, 6
  %10 = getelementptr inbounds i8, ptr %8, i16 %9
  %11 = call addrspace(1) ptr @N$VCPY(ptr addrspace(1) %4)
  store ptr %11, ptr %10
  %12 = getelementptr i8, ptr %10, i16 2
  store i16 %2, ptr %12
  %13 = getelementptr i8, ptr %10, i16 4
  store i16 %3, ptr %13
  ret void
}

define internal void @find(ptr addrspace(5) nocapture %0, ptr addrspace(5) nocapture readonly %1, ptr addrspace(5) nocapture readonly %2, ptr addrspace(5) %3) addrspace(1) nearcode memory(read, argmem: readwrite, inaccessiblemem: readwrite) {
b1:
  %4 = addrspacecast ptr addrspace(5) %3 to ptr addrspace(1)
  %5 = alloca [8 x i8]
  %6 = alloca [8 x i8]
  %7 = load ptr, ptr addrspace(5) %1
  %8 = getelementptr i8, ptr %7, i16 -4
  %9 = load i16, ptr %8
  %10 = getelementptr inbounds i8, ptr %6, i16 2
  %11 = getelementptr inbounds i8, ptr %6, i16 4
  %12 = addrspacecast ptr %6 to ptr addrspace(1)
  br label %b2

b2:
  %13 = phi i16 [ 0, %b1 ], [ %17, %b4 ]
  %14 = icmp ult i16 %13, %9
  br i1 %14, label %b3, label %b5

b3:
  %15 = load i16, ptr %8
  %16 = icmp ult i16 %13, %15
  br i1 %16, label %b6, label %b7

b4:
  %17 = add nuw i16 %13, 1
  br label %b2

b5:
  %18 = load ptr, ptr addrspace(5) %2
  %19 = getelementptr i8, ptr %18, i16 -4
  %20 = load i16, ptr %19
  %21 = getelementptr inbounds i8, ptr %5, i16 2
  %22 = getelementptr inbounds i8, ptr %5, i16 4
  %23 = addrspacecast ptr %5 to ptr addrspace(1)
  br label %b13

b6:
  %24 = mul i16 %13, 6
  %25 = getelementptr inbounds i8, ptr %7, i16 %24
  %26 = load ptr, ptr %25
  %27 = getelementptr i8, ptr %26, i16 -4
  %28 = load i16, ptr %27
  %29 = addrspacecast ptr %26 to ptr addrspace(1)
  store i16 %28, ptr %6, !tbaa !2
  store i16 %28, ptr %10, !tbaa !2
  store ptr addrspace(1) %29, ptr %11, !tbaa !2
  %30 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %12, ptr addrspace(1) %4)
  %31 = icmp eq i8 %30, 0
  br i1 %31, label %b8, label %b4

b7:
  call addrspace(1) void @N$EBND()
  unreachable

b8:
  %32 = phi i16 [ %13, %b6 ]
  %33 = load i16, ptr %8
  %34 = icmp ult i16 %32, %33
  br i1 %34, label %b11, label %b12

b11:
  %35 = mul i16 %32, 6
  %36 = getelementptr inbounds i8, ptr %7, i16 %35
  %37 = addrspacecast ptr %36 to ptr addrspace(1)
  store i8 0, ptr addrspace(5) %0
  %38 = getelementptr i8, ptr addrspace(5) %0, i16 2
  store ptr addrspace(1) %37, ptr addrspace(5) %38
  ret void

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b13:
  %39 = phi i16 [ 0, %b5 ], [ %43, %b15 ]
  %40 = icmp ult i16 %39, %20
  br i1 %40, label %b14, label %b16

b14:
  %41 = load i16, ptr %19
  %42 = icmp ult i16 %39, %41
  br i1 %42, label %b17, label %b18

b15:
  %43 = add nuw i16 %39, 1
  br label %b13

b16:
  store i8 1, ptr addrspace(5) %0
  ret void

b17:
  %44 = mul i16 %39, 6
  %45 = getelementptr inbounds i8, ptr %18, i16 %44
  %46 = load ptr, ptr %45
  %47 = getelementptr i8, ptr %46, i16 -4
  %48 = load i16, ptr %47
  %49 = addrspacecast ptr %46 to ptr addrspace(1)
  store i16 %48, ptr %5, !tbaa !2
  store i16 %48, ptr %21, !tbaa !2
  store ptr addrspace(1) %49, ptr %22, !tbaa !2
  %50 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %23, ptr addrspace(1) %4)
  %51 = icmp eq i8 %50, 0
  br i1 %51, label %b19, label %b15

b18:
  call addrspace(1) void @N$EBND()
  unreachable

b19:
  %52 = phi i16 [ %39, %b17 ]
  %53 = load i16, ptr %19
  %54 = icmp ult i16 %52, %53
  br i1 %54, label %b22, label %b23

b22:
  %55 = mul i16 %52, 6
  %56 = getelementptr inbounds i8, ptr %18, i16 %55
  %57 = addrspacecast ptr %56 to ptr addrspace(1)
  store i8 0, ptr addrspace(5) %0
  %58 = getelementptr i8, ptr addrspace(5) %0, i16 2
  store ptr addrspace(1) %57, ptr addrspace(5) %58
  ret void

b23:
  call addrspace(1) void @N$EBND()
  unreachable
}

define i16 @main() addrspace(1) nearcode memory(readwrite, argmem: none) {
b1:
  %0 = alloca [8 x i8]
  %1 = alloca [8 x i8]
  %2 = alloca [2 x i8]
  %3 = alloca [8 x i8]
  %4 = alloca [8 x i8]
  %5 = alloca [8 x i8]
  %6 = alloca [2 x i8]
  %7 = alloca [8 x i8]
  %8 = alloca [8 x i8]
  %9 = alloca [8 x i8]
  %10 = alloca [6 x i8]
  %11 = alloca [8 x i8]
  %12 = alloca [6 x i8]
  %13 = alloca [8 x i8]
  %14 = alloca [6 x i8]
  %15 = alloca [2 x i8]
  %16 = alloca [2 x i8]
  %17 = getelementptr i8, ptr @$str1, i16 6
  store ptr %17, ptr %6, !tbaa !2
  %18 = addrspacecast ptr %6 to ptr addrspace(5)
  %19 = getelementptr i8, ptr @$str2, i16 6
  %20 = addrspacecast ptr %19 to ptr addrspace(1)
  store i16 4, ptr %5, !tbaa !2
  %21 = getelementptr inbounds i8, ptr %5, i16 2
  store i16 4, ptr %21, !tbaa !2
  %22 = getelementptr inbounds i8, ptr %5, i16 4
  store ptr addrspace(1) %20, ptr %22, !tbaa !2
  %23 = addrspacecast ptr %5 to ptr addrspace(5)
  call addrspace(1) void @Catalog.add(ptr addrspace(5) %18, ptr addrspace(5) %23, i16 5, i16 40)
  %24 = getelementptr i8, ptr @$str3, i16 6
  %25 = addrspacecast ptr %24 to ptr addrspace(1)
  store i16 4, ptr %4, !tbaa !2
  %26 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 4, ptr %26, !tbaa !2
  %27 = getelementptr inbounds i8, ptr %4, i16 4
  store ptr addrspace(1) %25, ptr %27, !tbaa !2
  %28 = addrspacecast ptr %4 to ptr addrspace(5)
  call addrspace(1) void @Catalog.add(ptr addrspace(5) %18, ptr addrspace(5) %28, i16 30, i16 3)
  %29 = getelementptr i8, ptr @$str4, i16 6
  %30 = addrspacecast ptr %29 to ptr addrspace(1)
  store i16 3, ptr %3, !tbaa !2
  %31 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 3, ptr %31, !tbaa !2
  %32 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %30, ptr %32, !tbaa !2
  %33 = addrspacecast ptr %3 to ptr addrspace(5)
  call addrspace(1) void @Catalog.add(ptr addrspace(5) %18, ptr addrspace(5) %33, i16 12, i16 0)
  %34 = load ptr, ptr %6, !tbaa !2
  store ptr null, ptr %6, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr null)
  store ptr %34, ptr %16, !tbaa !2
  store ptr %17, ptr %2, !tbaa !2
  %35 = addrspacecast ptr %2 to ptr addrspace(5)
  store i16 4, ptr %1, !tbaa !2
  %36 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 4, ptr %36, !tbaa !2
  %37 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %25, ptr %37, !tbaa !2
  %38 = addrspacecast ptr %1 to ptr addrspace(5)
  call addrspace(1) void @Catalog.add(ptr addrspace(5) %35, ptr addrspace(5) %38, i16 28, i16 9)
  %39 = getelementptr i8, ptr @$str5, i16 6
  %40 = addrspacecast ptr %39 to ptr addrspace(1)
  store i16 3, ptr %0, !tbaa !2
  %41 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 3, ptr %41, !tbaa !2
  %42 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %40, ptr %42, !tbaa !2
  %43 = addrspacecast ptr %0 to ptr addrspace(5)
  call addrspace(1) void @Catalog.add(ptr addrspace(5) %35, ptr addrspace(5) %43, i16 2, i16 100)
  %44 = load ptr, ptr %2, !tbaa !2
  store ptr null, ptr %2, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr null)
  store ptr %44, ptr %15, !tbaa !2
  %45 = addrspacecast ptr %14 to ptr addrspace(5)
  %46 = addrspacecast ptr %16 to ptr addrspace(5)
  %47 = addrspacecast ptr %15 to ptr addrspace(5)
  store i16 3, ptr %13, !tbaa !2
  %48 = getelementptr inbounds i8, ptr %13, i16 2
  store i16 3, ptr %48, !tbaa !2
  %49 = getelementptr inbounds i8, ptr %13, i16 4
  store ptr addrspace(1) %40, ptr %49, !tbaa !2
  %50 = addrspacecast ptr %13 to ptr addrspace(5)
  call addrspace(1) void @find(ptr addrspace(5) %45, ptr addrspace(5) %46, ptr addrspace(5) %47, ptr addrspace(5) %50)
  %51 = load i8, ptr %14, !tbaa !2, !range !7
  %52 = icmp eq i8 %51, 0
  br i1 %52, label %b4, label %b3

b2:
  %53 = addrspacecast ptr %12 to ptr addrspace(5)
  store i16 4, ptr %11, !tbaa !2
  %54 = getelementptr inbounds i8, ptr %11, i16 2
  store i16 4, ptr %54, !tbaa !2
  %55 = getelementptr inbounds i8, ptr %11, i16 4
  store ptr addrspace(1) %25, ptr %55, !tbaa !2
  %56 = addrspacecast ptr %11 to ptr addrspace(5)
  call addrspace(1) void @find(ptr addrspace(5) %53, ptr addrspace(5) %46, ptr addrspace(5) %47, ptr addrspace(5) %56)
  %57 = addrspacecast ptr %10 to ptr addrspace(5)
  store i16 4, ptr %9, !tbaa !2
  %58 = getelementptr inbounds i8, ptr %9, i16 2
  store i16 4, ptr %58, !tbaa !2
  %59 = getelementptr inbounds i8, ptr %9, i16 4
  store ptr addrspace(1) %25, ptr %59, !tbaa !2
  %60 = addrspacecast ptr %9 to ptr addrspace(5)
  call addrspace(1) void @find(ptr addrspace(5) %57, ptr addrspace(5) %47, ptr addrspace(5) %46, ptr addrspace(5) %60)
  %61 = load i8, ptr %12
  %62 = getelementptr i8, ptr %12, i16 2
  %63 = load ptr addrspace(1), ptr %62
  %64 = load i8, ptr %10
  %65 = getelementptr i8, ptr %10, i16 2
  %66 = load ptr addrspace(1), ptr %65
  %67 = icmp eq i8 %61, 0
  br i1 %67, label %b8, label %b7

b3:
  %68 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @N$PS(ptr %68)
  call addrspace(1) void @N$PN()
  br label %b2

b4:
  %69 = getelementptr inbounds i8, ptr %14, i16 2
  %70 = load ptr addrspace(1), ptr %69, !tbaa !2
  %71 = load ptr, ptr addrspace(1) %70
  call addrspace(1) void @N$PS(ptr %71)
  %72 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @N$PS(ptr %72)
  %73 = getelementptr i8, ptr addrspace(1) %70, i16 4
  %74 = load i16, ptr addrspace(1) %73
  call addrspace(1) void @N$PU2(i16 %74)
  %75 = getelementptr i8, ptr @$str7, i16 6
  call addrspace(1) void @N$PS(ptr %75)
  %76 = getelementptr i8, ptr addrspace(1) %70, i16 2
  %77 = load i16, ptr addrspace(1) %76
  call addrspace(1) void @N$PU2(i16 %77)
  call addrspace(1) void @N$PN()
  br label %b2

b6:
  %78 = load ptr, ptr addrspace(5) %46
  %79 = getelementptr i8, ptr %78, i16 -4
  %80 = load i16, ptr %79
  br label %81

81:
  %82 = phi i16 [ 0, %b6 ], [ %85, %84 ]
  %83 = icmp ult i16 %82, %80
  br i1 %83, label %89, label %100

84:
  %85 = add i16 %82, 1
  br label %81

86:
  %87 = phi i16 [ %101, %100 ], [ %103, %102 ]
  %88 = icmp ule i16 %87, %80
  br i1 %88, label %95, label %99

89:
  %90 = mul i16 %82, 6
  %91 = getelementptr inbounds i8, ptr %78, i16 %90
  %92 = getelementptr i8, ptr %91, i16 2
  %93 = load i16, ptr %92
  %94 = icmp ule i16 %93, 20
  br i1 %94, label %84, label %102

95:
  %96 = addrspacecast ptr %8 to ptr addrspace(5)
  %97 = addrspacecast ptr %8 to ptr addrspace(1)
  %98 = icmp ne i16 %87, 0
  br i1 %98, label %b11, label %b12

99:
  call addrspace(1) void @N$EBND()
  unreachable

100:
  %101 = phi i16 [ %82, %81 ]
  br label %86

102:
  %103 = phi i16 [ %82, %89 ]
  br label %86

b7:
  %104 = getelementptr i8, ptr @$str10, i16 6
  call addrspace(1) void @N$PS(ptr %104)
  call addrspace(1) void @N$PN()
  br label %b6

b8:
  %105 = icmp eq i8 %64, 0
  br i1 %105, label %106, label %b7

106:
  %107 = getelementptr i8, ptr addrspace(1) %63, i16 2
  %108 = load i16, ptr addrspace(1) %107
  %109 = getelementptr i8, ptr addrspace(1) %66, i16 2
  %110 = load i16, ptr addrspace(1) %109
  %111 = icmp ule i16 %108, %110
  br i1 %111, label %112, label %113

112:
  br label %114

113:
  br label %114

114:
  %115 = phi ptr addrspace(1) [ %107, %112 ], [ %109, %113 ]
  %116 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %116)
  %117 = load i16, ptr addrspace(1) %115
  call addrspace(1) void @N$PU2(i16 %117)
  call addrspace(1) void @N$PN()
  br label %b6

b11:
  %118 = getelementptr inbounds i8, ptr %78, i16 0
  %119 = load ptr, ptr %118
  %120 = getelementptr i8, ptr %119, i16 -4
  %121 = load i16, ptr %120
  %122 = addrspacecast ptr %119 to ptr addrspace(1)
  %123 = icmp uge i16 %121, 1
  br i1 %123, label %124, label %134

124:
  store i16 1, ptr addrspace(5) %96
  %125 = getelementptr i8, ptr addrspace(5) %96, i16 2
  store i16 1, ptr addrspace(5) %125
  %126 = getelementptr i8, ptr addrspace(5) %96, i16 4
  store ptr addrspace(1) %122, ptr addrspace(5) %126
  call addrspace(1) void @N$PU2(i16 %87)
  %127 = getelementptr i8, ptr @$str11, i16 6
  call addrspace(1) void @N$PS(ptr %127)
  call addrspace(1) void @N$PV(ptr addrspace(1) %97)
  call addrspace(1) void @N$PN()
  %128 = load ptr, ptr %16, !tbaa !2
  %129 = getelementptr i8, ptr %128, i16 -4
  %130 = load i16, ptr %129
  %131 = getelementptr i8, ptr @$str12, i16 6
  %132 = getelementptr i8, ptr @$str13, i16 6
  %133 = getelementptr i8, ptr @$str14, i16 6
  br label %b14

134:
  call addrspace(1) void @N$EBND()
  unreachable

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b14:
  %135 = phi i16 [ 0, %124 ], [ %142, %b16 ]
  %136 = icmp ult i16 %135, %130
  br i1 %136, label %b15, label %b17

b15:
  %137 = mul i16 %135, 6
  %138 = getelementptr inbounds i8, ptr %128, i16 %137
  %139 = getelementptr i8, ptr %138, i16 4
  %140 = load i16, ptr %139
  %141 = icmp ult i16 %140, 5
  br i1 %141, label %b18, label %b16

b16:
  %142 = add i16 %135, 1
  br label %b14

b17:
  %143 = getelementptr i8, ptr @$str15, i16 6
  %144 = addrspacecast ptr %143 to ptr addrspace(1)
  store i16 3, ptr %7, !tbaa !2
  %145 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 3, ptr %145, !tbaa !2
  %146 = getelementptr inbounds i8, ptr %7, i16 4
  store ptr addrspace(1) %144, ptr %146, !tbaa !2
  %147 = addrspacecast ptr %7 to ptr addrspace(5)
  call addrspace(1) void @Catalog.add(ptr addrspace(5) %46, ptr addrspace(5) %147, i16 1, i16 500)
  %148 = load ptr, ptr %16, !tbaa !2
  %149 = getelementptr i8, ptr %148, i16 -4
  %150 = load i16, ptr %149
  call addrspace(1) void @N$PU2(i16 %150)
  %151 = getelementptr i8, ptr @$str16, i16 6
  call addrspace(1) void @N$PS(ptr %151)
  call addrspace(1) void @N$PN()
  %152 = load ptr, ptr %15, !tbaa !2
  %153 = icmp ne ptr %152, null
  br i1 %153, label %b23, label %b22

b18:
  call addrspace(1) void @N$PS(ptr %131)
  %154 = load ptr, ptr %138
  call addrspace(1) void @N$PS(ptr %154)
  call addrspace(1) void @N$PS(ptr %132)
  %155 = load i16, ptr %139
  call addrspace(1) void @N$PU2(i16 %155)
  call addrspace(1) void @N$PS(ptr %133)
  call addrspace(1) void @N$PN()
  br label %b16

b22:
  call addrspace(1) void @N$BDRP(ptr %152)
  %156 = load ptr, ptr %16, !tbaa !2
  %157 = icmp ne ptr %156, null
  br i1 %157, label %b28, label %b27

b23:
  %158 = getelementptr i8, ptr %152, i16 -4
  %159 = load i16, ptr %158
  br label %b24

b24:
  %160 = phi i16 [ 0, %b23 ], [ %165, %b26 ]
  %161 = icmp ult i16 %160, %159
  br i1 %161, label %b26, label %b25

b25:
  br label %b22

b26:
  %162 = mul i16 %160, 6
  %163 = getelementptr inbounds i8, ptr %152, i16 %162
  %164 = load ptr, ptr %163
  call addrspace(1) void @N$BDRP(ptr %164)
  %165 = add i16 %160, 1
  br label %b24

b27:
  call addrspace(1) void @N$BDRP(ptr %156)
  ret i16 0

b28:
  %166 = getelementptr i8, ptr %156, i16 -4
  %167 = load i16, ptr %166
  br label %b29

b29:
  %168 = phi i16 [ 0, %b28 ], [ %173, %b31 ]
  %169 = icmp ult i16 %168, %167
  br i1 %169, label %b31, label %b30

b30:
  br label %b27

b31:
  %170 = mul i16 %168, 6
  %171 = getelementptr inbounds i8, ptr %156, i16 %170
  %172 = load ptr, ptr %171
  call addrspace(1) void @N$BDRP(ptr %172)
  %173 = add i16 %168, 1
  br label %b29
}

declare ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare ptr @N$VCPY(ptr addrspace(1)) addrspace(1)

declare void @N$BDRP(ptr) addrspace(1)

declare i8 @N$VCMP(ptr addrspace(1), ptr addrspace(1)) addrspace(1) memory(read)

declare void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare void @llvm.memcpy.p0.p0.i16(ptr nocapture writeonly, ptr nocapture readonly, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: readwrite)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare void @N$PU2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare void @N$PV(ptr addrspace(1)) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i8 0, i8 2}
!6 = !{i16 0, i16 10923}
!7 = !{i8 0, i8 2}
