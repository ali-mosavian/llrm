target datalayout = "e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32"

@_vx = internal global [202 x i8] zeroinitializer
@_vy = internal global [202 x i8] zeroinitializer
@_vz = internal global [202 x i8] zeroinitializer
@_px = internal global [202 x i8] zeroinitializer
@_py = internal global [202 x i8] zeroinitializer
@_pz = internal global [202 x i8] zeroinitializer

define internal watcallcc void @advance_(i16 %0) memory(readwrite, argmem: none, inaccessiblemem: none) norecurse {
b1:
  %1 = alloca i16
  %2 = alloca i32
  %3 = alloca i16
  store i16 %0, ptr %1, !tbaa !9
  %4 = load i16, ptr %1, !tbaa !9
  %5 = sext i16 %4 to i32
  %6 = sub nsw i32 50, %5
  %7 = trunc i32 %6 to i16
  store i16 %7, ptr %3, !tbaa !9
  br label %b2

b2:
  %8 = load i16, ptr %3, !tbaa !9
  %9 = sext i16 %8 to i32
  %10 = load i16, ptr %1, !tbaa !9
  %11 = sext i16 %10 to i32
  %12 = add nsw i32 50, %11
  %13 = icmp sle i32 %9, %12
  %14 = zext i1 %13 to i32
  %15 = icmp ne i32 %14, 0
  br i1 %15, label %b4, label %b3

b3:
  ret void

b4:
  %16 = load i16, ptr %3, !tbaa !9
  %17 = sext i16 %16 to i32
  %18 = mul i32 %17, 2
  %19 = getelementptr inbounds i8, ptr @_px, i32 %18
  %20 = load i16, ptr %19, !tbaa !9
  %21 = load i16, ptr %3, !tbaa !9
  %22 = sext i16 %21 to i32
  %23 = mul i32 %22, 2
  %24 = getelementptr inbounds i8, ptr @_vx, i32 %23
  %25 = load i16, ptr %24, !tbaa !9
  %26 = sext i16 %20 to i32
  %27 = sext i16 %25 to i32
  %28 = add nsw i32 %26, %27
  %29 = trunc i32 %28 to i16
  %30 = load i16, ptr %3, !tbaa !9
  %31 = sext i16 %30 to i32
  %32 = mul i32 %31, 2
  %33 = getelementptr inbounds i8, ptr @_px, i32 %32
  store i16 %29, ptr %33, !tbaa !9
  %34 = load i16, ptr %3, !tbaa !9
  %35 = sext i16 %34 to i32
  %36 = mul i32 %35, 2
  %37 = getelementptr inbounds i8, ptr @_py, i32 %36
  %38 = load i16, ptr %37, !tbaa !9
  %39 = load i16, ptr %3, !tbaa !9
  %40 = sext i16 %39 to i32
  %41 = mul i32 %40, 2
  %42 = getelementptr inbounds i8, ptr @_vy, i32 %41
  %43 = load i16, ptr %42, !tbaa !9
  %44 = sext i16 %38 to i32
  %45 = sext i16 %43 to i32
  %46 = add nsw i32 %44, %45
  %47 = trunc i32 %46 to i16
  %48 = load i16, ptr %3, !tbaa !9
  %49 = sext i16 %48 to i32
  %50 = mul i32 %49, 2
  %51 = getelementptr inbounds i8, ptr @_py, i32 %50
  store i16 %47, ptr %51, !tbaa !9
  %52 = load i16, ptr %3, !tbaa !9
  %53 = sext i16 %52 to i32
  %54 = mul i32 %53, 2
  %55 = getelementptr inbounds i8, ptr @_pz, i32 %54
  %56 = load i16, ptr %55, !tbaa !9
  %57 = load i16, ptr %3, !tbaa !9
  %58 = sext i16 %57 to i32
  %59 = mul i32 %58, 2
  %60 = getelementptr inbounds i8, ptr @_vz, i32 %59
  %61 = load i16, ptr %60, !tbaa !9
  %62 = sext i16 %56 to i32
  %63 = sext i16 %61 to i32
  %64 = add nsw i32 %62, %63
  %65 = trunc i32 %64 to i16
  %66 = load i16, ptr %3, !tbaa !9
  %67 = sext i16 %66 to i32
  %68 = mul i32 %67, 2
  %69 = getelementptr inbounds i8, ptr @_pz, i32 %68
  store i16 %65, ptr %69, !tbaa !9
  %70 = load i16, ptr %3, !tbaa !9
  %71 = sext i16 %70 to i32
  %72 = add nsw i32 %71, 1
  %73 = trunc i32 %72 to i16
  store i16 %73, ptr %3, !tbaa !9
  br label %b2
}

define watcallcc i32 @bench_particle_() memory(readwrite, argmem: none, inaccessiblemem: none) norecurse {
b1:
  %0 = alloca i32
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i32
  %5 = trunc i32 0 to i16
  store i16 %5, ptr %1, !tbaa !9
  br label %b2

b2:
  %6 = load i16, ptr %1, !tbaa !9
  %7 = sext i16 %6 to i32
  %8 = icmp sle i32 %7, 100
  %9 = zext i1 %8 to i32
  %10 = icmp ne i32 %9, 0
  br i1 %10, label %b4, label %b3

b3:
  %11 = trunc i32 1 to i16
  store i16 %11, ptr %2, !tbaa !9
  br label %b5

b4:
  %12 = load i16, ptr %1, !tbaa !9
  %13 = sext i16 %12 to i32
  %14 = sub nsw i32 %13, 50
  %15 = trunc i32 %14 to i16
  store i16 %15, ptr %3, !tbaa !9
  %16 = load i16, ptr %3, !tbaa !9
  %17 = load i16, ptr %1, !tbaa !9
  %18 = sext i16 %17 to i32
  %19 = mul i32 %18, 2
  %20 = getelementptr inbounds i8, ptr @_px, i32 %19
  store i16 %16, ptr %20, !tbaa !9
  %21 = load i16, ptr %3, !tbaa !9
  %22 = sext i16 %21 to i32
  %23 = mul nsw i32 2, %22
  %24 = trunc i32 %23 to i16
  %25 = load i16, ptr %1, !tbaa !9
  %26 = sext i16 %25 to i32
  %27 = mul i32 %26, 2
  %28 = getelementptr inbounds i8, ptr @_py, i32 %27
  store i16 %24, ptr %28, !tbaa !9
  %29 = load i16, ptr %3, !tbaa !9
  %30 = sext i16 %29 to i32
  %31 = sub nsw i32 0, %30
  %32 = trunc i32 %31 to i16
  %33 = load i16, ptr %1, !tbaa !9
  %34 = sext i16 %33 to i32
  %35 = mul i32 %34, 2
  %36 = getelementptr inbounds i8, ptr @_pz, i32 %35
  store i16 %32, ptr %36, !tbaa !9
  %37 = load i16, ptr %3, !tbaa !9
  %38 = sext i16 %37 to i32
  %39 = and i32 %38, 7
  %40 = trunc i32 %39 to i16
  %41 = load i16, ptr %1, !tbaa !9
  %42 = sext i16 %41 to i32
  %43 = mul i32 %42, 2
  %44 = getelementptr inbounds i8, ptr @_vx, i32 %43
  store i16 %40, ptr %44, !tbaa !9
  %45 = load i16, ptr %3, !tbaa !9
  %46 = sext i16 %45 to i32
  %47 = and i32 %46, 3
  %48 = sub nsw i32 3, %47
  %49 = trunc i32 %48 to i16
  %50 = load i16, ptr %1, !tbaa !9
  %51 = sext i16 %50 to i32
  %52 = mul i32 %51, 2
  %53 = getelementptr inbounds i8, ptr @_vy, i32 %52
  store i16 %49, ptr %53, !tbaa !9
  %54 = trunc i32 1 to i16
  %55 = load i16, ptr %1, !tbaa !9
  %56 = sext i16 %55 to i32
  %57 = mul i32 %56, 2
  %58 = getelementptr inbounds i8, ptr @_vz, i32 %57
  store i16 %54, ptr %58, !tbaa !9
  %59 = load i16, ptr %1, !tbaa !9
  %60 = sext i16 %59 to i32
  %61 = add nsw i32 %60, 1
  %62 = trunc i32 %61 to i16
  store i16 %62, ptr %1, !tbaa !9
  br label %b2

b5:
  %63 = load i16, ptr %2, !tbaa !9
  %64 = sext i16 %63 to i32
  %65 = icmp sle i32 %64, 100
  %66 = zext i1 %65 to i32
  %67 = icmp ne i32 %66, 0
  br i1 %67, label %b7, label %b6

b6:
  store i32 0, ptr %4, !tbaa !11
  %68 = trunc i32 0 to i16
  store i16 %68, ptr %1, !tbaa !9
  br label %b8

b7:
  call watcallcc void @advance_(i16 50)
  %69 = load i16, ptr %2, !tbaa !9
  %70 = sext i16 %69 to i32
  %71 = add nsw i32 %70, 1
  %72 = trunc i32 %71 to i16
  store i16 %72, ptr %2, !tbaa !9
  br label %b5

b8:
  %73 = load i16, ptr %1, !tbaa !9
  %74 = sext i16 %73 to i32
  %75 = icmp sle i32 %74, 100
  %76 = zext i1 %75 to i32
  %77 = icmp ne i32 %76, 0
  br i1 %77, label %b10, label %b9

b9:
  %78 = load i32, ptr %4, !tbaa !11
  store i32 %78, ptr %0, !tbaa !11
  %79 = load i32, ptr %0, !tbaa !11
  ret i32 %79

b10:
  %80 = load i32, ptr %4, !tbaa !11
  %81 = load i16, ptr %1, !tbaa !9
  %82 = sext i16 %81 to i32
  %83 = mul i32 %82, 2
  %84 = getelementptr inbounds i8, ptr @_px, i32 %83
  %85 = load i16, ptr %84, !tbaa !9
  %86 = sext i16 %85 to i32
  %87 = add nsw i32 %80, %86
  %88 = load i16, ptr %1, !tbaa !9
  %89 = sext i16 %88 to i32
  %90 = mul i32 %89, 2
  %91 = getelementptr inbounds i8, ptr @_py, i32 %90
  %92 = load i16, ptr %91, !tbaa !9
  %93 = sext i16 %92 to i32
  %94 = mul nsw i32 3, %93
  %95 = add nsw i32 %87, %94
  %96 = load i16, ptr %1, !tbaa !9
  %97 = sext i16 %96 to i32
  %98 = mul i32 %97, 2
  %99 = getelementptr inbounds i8, ptr @_pz, i32 %98
  %100 = load i16, ptr %99, !tbaa !9
  %101 = sext i16 %100 to i32
  %102 = mul nsw i32 7, %101
  %103 = add nsw i32 %95, %102
  store i32 %103, ptr %4, !tbaa !11
  %104 = load i16, ptr %1, !tbaa !9
  %105 = sext i16 %104 to i32
  %106 = add nsw i32 %105, 1
  %107 = trunc i32 %106 to i16
  store i16 %107, ptr %1, !tbaa !9
  br label %b8
}

define i32 @_main() memory(readwrite, argmem: none) {
b1:
  %0 = alloca i32
  %1 = call watcallcc i32 @bench_particle_()
  %2 = call watcallcc i32 @report_(i32 %1)
  store i32 0, ptr %0, !tbaa !11
  %3 = load i32, ptr %0, !tbaa !11
  ret i32 %3
}

declare watcallcc i32 @report_(i32)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{!"Simple C/C++ TBAA"}
!6 = !{!"omnipotent char", !5, i64 0}
!7 = !{!6, !6, i64 0}
!8 = !{!"int2", !6, i64 0}
!9 = !{!8, !8, i64 0}
!10 = !{!"int4", !6, i64 0}
!11 = !{!10, !10, i64 0}
!12 = !{!"int8", !6, i64 0}
!13 = !{!12, !12, i64 0}
!14 = !{!"float4", !6, i64 0}
!15 = !{!14, !14, i64 0}
!16 = !{!"float8", !6, i64 0}
!17 = !{!16, !16, i64 0}
!18 = !{!"float10", !6, i64 0}
!19 = !{!18, !18, i64 0}
!20 = !{!"pointer2", !6, i64 0}
!21 = !{!20, !20, i64 0}
!22 = !{!"pointer4", !6, i64 0}
!23 = !{!22, !22, i64 0}
