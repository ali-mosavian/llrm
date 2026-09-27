target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [9 x i8] c"\08\00\02\00\02\00: \00"
@$str2 = internal constant [16 x i8] c"\08\00\09\00\09\00first six\00"

define internal i32 @Dice.iter(ptr addrspace(1) %0) addrspace(1) memory(argmem: read) willreturn {
b1:
  %1 = alloca [4 x i8]
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 4, i1 false)
  %2 = load i16, ptr addrspace(1) %0
  %3 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %4 = load i16, ptr addrspace(1) %3
  store i16 %2, ptr %1, !tbaa !2
  %5 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 %4, ptr %5, !tbaa !2
  %6 = addrspacecast ptr %1 to ptr addrspace(1)
  %7 = load i32, ptr addrspace(1) %6, !tbaa !2
  ret i32 %7
}

define internal i32 @Rolls.next(ptr addrspace(1) %0) addrspace(1) memory(argmem: readwrite) willreturn {
b1:
  %1 = alloca [4 x i8]
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 4, i1 false)
  %2 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %3 = load i16, ptr addrspace(1) %2
  %4 = icmp eq i16 %3, 0
  br i1 %4, label %b2, label %b3

b2:
  store i8 1, ptr %1, !tbaa !2
  %5 = addrspacecast ptr %1 to ptr addrspace(1)
  %6 = load i32, ptr addrspace(1) %5, !tbaa !2
  ret i32 %6

b3:
  %7 = add i16 %3, -1
  store i16 %7, ptr addrspace(1) %2
  %8 = load i16, ptr addrspace(1) %0
  %9 = mul i16 %8, 25173
  %10 = add i16 %9, 13849
  store i16 %10, ptr addrspace(1) %0
  %11 = lshr i16 %10, 8
  %12 = urem i16 %11, 6
  %13 = add i16 %12, 1
  store i8 0, ptr %1, !tbaa !2
  %14 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 %13, ptr %14, !tbaa !2
  %15 = addrspacecast ptr %1 to ptr addrspace(1)
  %16 = load i32, ptr addrspace(1) %15, !tbaa !2
  ret i32 %16
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca [4 x i8]
  %1 = alloca [4 x i8]
  %2 = alloca [4 x i8]
  %3 = alloca [4 x i8]
  %4 = alloca [4 x i8]
  %5 = alloca [4 x i8]
  %6 = alloca [4 x i8]
  %7 = alloca [4 x i8]
  %8 = alloca [12 x i8]
  %9 = alloca [4 x i8]
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr %5, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr %6, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr %7, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr %8, i8 0, i16 12, i1 false)
  call void @llvm.memset.p0.i16(ptr %9, i8 0, i16 4, i1 false)
  store i16 2024, ptr %9, !tbaa !2
  %10 = getelementptr inbounds i8, ptr %9, i16 2
  store i16 60, ptr %10, !tbaa !2
  br label %b2

b2:
  %11 = phi i16 [ 0, %b1 ], [ %14, %b3 ]
  %12 = icmp slt i16 %11, 6
  br i1 %12, label %b3, label %b5

b3:
  %13 = getelementptr inbounds i16, ptr %8, i16 %11
  store i16 0, ptr %13, !tbaa !2
  %14 = add i16 %11, 1
  br label %b2

b5:
  %15 = addrspacecast ptr %9 to ptr addrspace(1)
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 4, i1 false)
  %16 = load i16, ptr addrspace(1) %15
  %17 = getelementptr i8, ptr addrspace(1) %15, i16 2
  %18 = load i16, ptr addrspace(1) %17
  store i16 %16, ptr %2
  %19 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 %18, ptr %19
  %20 = addrspacecast ptr %2 to ptr addrspace(1)
  %21 = load i32, ptr addrspace(1) %20
  %22 = addrspacecast ptr %6 to ptr addrspace(1)
  store i32 %21, ptr addrspace(1) %22, !tbaa !2
  %23 = load i16, ptr %6, !tbaa !2
  %24 = getelementptr inbounds i8, ptr %6, i16 2
  %25 = load i16, ptr %24, !tbaa !2
  store i16 %23, ptr %7, !tbaa !2
  %26 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 %25, ptr %26, !tbaa !2
  %27 = addrspacecast ptr %7 to ptr addrspace(1)
  %28 = getelementptr i8, ptr addrspace(1) %27, i16 2
  %29 = addrspacecast ptr %5 to ptr addrspace(1)
  %30 = getelementptr inbounds i8, ptr %5, i16 2
  %31 = getelementptr inbounds i8, ptr %1, i16 2
  %32 = addrspacecast ptr %1 to ptr addrspace(1)
  br label %b7

b7:
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 4, i1 false)
  %33 = load i16, ptr addrspace(1) %28
  %34 = icmp eq i16 %33, 0
  br i1 %34, label %35, label %37

35:
  store i8 1, ptr %1
  %36 = load i32, ptr addrspace(1) %32
  br label %46

37:
  %38 = add i16 %33, -1
  store i16 %38, ptr addrspace(1) %28
  %39 = load i16, ptr addrspace(1) %27
  %40 = mul i16 %39, 25173
  %41 = add i16 %40, 13849
  store i16 %41, ptr addrspace(1) %27
  %42 = lshr i16 %41, 8
  %43 = urem i16 %42, 6
  %44 = add i16 %43, 1
  store i8 0, ptr %1
  store i16 %44, ptr %31
  %45 = load i32, ptr addrspace(1) %32
  br label %46

46:
  %47 = phi i32 [ %36, %35 ], [ %45, %37 ]
  store i32 %47, ptr addrspace(1) %29, !tbaa !2
  %48 = load i8, ptr %5, !tbaa !2
  %49 = icmp eq i8 %48, 0
  br i1 %49, label %b11, label %b10

b10:
  %50 = getelementptr i8, ptr @$str1, i16 6
  br label %b15

b11:
  %51 = load i16, ptr %30, !tbaa !2
  %52 = add i16 %51, -1
  %53 = icmp ult i16 %52, 6
  br i1 %53, label %b12, label %b13

b12:
  %54 = getelementptr inbounds i16, ptr %8, i16 %52
  %55 = load i16, ptr %54, !tbaa !2
  %56 = add i16 %55, 1
  store i16 %56, ptr %54, !tbaa !2
  br label %b7

b13:
  call addrspace(1) void @N$EBND()
  unreachable

b15:
  %57 = phi i16 [ 0, %b10 ], [ %59, %b16 ]
  %58 = icmp slt i16 %57, 6
  br i1 %58, label %b16, label %b18

b16:
  %59 = add i16 %57, 1
  call addrspace(1) void @N$PI2(i16 %59)
  call addrspace(1) void @N$PS(ptr %50)
  %60 = getelementptr inbounds i16, ptr %8, i16 %57
  %61 = load i16, ptr %60, !tbaa !2
  call addrspace(1) void @N$PFLD(i8 2, i8 10, i8 32, i8 0)
  call addrspace(1) void @N$PU2(i16 %61)
  call addrspace(1) void @N$PN()
  br label %b15

b18:
  store i16 7, ptr %4, !tbaa !2
  %62 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 100, ptr %62, !tbaa !2
  %63 = addrspacecast ptr %4 to ptr addrspace(1)
  %64 = getelementptr i8, ptr addrspace(1) %63, i16 2
  %65 = addrspacecast ptr %3 to ptr addrspace(1)
  %66 = getelementptr inbounds i8, ptr %3, i16 2
  %67 = getelementptr inbounds i8, ptr %0, i16 2
  %68 = addrspacecast ptr %0 to ptr addrspace(1)
  br label %b22

b22:
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 4, i1 false)
  %69 = load i16, ptr addrspace(1) %64
  %70 = icmp eq i16 %69, 0
  br i1 %70, label %71, label %73

71:
  store i8 1, ptr %0
  %72 = load i32, ptr addrspace(1) %68
  br label %82

73:
  %74 = add i16 %69, -1
  store i16 %74, ptr addrspace(1) %64
  %75 = load i16, ptr addrspace(1) %63
  %76 = mul i16 %75, 25173
  %77 = add i16 %76, 13849
  store i16 %77, ptr addrspace(1) %63
  %78 = lshr i16 %77, 8
  %79 = urem i16 %78, 6
  %80 = add i16 %79, 1
  store i8 0, ptr %0
  store i16 %80, ptr %67
  %81 = load i32, ptr addrspace(1) %68
  br label %82

82:
  %83 = phi i32 [ %72, %71 ], [ %81, %73 ]
  store i32 %83, ptr addrspace(1) %65, !tbaa !2
  %84 = load i8, ptr %3, !tbaa !2
  %85 = icmp eq i8 %84, 0
  br i1 %85, label %b26, label %b23

b23:
  ret i16 0

b26:
  %86 = load i16, ptr %66, !tbaa !2
  %87 = icmp eq i16 %86, 6
  br i1 %87, label %b27, label %b29

b27:
  %88 = getelementptr i8, ptr @$str2, i16 6
  call addrspace(1) void @N$PS(ptr %88)
  call addrspace(1) void @N$PN()
  br label %b23

b29:
  br label %b22
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$EBND() addrspace(1)

declare void @N$PI2(i16) addrspace(1)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PFLD(i8, i8, i8, i8) addrspace(1)

declare void @N$PU2(i16) addrspace(1)

declare void @N$PN() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
