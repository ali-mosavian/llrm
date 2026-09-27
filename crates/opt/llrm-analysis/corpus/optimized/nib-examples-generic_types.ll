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
  br i1 %6, label %b2, label %b3

b2:
  store i8 1, ptr %2, !tbaa !2
  %7 = getelementptr inbounds i8, ptr %2, i16 2
  store i8 0, ptr %7, !tbaa !2
  %8 = addrspacecast ptr %2 to ptr addrspace(1)
  %9 = load i32, ptr addrspace(1) %8, !tbaa !2
  ret i32 %9

b3:
  %10 = call addrspace(1) ptr @N$BGRW(ptr %3, i16 1, i16 2)
  store ptr %10, ptr addrspace(1) %0
  %11 = shl i16 %5, 1
  %12 = getelementptr i8, ptr %10, i16 %11
  store i16 %1, ptr %12
  store i8 0, ptr %2, !tbaa !2
  %13 = addrspacecast ptr %2 to ptr addrspace(1)
  %14 = load i32, ptr addrspace(1) %13, !tbaa !2
  ret i32 %14
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca [4 x i8]
  %1 = alloca [4 x i8]
  %2 = alloca [6 x i8]
  %3 = alloca [4 x i8]
  %4 = alloca [4 x i8]
  %5 = alloca [2 x i8]
  %6 = alloca [4 x i8]
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 6, i1 false)
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr %5, i8 0, i16 2, i1 false)
  call void @llvm.memset.p0.i16(ptr %6, i8 0, i16 4, i1 false)
  %7 = getelementptr i8, ptr @$str1, i16 6
  store ptr %7, ptr %6, !tbaa !2
  %8 = getelementptr inbounds i8, ptr %6, i16 2
  store i16 42, ptr %8, !tbaa !2
  call addrspace(1) void @N$PS(ptr %7)
  %9 = getelementptr i8, ptr @$str2, i16 6
  call addrspace(1) void @N$PS(ptr %9)
  call addrspace(1) void @N$PI2(i16 42)
  call addrspace(1) void @N$PS(ptr %9)
  call addrspace(1) void @N$PU1(i8 7)
  call addrspace(1) void @N$PS(ptr %9)
  call addrspace(1) void @N$PB(i8 -1)
  call addrspace(1) void @N$PN()
  %10 = getelementptr i8, ptr @$str3, i16 6
  %11 = call addrspace(1) ptr @N$BGRW(ptr %10, i16 2, i16 2)
  %12 = getelementptr i8, ptr %11, i16 0
  store i16 5, ptr %12
  %13 = getelementptr i8, ptr %11, i16 2
  store i16 9, ptr %13
  store ptr %11, ptr %5, !tbaa !2
  %14 = addrspacecast ptr %5 to ptr addrspace(1)
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 4, i1 false)
  %15 = getelementptr i8, ptr %11, i16 -4
  %16 = load i16, ptr %15
  %17 = icmp eq i16 %16, 0
  br i1 %17, label %18, label %21

18:
  store i8 0, ptr %1
  %19 = addrspacecast ptr %1 to ptr addrspace(1)
  %20 = load i32, ptr addrspace(1) %19
  br label %32

21:
  %22 = add i16 %16, -1
  %23 = icmp ult i16 %22, %16
  br i1 %23, label %24, label %31

24:
  %25 = shl i16 %22, 1
  %26 = getelementptr i8, ptr %11, i16 %25
  %27 = load i16, ptr %26
  store i8 1, ptr %1
  %28 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 %27, ptr %28
  %29 = addrspacecast ptr %1 to ptr addrspace(1)
  %30 = load i32, ptr addrspace(1) %29
  br label %32

31:
  call addrspace(1) void @N$EBND()
  unreachable

32:
  %33 = phi i32 [ %20, %18 ], [ %30, %24 ]
  %34 = addrspacecast ptr %4 to ptr addrspace(1)
  store i32 %33, ptr addrspace(1) %34, !tbaa !2
  %35 = load i8, ptr %4, !tbaa !2
  %36 = icmp eq i8 %35, 1
  br i1 %36, label %b4, label %b3

b2:
  %37 = call addrspace(1) ptr @N$BGRW(ptr %10, i16 3, i16 2)
  %38 = getelementptr i8, ptr %37, i16 0
  store i16 11, ptr %38
  %39 = getelementptr i8, ptr %37, i16 2
  store i16 12, ptr %39
  %40 = getelementptr i8, ptr %37, i16 4
  store i16 13, ptr %40
  %41 = getelementptr i8, ptr %37, i16 -4
  %42 = load i16, ptr %41
  %43 = addrspacecast ptr %3 to ptr addrspace(1)
  %44 = getelementptr i8, ptr @$str6, i16 6
  %45 = getelementptr i8, ptr @$str7, i16 6
  %46 = addrspacecast ptr %0 to ptr addrspace(1)
  %47 = getelementptr inbounds i8, ptr %0, i16 2
  br label %b6

b3:
  %48 = getelementptr i8, ptr @$str5, i16 6
  call addrspace(1) void @N$PS(ptr %48)
  call addrspace(1) void @N$PN()
  br label %b2

b4:
  %49 = getelementptr inbounds i8, ptr %4, i16 2
  %50 = load i16, ptr %49, !tbaa !2
  %51 = getelementptr i8, ptr @$str4, i16 6
  call addrspace(1) void @N$PS(ptr %51)
  call addrspace(1) void @N$PI2(i16 %50)
  call addrspace(1) void @N$PN()
  br label %b2

b6:
  %52 = phi i16 [ 0, %b2 ], [ %82, %b10 ]
  %53 = icmp ult i16 %52, %42
  br i1 %53, label %b7, label %b9

b7:
  %54 = shl i16 %52, 1
  %55 = getelementptr i8, ptr %37, i16 %54
  %56 = load i16, ptr %55
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 4, i1 false)
  %57 = load ptr, ptr addrspace(1) %14
  %58 = getelementptr i8, ptr %57, i16 -4
  %59 = load i16, ptr %58
  %60 = icmp eq i16 %59, 3
  br i1 %60, label %61, label %63

61:
  store i8 1, ptr %0
  store i8 0, ptr %47
  %62 = load i32, ptr addrspace(1) %46
  br label %68

63:
  %64 = call addrspace(1) ptr @N$BGRW(ptr %57, i16 1, i16 2)
  store ptr %64, ptr addrspace(1) %14
  %65 = shl i16 %59, 1
  %66 = getelementptr i8, ptr %64, i16 %65
  store i16 %56, ptr %66
  store i8 0, ptr %0
  %67 = load i32, ptr addrspace(1) %46
  br label %68

68:
  %69 = phi i32 [ %62, %61 ], [ %67, %63 ]
  store i32 %69, ptr addrspace(1) %43, !tbaa !2
  %70 = load i8, ptr %3, !tbaa !2
  %71 = icmp eq i8 %70, 0
  br i1 %71, label %b12, label %b11

b9:
  call addrspace(1) void @N$BDRP(ptr %37)
  call addrspace(1) void @N$PU1(i8 -56)
  call addrspace(1) void @N$PS(ptr %9)
  call addrspace(1) void @N$PI2(i16 -1)
  call addrspace(1) void @N$PN()
  %72 = getelementptr inbounds i16, ptr %2, i16 0
  store i16 4, ptr %72, !tbaa !2
  %73 = getelementptr inbounds i16, ptr %2, i16 1
  store i16 6, ptr %73, !tbaa !2
  %74 = getelementptr inbounds i16, ptr %2, i16 2
  store i16 1, ptr %74, !tbaa !2
  %75 = addrspacecast ptr %2 to ptr addrspace(1)
  %76 = getelementptr i8, ptr addrspace(1) %75, i16 0
  %77 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @N$PS(ptr %77)
  %78 = load i16, ptr addrspace(1) %76
  call addrspace(1) void @N$PI2(i16 %78)
  call addrspace(1) void @N$PN()
  %79 = getelementptr i8, ptr @$str11, i16 6
  call addrspace(1) void @N$PS(ptr %79)
  call addrspace(1) void @N$PN()
  %80 = load ptr, ptr %5, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %80)
  %81 = load ptr, ptr %6, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %81)
  ret i16 0

b10:
  %82 = add i16 %52, 1
  br label %b6

b11:
  call addrspace(1) void @N$PS(ptr %45)
  %83 = load i16, ptr %55
  call addrspace(1) void @N$PI2(i16 %83)
  call addrspace(1) void @N$PN()
  br label %b10

b12:
  call addrspace(1) void @N$PS(ptr %44)
  %84 = load i16, ptr %55
  call addrspace(1) void @N$PI2(i16 %84)
  call addrspace(1) void @N$PN()
  br label %b10
}

define internal void @"first[i16]"(ptr addrspace(1) %0, ptr addrspace(1) noalias readonly dereferenceable(8) %1) addrspace(1) memory(argmem: readwrite) willreturn {
b1:
  %2 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %3 = load ptr addrspace(1), ptr addrspace(1) %2
  %4 = load i16, ptr addrspace(1) %1
  %5 = icmp eq i16 %4, 0
  br i1 %5, label %b4, label %b3

b3:
  %6 = getelementptr i8, ptr addrspace(1) %3, i16 0
  store i8 0, ptr addrspace(1) %0
  %7 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr addrspace(1) %6, ptr addrspace(1) %7
  ret void

b4:
  store i8 1, ptr addrspace(1) %0
  ret void
}

define internal i16 @"larger[i16]"(i16 %0, i16 %1) addrspace(1) memory(none) willreturn {
b1:
  ret i16 -1
}

define internal i8 @"larger[u8]"(i8 %0, i8 %1) addrspace(1) memory(none) willreturn {
b1:
  ret i8 -56
}

define internal i32 @"Stack.top[i16]"(ptr addrspace(1) %0) addrspace(1) {
b1:
  %1 = alloca [4 x i8]
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 4, i1 false)
  %2 = load ptr, ptr addrspace(1) %0
  %3 = getelementptr i8, ptr %2, i16 -4
  %4 = load i16, ptr %3
  %5 = icmp eq i16 %4, 0
  br i1 %5, label %b2, label %b3

b2:
  store i8 0, ptr %1, !tbaa !2
  %6 = addrspacecast ptr %1 to ptr addrspace(1)
  %7 = load i32, ptr addrspace(1) %6, !tbaa !2
  ret i32 %7

b3:
  %8 = add i16 %4, -1
  %9 = icmp ult i16 %8, %4
  br i1 %9, label %b5, label %b6

b5:
  %10 = shl i16 %8, 1
  %11 = getelementptr i8, ptr %2, i16 %10
  %12 = load i16, ptr %11
  store i8 1, ptr %1, !tbaa !2
  %13 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 %12, ptr %13, !tbaa !2
  %14 = addrspacecast ptr %1 to ptr addrspace(1)
  %15 = load i32, ptr addrspace(1) %14, !tbaa !2
  ret i32 %15

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
