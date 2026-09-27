target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [10 x i8] c"\08\00\03\00\03\00ada\00"
@$str2 = internal constant [8 x i8] c"\08\00\01\00\01\00 \00"
@$str3 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str4 = internal constant [11 x i8] c"\08\00\04\00\04\00top \00"
@$str5 = internal constant [12 x i8] c"\08\00\05\00\05\00empty\00"
@$str6 = internal constant [14 x i8] c"\08\00\07\00\07\00stored \00"
@$str7 = internal constant [15 x i8] c"\08\00\08\00\08\00full at \00"
@$str8 = internal constant [18 x i8] c"\08\00\0B\00\0B\00first roll \00"
@$str9 = internal constant [15 x i8] c"\08\00\08\00\08\00no rolls\00"
@$str10 = internal constant [11 x i8] c"\08\00\04\00\04\00held\00"
@$str11 = internal constant [19 x i8] c"\08\00\0C\00\0C\00nothing held\00"

define internal i32 @store(ptr addrspace(1) %0, i16 %1) addrspace(1) {
b1:
  %2 = alloca [4 x i8]
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 4, i1 false)
  %3 = load ptr, ptr addrspace(1) %0
  %4 = getelementptr i8, ptr %3, i16 -4
  %5 = load i16, ptr %4
  %6 = icmp eq i16 %5, 3
  %7 = sext i1 %6 to i8
  %8 = icmp ne i8 %7, 0
  br i1 %8, label %b2, label %b3

b2:
  store i8 1, ptr %2, !tbaa !2
  %9 = getelementptr inbounds i8, ptr %2, i16 2
  store i8 0, ptr %9, !tbaa !2
  %10 = addrspacecast ptr %2 to ptr addrspace(1)
  %11 = load i32, ptr addrspace(1) %10, !tbaa !2
  ret i32 %11

b3:
  br label %b4

b4:
  %12 = load ptr, ptr addrspace(1) %0
  %13 = getelementptr i8, ptr %12, i16 -4
  %14 = load i16, ptr %13
  %15 = call addrspace(1) ptr @N$BGRW(ptr %12, i16 1, i16 2)
  store ptr %15, ptr addrspace(1) %0
  %16 = mul i16 %14, 2
  %17 = getelementptr i8, ptr %15, i16 %16
  store i16 %1, ptr %17
  store i8 0, ptr %2, !tbaa !2
  %18 = addrspacecast ptr %2 to ptr addrspace(1)
  %19 = load i32, ptr addrspace(1) %18, !tbaa !2
  ret i32 %19
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca [4 x i8]
  %1 = alloca [8 x i8]
  %2 = alloca [6 x i8]
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca [6 x i8]
  %6 = alloca i16
  %7 = alloca i8
  %8 = alloca i8
  %9 = alloca i8
  %10 = alloca [4 x i8]
  %11 = alloca i16
  %12 = alloca ptr
  %13 = alloca i16
  %14 = alloca [4 x i8]
  %15 = alloca [2 x i8]
  %16 = alloca [2 x i8]
  %17 = alloca [4 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 6, i1 false)
  store i16 0, ptr %3
  store i16 0, ptr %4
  call void @llvm.memset.p0.i16(ptr %5, i8 0, i16 6, i1 false)
  store i16 0, ptr %6
  store i8 0, ptr %7
  store i8 0, ptr %8
  store i8 0, ptr %9
  call void @llvm.memset.p0.i16(ptr %10, i8 0, i16 4, i1 false)
  store i16 0, ptr %11
  store ptr null, ptr %12
  store i16 0, ptr %13
  call void @llvm.memset.p0.i16(ptr %14, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr %15, i8 0, i16 2, i1 false)
  call void @llvm.memset.p0.i16(ptr %16, i8 0, i16 2, i1 false)
  call void @llvm.memset.p0.i16(ptr %17, i8 0, i16 4, i1 false)
  %18 = getelementptr i8, ptr @$str1, i16 6
  store ptr %18, ptr %17, !tbaa !2
  %19 = getelementptr inbounds i8, ptr %17, i16 2
  store i16 42, ptr %19, !tbaa !2
  store i8 7, ptr %16, !tbaa !2
  %20 = getelementptr inbounds i8, ptr %16, i16 1
  store i8 -1, ptr %20, !tbaa !2
  %21 = load ptr, ptr %17, !tbaa !2
  call addrspace(1) void @N$PS(ptr %21)
  %22 = getelementptr i8, ptr @$str2, i16 6
  call addrspace(1) void @N$PS(ptr %22)
  %23 = getelementptr inbounds i8, ptr %17, i16 2
  %24 = load i16, ptr %23, !tbaa !2
  call addrspace(1) void @N$PI2(i16 %24)
  %25 = getelementptr i8, ptr @$str2, i16 6
  call addrspace(1) void @N$PS(ptr %25)
  %26 = load i8, ptr %16, !tbaa !2
  call addrspace(1) void @N$PU1(i8 %26)
  %27 = getelementptr i8, ptr @$str2, i16 6
  call addrspace(1) void @N$PS(ptr %27)
  %28 = getelementptr inbounds i8, ptr %16, i16 1
  %29 = load i8, ptr %28, !tbaa !2
  call addrspace(1) void @N$PB(i8 %29)
  call addrspace(1) void @N$PN()
  %30 = getelementptr i8, ptr @$str3, i16 6
  %31 = call addrspace(1) ptr @N$BGRW(ptr %30, i16 2, i16 2)
  %32 = getelementptr i8, ptr %31, i16 0
  store i16 5, ptr %32
  %33 = getelementptr i8, ptr %31, i16 2
  store i16 9, ptr %33
  store ptr %31, ptr %15, !tbaa !2
  %34 = addrspacecast ptr %15 to ptr addrspace(1)
  %35 = call addrspace(1) i32 @"Stack.top[i16]"(ptr addrspace(1) %34)
  %36 = addrspacecast ptr %14 to ptr addrspace(1)
  store i32 %35, ptr addrspace(1) %36, !tbaa !2
  %37 = load i8, ptr %14, !tbaa !2
  %38 = icmp eq i8 %37, 1
  %39 = sext i1 %38 to i8
  %40 = icmp ne i8 %39, 0
  br i1 %40, label %b4, label %b3

b2:
  %41 = getelementptr i8, ptr @$str3, i16 6
  %42 = call addrspace(1) ptr @N$BGRW(ptr %41, i16 3, i16 2)
  %43 = getelementptr i8, ptr %42, i16 0
  store i16 11, ptr %43
  %44 = getelementptr i8, ptr %42, i16 2
  store i16 12, ptr %44
  %45 = getelementptr i8, ptr %42, i16 4
  store i16 13, ptr %45
  store ptr %42, ptr %12, !tbaa !2
  %46 = load ptr, ptr %12, !tbaa !2
  %47 = getelementptr i8, ptr %46, i16 -4
  %48 = load i16, ptr %47
  store i16 0, ptr %11, !tbaa !2
  br label %b6

b3:
  %49 = getelementptr i8, ptr @$str5, i16 6
  call addrspace(1) void @N$PS(ptr %49)
  call addrspace(1) void @N$PN()
  br label %b2

b4:
  %50 = getelementptr inbounds i8, ptr %14, i16 2
  %51 = load i16, ptr %50, !tbaa !2
  %52 = getelementptr inbounds i8, ptr %14, i16 2
  %53 = load i16, ptr %52, !tbaa !2
  store i16 %53, ptr %13, !tbaa !2
  %54 = getelementptr i8, ptr @$str4, i16 6
  call addrspace(1) void @N$PS(ptr %54)
  %55 = load i16, ptr %13, !tbaa !2
  call addrspace(1) void @N$PI2(i16 %55)
  call addrspace(1) void @N$PN()
  br label %b2

b6:
  %56 = load i16, ptr %11, !tbaa !2
  %57 = icmp ult i16 %56, %48
  %58 = sext i1 %57 to i8
  %59 = icmp ne i8 %58, 0
  br i1 %59, label %b7, label %b9

b7:
  %60 = mul i16 %56, 2
  %61 = getelementptr i8, ptr %46, i16 %60
  %62 = addrspacecast ptr %15 to ptr addrspace(1)
  %63 = load i16, ptr %61
  %64 = call addrspace(1) i32 @store(ptr addrspace(1) %62, i16 %63)
  %65 = addrspacecast ptr %10 to ptr addrspace(1)
  store i32 %64, ptr addrspace(1) %65, !tbaa !2
  %66 = load i8, ptr %10, !tbaa !2
  %67 = icmp eq i8 %66, 0
  %68 = sext i1 %67 to i8
  %69 = icmp ne i8 %68, 0
  br i1 %69, label %b12, label %b11

b8:
  %70 = load i16, ptr %11, !tbaa !2
  %71 = add i16 %70, 1
  store i16 %71, ptr %11, !tbaa !2
  br label %b6

b9:
  %72 = load ptr, ptr %12, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %72)
  store i8 3, ptr %9, !tbaa !2
  store i8 -56, ptr %8, !tbaa !2
  %73 = load i8, ptr %9, !tbaa !2
  %74 = load i8, ptr %8, !tbaa !2
  %75 = call addrspace(1) i8 @"larger[u8]"(i8 %73, i8 %74)
  store i8 %75, ptr %7, !tbaa !2
  %76 = call addrspace(1) i16 @"larger[i16]"(i16 -1, i16 -5)
  store i16 %76, ptr %6, !tbaa !2
  %77 = load i8, ptr %7, !tbaa !2
  call addrspace(1) void @N$PU1(i8 %77)
  %78 = getelementptr i8, ptr @$str2, i16 6
  call addrspace(1) void @N$PS(ptr %78)
  %79 = load i16, ptr %6, !tbaa !2
  call addrspace(1) void @N$PI2(i16 %79)
  call addrspace(1) void @N$PN()
  store i16 3, ptr %3, !tbaa !2
  store i16 3, ptr %4, !tbaa !2
  %80 = sub i16 0, 0
  %81 = getelementptr inbounds i16, ptr %5, i16 %80
  store i16 4, ptr %81, !tbaa !2
  %82 = sub i16 1, 0
  %83 = getelementptr inbounds i16, ptr %5, i16 %82
  store i16 6, ptr %83, !tbaa !2
  %84 = sub i16 2, 0
  %85 = getelementptr inbounds i16, ptr %5, i16 %84
  store i16 1, ptr %85, !tbaa !2
  %86 = addrspacecast ptr %2 to ptr addrspace(1)
  %87 = addrspacecast ptr %5 to ptr addrspace(1)
  store i16 3, ptr %1, !tbaa !2
  %88 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 3, ptr %88, !tbaa !2
  %89 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %87, ptr %89, !tbaa !2
  %90 = addrspacecast ptr %1 to ptr addrspace(1)
  call addrspace(1) void @"first[i16]"(ptr addrspace(1) %86, ptr addrspace(1) %90)
  %91 = load i8, ptr %2, !tbaa !2
  %92 = icmp eq i8 %91, 0
  %93 = sext i1 %92 to i8
  %94 = icmp ne i8 %93, 0
  br i1 %94, label %b16, label %b15

b10:
  br label %b8

b11:
  %95 = getelementptr inbounds i8, ptr %10, i16 2
  %96 = load i8, ptr %95, !tbaa !2
  %97 = getelementptr i8, ptr @$str7, i16 6
  call addrspace(1) void @N$PS(ptr %97)
  %98 = load i16, ptr %61
  call addrspace(1) void @N$PI2(i16 %98)
  call addrspace(1) void @N$PN()
  br label %b10

b12:
  %99 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @N$PS(ptr %99)
  %100 = load i16, ptr %61
  call addrspace(1) void @N$PI2(i16 %100)
  call addrspace(1) void @N$PN()
  br label %b10

b14:
  store i8 0, ptr %0, !tbaa !2
  %101 = load i8, ptr %0, !tbaa !2
  %102 = icmp eq i8 %101, 1
  %103 = sext i1 %102 to i8
  %104 = icmp ne i8 %103, 0
  br i1 %104, label %b20, label %b19

b15:
  %105 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %105)
  call addrspace(1) void @N$PN()
  br label %b14

b16:
  %106 = getelementptr inbounds i8, ptr %2, i16 2
  %107 = load ptr addrspace(1), ptr %106, !tbaa !2
  %108 = getelementptr inbounds i8, ptr %2, i16 2
  %109 = load ptr addrspace(1), ptr %108, !tbaa !2
  %110 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @N$PS(ptr %110)
  %111 = load i16, ptr addrspace(1) %109
  call addrspace(1) void @N$PI2(i16 %111)
  call addrspace(1) void @N$PN()
  br label %b14

b18:
  %112 = load ptr, ptr %15, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %112)
  %113 = load ptr, ptr %17, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %113)
  ret i16 0

b19:
  %114 = getelementptr i8, ptr @$str11, i16 6
  call addrspace(1) void @N$PS(ptr %114)
  call addrspace(1) void @N$PN()
  br label %b18

b20:
  %115 = getelementptr inbounds i8, ptr %0, i16 2
  %116 = load i16, ptr %115, !tbaa !2
  %117 = getelementptr inbounds i8, ptr %0, i16 2
  %118 = load i16, ptr %117, !tbaa !2
  %119 = getelementptr i8, ptr @$str10, i16 6
  call addrspace(1) void @N$PS(ptr %119)
  call addrspace(1) void @N$PN()
  br label %b18
}

define internal void @"first[i16]"(ptr addrspace(1) %0, ptr addrspace(1) noalias readonly dereferenceable(8) %1) addrspace(1) {
b1:
  %2 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %3 = load ptr addrspace(1), ptr addrspace(1) %2
  %4 = load i16, ptr addrspace(1) %1
  %5 = icmp eq i16 %4, 0
  %6 = sext i1 %5 to i8
  %7 = icmp ne i8 %6, 0
  br i1 %7, label %b4, label %b3

b3:
  %8 = getelementptr i8, ptr addrspace(1) %3, i16 0
  %9 = load i16, ptr addrspace(1) %8
  store i8 0, ptr addrspace(1) %0
  %10 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr addrspace(1) %8, ptr addrspace(1) %10
  ret void

b4:
  store i8 1, ptr addrspace(1) %0
  ret void
}

define internal i16 @"larger[i16]"(i16 %0, i16 %1) addrspace(1) {
b1:
  %2 = alloca i16
  store i16 0, ptr %2
  %3 = icmp sgt i16 %0, %1
  %4 = sext i1 %3 to i8
  %5 = icmp ne i8 %4, 0
  br i1 %5, label %b2, label %b3

b2:
  store i16 %0, ptr %2, !tbaa !2
  br label %b4

b3:
  store i16 %1, ptr %2, !tbaa !2
  br label %b4

b4:
  %6 = load i16, ptr %2, !tbaa !2
  ret i16 %6
}

define internal i8 @"larger[u8]"(i8 %0, i8 %1) addrspace(1) {
b1:
  %2 = alloca i8
  store i8 0, ptr %2
  %3 = zext i8 %0 to i16
  %4 = zext i8 %1 to i16
  %5 = icmp sgt i16 %3, %4
  %6 = sext i1 %5 to i8
  %7 = icmp ne i8 %6, 0
  br i1 %7, label %b2, label %b3

b2:
  store i8 %0, ptr %2, !tbaa !2
  br label %b4

b3:
  store i8 %1, ptr %2, !tbaa !2
  br label %b4

b4:
  %8 = load i8, ptr %2, !tbaa !2
  ret i8 %8
}

define internal i32 @"Stack.top[i16]"(ptr addrspace(1) %0) addrspace(1) {
b1:
  %1 = alloca [4 x i8]
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 4, i1 false)
  %2 = load ptr, ptr addrspace(1) %0
  %3 = getelementptr i8, ptr %2, i16 -4
  %4 = load i16, ptr %3
  %5 = icmp eq i16 %4, 0
  %6 = sext i1 %5 to i8
  %7 = icmp ne i8 %6, 0
  br i1 %7, label %b2, label %b3

b2:
  store i8 0, ptr %1, !tbaa !2
  %8 = addrspacecast ptr %1 to ptr addrspace(1)
  %9 = load i32, ptr addrspace(1) %8, !tbaa !2
  ret i32 %9

b3:
  br label %b4

b4:
  %10 = load ptr, ptr addrspace(1) %0
  %11 = load ptr, ptr addrspace(1) %0
  %12 = getelementptr i8, ptr %11, i16 -4
  %13 = load i16, ptr %12
  %14 = sub i16 %13, 1
  %15 = getelementptr i8, ptr %10, i16 -4
  %16 = load i16, ptr %15
  %17 = icmp ult i16 %14, %16
  %18 = sext i1 %17 to i8
  %19 = icmp ne i8 %18, 0
  br i1 %19, label %b5, label %b6

b5:
  %20 = mul i16 %14, 2
  %21 = getelementptr i8, ptr %10, i16 %20
  %22 = load i16, ptr %21
  store i8 1, ptr %1, !tbaa !2
  %23 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 %22, ptr %23, !tbaa !2
  %24 = addrspacecast ptr %1 to ptr addrspace(1)
  %25 = load i32, ptr addrspace(1) %24, !tbaa !2
  ret i32 %25

b6:
  call addrspace(1) void @N$EBND()
  unreachable
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PI2(i16) addrspace(1)

declare void @N$PU1(i8) addrspace(1)

declare void @N$PB(i8) addrspace(1)

declare void @N$PN() addrspace(1)

declare void @N$BDRP(ptr) addrspace(1)

declare void @N$EBND() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
