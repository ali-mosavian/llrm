target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [9 x i8] c"\08\00\02\00\02\00: \00"
@$str2 = internal constant [16 x i8] c"\08\00\09\00\09\00first six\00"

define internal i32 @Dice.iter(ptr addrspace(1) %0) addrspace(1) {
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

define internal i32 @Rolls.next(ptr addrspace(1) %0) addrspace(1) {
b1:
  %1 = alloca [4 x i8]
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 4, i1 false)
  %2 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %3 = load i16, ptr addrspace(1) %2
  %4 = icmp eq i16 %3, 0
  %5 = sext i1 %4 to i8
  %6 = icmp ne i8 %5, 0
  br i1 %6, label %b2, label %b3

b2:
  store i8 1, ptr %1, !tbaa !2
  %7 = addrspacecast ptr %1 to ptr addrspace(1)
  %8 = load i32, ptr addrspace(1) %7, !tbaa !2
  ret i32 %8

b3:
  br label %b4

b4:
  %9 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %10 = load i16, ptr addrspace(1) %9
  %11 = sub i16 %10, 1
  %12 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %11, ptr addrspace(1) %12
  %13 = load i16, ptr addrspace(1) %0
  %14 = mul i16 %13, 25173
  %15 = add i16 %14, 13849
  store i16 %15, ptr addrspace(1) %0
  %16 = load i16, ptr addrspace(1) %0
  %17 = lshr i16 %16, 8
  %18 = urem i16 %17, 6
  %19 = add i16 %18, 1
  store i8 0, ptr %1, !tbaa !2
  %20 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 %19, ptr %20, !tbaa !2
  %21 = addrspacecast ptr %1 to ptr addrspace(1)
  %22 = load i32, ptr addrspace(1) %21, !tbaa !2
  ret i32 %22
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca i16
  %1 = alloca [4 x i8]
  %2 = alloca [4 x i8]
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca [4 x i8]
  %7 = alloca [4 x i8]
  %8 = alloca [4 x i8]
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca [12 x i8]
  %14 = alloca i16
  %15 = alloca [4 x i8]
  store i16 0, ptr %0
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 4, i1 false)
  store i16 0, ptr %3
  store i16 0, ptr %4
  store i16 0, ptr %5
  call void @llvm.memset.p0.i16(ptr %6, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr %7, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr %8, i8 0, i16 4, i1 false)
  store i16 0, ptr %9
  store i16 0, ptr %10
  store i16 0, ptr %11
  store i16 0, ptr %12
  call void @llvm.memset.p0.i16(ptr %13, i8 0, i16 12, i1 false)
  store i16 0, ptr %14
  call void @llvm.memset.p0.i16(ptr %15, i8 0, i16 4, i1 false)
  store i16 2024, ptr %15, !tbaa !2
  %16 = getelementptr inbounds i8, ptr %15, i16 2
  store i16 60, ptr %16, !tbaa !2
  store i16 0, ptr %14, !tbaa !2
  store i16 6, ptr %11, !tbaa !2
  store i16 6, ptr %12, !tbaa !2
  store i16 0, ptr %10, !tbaa !2
  store i16 6, ptr %9, !tbaa !2
  br label %b2

b2:
  %17 = load i16, ptr %10, !tbaa !2
  %18 = load i16, ptr %9, !tbaa !2
  %19 = icmp slt i16 %17, %18
  %20 = sext i1 %19 to i8
  %21 = icmp ne i8 %20, 0
  br i1 %21, label %b3, label %b5

b3:
  %22 = load i16, ptr %10, !tbaa !2
  %23 = load i16, ptr %14, !tbaa !2
  %24 = sub i16 %22, 0
  %25 = getelementptr inbounds i16, ptr %13, i16 %24
  store i16 %23, ptr %25, !tbaa !2
  br label %b4

b4:
  %26 = load i16, ptr %10, !tbaa !2
  %27 = add i16 %26, 1
  store i16 %27, ptr %10, !tbaa !2
  br label %b2

b5:
  %28 = addrspacecast ptr %15 to ptr addrspace(1)
  %29 = call addrspace(1) i32 @Dice.iter(ptr addrspace(1) %28)
  %30 = addrspacecast ptr %7 to ptr addrspace(1)
  store i32 %29, ptr addrspace(1) %30, !tbaa !2
  %31 = load i16, ptr %7, !tbaa !2
  %32 = getelementptr inbounds i8, ptr %7, i16 2
  %33 = load i16, ptr %32, !tbaa !2
  store i16 %31, ptr %8, !tbaa !2
  %34 = getelementptr inbounds i8, ptr %8, i16 2
  store i16 %33, ptr %34, !tbaa !2
  br label %b6

b6:
  br label %b7

b7:
  %35 = addrspacecast ptr %8 to ptr addrspace(1)
  %36 = call addrspace(1) i32 @Rolls.next(ptr addrspace(1) %35)
  %37 = addrspacecast ptr %6 to ptr addrspace(1)
  store i32 %36, ptr addrspace(1) %37, !tbaa !2
  %38 = load i8, ptr %6, !tbaa !2
  %39 = icmp eq i8 %38, 0
  %40 = sext i1 %39 to i8
  %41 = icmp ne i8 %40, 0
  br i1 %41, label %b11, label %b10

b8:
  store i16 0, ptr %4, !tbaa !2
  store i16 6, ptr %3, !tbaa !2
  br label %b15

b9:
  br label %b6

b10:
  br label %b8

b11:
  %42 = getelementptr inbounds i8, ptr %6, i16 2
  %43 = load i16, ptr %42, !tbaa !2
  %44 = getelementptr inbounds i8, ptr %6, i16 2
  %45 = load i16, ptr %44, !tbaa !2
  store i16 %45, ptr %5, !tbaa !2
  %46 = load i16, ptr %5, !tbaa !2
  %47 = sub i16 %46, 1
  %48 = icmp ult i16 %47, 6
  %49 = sext i1 %48 to i8
  %50 = icmp ne i8 %49, 0
  br i1 %50, label %b12, label %b13

b12:
  %51 = sub i16 %47, 0
  %52 = getelementptr inbounds i16, ptr %13, i16 %51
  %53 = load i16, ptr %52, !tbaa !2
  %54 = add i16 %53, 1
  %55 = sub i16 %47, 0
  %56 = getelementptr inbounds i16, ptr %13, i16 %55
  store i16 %54, ptr %56, !tbaa !2
  br label %b9

b13:
  call addrspace(1) void @N$EBND()
  unreachable

b15:
  %57 = load i16, ptr %4, !tbaa !2
  %58 = load i16, ptr %3, !tbaa !2
  %59 = icmp slt i16 %57, %58
  %60 = sext i1 %59 to i8
  %61 = icmp ne i8 %60, 0
  br i1 %61, label %b16, label %b18

b16:
  %62 = load i16, ptr %4, !tbaa !2
  %63 = add i16 %62, 1
  call addrspace(1) void @N$PI2(i16 %63)
  %64 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %64)
  %65 = load i16, ptr %4, !tbaa !2
  %66 = icmp ult i16 %65, 6
  %67 = sext i1 %66 to i8
  %68 = icmp ne i8 %67, 0
  br i1 %68, label %b19, label %b20

b17:
  %69 = load i16, ptr %4, !tbaa !2
  %70 = add i16 %69, 1
  store i16 %70, ptr %4, !tbaa !2
  br label %b15

b18:
  store i16 7, ptr %2, !tbaa !2
  %71 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 100, ptr %71, !tbaa !2
  br label %b21

b19:
  %72 = sub i16 %65, 0
  %73 = getelementptr inbounds i16, ptr %13, i16 %72
  %74 = load i16, ptr %73, !tbaa !2
  call addrspace(1) void @N$PFLD(i8 2, i8 10, i8 32, i8 0)
  call addrspace(1) void @N$PU2(i16 %74)
  call addrspace(1) void @N$PN()
  br label %b17

b20:
  call addrspace(1) void @N$EBND()
  unreachable

b21:
  br label %b22

b22:
  %75 = addrspacecast ptr %2 to ptr addrspace(1)
  %76 = call addrspace(1) i32 @Rolls.next(ptr addrspace(1) %75)
  %77 = addrspacecast ptr %1 to ptr addrspace(1)
  store i32 %76, ptr addrspace(1) %77, !tbaa !2
  %78 = load i8, ptr %1, !tbaa !2
  %79 = icmp eq i8 %78, 0
  %80 = sext i1 %79 to i8
  %81 = icmp ne i8 %80, 0
  br i1 %81, label %b26, label %b25

b23:
  ret i16 0

b24:
  br label %b21

b25:
  br label %b23

b26:
  %82 = getelementptr inbounds i8, ptr %1, i16 2
  %83 = load i16, ptr %82, !tbaa !2
  %84 = getelementptr inbounds i8, ptr %1, i16 2
  %85 = load i16, ptr %84, !tbaa !2
  store i16 %85, ptr %0, !tbaa !2
  %86 = load i16, ptr %0, !tbaa !2
  %87 = icmp eq i16 %86, 6
  %88 = sext i1 %87 to i8
  %89 = icmp ne i8 %88, 0
  br i1 %89, label %b27, label %b28

b27:
  %90 = getelementptr i8, ptr @$str2, i16 6
  call addrspace(1) void @N$PS(ptr %90)
  call addrspace(1) void @N$PN()
  br label %b23

b28:
  br label %b29

b29:
  br label %b24
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
