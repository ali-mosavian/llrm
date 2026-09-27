target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [11 x i8] c"\08\00\04\00\04\00lap \00"
@$str2 = internal constant [9 x i8] c"\08\00\02\00\02\00: \00"
@$str3 = internal constant [8 x i8] c"\08\00\01\00\01\00s\00"
@$str4 = internal constant [19 x i8] c"\08\00\0C\00\0C\00fastest lap \00"
@$str5 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str6 = internal constant [10 x i8] c"\08\00\03\00\03\00ada\00"
@$str7 = internal constant [10 x i8] c"\08\00\03\00\03\00bob\00"
@$str8 = internal constant [13 x i8] c"\08\00\06\00\06\00 made \00"
@$str9 = internal constant [11 x i8] c"\08\00\04\00\04\00 is \00"
@$str10 = internal constant [13 x i8] c"\08\00\06\00\06\00 short\00"
@$str11 = internal constant [15 x i8] c"\08\00\08\00\08\00s pace: \00"
@$str12 = internal constant [20 x i8] c"\08\00\0D\00\0D\00s for 12 laps\00"

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca [8 x i8]
  %4 = alloca i8
  %5 = alloca i16
  %6 = alloca [8 x i8]
  %7 = alloca [8 x i8]
  %8 = alloca i16
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca [4 x i8]
  %13 = alloca i16
  %14 = alloca ptr
  %15 = alloca i16
  %16 = alloca [6 x i8]
  %17 = alloca i16
  %18 = alloca i16
  %19 = alloca [8 x i8]
  %20 = alloca i16
  %21 = alloca i16
  %22 = alloca i16
  %23 = alloca [8 x i8]
  store i16 0, ptr %0
  store i16 0, ptr %1
  store i16 0, ptr %2
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 8, i1 false)
  store i8 0, ptr %4
  store i16 0, ptr %5
  call void @llvm.memset.p0.i16(ptr %6, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %7, i8 0, i16 8, i1 false)
  store i16 0, ptr %8
  store i16 0, ptr %9
  store i16 0, ptr %10
  store i16 0, ptr %11
  call void @llvm.memset.p0.i16(ptr %12, i8 0, i16 4, i1 false)
  store i16 0, ptr %13
  store ptr null, ptr %14
  store i16 0, ptr %15
  call void @llvm.memset.p0.i16(ptr %16, i8 0, i16 6, i1 false)
  store i16 0, ptr %17
  store i16 0, ptr %18
  call void @llvm.memset.p0.i16(ptr %19, i8 0, i16 8, i1 false)
  store i16 0, ptr %20
  store i16 0, ptr %21
  store i16 0, ptr %22
  call void @llvm.memset.p0.i16(ptr %23, i8 0, i16 8, i1 false)
  store i16 4, ptr %21, !tbaa !2
  store i16 4, ptr %22, !tbaa !2
  %24 = sub i16 0, 0
  %25 = getelementptr inbounds i16, ptr %23, i16 %24
  store i16 62, ptr %25, !tbaa !2
  %26 = sub i16 1, 0
  %27 = getelementptr inbounds i16, ptr %23, i16 %26
  store i16 58, ptr %27, !tbaa !2
  %28 = sub i16 2, 0
  %29 = getelementptr inbounds i16, ptr %23, i16 %28
  store i16 60, ptr %29, !tbaa !2
  %30 = sub i16 3, 0
  %31 = getelementptr inbounds i16, ptr %23, i16 %30
  store i16 57, ptr %31, !tbaa !2
  store i16 0, ptr %20, !tbaa !2
  %32 = addrspacecast ptr %23 to ptr addrspace(1)
  store i16 4, ptr %19, !tbaa !2
  %33 = getelementptr inbounds i8, ptr %19, i16 2
  store i16 4, ptr %33, !tbaa !2
  %34 = getelementptr inbounds i8, ptr %19, i16 4
  store ptr addrspace(1) %32, ptr %34, !tbaa !2
  %35 = addrspacecast ptr %19 to ptr addrspace(1)
  store i16 0, ptr %18, !tbaa !2
  %36 = load i16, ptr addrspace(1) %35
  store i16 0, ptr %17, !tbaa !2
  br label %b3

b2:
  %37 = getelementptr i8, ptr @$str4, i16 6
  call addrspace(1) void @N$PS(ptr %37)
  %38 = load i16, ptr %20, !tbaa !2
  %39 = add i16 %38, 1
  call addrspace(1) void @N$PU2(i16 %39)
  call addrspace(1) void @N$PN()
  %40 = getelementptr i8, ptr @$str5, i16 6
  %41 = call addrspace(1) ptr @N$BGRW(ptr %40, i16 2, i16 4)
  %42 = getelementptr i8, ptr %41, i16 0
  %43 = getelementptr i8, ptr @$str6, i16 6
  store ptr %43, ptr %42
  %44 = getelementptr i8, ptr %42, i16 2
  store i16 12, ptr %44
  %45 = getelementptr i8, ptr %41, i16 4
  %46 = getelementptr i8, ptr @$str7, i16 6
  store ptr %46, ptr %45
  %47 = getelementptr i8, ptr %45, i16 2
  store i16 9, ptr %47
  store ptr %41, ptr %14, !tbaa !2
  store i16 10, ptr %13, !tbaa !2
  store i16 2, ptr %10, !tbaa !2
  store i16 2, ptr %11, !tbaa !2
  store i16 0, ptr %9, !tbaa !2
  store i16 2, ptr %8, !tbaa !2
  br label %b13

b3:
  %48 = load i16, ptr %17, !tbaa !2
  %49 = icmp ult i16 %48, %36
  %50 = sext i1 %49 to i8
  %51 = icmp ne i8 %50, 0
  br i1 %51, label %b4, label %b6

b4:
  %52 = getelementptr i8, ptr addrspace(1) %35, i16 4
  %53 = load ptr addrspace(1), ptr addrspace(1) %52, !tbaa !2
  %54 = mul i16 %48, 2
  %55 = getelementptr i8, ptr addrspace(1) %53, i16 %54
  %56 = load i16, ptr %18, !tbaa !2
  store i16 %56, ptr %16, !tbaa !2
  %57 = getelementptr inbounds i8, ptr %16, i16 2
  store ptr addrspace(1) %55, ptr %57, !tbaa !2
  %58 = load i16, ptr %16, !tbaa !2
  %59 = getelementptr inbounds i8, ptr %16, i16 2
  %60 = load ptr addrspace(1), ptr %59, !tbaa !2
  store i16 %58, ptr %15, !tbaa !2
  %61 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %61)
  %62 = load i16, ptr %15, !tbaa !2
  %63 = add i16 %62, 1
  call addrspace(1) void @N$PU2(i16 %63)
  %64 = getelementptr i8, ptr @$str2, i16 6
  call addrspace(1) void @N$PS(ptr %64)
  %65 = load i16, ptr addrspace(1) %60
  call addrspace(1) void @N$PI2(i16 %65)
  %66 = getelementptr i8, ptr @$str3, i16 6
  call addrspace(1) void @N$PS(ptr %66)
  call addrspace(1) void @N$PN()
  %67 = load i16, ptr addrspace(1) %60
  %68 = load i16, ptr %20, !tbaa !2
  %69 = icmp ult i16 %68, 4
  %70 = sext i1 %69 to i8
  %71 = icmp ne i8 %70, 0
  br i1 %71, label %b8, label %b9

b5:
  %72 = load i16, ptr %17, !tbaa !2
  %73 = add i16 %72, 1
  store i16 %73, ptr %17, !tbaa !2
  br label %b3

b6:
  br label %b2

b7:
  %74 = load i16, ptr %18, !tbaa !2
  %75 = add i16 %74, 1
  store i16 %75, ptr %18, !tbaa !2
  br label %b5

b8:
  %76 = sub i16 %68, 0
  %77 = getelementptr inbounds i16, ptr %23, i16 %76
  %78 = load i16, ptr %77, !tbaa !2
  %79 = icmp slt i16 %67, %78
  %80 = sext i1 %79 to i8
  %81 = icmp ne i8 %80, 0
  br i1 %81, label %b10, label %b11

b9:
  call addrspace(1) void @N$EBND()
  unreachable

b10:
  %82 = load i16, ptr %15, !tbaa !2
  store i16 %82, ptr %20, !tbaa !2
  br label %b12

b11:
  br label %b12

b12:
  br label %b7

b13:
  %83 = load i16, ptr %9, !tbaa !2
  %84 = load i16, ptr %8, !tbaa !2
  %85 = icmp slt i16 %83, %84
  %86 = sext i1 %85 to i8
  %87 = icmp ne i8 %86, 0
  br i1 %87, label %b14, label %b16

b14:
  %88 = load i16, ptr %9, !tbaa !2
  %89 = load i16, ptr %13, !tbaa !2
  %90 = sub i16 %88, 0
  %91 = getelementptr inbounds i16, ptr %12, i16 %90
  store i16 %89, ptr %91, !tbaa !2
  br label %b15

b15:
  %92 = load i16, ptr %9, !tbaa !2
  %93 = add i16 %92, 1
  store i16 %93, ptr %9, !tbaa !2
  br label %b13

b16:
  %94 = load ptr, ptr %14, !tbaa !2
  %95 = getelementptr i8, ptr %94, i16 -4
  %96 = load i16, ptr %95
  %97 = addrspacecast ptr %94 to ptr addrspace(1)
  store i16 %96, ptr %7, !tbaa !2
  %98 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 %96, ptr %98, !tbaa !2
  %99 = getelementptr inbounds i8, ptr %7, i16 4
  store ptr addrspace(1) %97, ptr %99, !tbaa !2
  %100 = addrspacecast ptr %7 to ptr addrspace(1)
  %101 = addrspacecast ptr %12 to ptr addrspace(1)
  store i16 2, ptr %6, !tbaa !2
  %102 = getelementptr inbounds i8, ptr %6, i16 2
  store i16 2, ptr %102, !tbaa !2
  %103 = getelementptr inbounds i8, ptr %6, i16 4
  store ptr addrspace(1) %101, ptr %103, !tbaa !2
  %104 = addrspacecast ptr %6 to ptr addrspace(1)
  store i16 0, ptr %5, !tbaa !2
  br label %b18

b17:
  store i16 58, ptr %2, !tbaa !2
  store i16 61, ptr %1, !tbaa !2
  br label %b31

b18:
  %105 = load i16, ptr %5, !tbaa !2
  %106 = load i16, ptr addrspace(1) %100
  %107 = icmp ult i16 %105, %106
  %108 = sext i1 %107 to i8
  store i8 %108, ptr %4, !tbaa !2
  %109 = icmp ne i8 %108, 0
  br i1 %109, label %b21, label %b22

b19:
  %110 = load i16, ptr %5, !tbaa !2
  %111 = load i16, ptr addrspace(1) %100, !tbaa !2
  %112 = icmp ult i16 %110, %111
  %113 = sext i1 %112 to i8
  %114 = icmp ne i8 %113, 0
  br i1 %114, label %b24, label %b25

b20:
  br label %b17

b21:
  %115 = load i16, ptr %5, !tbaa !2
  %116 = load i16, ptr addrspace(1) %104
  %117 = icmp ult i16 %115, %116
  %118 = sext i1 %117 to i8
  store i8 %118, ptr %4, !tbaa !2
  br label %b22

b22:
  %119 = load i8, ptr %4, !tbaa !2
  %120 = icmp ne i8 %119, 0
  br i1 %120, label %b19, label %b20

b23:
  %121 = load i16, ptr %5, !tbaa !2
  %122 = add i16 %121, 1
  store i16 %122, ptr %5, !tbaa !2
  br label %b18

b24:
  %123 = getelementptr i8, ptr addrspace(1) %100, i16 4
  %124 = load ptr addrspace(1), ptr addrspace(1) %123, !tbaa !2
  %125 = mul i16 %110, 4
  %126 = getelementptr i8, ptr addrspace(1) %124, i16 %125
  %127 = load i16, ptr %5, !tbaa !2
  %128 = load i16, ptr addrspace(1) %104, !tbaa !2
  %129 = icmp ult i16 %127, %128
  %130 = sext i1 %129 to i8
  %131 = icmp ne i8 %130, 0
  br i1 %131, label %b26, label %b27

b25:
  call addrspace(1) void @N$EBND()
  unreachable

b26:
  %132 = getelementptr i8, ptr addrspace(1) %104, i16 4
  %133 = load ptr addrspace(1), ptr addrspace(1) %132, !tbaa !2
  %134 = mul i16 %127, 2
  %135 = getelementptr i8, ptr addrspace(1) %133, i16 %134
  store ptr addrspace(1) %126, ptr %3, !tbaa !2
  %136 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %135, ptr %136, !tbaa !2
  %137 = load ptr addrspace(1), ptr %3, !tbaa !2
  %138 = getelementptr inbounds i8, ptr %3, i16 4
  %139 = load ptr addrspace(1), ptr %138, !tbaa !2
  %140 = getelementptr i8, ptr addrspace(1) %137, i16 2
  %141 = load i16, ptr addrspace(1) %140
  %142 = load i16, ptr addrspace(1) %139
  %143 = icmp sge i16 %141, %142
  %144 = sext i1 %143 to i8
  %145 = icmp ne i8 %144, 0
  br i1 %145, label %b28, label %b29

b27:
  call addrspace(1) void @N$EBND()
  unreachable

b28:
  %146 = load ptr, ptr addrspace(1) %137
  call addrspace(1) void @N$PS(ptr %146)
  %147 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @N$PS(ptr %147)
  %148 = load i16, ptr addrspace(1) %139
  call addrspace(1) void @N$PI2(i16 %148)
  call addrspace(1) void @N$PN()
  br label %b30

b29:
  %149 = load ptr, ptr addrspace(1) %137
  call addrspace(1) void @N$PS(ptr %149)
  %150 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %150)
  %151 = load i16, ptr addrspace(1) %139
  %152 = getelementptr i8, ptr addrspace(1) %137, i16 2
  %153 = load i16, ptr addrspace(1) %152
  %154 = sub i16 %151, %153
  call addrspace(1) void @N$PI2(i16 %154)
  %155 = getelementptr i8, ptr @$str10, i16 6
  call addrspace(1) void @N$PS(ptr %155)
  call addrspace(1) void @N$PN()
  br label %b30

b30:
  br label %b23

b31:
  %156 = load i16, ptr %2, !tbaa !2
  %157 = load i16, ptr %1, !tbaa !2
  %158 = icmp slt i16 %156, %157
  %159 = sext i1 %158 to i8
  %160 = icmp ne i8 %159, 0
  br i1 %160, label %b32, label %b34

b32:
  %161 = load i16, ptr %2, !tbaa !2
  call addrspace(1) void @N$PI2(i16 %161)
  %162 = getelementptr i8, ptr @$str11, i16 6
  call addrspace(1) void @N$PS(ptr %162)
  %163 = load i16, ptr %2, !tbaa !2
  %164 = mul i16 %163, 12
  call addrspace(1) void @N$PI2(i16 %164)
  %165 = getelementptr i8, ptr @$str12, i16 6
  call addrspace(1) void @N$PS(ptr %165)
  call addrspace(1) void @N$PN()
  br label %b33

b33:
  %166 = load i16, ptr %2, !tbaa !2
  %167 = add i16 %166, 1
  store i16 %167, ptr %2, !tbaa !2
  br label %b31

b34:
  %168 = load ptr, ptr %14, !tbaa !2
  %169 = icmp ne ptr %168, null
  %170 = sext i1 %169 to i8
  %171 = icmp ne i8 %170, 0
  br i1 %171, label %b36, label %b35

b35:
  call addrspace(1) void @N$BDRP(ptr %168)
  ret i16 0

b36:
  %172 = getelementptr i8, ptr %168, i16 -4
  %173 = load i16, ptr %172
  store i16 0, ptr %0, !tbaa !2
  br label %b37

b37:
  %174 = load i16, ptr %0, !tbaa !2
  %175 = icmp ult i16 %174, %173
  %176 = sext i1 %175 to i8
  %177 = icmp ne i8 %176, 0
  br i1 %177, label %b39, label %b38

b38:
  br label %b35

b39:
  %178 = mul i16 %174, 4
  %179 = getelementptr i8, ptr %168, i16 %178
  %180 = load ptr, ptr %179
  call addrspace(1) void @N$BDRP(ptr %180)
  %181 = add i16 %174, 1
  store i16 %181, ptr %0, !tbaa !2
  br label %b37
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PU2(i16) addrspace(1)

declare void @N$PN() addrspace(1)

declare ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare void @N$PI2(i16) addrspace(1)

declare void @N$EBND() addrspace(1)

declare void @N$BDRP(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
