target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@_patterns = internal constant [81 x i8] c"Oliver|Scrooge|Copperfield|Dombey|Marley|Micawber|Heep|ghost|gentleman|Tiny Tim|\00"
@_queue = internal global [80 x i8] zeroinitializer
@_nxt = internal global [2560 x i8] zeroinitializer
@_fail = internal global [80 x i8] zeroinitializer
@_classof = internal global [256 x i8] zeroinitializer
@_hit = internal global [80 x i8] zeroinitializer

define cc 1005 i32 @"_bench_grep@3"(ptr noundef nocapture writeonly %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca [1024 x i8]
  br label %b2

b2:
  %2 = phi i16 [ 1, %b1 ], [ %17, %b7 ]
  %3 = phi i8 [ 1, %b1 ], [ %18, %b7 ]
  %4 = phi i8 [ 1, %b1 ], [ %19, %b7 ]
  %5 = phi i16 [ 0, %b1 ], [ %20, %b7 ]
  %6 = phi ptr [ @_patterns, %b1 ], [ %21, %b7 ]
  %7 = load i8, ptr %6, !tbaa !7
  %8 = zext i8 %7 to i16
  switch i8 %7, label %b5 [
    i8 0, label %b3
    i8 124, label %b6
  ]

b3:
  %9 = icmp sgt i16 %2, 1
  br i1 %9, label %10, label %b18

10:
  %11 = add i16 %2, -1
  br label %b14

b5:
  %12 = getelementptr inbounds nuw i8, ptr @_classof, i16 %8
  %13 = load i8, ptr %12, !tbaa !7
  %14 = icmp eq i8 %13, 0
  br i1 %14, label %b9, label %b8

b6:
  %15 = add i8 %4, 1
  %16 = getelementptr inbounds nuw i8, ptr @_hit, i16 %5
  store i8 %4, ptr %16, !tbaa !7
  br label %b7

b7:
  %17 = phi i16 [ %2, %b6 ], [ %23, %b10 ]
  %18 = phi i8 [ %3, %b6 ], [ %34, %b10 ]
  %19 = phi i8 [ %15, %b6 ], [ %4, %b10 ]
  %20 = phi i16 [ 0, %b6 ], [ %35, %b10 ]
  %21 = getelementptr inbounds nuw i8, ptr %6, i16 1
  br label %b2

b8:
  %22 = phi i8 [ %31, %b9 ], [ %13, %b5 ]
  %23 = phi i16 [ %30, %b9 ], [ %2, %b5 ]
  %24 = shl nuw nsw i16 %5, 5
  %25 = zext i8 %22 to i16
  %26 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %24
  %27 = getelementptr inbounds nuw i8, ptr %26, i16 %25
  %28 = load i8, ptr %27, !tbaa !7
  %29 = icmp eq i8 %28, 0
  br i1 %29, label %b11, label %b10

b9:
  %30 = add nsw i16 %2, 1
  %31 = trunc i16 %2 to i8
  %32 = getelementptr i8, ptr @_classof, i16 %8
  store i8 %31, ptr %32, !tbaa !7
  br label %b8

b10:
  %33 = phi i8 [ %3, %b11 ], [ %28, %b8 ]
  %34 = phi i8 [ %36, %b11 ], [ %3, %b8 ]
  %35 = zext i8 %33 to i16
  br label %b7

b11:
  %36 = add i8 %3, 1
  %37 = getelementptr i8, ptr @_nxt, i16 %24
  %38 = getelementptr i8, ptr %37, i16 %25
  store i8 %3, ptr %38, !tbaa !7
  br label %b10

b13:
  %39 = icmp sgt i16 %47, 0
  br i1 %39, label %40, label %b18

40:
  br label %b19

b14:
  %41 = phi i16 [ 0, %10 ], [ %48, %b15 ]
  %42 = phi i16 [ %47, %b15 ], [ 0, %10 ]
  %43 = getelementptr i8, ptr @_nxt, i16 %41
  %44 = getelementptr i8, ptr %43, i16 1
  %45 = load i8, ptr %44, !tbaa !7
  %46 = icmp eq i8 %45, 0
  br i1 %46, label %b15, label %b16

b15:
  %47 = phi i16 [ %49, %b16 ], [ %42, %b14 ]
  %48 = add nuw nsw i16 %41, 1
  %exitcond2 = icmp eq i16 %11, %48
  br i1 %exitcond2, label %b13, label %b14

b16:
  %49 = add nsw i16 %42, 1
  %50 = getelementptr inbounds i8, ptr @_queue, i16 %42
  store i8 %45, ptr %50, !tbaa !7
  br label %b15

b18:
  %51 = call cc 1005 addrspace(1) i16 @"_input_read@3"(ptr noundef nonnull %1, i16 noundef 1024)
  %52 = icmp sgt i16 %51, 0
  br i1 %52, label %53, label %b29

53:
  br label %b30

b19:
  %54 = phi i16 [ %93, %63 ], [ %47, %40 ]
  %55 = phi i16 [ %56, %63 ], [ 0, %40 ]
  %56 = add nuw nsw i16 %55, 1
  %57 = getelementptr inbounds nuw i8, ptr @_queue, i16 %55
  %58 = load i8, ptr %57, !tbaa !7
  %59 = zext i8 %58 to i16
  %60 = shl nuw nsw i16 %59, 5
  %61 = getelementptr inbounds nuw i8, ptr @_fail, i16 %59
  %62 = load i8, ptr %61, !tbaa !7
  br label %b22

63:
  %64 = icmp slt i16 %56, %93
  br i1 %64, label %b19, label %b18

b22:
  %65 = phi i8 [ %62, %b19 ], [ %92, %b27 ]
  %66 = phi i16 [ 0, %b19 ], [ %94, %b27 ]
  %67 = phi i16 [ %54, %b19 ], [ %93, %b27 ]
  %68 = getelementptr i8, ptr @_nxt, i16 %60
  %69 = getelementptr i8, ptr %68, i16 %66
  %70 = load i8, ptr %69, !tbaa !7
  %71 = zext i8 %70 to i16
  %72 = zext i8 %65 to i16
  %73 = shl nuw nsw i16 %72, 5
  %74 = add i16 %66, %73
  %75 = getelementptr i8, ptr @_nxt, i16 %74
  %76 = load i8, ptr %75, !tbaa !7
  %77 = zext i8 %76 to i16
  %78 = icmp eq i8 %70, 0
  br i1 %78, label %b23, label %b24

b23:
  %79 = getelementptr i8, ptr @_nxt, i16 %60
  %80 = getelementptr i8, ptr %79, i16 %66
  store i8 %76, ptr %80, !tbaa !7
  br label %b27

b24:
  %81 = getelementptr inbounds nuw i8, ptr @_fail, i16 %71
  store i8 %76, ptr %81, !tbaa !7
  %82 = getelementptr inbounds nuw i8, ptr @_hit, i16 %71
  %83 = load i8, ptr %82, !tbaa !7
  %84 = icmp eq i8 %83, 0
  %85 = getelementptr i8, ptr @_fail, i16 %59
  %86 = load i8, ptr %85, !tbaa !7
  br i1 %84, label %b26, label %b25

b25:
  %87 = add nuw nsw i16 %67, 1
  %88 = getelementptr inbounds nuw i8, ptr @_queue, i16 %67
  store i8 %70, ptr %88, !tbaa !7
  br label %b27

b26:
  %89 = getelementptr inbounds nuw i8, ptr @_hit, i16 %77
  %90 = load i8, ptr %89, !tbaa !7
  %91 = getelementptr i8, ptr @_hit, i16 %71
  store i8 %90, ptr %91, !tbaa !7
  br label %b25

b27:
  %92 = phi i8 [ %86, %b25 ], [ %65, %b23 ]
  %93 = phi i16 [ %87, %b25 ], [ %67, %b23 ]
  %94 = add nuw nsw i16 %66, 1
  %exitcond1 = icmp eq i16 %2, %94
  br i1 %exitcond1, label %63, label %b22

95:
  %96 = zext nneg i16 %139 to i32
  %97 = add nsw i32 %141, %96
  br label %b29

b29:
  %98 = phi i32 [ %136, %95 ], [ 0, %b18 ]
  %99 = phi i32 [ %97, %95 ], [ 0, %b18 ]
  store i32 %98, ptr %0, !tbaa !11
  ret i32 %99

b30:
  %100 = phi i16 [ %108, %b32 ], [ %51, %53 ]
  %101 = phi i32 [ %107, %b32 ], [ 0, %53 ]
  %102 = phi i32 [ %136, %b32 ], [ 0, %53 ]
  %103 = phi i32 [ %141, %b32 ], [ 0, %53 ]
  %104 = phi i16 [ %139, %b32 ], [ 0, %53 ]
  %105 = phi i16 [ %126, %b32 ], [ 0, %53 ]
  %106 = zext nneg i16 %100 to i32
  %107 = add i32 %101, %106
  br label %b33

b32:
  %108 = call cc 1005 addrspace(1) i16 @"_input_read@3"(ptr noundef nonnull %1, i16 noundef 1024)
  %109 = icmp sgt i16 %108, 0
  br i1 %109, label %b30, label %95

b33:
  %110 = phi i32 [ %101, %b30 ], [ %142, %b33 ]
  %111 = phi i32 [ %102, %b30 ], [ %136, %b33 ]
  %112 = phi i32 [ %103, %b30 ], [ %141, %b33 ]
  %113 = phi i16 [ %104, %b30 ], [ %139, %b33 ]
  %114 = phi i16 [ 0, %b30 ], [ %143, %b33 ]
  %115 = phi i16 [ %105, %b30 ], [ %126, %b33 ]
  %116 = getelementptr i8, ptr %1, i16 %114
  %117 = load i8, ptr %116, !tbaa !7
  %118 = zext i8 %117 to i16
  %119 = shl nuw nsw i16 %115, 5
  %120 = getelementptr inbounds nuw i8, ptr @_classof, i16 %118
  %121 = load i8, ptr %120, !tbaa !7
  %122 = zext i8 %121 to i16
  %123 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %119
  %124 = getelementptr inbounds nuw i8, ptr %123, i16 %122
  %125 = load i8, ptr %124, !tbaa !7
  %126 = zext i8 %125 to i16
  %127 = getelementptr inbounds nuw i8, ptr @_hit, i16 %126
  %128 = load i8, ptr %127, !tbaa !7
  %129 = zext i8 %128 to i32
  %130 = icmp eq i32 %129, 0
  %131 = shl i32 %111, 1
  %132 = add i32 %131, %129
  %133 = add i32 %110, %132
  %134 = and i32 %133, 536870911
  %135 = select i1 %130, i16 %113, i16 1
  %136 = select i1 %130, i32 %111, i32 %134
  %137 = icmp eq i16 %118, 10
  %138 = zext nneg i16 %135 to i32
  %139 = select i1 %137, i16 0, i16 %135
  %140 = select i1 %137, i32 %138, i32 0
  %141 = add nsw i32 %140, %112
  %142 = add nsw i32 %110, 1
  %143 = add nuw nsw i16 %114, 1
  %exitcond = icmp eq i16 %100, %143
  br i1 %exitcond, label %b32, label %b33
}

define i16 @_main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i32
  %1 = call cc 1005 addrspace(1) i32 @"_bench_grep@3"(ptr noundef nonnull %0)
  %2 = tail call cc 1005 addrspace(1) i16 @"_report@3"(i32 noundef %1)
  %3 = load i32, ptr %0, !tbaa !11
  %4 = tail call cc 1005 addrspace(1) i16 @"_report@3"(i32 noundef %3)
  ret i16 0
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare cc 1005 i16 @"_input_read@3"(ptr, i16) addrspace(1)

declare cc 1005 i16 @"_report@3"(i32) addrspace(1)

declare i16 @llvm.smax.i16(i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)

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
