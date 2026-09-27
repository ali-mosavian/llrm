target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [13 x i8] c"\08\00\06\00\06\00nobody\00"
@$str2 = internal constant [10 x i8] c"\08\00\03\00\03\00ada\00"
@$str3 = internal constant [10 x i8] c"\08\00\03\00\03\00bob\00"
@$str4 = internal constant [14 x i8] c"\08\00\07\00\07\00leader \00"
@$str5 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str6 = internal constant [9 x i8] c"\08\00\02\00\02\00cy\00"
@$str7 = internal constant [9 x i8] c"\08\00\02\00\02\00di\00"
@$str8 = internal constant [9 x i8] c"\08\00\02\00\02\00ed\00"
@$str9 = internal constant [12 x i8] c"\08\00\05\00\05\00best \00"
@$str10 = internal constant [13 x i8] c"\08\00\06\00\06\00 with \00"
@$str11 = internal constant [14 x i8] c"\08\00\07\00\07\00no team\00"
@$str12 = internal constant [13 x i8] c"\08\00\06\00\06\00first \00"
@$str13 = internal constant [27 x i8] c"\08\00\14\00\14\00move north then east\00"
@$str14 = internal constant [8 x i8] c"\08\00\01\00\01\00[\00"
@$str15 = internal constant [8 x i8] c"\08\00\01\00\01\00]\00"

define internal ptr addrspace(1) @leader(ptr addrspace(1) %0, ptr addrspace(1) %1) addrspace(1) memory(argmem: read) willreturn {
b1:
  %2 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %3 = load i16, ptr addrspace(1) %2
  %4 = getelementptr i8, ptr addrspace(1) %1, i16 2
  %5 = load i16, ptr addrspace(1) %4
  %6 = icmp sge i16 %3, %5
  br i1 %6, label %b4, label %b3

b3:
  br label %b4

b4:
  %7 = phi ptr addrspace(1) [ %0, %b1 ], [ %1, %b3 ]
  ret ptr addrspace(1) %7
}

define internal void @best(ptr addrspace(1) %0, ptr addrspace(1) noalias readonly dereferenceable(8) %1) addrspace(1) {
b1:
  %2 = load i16, ptr addrspace(1) %1
  %3 = icmp eq i16 %2, 0
  br i1 %3, label %b2, label %b3

b2:
  store i8 1, ptr addrspace(1) %0
  ret void

b3:
  %4 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %5 = load ptr addrspace(1), ptr addrspace(1) %4
  br label %b5

b5:
  %6 = phi i16 [ 0, %b3 ], [ %20, %b15 ]
  %7 = phi i16 [ 1, %b3 ], [ %21, %b15 ]
  %8 = icmp ult i16 %7, %2
  br i1 %8, label %b6, label %b8

b6:
  %9 = shl i16 %7, 2
  %10 = getelementptr i8, ptr addrspace(1) %5, i16 %9
  %11 = getelementptr i8, ptr addrspace(1) %10, i16 2
  %12 = load i16, ptr addrspace(1) %11
  %13 = icmp ult i16 %6, %2
  br i1 %13, label %b11, label %b12

b8:
  %14 = icmp ult i16 %6, %2
  br i1 %14, label %b16, label %b17

b11:
  %15 = shl i16 %6, 2
  %16 = getelementptr i8, ptr addrspace(1) %5, i16 %15
  %17 = getelementptr i8, ptr addrspace(1) %16, i16 2
  %18 = load i16, ptr addrspace(1) %17
  %19 = icmp sgt i16 %12, %18
  br i1 %19, label %b15, label %b14

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b14:
  br label %b15

b15:
  %20 = phi i16 [ %7, %b11 ], [ %6, %b14 ]
  %21 = add i16 %7, 1
  br label %b5

b16:
  %22 = shl i16 %6, 2
  %23 = getelementptr i8, ptr addrspace(1) %5, i16 %22
  store i8 0, ptr addrspace(1) %0
  %24 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr addrspace(1) %23, ptr addrspace(1) %24
  ret void

b17:
  call addrspace(1) void @N$EBND()
  unreachable
}

define internal void @first_name(ptr addrspace(1) %0, ptr addrspace(1) noalias readonly dereferenceable(8) %1) addrspace(1) willreturn {
b1:
  %2 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %3 = load ptr addrspace(1), ptr addrspace(1) %2
  %4 = load i16, ptr addrspace(1) %1
  %5 = icmp sge i16 %4, 1
  br i1 %5, label %b4, label %b3

b3:
  %6 = getelementptr i8, ptr @$str1, i16 6
  %7 = getelementptr i8, ptr %6, i16 -4
  %8 = load i16, ptr %7
  %9 = addrspacecast ptr %6 to ptr addrspace(1)
  store i16 %8, ptr addrspace(1) %0
  %10 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %8, ptr addrspace(1) %10
  %11 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store ptr addrspace(1) %9, ptr addrspace(1) %11
  ret void

b4:
  %12 = getelementptr i8, ptr addrspace(1) %3, i16 0
  %13 = load ptr, ptr addrspace(1) %12
  %14 = getelementptr i8, ptr %13, i16 -4
  %15 = load i16, ptr %14
  %16 = addrspacecast ptr %13 to ptr addrspace(1)
  store i16 %15, ptr addrspace(1) %0
  %17 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %15, ptr addrspace(1) %17
  %18 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store ptr addrspace(1) %16, ptr addrspace(1) %18
  ret void
}

define internal void @word(ptr addrspace(1) %0, ptr addrspace(1) noalias readonly dereferenceable(8) %1, i16 %2) addrspace(1) {
b1:
  %3 = load i16, ptr addrspace(1) %1
  %4 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %5 = load ptr addrspace(1), ptr addrspace(1) %4
  br label %b2

b2:
  %6 = phi i16 [ %2, %b1 ], [ %9, %b3 ]
  %7 = icmp ult i16 %6, %3
  %8 = sext i1 %7 to i8
  br i1 %7, label %b5, label %b6

b3:
  %9 = add i16 %6, 1
  br label %b2

b4:
  %10 = icmp ule i16 %6, %3
  br i1 %10, label %b9, label %b10

b5:
  %11 = getelementptr i8, ptr addrspace(1) %5, i16 %6
  %12 = load i8, ptr addrspace(1) %11
  %13 = icmp ne i8 %12, 32
  %14 = sext i1 %13 to i8
  br label %b6

b6:
  %15 = phi i8 [ %8, %b2 ], [ %14, %b5 ]
  %16 = icmp ne i8 %15, 0
  br i1 %16, label %b3, label %b4

b9:
  %17 = icmp ule i16 %2, %6
  br i1 %17, label %b11, label %b12

b10:
  call addrspace(1) void @N$EBND()
  unreachable

b11:
  %18 = getelementptr i8, ptr addrspace(1) %5, i16 %2
  %19 = sub i16 %6, %2
  store i16 %19, ptr addrspace(1) %0
  %20 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %19, ptr addrspace(1) %20
  %21 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store ptr addrspace(1) %18, ptr addrspace(1) %21
  ret void

b12:
  call addrspace(1) void @N$EBND()
  unreachable
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca [8 x i8]
  %1 = alloca [8 x i8]
  %2 = alloca [8 x i8]
  %3 = alloca [8 x i8]
  %4 = alloca [8 x i8]
  %5 = alloca [8 x i8]
  %6 = alloca [8 x i8]
  %7 = alloca [8 x i8]
  %8 = alloca [6 x i8]
  %9 = alloca [8 x i8]
  %10 = alloca [4 x i8]
  %11 = alloca [4 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %5, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %6, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %7, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %8, i8 0, i16 6, i1 false)
  call void @llvm.memset.p0.i16(ptr %9, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %10, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr %11, i8 0, i16 4, i1 false)
  %12 = getelementptr i8, ptr @$str2, i16 6
  store ptr %12, ptr %11, !tbaa !2
  %13 = getelementptr inbounds i8, ptr %11, i16 2
  store i16 31, ptr %13, !tbaa !2
  %14 = getelementptr i8, ptr @$str3, i16 6
  store ptr %14, ptr %10, !tbaa !2
  %15 = getelementptr inbounds i8, ptr %10, i16 2
  store i16 45, ptr %15, !tbaa !2
  %16 = getelementptr i8, ptr %14, i16 -4
  %17 = load i16, ptr %16
  %18 = addrspacecast ptr %14 to ptr addrspace(1)
  store i16 %17, ptr %9, !tbaa !2
  %19 = getelementptr inbounds i8, ptr %9, i16 2
  store i16 %17, ptr %19, !tbaa !2
  %20 = getelementptr inbounds i8, ptr %9, i16 4
  store ptr addrspace(1) %18, ptr %20, !tbaa !2
  %21 = addrspacecast ptr %9 to ptr addrspace(1)
  %22 = getelementptr i8, ptr @$str4, i16 6
  call addrspace(1) void @N$PS(ptr %22)
  call addrspace(1) void @N$PV(ptr addrspace(1) %21)
  call addrspace(1) void @N$PN()
  %23 = getelementptr i8, ptr @$str5, i16 6
  %24 = call addrspace(1) ptr @N$BGRW(ptr %23, i16 3, i16 4)
  %25 = getelementptr i8, ptr %24, i16 0
  %26 = getelementptr i8, ptr @$str6, i16 6
  store ptr %26, ptr %25
  %27 = getelementptr i8, ptr %25, i16 2
  store i16 12, ptr %27
  %28 = getelementptr i8, ptr %24, i16 4
  %29 = getelementptr i8, ptr @$str7, i16 6
  store ptr %29, ptr %28
  %30 = getelementptr i8, ptr %28, i16 2
  store i16 58, ptr %30
  %31 = getelementptr i8, ptr %24, i16 8
  %32 = getelementptr i8, ptr @$str8, i16 6
  store ptr %32, ptr %31
  %33 = getelementptr i8, ptr %31, i16 2
  store i16 40, ptr %33
  %34 = addrspacecast ptr %8 to ptr addrspace(1)
  %35 = getelementptr i8, ptr %24, i16 -4
  %36 = load i16, ptr %35
  %37 = addrspacecast ptr %24 to ptr addrspace(1)
  store i16 %36, ptr %7, !tbaa !2
  %38 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 %36, ptr %38, !tbaa !2
  %39 = getelementptr inbounds i8, ptr %7, i16 4
  store ptr addrspace(1) %37, ptr %39, !tbaa !2
  %40 = addrspacecast ptr %7 to ptr addrspace(1)
  call addrspace(1) void @best(ptr addrspace(1) %34, ptr addrspace(1) %40)
  %41 = load i8, ptr %8, !tbaa !2
  %42 = icmp eq i8 %41, 0
  br i1 %42, label %b4, label %b3

b2:
  %43 = addrspacecast ptr %6 to ptr addrspace(1)
  %44 = load i16, ptr %35
  store i16 %44, ptr %5, !tbaa !2
  %45 = getelementptr inbounds i8, ptr %5, i16 2
  store i16 %44, ptr %45, !tbaa !2
  %46 = getelementptr inbounds i8, ptr %5, i16 4
  store ptr addrspace(1) %37, ptr %46, !tbaa !2
  %47 = addrspacecast ptr %5 to ptr addrspace(1)
  call addrspace(1) void @first_name(ptr addrspace(1) %43, ptr addrspace(1) %47)
  %48 = getelementptr i8, ptr @$str12, i16 6
  call addrspace(1) void @N$PS(ptr %48)
  call addrspace(1) void @N$PV(ptr addrspace(1) %43)
  call addrspace(1) void @N$PN()
  %49 = getelementptr i8, ptr @$str13, i16 6
  %50 = addrspacecast ptr %4 to ptr addrspace(1)
  %51 = getelementptr i8, ptr %49, i16 -4
  %52 = load i16, ptr %51
  %53 = addrspacecast ptr %49 to ptr addrspace(1)
  store i16 %52, ptr %3, !tbaa !2
  %54 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 %52, ptr %54, !tbaa !2
  %55 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %53, ptr %55, !tbaa !2
  %56 = addrspacecast ptr %3 to ptr addrspace(1)
  call addrspace(1) void @word(ptr addrspace(1) %50, ptr addrspace(1) %56, i16 0)
  %57 = addrspacecast ptr %2 to ptr addrspace(1)
  %58 = load i16, ptr addrspace(1) %50, !tbaa !2
  store i16 %58, ptr addrspace(1) %57, !tbaa !2
  %59 = getelementptr i8, ptr addrspace(1) %50, i16 2
  %60 = load i16, ptr addrspace(1) %59, !tbaa !2
  %61 = getelementptr i8, ptr addrspace(1) %57, i16 2
  store i16 %60, ptr addrspace(1) %61, !tbaa !2
  %62 = getelementptr i8, ptr addrspace(1) %50, i16 4
  %63 = load ptr addrspace(1), ptr addrspace(1) %62, !tbaa !2
  %64 = getelementptr i8, ptr addrspace(1) %57, i16 4
  store ptr addrspace(1) %63, ptr addrspace(1) %64, !tbaa !2
  %65 = getelementptr i8, ptr @$str14, i16 6
  %66 = getelementptr i8, ptr @$str15, i16 6
  %67 = addrspacecast ptr %1 to ptr addrspace(1)
  %68 = getelementptr inbounds i8, ptr %0, i16 2
  %69 = getelementptr inbounds i8, ptr %0, i16 4
  %70 = addrspacecast ptr %0 to ptr addrspace(1)
  %71 = getelementptr i8, ptr addrspace(1) %67, i16 2
  %72 = getelementptr i8, ptr addrspace(1) %67, i16 4
  br label %b6

b3:
  %73 = getelementptr i8, ptr @$str11, i16 6
  call addrspace(1) void @N$PS(ptr %73)
  call addrspace(1) void @N$PN()
  br label %b2

b4:
  %74 = getelementptr inbounds i8, ptr %8, i16 2
  %75 = load ptr addrspace(1), ptr %74, !tbaa !2
  %76 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %76)
  %77 = load ptr, ptr addrspace(1) %75
  call addrspace(1) void @N$PS(ptr %77)
  %78 = getelementptr i8, ptr @$str10, i16 6
  call addrspace(1) void @N$PS(ptr %78)
  %79 = getelementptr i8, ptr addrspace(1) %75, i16 2
  %80 = load i16, ptr addrspace(1) %79
  call addrspace(1) void @N$PI2(i16 %80)
  call addrspace(1) void @N$PN()
  br label %b2

b6:
  %81 = phi i16 [ 0, %b2 ], [ %86, %b10 ]
  %82 = load i16, ptr addrspace(1) %57
  %83 = icmp ugt i16 %82, 0
  br i1 %83, label %b7, label %b8

b7:
  call addrspace(1) void @N$PS(ptr %65)
  call addrspace(1) void @N$PV(ptr addrspace(1) %57)
  call addrspace(1) void @N$PS(ptr %66)
  call addrspace(1) void @N$PN()
  %84 = load i16, ptr addrspace(1) %57
  %85 = add i16 %84, 1
  %86 = add i16 %81, %85
  %87 = load i16, ptr %51
  %88 = icmp uge i16 %86, %87
  br i1 %88, label %b8, label %b10

b8:
  call addrspace(1) void @N$BDRP(ptr %49)
  %89 = icmp ne ptr %24, null
  br i1 %89, label %b13, label %b12

b10:
  store i16 %87, ptr %0, !tbaa !2
  store i16 %87, ptr %68, !tbaa !2
  store ptr addrspace(1) %53, ptr %69, !tbaa !2
  call addrspace(1) void @word(ptr addrspace(1) %67, ptr addrspace(1) %70, i16 %86)
  %90 = load i16, ptr addrspace(1) %67, !tbaa !2
  store i16 %90, ptr addrspace(1) %57, !tbaa !2
  %91 = load i16, ptr addrspace(1) %71, !tbaa !2
  store i16 %91, ptr addrspace(1) %61, !tbaa !2
  %92 = load ptr addrspace(1), ptr addrspace(1) %72, !tbaa !2
  store ptr addrspace(1) %92, ptr addrspace(1) %64, !tbaa !2
  br label %b6

b12:
  call addrspace(1) void @N$BDRP(ptr %24)
  %93 = load ptr, ptr %10, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %93)
  %94 = load ptr, ptr %11, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %94)
  ret i16 0

b13:
  %95 = load i16, ptr %35
  br label %b14

b14:
  %96 = phi i16 [ 0, %b13 ], [ %101, %b16 ]
  %97 = icmp ult i16 %96, %95
  br i1 %97, label %b16, label %b12

b16:
  %98 = shl i16 %96, 2
  %99 = getelementptr i8, ptr %24, i16 %98
  %100 = load ptr, ptr %99
  call addrspace(1) void @N$BDRP(ptr %100)
  %101 = add i16 %96, 1
  br label %b14
}

declare void @N$EBND() addrspace(1)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PV(ptr addrspace(1)) addrspace(1)

declare void @N$PN() addrspace(1)

declare ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare void @N$PI2(i16) addrspace(1)

declare void @N$BDRP(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
