target datalayout = "e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32"

@_far_a = global [200 x i8] zeroinitializer
@_near_a = global [200 x i8] zeroinitializer
@_near_b = global [200 x i8] zeroinitializer

define internal cc 1002 i32 @walk_(ptr noundef %0) memory(readwrite, argmem: read, inaccessiblemem: none) nounwind norecurse {
b1:
  %1 = alloca ptr
  %2 = alloca i32
  %3 = alloca i32
  %4 = alloca i16
  %5 = alloca i16
  store ptr %0, ptr %1, !tbaa !23
  store i32 0, ptr %3, !tbaa !11
  store i16 0, ptr %4, !tbaa !9
  br label %b2

b2:
  %6 = load i16, ptr %4, !tbaa !9
  %7 = sext i16 %6 to i32
  %8 = icmp slt i32 %7, 100
  br i1 %8, label %b4, label %b3

b3:
  %9 = load i32, ptr %3, !tbaa !11
  store i32 %9, ptr %2, !tbaa !11
  %10 = load i32, ptr %2, !tbaa !11
  ret i32 %10

b4:
  store i16 0, ptr %5, !tbaa !9
  br label %b5

b5:
  %11 = load i16, ptr %5, !tbaa !9
  %12 = sext i16 %11 to i32
  %13 = icmp slt i32 %12, 100
  br i1 %13, label %b7, label %b6

b6:
  %14 = load i32, ptr %3, !tbaa !11
  %15 = load ptr, ptr %1, !tbaa !23
  %16 = load i16, ptr %4, !tbaa !9
  %17 = sext i16 %16 to i32
  %18 = getelementptr inbounds i16, ptr %15, i32 %17
  %19 = load i16, ptr %18, !tbaa !9
  %20 = load i16, ptr %4, !tbaa !9
  %21 = sext i16 %20 to i32
  %22 = getelementptr inbounds i16, ptr @_near_b, i32 %21
  %23 = load i16, ptr %22, !tbaa !9
  %24 = sext i16 %19 to i32
  %25 = sext i16 %23 to i32
  %26 = add nsw i32 %24, %25
  %27 = add nsw i32 %14, %26
  store i32 %27, ptr %3, !tbaa !11
  store i16 0, ptr %5, !tbaa !9
  br label %b8

b7:
  %28 = load i16, ptr %5, !tbaa !9
  %29 = sext i16 %28 to i32
  %30 = getelementptr inbounds i16, ptr @_near_a, i32 %29
  %31 = load i16, ptr %30, !tbaa !9
  %32 = load i16, ptr %4, !tbaa !9
  %33 = sext i16 %31 to i32
  %34 = sext i16 %32 to i32
  %35 = add nsw i32 %33, %34
  %36 = trunc i32 %35 to i16
  %37 = load i16, ptr %5, !tbaa !9
  %38 = sext i16 %37 to i32
  %39 = getelementptr inbounds i16, ptr @_near_b, i32 %38
  store i16 %36, ptr %39, !tbaa !9
  %40 = load i16, ptr %5, !tbaa !9
  %41 = sext i16 %40 to i32
  %42 = add nsw i32 %41, 1
  %43 = trunc i32 %42 to i16
  store i16 %43, ptr %5, !tbaa !9
  br label %b5

b8:
  %44 = load i16, ptr %5, !tbaa !9
  %45 = sext i16 %44 to i32
  %46 = icmp slt i32 %45, 100
  br i1 %46, label %b10, label %b9

b9:
  %47 = load i16, ptr %4, !tbaa !9
  %48 = sext i16 %47 to i32
  %49 = add nsw i32 %48, 1
  %50 = trunc i32 %49 to i16
  store i16 %50, ptr %4, !tbaa !9
  br label %b2

b10:
  %51 = load i16, ptr %5, !tbaa !9
  %52 = sext i16 %51 to i32
  %53 = getelementptr inbounds i16, ptr @_near_b, i32 %52
  %54 = load i16, ptr %53, !tbaa !9
  %55 = load i16, ptr %5, !tbaa !9
  %56 = sext i16 %55 to i32
  %57 = getelementptr inbounds i16, ptr @_near_a, i32 %56
  store i16 %54, ptr %57, !tbaa !9
  %58 = load i16, ptr %5, !tbaa !9
  %59 = sext i16 %58 to i32
  %60 = add nsw i32 %59, 1
  %61 = trunc i32 %60 to i16
  store i16 %61, ptr %5, !tbaa !9
  br label %b8
}

define i32 @_main() {
b1:
  %0 = alloca i32
  %1 = alloca i16
  store i16 0, ptr %1, !tbaa !9
  br label %b2

b2:
  %2 = load i16, ptr %1, !tbaa !9
  %3 = sext i16 %2 to i32
  %4 = icmp slt i32 %3, 100
  br i1 %4, label %b4, label %b3

b3:
  %5 = call cc 1002 i32 @walk_(ptr noundef @_far_a)
  %6 = call cc 1002 i32 @report_(i32 noundef %5)
  store i32 0, ptr %0, !tbaa !11
  %7 = load i32, ptr %0, !tbaa !11
  ret i32 %7

b4:
  %8 = load i16, ptr %1, !tbaa !9
  %9 = load i16, ptr %1, !tbaa !9
  %10 = sext i16 %9 to i32
  %11 = getelementptr inbounds i16, ptr @_near_a, i32 %10
  store i16 %8, ptr %11, !tbaa !9
  %12 = load i16, ptr %1, !tbaa !9
  %13 = sext i16 %12 to i32
  %14 = mul nsw i32 %13, 3
  %15 = trunc i32 %14 to i16
  %16 = load i16, ptr %1, !tbaa !9
  %17 = sext i16 %16 to i32
  %18 = getelementptr inbounds i16, ptr @_far_a, i32 %17
  store i16 %15, ptr %18, !tbaa !9
  %19 = load i16, ptr %1, !tbaa !9
  %20 = sext i16 %19 to i32
  %21 = add nsw i32 %20, 1
  %22 = trunc i32 %21 to i16
  store i16 %22, ptr %1, !tbaa !9
  br label %b2
}

declare cc 1002 i32 @report_(i32)

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
