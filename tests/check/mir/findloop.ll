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

define internal fastcc void @Catalog.add(ptr addrspace(5) nocapture %0, ptr addrspace(5) %1, i16 range(i16 1, 31) %2, i16 range(i16 0, 501) %3) addrspace(1) {
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

define internal fastcc void @find(ptr addrspace(5) nocapture %0, ptr addrspace(5) nocapture readonly %1, ptr addrspace(5) nocapture readonly %2, ptr addrspace(5) %3) addrspace(1) memory(read, argmem: readwrite, inaccessiblemem: readwrite) {
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
  %lsr.iv = phi ptr [ %7, %b1 ], [ %lsr.iv.next, %b4 ]
  %13 = phi i16 [ 0, %b1 ], [ %17, %b4 ]
  %14 = icmp ult i16 %13, %9
  br i1 %14, label %b3, label %b5

b3:
  %15 = load i16, ptr %8
  %16 = icmp ult i16 %13, %15
  br i1 %16, label %b6, label %b7

b4:
  %17 = add nuw i16 %13, 1
  %lsr.iv.next = getelementptr i8, ptr %lsr.iv, i16 6
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
  %24 = load ptr, ptr %lsr.iv
  %25 = getelementptr i8, ptr %24, i16 -4
  %26 = load i16, ptr %25
  %27 = addrspacecast ptr %24 to ptr addrspace(1)
  store i16 %26, ptr %6, !tbaa !2
  store i16 %26, ptr %10, !tbaa !2
  store ptr addrspace(1) %27, ptr %11, !tbaa !2
  %28 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %12, ptr addrspace(1) %4)
  %29 = icmp eq i8 %28, 0
  br i1 %29, label %b8, label %b4

b7:
  call addrspace(1) void @N$EBND()
  unreachable

b8:
  %30 = phi i16 [ %13, %b6 ]
  %31 = load i16, ptr %8
  %32 = icmp ult i16 %30, %31
  br i1 %32, label %b11, label %b12

b11:
  %33 = mul i16 %30, 6
  %34 = getelementptr inbounds i8, ptr %7, i16 %33
  %35 = addrspacecast ptr %34 to ptr addrspace(1)
  store i8 0, ptr addrspace(5) %0
  %36 = getelementptr i8, ptr addrspace(5) %0, i16 2
  store ptr addrspace(1) %35, ptr addrspace(5) %36
  ret void

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b13:
  %lsr.iv1 = phi ptr [ %18, %b5 ], [ %lsr.iv.next1, %b15 ]
  %37 = phi i16 [ 0, %b5 ], [ %41, %b15 ]
  %38 = icmp ult i16 %37, %20
  br i1 %38, label %b14, label %b16

b14:
  %39 = load i16, ptr %19
  %40 = icmp ult i16 %37, %39
  br i1 %40, label %b17, label %b18

b15:
  %41 = add nuw i16 %37, 1
  %lsr.iv.next1 = getelementptr i8, ptr %lsr.iv1, i16 6
  br label %b13

b16:
  store i8 1, ptr addrspace(5) %0
  ret void

b17:
  %42 = load ptr, ptr %lsr.iv1
  %43 = getelementptr i8, ptr %42, i16 -4
  %44 = load i16, ptr %43
  %45 = addrspacecast ptr %42 to ptr addrspace(1)
  store i16 %44, ptr %5, !tbaa !2
  store i16 %44, ptr %21, !tbaa !2
  store ptr addrspace(1) %45, ptr %22, !tbaa !2
  %46 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %23, ptr addrspace(1) %4)
  %47 = icmp eq i8 %46, 0
  br i1 %47, label %b19, label %b15

b18:
  call addrspace(1) void @N$EBND()
  unreachable

b19:
  %48 = phi i16 [ %37, %b17 ]
  %49 = load i16, ptr %19
  %50 = icmp ult i16 %48, %49
  br i1 %50, label %b22, label %b23

b22:
  %51 = mul i16 %48, 6
  %52 = getelementptr inbounds i8, ptr %18, i16 %51
  %53 = addrspacecast ptr %52 to ptr addrspace(1)
  store i8 0, ptr addrspace(5) %0
  %54 = getelementptr i8, ptr addrspace(5) %0, i16 2
  store ptr addrspace(1) %53, ptr addrspace(5) %54
  ret void

b23:
  call addrspace(1) void @N$EBND()
  unreachable
}

define i16 @main() addrspace(1) memory(readwrite, argmem: none) {
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
  call fastcc addrspace(1) void @Catalog.add(ptr addrspace(5) %18, ptr addrspace(5) %23, i16 5, i16 40)
  %24 = getelementptr i8, ptr @$str3, i16 6
  %25 = addrspacecast ptr %24 to ptr addrspace(1)
  store i16 4, ptr %4, !tbaa !2
  %26 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 4, ptr %26, !tbaa !2
  %27 = getelementptr inbounds i8, ptr %4, i16 4
  store ptr addrspace(1) %25, ptr %27, !tbaa !2
  %28 = addrspacecast ptr %4 to ptr addrspace(5)
  call fastcc addrspace(1) void @Catalog.add(ptr addrspace(5) %18, ptr addrspace(5) %28, i16 30, i16 3)
  %29 = getelementptr i8, ptr @$str4, i16 6
  %30 = addrspacecast ptr %29 to ptr addrspace(1)
  store i16 3, ptr %3, !tbaa !2
  %31 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 3, ptr %31, !tbaa !2
  %32 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %30, ptr %32, !tbaa !2
  %33 = addrspacecast ptr %3 to ptr addrspace(5)
  call fastcc addrspace(1) void @Catalog.add(ptr addrspace(5) %18, ptr addrspace(5) %33, i16 12, i16 0)
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
  call fastcc addrspace(1) void @Catalog.add(ptr addrspace(5) %35, ptr addrspace(5) %38, i16 28, i16 9)
  %39 = getelementptr i8, ptr @$str5, i16 6
  %40 = addrspacecast ptr %39 to ptr addrspace(1)
  store i16 3, ptr %0, !tbaa !2
  %41 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 3, ptr %41, !tbaa !2
  %42 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %40, ptr %42, !tbaa !2
  %43 = addrspacecast ptr %0 to ptr addrspace(5)
  call fastcc addrspace(1) void @Catalog.add(ptr addrspace(5) %35, ptr addrspace(5) %43, i16 2, i16 100)
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
  call fastcc addrspace(1) void @find(ptr addrspace(5) %45, ptr addrspace(5) %46, ptr addrspace(5) %47, ptr addrspace(5) %50)
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
  call fastcc addrspace(1) void @find(ptr addrspace(5) %53, ptr addrspace(5) %46, ptr addrspace(5) %47, ptr addrspace(5) %56)
  %57 = addrspacecast ptr %10 to ptr addrspace(5)
  store i16 4, ptr %9, !tbaa !2
  %58 = getelementptr inbounds i8, ptr %9, i16 2
  store i16 4, ptr %58, !tbaa !2
  %59 = getelementptr inbounds i8, ptr %9, i16 4
  store ptr addrspace(1) %25, ptr %59, !tbaa !2
  %60 = addrspacecast ptr %9 to ptr addrspace(5)
  call fastcc addrspace(1) void @find(ptr addrspace(5) %57, ptr addrspace(5) %47, ptr addrspace(5) %46, ptr addrspace(5) %60)
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
  %lsr.iv3 = phi ptr [ %78, %b6 ], [ %lsr.iv.next2, %84 ]
  %82 = phi i16 [ 0, %b6 ], [ %85, %84 ]
  %83 = icmp ult i16 %82, %80
  br i1 %83, label %89, label %98

84:
  %85 = add i16 %82, 1
  %lsr.iv.next2 = getelementptr i8, ptr %lsr.iv3, i16 6
  br label %81

86:
  %87 = phi i16 [ %99, %98 ], [ %101, %100 ]
  %88 = icmp ule i16 %87, %80
  br i1 %88, label %93, label %97

89:
  %90 = getelementptr i8, ptr %lsr.iv3, i16 2
  %91 = load i16, ptr %90
  %92 = icmp ule i16 %91, 20
  br i1 %92, label %84, label %100

93:
  %94 = addrspacecast ptr %8 to ptr addrspace(5)
  %95 = addrspacecast ptr %8 to ptr addrspace(1)
  %96 = icmp ne i16 %87, 0
  br i1 %96, label %b11, label %b12

97:
  call addrspace(1) void @N$EBND()
  unreachable

98:
  %99 = phi i16 [ %82, %81 ]
  br label %86

100:
  %101 = phi i16 [ %82, %89 ]
  br label %86

b7:
  %102 = getelementptr i8, ptr @$str10, i16 6
  call addrspace(1) void @N$PS(ptr %102)
  call addrspace(1) void @N$PN()
  br label %b6

b8:
  %103 = icmp eq i8 %64, 0
  br i1 %103, label %104, label %b7

104:
  %105 = getelementptr i8, ptr addrspace(1) %63, i16 2
  %106 = load i16, ptr addrspace(1) %105
  %107 = getelementptr i8, ptr addrspace(1) %66, i16 2
  %108 = load i16, ptr addrspace(1) %107
  %109 = icmp ule i16 %106, %108
  br i1 %109, label %110, label %111

110:
  br label %112

111:
  br label %112

112:
  %113 = phi ptr addrspace(1) [ %105, %110 ], [ %107, %111 ]
  %114 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %114)
  %115 = load i16, ptr addrspace(1) %113
  call addrspace(1) void @N$PU2(i16 %115)
  call addrspace(1) void @N$PN()
  br label %b6

b11:
  %116 = getelementptr inbounds i8, ptr %78, i16 0
  %117 = load ptr, ptr %116
  %118 = getelementptr i8, ptr %117, i16 -4
  %119 = load i16, ptr %118
  %120 = addrspacecast ptr %117 to ptr addrspace(1)
  %121 = icmp uge i16 %119, 1
  br i1 %121, label %122, label %134

122:
  store i16 1, ptr addrspace(5) %94
  %123 = getelementptr i8, ptr addrspace(5) %94, i16 2
  store i16 1, ptr addrspace(5) %123
  %124 = getelementptr i8, ptr addrspace(5) %94, i16 4
  store ptr addrspace(1) %120, ptr addrspace(5) %124
  call addrspace(1) void @N$PU2(i16 %87)
  %125 = getelementptr i8, ptr @$str11, i16 6
  call addrspace(1) void @N$PS(ptr %125)
  call addrspace(1) void @N$PV(ptr addrspace(1) %95)
  call addrspace(1) void @N$PN()
  %126 = load ptr, ptr %16, !tbaa !2
  %127 = getelementptr i8, ptr %126, i16 -4
  %128 = load i16, ptr %127
  %129 = getelementptr i8, ptr @$str12, i16 6
  %130 = getelementptr i8, ptr @$str13, i16 6
  %131 = getelementptr i8, ptr @$str14, i16 6
  %132 = sub i16 0, %128
  %133 = icmp ule i16 %128, 0
  br i1 %133, label %b17, label %177

134:
  call addrspace(1) void @N$EBND()
  unreachable

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b15:
  %lsr.iv41 = phi ptr [ %lsr.iv.next3, %b16 ], [ %126, %177 ]
  %lsr.iv51 = phi i16 [ %lsr.iv.next4, %b16 ], [ %132, %177 ]
  %135 = getelementptr i8, ptr %lsr.iv41, i16 4
  %136 = load i16, ptr %135
  %137 = icmp ult i16 %136, 5
  br i1 %137, label %b18, label %b16

b16:
  %lsr.iv.next3 = getelementptr i8, ptr %lsr.iv41, i16 6
  %lsr.iv.next4 = add i16 %lsr.iv51, 1
  %138 = icmp ne i16 %lsr.iv.next4, 0
  br i1 %138, label %b15, label %178

b17:
  %139 = getelementptr i8, ptr @$str15, i16 6
  %140 = addrspacecast ptr %139 to ptr addrspace(1)
  store i16 3, ptr %7, !tbaa !2
  %141 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 3, ptr %141, !tbaa !2
  %142 = getelementptr inbounds i8, ptr %7, i16 4
  store ptr addrspace(1) %140, ptr %142, !tbaa !2
  %143 = addrspacecast ptr %7 to ptr addrspace(5)
  call fastcc addrspace(1) void @Catalog.add(ptr addrspace(5) %46, ptr addrspace(5) %143, i16 1, i16 500)
  %144 = load ptr, ptr %16, !tbaa !2
  %145 = getelementptr i8, ptr %144, i16 -4
  %146 = load i16, ptr %145
  call addrspace(1) void @N$PU2(i16 %146)
  %147 = getelementptr i8, ptr @$str16, i16 6
  call addrspace(1) void @N$PS(ptr %147)
  call addrspace(1) void @N$PN()
  %148 = load ptr, ptr %15, !tbaa !2
  %149 = icmp ne ptr %148, null
  br i1 %149, label %b23, label %b22

b18:
  call addrspace(1) void @N$PS(ptr %129)
  %150 = load ptr, ptr %lsr.iv41
  call addrspace(1) void @N$PS(ptr %150)
  call addrspace(1) void @N$PS(ptr %130)
  %151 = getelementptr i8, ptr %lsr.iv41, i16 4
  %152 = load i16, ptr %151
  call addrspace(1) void @N$PU2(i16 %152)
  call addrspace(1) void @N$PS(ptr %131)
  call addrspace(1) void @N$PN()
  br label %b16

b22:
  call addrspace(1) void @N$BDRP(ptr %148)
  %153 = load ptr, ptr %16, !tbaa !2
  %154 = icmp ne ptr %153, null
  br i1 %154, label %b28, label %b27

b23:
  %155 = getelementptr i8, ptr %148, i16 -4
  %156 = load i16, ptr %155
  %157 = mul i16 %156, 6
  %158 = sub i16 0, %157
  %159 = getelementptr i8, ptr %148, i16 %157
  %160 = icmp ule i16 %156, 0
  br i1 %160, label %b25, label %173

b25:
  br label %b22

b26:
  %lsr.iv1 = phi i16 [ %lsr.iv.next, %b26 ], [ %158, %173 ]
  %161 = getelementptr i8, ptr %159, i16 %lsr.iv1
  %162 = load ptr, ptr %161
  call addrspace(1) void @N$BDRP(ptr %162)
  %lsr.iv.next = add i16 %lsr.iv1, 6
  %163 = icmp ne i16 %lsr.iv.next, 0
  br i1 %163, label %b26, label %174

b27:
  call addrspace(1) void @N$BDRP(ptr %153)
  ret i16 0

b28:
  %164 = getelementptr i8, ptr %153, i16 -4
  %165 = load i16, ptr %164
  %166 = mul i16 %165, 6
  %167 = sub i16 0, %166
  %168 = getelementptr i8, ptr %153, i16 %166
  %169 = icmp ule i16 %165, 0
  br i1 %169, label %b30, label %175

b30:
  br label %b27

b31:
  %lsr.iv21 = phi i16 [ %lsr.iv.next1, %b31 ], [ %167, %175 ]
  %170 = getelementptr i8, ptr %168, i16 %lsr.iv21
  %171 = load ptr, ptr %170
  call addrspace(1) void @N$BDRP(ptr %171)
  %lsr.iv.next1 = add i16 %lsr.iv21, 6
  %172 = icmp ne i16 %lsr.iv.next1, 0
  br i1 %172, label %b31, label %176

173:
  br label %b26

174:
  br label %b25

175:
  br label %b31

176:
  br label %b30

177:
  br label %b15

178:
  br label %b17
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
