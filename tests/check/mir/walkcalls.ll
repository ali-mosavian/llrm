; RUN: llrm-mir %s
; CHECK: define {{.*}} @"_walk@3"(

target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-n8:16:32"

define internal regparm3cc i32 @"_walk@3"(ptr nocapture readonly %0, i16 %1, i16 range(i16 1, -32768) %2) memory(argmem: read) {
b1:
  %3 = icmp eq i16 %1, 0
  br i1 %3, label %b3, label %b2

b2:
  %4 = add nsw i16 %2, 1
  %5 = mul nsw i16 3, %1
  %6 = mul i16 %5, 2
  %7 = getelementptr i8, ptr %0, i16 2
  %8 = getelementptr i8, ptr %7, i16 %6
  %9 = load i16, ptr %8, !tbaa !9
  %10 = call regparm3cc i32 @"_walk@3"(ptr %0, i16 %9, i16 %4)
  %11 = mul i16 %5, 2
  %12 = getelementptr inbounds i8, ptr %0, i16 %11
  %13 = load i16, ptr %12, !tbaa !9
  %14 = sext i16 %13 to i32
  %15 = sext i16 %2 to i32
  %16 = mul nsw i32 %14, %15
  %17 = add nsw i32 %10, %16
  %18 = mul i16 %5, 2
  %19 = getelementptr i8, ptr %0, i16 4
  %20 = getelementptr i8, ptr %19, i16 %18
  %21 = load i16, ptr %20, !tbaa !9
  %22 = call regparm3cc i32 @"_walk@3"(ptr %0, i16 %21, i16 %4)
  %23 = add nsw i32 %17, %22
  br label %b4

b3:
  br label %b4

b4:
  %24 = phi i32 [ %23, %b2 ], [ 0, %b3 ]
  ret i32 %24
}

define regparm3cc i32 @"_bench_bintree@3"(i16 %0) addrspace(1) memory(none) norecurse {
b1:
  %1 = alloca [1806 x i8]
  %2 = getelementptr inbounds i8, ptr %1, i16 0
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 1806, i1 false)
  br label %b7

b6:
  %3 = call regparm3cc i32 @"_walk@3"(ptr %1, i16 1, i16 1)
  ret i32 %3

b7:
  %4 = phi i16 [ %0, %b1 ], [ %7, %b8 ]
  %5 = phi i16 [ 1, %b1 ], [ %12, %b8 ]
  %6 = mul i16 %4, 25173
  %7 = add i16 %6, 13849
  %8 = and i16 %7, 4095
  %9 = mul i16 %5, 6
  %10 = getelementptr inbounds i8, ptr %1, i16 %9
  store i16 %8, ptr %10, !tbaa !9
  %11 = icmp sgt i16 %5, 1
  br i1 %11, label %14, label %b8

b8:
  %12 = add nsw i16 %5, 1
  %13 = icmp sle i16 %12, 300
  br i1 %13, label %b7, label %b6

14:
  br label %15

15:
  %16 = phi ptr [ %1, %14 ], [ %16, %36 ]
  %17 = phi i16 [ 1, %14 ], [ %33, %36 ]
  %18 = phi i16 [ %5, %14 ], [ %18, %36 ]
  %19 = mul i16 %18, 6
  %20 = getelementptr inbounds i8, ptr %16, i16 %19
  %21 = load i16, ptr %20, !tbaa !9
  %22 = mul nsw i16 3, %17
  %23 = mul i16 %22, 2
  %24 = getelementptr inbounds i8, ptr %16, i16 %23
  %25 = load i16, ptr %24, !tbaa !9
  %26 = icmp slt i16 %21, %25
  br i1 %26, label %35, label %27

27:
  br label %28

28:
  %29 = phi i16 [ 2, %27 ], [ 1, %35 ]
  %30 = add nsw i16 %22, %29
  %31 = mul i16 %30, 2
  %32 = getelementptr inbounds i8, ptr %16, i16 %31
  %33 = load i16, ptr %32, !tbaa !9
  %34 = icmp eq i16 %33, 0
  br i1 %34, label %37, label %36

35:
  br label %28

36:
  br label %15

37:
  %38 = phi i16 [ %18, %28 ]
  store i16 %38, ptr %32, !tbaa !9
  br label %b8
}

define i16 @_main() addrspace(1) memory(readwrite, argmem: none) {
b1:
  %0 = call regparm3cc addrspace(1) i32 @"_bench_bintree@3"(i16 1)
  %1 = call regparm3cc addrspace(1) i16 @"_report@3"(i32 %0)
  ret i16 0
}

declare regparm3cc i16 @"_report@3"(i32) addrspace(1)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

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
