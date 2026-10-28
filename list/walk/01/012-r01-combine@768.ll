@_patterns = internal constant [81 x i8] c"Oliver|Scrooge|Copperfield|Dombey|Marley|Micawber|Heep|ghost|gentleman|Tiny Tim|\00"
@_queue = internal global [80 x i8] zeroinitializer
@_nxt = internal global [2560 x i8] zeroinitializer
@_fail = internal global [80 x i8] zeroinitializer
@_classof = internal global [256 x i8] zeroinitializer
@_hit = internal global [80 x i8] zeroinitializer

declare cc 1005 i32 @"_bench_grep@3"(ptr noundef) addrspace(1) "llrm-fp-eval"="x87-extended"

declare i16 @_main() addrspace(1) "llrm-fp-eval"="x87-extended"

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare cc 1005 i16 @"_input_read@3"(ptr, i16) addrspace(1)

declare cc 1005 i16 @"_report@3"(i32) addrspace(1)

define internal cc 1005 i32 @pipeline.body(ptr noundef %0) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca [1024 x i8]
  br label %b2

b2:
  %2 = phi i16 [ 1, %b1 ], [ %16, %b7 ]
  %3 = phi i8 [ 1, %b1 ], [ %17, %b7 ]
  %4 = phi i8 [ 1, %b1 ], [ %18, %b7 ]
  %5 = phi i16 [ 0, %b1 ], [ %19, %b7 ]
  %6 = phi ptr [ @_patterns, %b1 ], [ %20, %b7 ]
  %7 = load i8, ptr %6, !tbaa !7
  switch i8 %7, label %b5 [
    i8 0, label %b3
    i8 124, label %b6
  ]

b3:
  %8 = icmp sgt i16 %2, 1
  br i1 %8, label %9, label %b13

9:
  br label %b14

b5:
  %10 = zext i8 %7 to i16
  %11 = getelementptr inbounds nuw i8, ptr @_classof, i16 %10
  %12 = load i8, ptr %11, !tbaa !7
  %13 = icmp eq i8 %12, 0
  br i1 %13, label %b9, label %b8

b6:
  %14 = add i8 %4, 1
  %15 = getelementptr inbounds nuw i8, ptr @_hit, i16 %5
  store i8 %4, ptr %15, !tbaa !7
  br label %b7

b7:
  %16 = phi i16 [ %2, %b6 ], [ %21, %b10 ]
  %17 = phi i8 [ %3, %b6 ], [ %31, %b10 ]
  %18 = phi i8 [ %14, %b6 ], [ %4, %b10 ]
  %19 = phi i16 [ 0, %b6 ], [ %33, %b10 ]
  %20 = getelementptr inbounds nuw i8, ptr %6, i16 1
  br label %b2

b8:
  %21 = phi i16 [ %29, %b9 ], [ %2, %b5 ]
  %22 = shl nuw nsw i16 %5, 5
  %23 = load i8, ptr %11, !tbaa !7
  %24 = zext i8 %23 to i16
  %25 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %22
  %26 = getelementptr inbounds nuw i8, ptr %25, i16 %24
  %27 = load i8, ptr %26, !tbaa !7
  %28 = icmp eq i8 %27, 0
  br i1 %28, label %b11, label %b10

b9:
  %29 = add nsw i16 %2, 1
  %30 = trunc i16 %2 to i8
  store i8 %30, ptr %11, !tbaa !7
  br label %b8

b10:
  %31 = phi i8 [ %34, %b11 ], [ %3, %b8 ]
  %32 = load i8, ptr %26, !tbaa !7
  %33 = zext i8 %32 to i16
  br label %b7

b11:
  %34 = add i8 %3, 1
  store i8 %3, ptr %26, !tbaa !7
  br label %b10

35:
  br label %b13

b13:
  %36 = phi i16 [ %45, %35 ], [ 0, %b3 ]
  %37 = icmp sgt i16 %36, 0
  %38 = icmp sgt i16 %2, 0
  %or.cond = and i1 %37, %38
  br i1 %or.cond, label %39, label %b18

39:
  br label %b19

b14:
  %40 = phi i16 [ 1, %9 ], [ %46, %b15 ]
  %41 = phi i16 [ 0, %9 ], [ %45, %b15 ]
  %42 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %40
  %43 = load i8, ptr %42, !tbaa !7
  %44 = icmp eq i8 %43, 0
  br i1 %44, label %b15, label %b16

b15:
  %45 = phi i16 [ %48, %b16 ], [ %41, %b14 ]
  %46 = add nuw nsw i16 %40, 1
  %47 = icmp slt i16 %46, %2
  br i1 %47, label %b14, label %35

b16:
  %48 = add nsw i16 %41, 1
  %49 = getelementptr inbounds i8, ptr @_queue, i16 %41
  store i8 %43, ptr %49, !tbaa !7
  br label %b15

50:
  br label %b18

b18:
  %51 = call cc 1005 addrspace(1) i16 @"_input_read@3"(ptr noundef nonnull %1, i16 noundef 1024)
  %52 = icmp sgt i16 %51, 0
  br i1 %52, label %53, label %b29

53:
  br label %b30

b19:
  %54 = phi i16 [ %36, %39 ], [ %86, %63 ]
  %55 = phi i16 [ 0, %39 ], [ %56, %63 ]
  %56 = add nuw nsw i16 %55, 1
  %57 = getelementptr inbounds nuw i8, ptr @_queue, i16 %55
  %58 = load i8, ptr %57, !tbaa !7
  %59 = zext i8 %58 to i16
  %60 = shl nuw nsw i16 %59, 5
  %61 = getelementptr i8, ptr @_nxt, i16 %60
  %62 = getelementptr inbounds nuw i8, ptr @_fail, i16 %59
  br label %b22

63:
  %64 = icmp slt i16 %56, %86
  br i1 %64, label %b19, label %50

b22:
  %65 = phi i16 [ 0, %b19 ], [ %87, %b27 ]
  %66 = phi i16 [ %54, %b19 ], [ %86, %b27 ]
  %67 = getelementptr i8, ptr %61, i16 %65
  %68 = load i8, ptr %67, !tbaa !7
  %69 = load i8, ptr %62, !tbaa !7
  %70 = zext i8 %69 to i16
  %71 = shl nuw nsw i16 %70, 5
  %72 = getelementptr i8, ptr @_nxt, i16 %71
  %73 = getelementptr i8, ptr %72, i16 %65
  %74 = load i8, ptr %73, !tbaa !7
  %75 = zext i8 %74 to i16
  %76 = icmp eq i8 %68, 0
  br i1 %76, label %b23, label %b24

b23:
  store i8 %74, ptr %67, !tbaa !7
  br label %b27

b24:
  %77 = zext i8 %68 to i16
  %78 = getelementptr inbounds nuw i8, ptr @_fail, i16 %77
  store i8 %74, ptr %78, !tbaa !7
  %79 = getelementptr inbounds nuw i8, ptr @_hit, i16 %77
  %80 = load i8, ptr %79, !tbaa !7
  %81 = icmp eq i8 %80, 0
  br i1 %81, label %b26, label %b25

b25:
  %82 = add nsw i16 %66, 1
  %83 = getelementptr inbounds i8, ptr @_queue, i16 %66
  store i8 %68, ptr %83, !tbaa !7
  br label %b27

b26:
  %84 = getelementptr inbounds nuw i8, ptr @_hit, i16 %75
  %85 = load i8, ptr %84, !tbaa !7
  store i8 %85, ptr %79, !tbaa !7
  br label %b25

b27:
  %86 = phi i16 [ %82, %b25 ], [ %66, %b23 ]
  %87 = add nuw nsw i16 %65, 1
  %88 = icmp slt i16 %87, %2
  br i1 %88, label %b22, label %63

89:
  %90 = zext nneg i16 %131 to i32
  %91 = add nsw i32 %133, %90
  br label %b29

b29:
  %92 = phi i32 [ %128, %89 ], [ 0, %b18 ]
  %93 = phi i32 [ %91, %89 ], [ 0, %b18 ]
  store i32 %92, ptr %0, !tbaa !11
  ret i32 %93

b30:
  %94 = phi i16 [ %51, %53 ], [ %100, %b32 ]
  %95 = phi i32 [ 0, %53 ], [ %134, %b32 ]
  %96 = phi i32 [ 0, %53 ], [ %128, %b32 ]
  %97 = phi i32 [ 0, %53 ], [ %133, %b32 ]
  %98 = phi i16 [ 0, %53 ], [ %131, %b32 ]
  %99 = phi i16 [ 0, %53 ], [ %118, %b32 ]
  br label %b33

b32:
  %100 = call cc 1005 addrspace(1) i16 @"_input_read@3"(ptr noundef nonnull %1, i16 noundef 1024)
  %101 = icmp sgt i16 %100, 0
  br i1 %101, label %b30, label %89

b33:
  %102 = phi i32 [ %95, %b30 ], [ %134, %b33 ]
  %103 = phi i32 [ %96, %b30 ], [ %128, %b33 ]
  %104 = phi i32 [ %97, %b30 ], [ %133, %b33 ]
  %105 = phi i16 [ %98, %b30 ], [ %131, %b33 ]
  %106 = phi i16 [ 0, %b30 ], [ %135, %b33 ]
  %107 = phi i16 [ %99, %b30 ], [ %118, %b33 ]
  %108 = getelementptr inbounds nuw i8, ptr %1, i16 %106
  %109 = load i8, ptr %108, !tbaa !7
  %110 = shl nuw nsw i16 %107, 5
  %111 = zext i8 %109 to i16
  %112 = getelementptr inbounds nuw i8, ptr @_classof, i16 %111
  %113 = load i8, ptr %112, !tbaa !7
  %114 = zext i8 %113 to i16
  %115 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %110
  %116 = getelementptr inbounds nuw i8, ptr %115, i16 %114
  %117 = load i8, ptr %116, !tbaa !7
  %118 = zext i8 %117 to i16
  %119 = getelementptr inbounds nuw i8, ptr @_hit, i16 %118
  %120 = load i8, ptr %119, !tbaa !7
  %121 = icmp eq i8 %120, 0
  %122 = shl nuw nsw i32 %103, 1
  %123 = add nsw i32 %122, %102
  %124 = zext i8 %120 to i32
  %125 = add nsw i32 %123, %124
  %126 = and i32 %125, 536870911
  %127 = select i1 %121, i16 %105, i16 1
  %128 = select i1 %121, i32 %103, i32 %126
  %129 = icmp eq i8 %109, 10
  %130 = zext nneg i16 %127 to i32
  %131 = select i1 %129, i16 0, i16 %127
  %132 = select i1 %129, i32 %130, i32 0
  %133 = add nsw i32 %132, %104
  %134 = add nsw i32 %102, 1
  %135 = add nuw nsw i16 %106, 1
  %136 = icmp slt i16 %135, %94
  br i1 %136, label %b33, label %b32
}

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
