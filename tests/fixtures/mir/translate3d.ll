target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-i32:16-i64:16-n8:16:32"

@"X!" = internal global [18 x i8] zeroinitializer
@"Y!" = internal global [18 x i8] zeroinitializer
@"Z!" = internal global [18 x i8] zeroinitializer
@"XS%" = internal global [22 x i8] zeroinitializer
@"YS%" = internal global [22 x i8] zeroinitializer
@"NDTS%" = internal global [2 x i8] zeroinitializer
@"OBJ%" = internal global [2 x i8] zeroinitializer

define internal cc1000 void @__main() addrspace(1) memory(readwrite, argmem: none) {
b1:
  call cc1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 4096, i16 4, i16 257, ptr @"X!")
  call cc1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 4096, i16 4, i16 257, ptr @"Y!")
  call cc1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 4096, i16 4, i16 257, ptr @"Z!")
  call cc1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 4096, i16 0, i16 1, i16 2, i16 258, ptr @"XS%")
  call cc1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 4096, i16 0, i16 1, i16 2, i16 258, ptr @"YS%")
  store i16 100, ptr @"NDTS%", !tbaa !2
  store i16 1, ptr @"OBJ%", !tbaa !2
  call cc1000 addrspace(1) void @TRANSLATE3D()
  ret void
}

define cc1000 void @TRANSLATE3D() addrspace(1) memory(readwrite, argmem: none, inaccessiblemem: none) {
b1:
  %0 = load i16, ptr @"OBJ%", !tbaa !2
  %1 = icmp eq i16 %0, 1
  br i1 %1, label %b3, label %b5

b2:
  %lsr.iv.next = add i16 %lsr.iv2, 2
  %lsr.iv.next1 = add i16 %lsr.iv11, 4
  %2 = icmp ne i16 %lsr.iv.next, %30
  br i1 %2, label %b12, label %58

b3:
  br label %b5

b5:
  %3 = phi i16 [ 1, %b3 ], [ 0, %b1 ]
  %4 = icmp sgt i16 %0, 4
  br i1 %4, label %b6, label %b8

b6:
  br label %b8

b8:
  %5 = phi i16 [ 8, %b6 ], [ %3, %b5 ]
  %6 = load i16, ptr @"NDTS%", !tbaa !2
  %7 = sub i16 %6, 1
  %8 = getelementptr i8, ptr @"XS%", i16 2
  %9 = load i16, ptr %8, !tbaa !2
  %10 = inttoptr i16 %9 to ptr addrspace(2)
  %11 = addrspacecast ptr addrspace(2) %10 to ptr addrspace(1)
  %12 = getelementptr i8, ptr @"YS%", i16 2
  %13 = load i16, ptr %12, !tbaa !2
  %14 = inttoptr i16 %13 to ptr addrspace(2)
  %15 = addrspacecast ptr addrspace(2) %14 to ptr addrspace(1)
  %16 = getelementptr i8, ptr @"Z!", i16 2
  %17 = load i16, ptr %16, !tbaa !2
  %18 = inttoptr i16 %17 to ptr addrspace(2)
  %19 = addrspacecast ptr addrspace(2) %18 to ptr addrspace(1)
  %20 = getelementptr i8, ptr @"X!", i16 2
  %21 = load i16, ptr %20, !tbaa !2
  %22 = inttoptr i16 %21 to ptr addrspace(2)
  %23 = addrspacecast ptr addrspace(2) %22 to ptr addrspace(1)
  %24 = getelementptr i8, ptr @"Y!", i16 2
  %25 = load i16, ptr %24, !tbaa !2
  %26 = inttoptr i16 %25 to ptr addrspace(2)
  %27 = addrspacecast ptr addrspace(2) %26 to ptr addrspace(1)
  %28 = sitofp i16 %5 to float
  %29 = shl i16 %7, 1
  %30 = add i16 %29, 2
  %31 = icmp slt i16 %7, 0
  br i1 %31, label %b13, label %57

b12:
  %lsr.iv2 = phi i16 [ %lsr.iv.next, %b2 ], [ 0, %57 ]
  %lsr.iv11 = phi i16 [ %lsr.iv.next1, %b2 ], [ 0, %57 ]
  %32 = getelementptr i8, ptr addrspace(1) %11, i16 %lsr.iv2
  %33 = load i16, ptr addrspace(1) %32, !tbaa !4
  %34 = getelementptr i8, ptr addrspace(1) %11, i16 %lsr.iv2
  %35 = getelementptr i8, ptr addrspace(1) %34, i16 8194
  store i16 %33, ptr addrspace(1) %35, !tbaa !4
  %36 = getelementptr i8, ptr addrspace(1) %15, i16 %lsr.iv2
  %37 = load i16, ptr addrspace(1) %36, !tbaa !4
  %38 = getelementptr i8, ptr addrspace(1) %15, i16 %lsr.iv2
  %39 = getelementptr i8, ptr addrspace(1) %38, i16 8194
  store i16 %37, ptr addrspace(1) %39, !tbaa !4
  %40 = getelementptr i8, ptr addrspace(1) %19, i16 %lsr.iv11
  %41 = load float, ptr addrspace(1) %40, !tbaa !4
  %42 = fcmp ole float %41, %28
  br i1 %42, label %b2, label %b16

b13:
  ret void

b16:
  %43 = getelementptr i8, ptr addrspace(1) %23, i16 %lsr.iv11
  %44 = load float, ptr addrspace(1) %43, !tbaa !4
  %45 = fmul float %44, 2.560000e+02
  %46 = fdiv float %45, %41
  %47 = call i16 @llvm.lrint.i16.f32(float %46)
  %48 = getelementptr i8, ptr addrspace(1) %11, i16 %lsr.iv2
  store i16 %47, ptr addrspace(1) %48, !tbaa !4
  %49 = getelementptr i8, ptr addrspace(1) %27, i16 %lsr.iv11
  %50 = load float, ptr addrspace(1) %49, !tbaa !4
  %51 = fmul float %50, 2.560000e+02
  %52 = getelementptr i8, ptr addrspace(1) %19, i16 %lsr.iv11
  %53 = load float, ptr addrspace(1) %52, !tbaa !4
  %54 = fdiv float %51, %53
  %55 = call i16 @llvm.lrint.i16.f32(float %54)
  %56 = getelementptr i8, ptr addrspace(1) %15, i16 %lsr.iv2
  store i16 %55, ptr addrspace(1) %56, !tbaa !4
  br label %b2

57:
  br label %b12

58:
  br label %b13
}

declare cc1000 void @llrm.qb.B$DDIM(i16, i16, i16, i16, ptr) addrspace(1) nocallback

declare i16 @llvm.lrint.i16.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
