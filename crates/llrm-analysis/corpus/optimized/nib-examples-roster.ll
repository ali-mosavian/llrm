target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [9 x i8] c"\08\00\02\00\02\00 (\00"
@$str2 = internal constant [8 x i8] c"\08\00\01\00\01\00)\00"
@$str3 = internal constant [13 x i8] c"\08\00\06\00\06\00master\00"
@$str4 = internal constant [13 x i8] c"\08\00\06\00\06\00expert\00"
@$str5 = internal constant [11 x i8] c"\08\00\04\00\04\00club\00"
@$str6 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str7 = internal constant [10 x i8] c"\08\00\03\00\03\00ada\00"
@$str8 = internal constant [10 x i8] c"\08\00\03\00\03\00bob\00"
@$str9 = internal constant [9 x i8] c"\08\00\02\00\02\00cy\00"
@$str10 = internal constant [10 x i8] c"\08\00\03\00\03\00dee\00"
@$str11 = internal constant [17 x i8] c"\08\00\0A\00\0A\00signed dee\00"
@$str12 = internal constant [22 x i8] c"\08\00\0F\00\0F\00refused rating \00"
@$str13 = internal constant [9 x i8] c"\08\00\02\00\02\00 *\00"
@$str14 = internal constant [8 x i8] c"\08\00\01\00\01\00 \00"
@$str15 = internal constant [26 x i8] c"\08\00\13\00\13\00 above 1500, first \00"
@$str16 = internal constant [11 x i8] c"\08\00\04\00\04\00 at \00"

define internal void @Player.new(ptr addrspace(1) %0, ptr addrspace(1) noalias readonly dereferenceable(8) %1, i16 %2) addrspace(1) {
b1:
  %3 = icmp slt i16 %2, 0
  br i1 %3, label %b2, label %b3

b2:
  store i8 1, ptr addrspace(1) %0
  %4 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 0, ptr addrspace(1) %4
  %5 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %2, ptr addrspace(1) %5
  ret void

b3:
  %6 = call addrspace(1) ptr @N$VCPY(ptr addrspace(1) %1)
  store i8 0, ptr addrspace(1) %0
  %7 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr %6, ptr addrspace(1) %7
  %8 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %2, ptr addrspace(1) %8
  ret void
}

define internal ptr @Player.display(ptr addrspace(1) %0) addrspace(1) {
b1:
  call addrspace(1) void @N$PBEG()
  %1 = load ptr, ptr addrspace(1) %0
  call addrspace(1) void @N$PS(ptr %1)
  %2 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %2)
  %3 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %4 = load i16, ptr addrspace(1) %3
  call addrspace(1) void @N$PI2(i16 %4)
  %5 = getelementptr i8, ptr @$str2, i16 6
  call addrspace(1) void @N$PS(ptr %5)
  %6 = call addrspace(1) ptr @N$PEND()
  ret ptr %6
}

define internal ptr @Player.grade(ptr addrspace(1) %0) addrspace(1) memory(argmem: read) willreturn {
b1:
  %1 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %2 = load i16, ptr addrspace(1) %1
  %3 = icmp sge i16 %2, 2000
  br i1 %3, label %b2, label %b3

b2:
  %4 = getelementptr i8, ptr @$str3, i16 6
  ret ptr %4

b3:
  %5 = icmp sge i16 %2, 1500
  br i1 %5, label %b5, label %b6

b5:
  %6 = getelementptr i8, ptr @$str4, i16 6
  ret ptr %6

b6:
  %7 = getelementptr i8, ptr @$str5, i16 6
  ret ptr %7
}

define internal ptr addrspace(1) @best(ptr addrspace(1) %0) addrspace(1) {
b1:
  %1 = load ptr, ptr addrspace(1) %0
  %2 = getelementptr i8, ptr %1, i16 -4
  %3 = load i16, ptr %2
  br label %b2

b2:
  %4 = phi i16 [ 0, %b1 ], [ %25, %b12 ]
  %5 = phi i16 [ 0, %b1 ], [ %26, %b12 ]
  %6 = icmp ult i16 %5, %3
  br i1 %6, label %b3, label %b5

b3:
  %7 = load ptr, ptr addrspace(1) %0
  %8 = getelementptr i8, ptr %7, i16 -4
  %9 = load i16, ptr %8
  %10 = icmp ult i16 %5, %9
  br i1 %10, label %b6, label %b7

b5:
  %11 = load ptr, ptr addrspace(1) %0
  %12 = getelementptr i8, ptr %11, i16 -4
  %13 = load i16, ptr %12
  %14 = icmp ult i16 %4, %13
  br i1 %14, label %b13, label %b14

b6:
  %15 = shl i16 %5, 2
  %16 = getelementptr i8, ptr %7, i16 %15
  %17 = getelementptr i8, ptr %16, i16 2
  %18 = load i16, ptr %17
  %19 = icmp ult i16 %4, %9
  br i1 %19, label %b8, label %b9

b7:
  call addrspace(1) void @N$EBND()
  unreachable

b8:
  %20 = shl i16 %4, 2
  %21 = getelementptr i8, ptr %7, i16 %20
  %22 = getelementptr i8, ptr %21, i16 2
  %23 = load i16, ptr %22
  %24 = icmp sgt i16 %18, %23
  br i1 %24, label %b12, label %b11

b9:
  call addrspace(1) void @N$EBND()
  unreachable

b11:
  br label %b12

b12:
  %25 = phi i16 [ %5, %b8 ], [ %4, %b11 ]
  %26 = add i16 %5, 1
  br label %b2

b13:
  %27 = shl i16 %4, 2
  %28 = getelementptr i8, ptr %11, i16 %27
  %29 = addrspacecast ptr %28 to ptr addrspace(1)
  ret ptr addrspace(1) %29

b14:
  call addrspace(1) void @N$EBND()
  unreachable
}

define internal void @sign_up(ptr addrspace(1) %0, ptr addrspace(1) %1, ptr addrspace(1) noalias readonly dereferenceable(8) %2, i16 %3) addrspace(1) {
b1:
  %4 = alloca [6 x i8]
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 6, i1 false)
  %5 = addrspacecast ptr %4 to ptr addrspace(1)
  %6 = icmp slt i16 %3, 0
  br i1 %6, label %7, label %10

7:
  store i8 1, ptr addrspace(1) %5
  %8 = getelementptr i8, ptr addrspace(1) %5, i16 2
  store i8 0, ptr addrspace(1) %8
  %9 = getelementptr i8, ptr addrspace(1) %5, i16 4
  store i16 %3, ptr addrspace(1) %9
  br label %14

10:
  %11 = call addrspace(1) ptr @N$VCPY(ptr addrspace(1) %2)
  store i8 0, ptr addrspace(1) %5
  %12 = getelementptr i8, ptr addrspace(1) %5, i16 2
  store ptr %11, ptr addrspace(1) %12
  %13 = getelementptr i8, ptr addrspace(1) %5, i16 4
  store i16 %3, ptr addrspace(1) %13
  br label %14

14:
  %15 = load i8, ptr %4, !tbaa !2
  %16 = icmp eq i8 %15, 1
  br i1 %16, label %b2, label %b3

b2:
  %17 = getelementptr inbounds i8, ptr %4, i16 2
  %18 = load i16, ptr %17, !tbaa !2
  %19 = getelementptr inbounds i8, ptr %4, i16 4
  %20 = load i16, ptr %19, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %21 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %18, ptr addrspace(1) %21
  %22 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %20, ptr addrspace(1) %22
  ret void

b3:
  %23 = getelementptr inbounds i8, ptr %4, i16 2
  %24 = load ptr, ptr %23, !tbaa !2
  %25 = getelementptr inbounds i8, ptr %4, i16 4
  %26 = load i16, ptr %25, !tbaa !2
  %27 = load ptr, ptr addrspace(1) %1
  %28 = getelementptr i8, ptr %27, i16 -4
  %29 = load i16, ptr %28
  %30 = call addrspace(1) ptr @N$BGRW(ptr %27, i16 1, i16 4)
  store ptr %30, ptr addrspace(1) %1
  %31 = shl i16 %29, 2
  %32 = getelementptr i8, ptr %30, i16 %31
  store ptr %24, ptr %32
  %33 = getelementptr i8, ptr %32, i16 2
  store i16 %26, ptr %33
  store i8 0, ptr addrspace(1) %0
  call addrspace(1) void @N$BDRP(ptr null)
  ret void
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca [8 x i8]
  %1 = alloca [6 x i8]
  %2 = alloca [8 x i8]
  %3 = alloca [6 x i8]
  %4 = alloca ptr
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 6, i1 false)
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 6, i1 false)
  store ptr null, ptr %4
  %5 = getelementptr i8, ptr @$str6, i16 6
  store ptr %5, ptr %4, !tbaa !2
  %6 = call addrspace(1) ptr @N$BGRW(ptr %5, i16 3, i16 4)
  %7 = getelementptr i8, ptr %6, i16 0
  %8 = getelementptr i8, ptr @$str7, i16 6
  store ptr %8, ptr %7
  %9 = getelementptr i8, ptr %7, i16 2
  store i16 2150, ptr %9
  %10 = getelementptr i8, ptr %6, i16 4
  %11 = getelementptr i8, ptr @$str8, i16 6
  store ptr %11, ptr %10
  %12 = getelementptr i8, ptr %10, i16 2
  store i16 1480, ptr %12
  %13 = getelementptr i8, ptr %6, i16 8
  %14 = getelementptr i8, ptr @$str9, i16 6
  store ptr %14, ptr %13
  %15 = getelementptr i8, ptr %13, i16 2
  store i16 1620, ptr %15
  %16 = getelementptr i8, ptr %6, i16 -4
  %17 = load i16, ptr %16
  %18 = addrspacecast ptr %3 to ptr addrspace(1)
  %19 = addrspacecast ptr %4 to ptr addrspace(1)
  %20 = getelementptr inbounds i8, ptr %2, i16 2
  %21 = getelementptr inbounds i8, ptr %2, i16 4
  %22 = addrspacecast ptr %2 to ptr addrspace(1)
  br label %b2

b2:
  %23 = phi i16 [ 0, %b1 ], [ %34, %b3 ]
  %24 = icmp ult i16 %23, %17
  br i1 %24, label %b3, label %b5

b3:
  %25 = shl i16 %23, 2
  %26 = getelementptr i8, ptr %6, i16 %25
  %27 = getelementptr i8, ptr %26, i16 2
  %28 = addrspacecast ptr %27 to ptr addrspace(1)
  %29 = load ptr, ptr %26
  %30 = getelementptr i8, ptr %29, i16 -4
  %31 = load i16, ptr %30
  %32 = addrspacecast ptr %29 to ptr addrspace(1)
  store i16 %31, ptr %2, !tbaa !2
  store i16 %31, ptr %20, !tbaa !2
  store ptr addrspace(1) %32, ptr %21, !tbaa !2
  %33 = load i16, ptr addrspace(1) %28
  call addrspace(1) void @sign_up(ptr addrspace(1) %18, ptr addrspace(1) %19, ptr addrspace(1) %22, i16 %33)
  %34 = add i16 %23, 1
  br label %b2

b5:
  %35 = icmp ne ptr %6, null
  br i1 %35, label %b7, label %b6

b6:
  call addrspace(1) void @N$BDRP(ptr %6)
  %36 = addrspacecast ptr %1 to ptr addrspace(1)
  %37 = getelementptr i8, ptr @$str10, i16 6
  %38 = getelementptr i8, ptr %37, i16 -4
  %39 = load i16, ptr %38
  %40 = addrspacecast ptr %37 to ptr addrspace(1)
  store i16 %39, ptr %0, !tbaa !2
  %41 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 %39, ptr %41, !tbaa !2
  %42 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %40, ptr %42, !tbaa !2
  %43 = addrspacecast ptr %0 to ptr addrspace(1)
  call addrspace(1) void @sign_up(ptr addrspace(1) %36, ptr addrspace(1) %19, ptr addrspace(1) %43, i16 -3)
  %44 = load i8, ptr %1, !tbaa !2
  %45 = icmp eq i8 %44, 0
  br i1 %45, label %b13, label %b12

b7:
  %46 = load i16, ptr %16
  br label %b8

b8:
  %47 = phi i16 [ 0, %b7 ], [ %52, %b10 ]
  %48 = icmp ult i16 %47, %46
  br i1 %48, label %b10, label %b6

b10:
  %49 = shl i16 %47, 2
  %50 = getelementptr i8, ptr %6, i16 %49
  %51 = load ptr, ptr %50
  call addrspace(1) void @N$BDRP(ptr %51)
  %52 = add i16 %47, 1
  br label %b8

b11:
  %53 = load ptr, ptr %4, !tbaa !2
  %54 = getelementptr i8, ptr %53, i16 -4
  %55 = load i16, ptr %54
  %56 = icmp ugt i16 %55, 1
  br i1 %56, label %b15, label %b16

b12:
  %57 = getelementptr inbounds i8, ptr %1, i16 4
  %58 = load i16, ptr %57, !tbaa !2
  %59 = getelementptr i8, ptr @$str12, i16 6
  call addrspace(1) void @N$PS(ptr %59)
  call addrspace(1) void @N$PI2(i16 %58)
  call addrspace(1) void @N$PN()
  br label %b11

b13:
  %60 = getelementptr i8, ptr @$str11, i16 6
  call addrspace(1) void @N$PS(ptr %60)
  call addrspace(1) void @N$PN()
  br label %b11

b15:
  %61 = getelementptr i8, ptr %53, i16 4
  %62 = getelementptr i8, ptr %61, i16 2
  %63 = load i16, ptr %62
  %64 = add i16 %63, 25
  store i16 %64, ptr %62
  %65 = call addrspace(1) ptr addrspace(1) @best(ptr addrspace(1) %19)
  %66 = load ptr, ptr %4, !tbaa !2
  %67 = getelementptr i8, ptr %66, i16 -4
  %68 = load i16, ptr %67
  %69 = getelementptr i8, ptr @$str1, i16 6
  %70 = getelementptr i8, ptr @$str2, i16 6
  %71 = getelementptr i8, ptr @$str14, i16 6
  %72 = getelementptr i8, ptr @$str5, i16 6
  %73 = getelementptr i8, ptr @$str4, i16 6
  %74 = getelementptr i8, ptr @$str3, i16 6
  %75 = getelementptr i8, ptr @$str13, i16 6
  br label %b17

b16:
  call addrspace(1) void @N$EBND()
  unreachable

b17:
  %76 = phi i16 [ 0, %b15 ], [ %98, %96 ]
  %77 = icmp ult i16 %76, %68
  br i1 %77, label %b18, label %b20

b18:
  %78 = shl i16 %76, 2
  %79 = getelementptr i8, ptr %66, i16 %78
  %80 = addrspacecast ptr %79 to ptr addrspace(1)
  %81 = icmp eq ptr addrspace(1) %80, %65
  br i1 %81, label %b23, label %b22

b20:
  %82 = load ptr, ptr %4, !tbaa !2
  %83 = getelementptr i8, ptr %82, i16 -4
  %84 = load i16, ptr %83
  br label %b24

b22:
  br label %b23

b23:
  %85 = phi ptr [ %75, %b18 ], [ %5, %b22 ]
  call addrspace(1) void @N$PBEG()
  %86 = load ptr, ptr addrspace(1) %80
  call addrspace(1) void @N$PS(ptr %86)
  call addrspace(1) void @N$PS(ptr %69)
  %87 = getelementptr i8, ptr addrspace(1) %80, i16 2
  %88 = load i16, ptr addrspace(1) %87
  call addrspace(1) void @N$PI2(i16 %88)
  call addrspace(1) void @N$PS(ptr %70)
  %89 = call addrspace(1) ptr @N$PEND()
  %90 = load i16, ptr addrspace(1) %87
  %91 = icmp sge i16 %90, 2000
  br i1 %91, label %96, label %92

92:
  %93 = load i16, ptr addrspace(1) %87
  %94 = icmp sge i16 %93, 1500
  br i1 %94, label %96, label %95

95:
  br label %96

96:
  %97 = phi ptr [ %74, %b23 ], [ %73, %92 ], [ %72, %95 ]
  call addrspace(1) void @N$PS(ptr %89)
  call addrspace(1) void @N$PS(ptr %71)
  call addrspace(1) void @N$PS(ptr %97)
  call addrspace(1) void @N$PS(ptr %85)
  call addrspace(1) void @N$PN()
  call addrspace(1) void @N$BDRP(ptr %97)
  call addrspace(1) void @N$BDRP(ptr %89)
  call addrspace(1) void @N$BDRP(ptr %85)
  %98 = add i16 %76, 1
  br label %b17

b24:
  %99 = phi ptr [ %5, %b20 ], [ %123, %b32 ]
  %100 = phi i16 [ 0, %b20 ], [ %124, %b32 ]
  %101 = icmp ult i16 %100, %84
  br i1 %101, label %b25, label %b27

b25:
  %102 = load ptr, ptr %4, !tbaa !2
  %103 = getelementptr i8, ptr %102, i16 -4
  %104 = load i16, ptr %103
  %105 = icmp ult i16 %100, %104
  br i1 %105, label %b28, label %b29

b27:
  %106 = getelementptr i8, ptr %99, i16 -4
  %107 = load i16, ptr %106
  %108 = icmp ugt i16 %107, 0
  br i1 %108, label %b35, label %b36

b28:
  %109 = shl i16 %100, 2
  %110 = getelementptr i8, ptr %102, i16 %109
  %111 = getelementptr i8, ptr %110, i16 2
  %112 = load i16, ptr %111
  %113 = icmp sgt i16 %112, 1500
  br i1 %113, label %b30, label %b32

b29:
  call addrspace(1) void @N$EBND()
  unreachable

b30:
  %114 = getelementptr i8, ptr %99, i16 -4
  %115 = load i16, ptr %114
  %116 = call addrspace(1) ptr @N$BGRW(ptr %99, i16 1, i16 4)
  %117 = shl i16 %115, 2
  %118 = getelementptr i8, ptr %116, i16 %117
  %119 = load ptr, ptr %4, !tbaa !2
  %120 = getelementptr i8, ptr %119, i16 -4
  %121 = load i16, ptr %120
  %122 = icmp ult i16 %100, %121
  br i1 %122, label %b33, label %b34

b32:
  %123 = phi ptr [ %99, %b28 ], [ %116, %b33 ]
  %124 = add i16 %100, 1
  br label %b24

b33:
  %125 = getelementptr i8, ptr %119, i16 %109
  %126 = getelementptr i8, ptr %125, i16 2
  %127 = load i16, ptr %126
  store i16 %127, ptr %118
  %128 = getelementptr i8, ptr %118, i16 2
  store i16 %100, ptr %128
  br label %b32

b34:
  call addrspace(1) void @N$EBND()
  unreachable

b35:
  %129 = getelementptr i8, ptr %99, i16 0
  %130 = getelementptr i8, ptr %129, i16 2
  %131 = addrspacecast ptr %130 to ptr addrspace(1)
  call addrspace(1) void @N$PU2(i16 %107)
  %132 = getelementptr i8, ptr @$str15, i16 6
  call addrspace(1) void @N$PS(ptr %132)
  %133 = load ptr, ptr %4, !tbaa !2
  %134 = load i16, ptr addrspace(1) %131
  %135 = getelementptr i8, ptr %133, i16 -4
  %136 = load i16, ptr %135
  %137 = icmp ult i16 %134, %136
  br i1 %137, label %b37, label %b38

b36:
  call addrspace(1) void @N$EBND()
  unreachable

b37:
  %138 = shl i16 %134, 2
  %139 = getelementptr i8, ptr %133, i16 %138
  %140 = load ptr, ptr %139
  call addrspace(1) void @N$PS(ptr %140)
  %141 = getelementptr i8, ptr @$str16, i16 6
  call addrspace(1) void @N$PS(ptr %141)
  %142 = load i16, ptr %129
  call addrspace(1) void @N$PI2(i16 %142)
  call addrspace(1) void @N$PN()
  call addrspace(1) void @N$BDRP(ptr %99)
  call addrspace(1) void @N$BDRP(ptr null)
  %143 = load ptr, ptr %4, !tbaa !2
  %144 = icmp ne ptr %143, null
  br i1 %144, label %b40, label %b39

b38:
  call addrspace(1) void @N$EBND()
  unreachable

b39:
  call addrspace(1) void @N$BDRP(ptr %143)
  ret i16 0

b40:
  %145 = getelementptr i8, ptr %143, i16 -4
  %146 = load i16, ptr %145
  br label %b41

b41:
  %147 = phi i16 [ 0, %b40 ], [ %152, %b43 ]
  %148 = icmp ult i16 %147, %146
  br i1 %148, label %b43, label %b39

b43:
  %149 = shl i16 %147, 2
  %150 = getelementptr i8, ptr %143, i16 %149
  %151 = load ptr, ptr %150
  call addrspace(1) void @N$BDRP(ptr %151)
  %152 = add i16 %147, 1
  br label %b41
}

declare ptr @N$VCPY(ptr addrspace(1)) addrspace(1)

declare void @N$PBEG() addrspace(1)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PI2(i16) addrspace(1)

declare ptr @N$PEND() addrspace(1)

declare void @N$EBND() addrspace(1)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare void @N$BDRP(ptr) addrspace(1)

declare void @N$PN() addrspace(1)

declare void @N$PU2(i16) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
