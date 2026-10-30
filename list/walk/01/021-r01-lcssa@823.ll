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

declare i16 @llvm.smax.i16(i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)

define internal cc 1005 i32 @pipeline.body(ptr noundef %0) "llrm-fp-eval"="x87-extended" {
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
  switch i8 %7, label %b5 [
    i8 0, label %b3
    i8 124, label %b6
  ]

b3:
  %8 = phi i16 [ %2, %b2 ]
  %9 = icmp sgt i16 %8, 1
  br i1 %9, label %10, label %b18

10:
  br label %b14

b5:
  %11 = zext i8 %7 to i16
  %12 = getelementptr inbounds nuw i8, ptr @_classof, i16 %11
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
  %18 = phi i8 [ %3, %b6 ], [ %33, %b10 ]
  %19 = phi i8 [ %15, %b6 ], [ %4, %b10 ]
  %20 = phi i16 [ 0, %b6 ], [ %34, %b10 ]
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
  store i8 %31, ptr %12, !tbaa !7
  br label %b8

b10:
  %32 = phi i8 [ %3, %b11 ], [ %28, %b8 ]
  %33 = phi i8 [ %35, %b11 ], [ %3, %b8 ]
  %34 = zext i8 %32 to i16
  br label %b7

b11:
  %35 = add i8 %3, 1
  store i8 %3, ptr %27, !tbaa !7
  br label %b10

b13:
  %36 = phi i16 [ %45, %b15 ]
  %37 = icmp sgt i16 %36, 0
  %38 = icmp sgt i16 %8, 0
  %or.cond = and i1 %37, %38
  br i1 %or.cond, label %39, label %b18

39:
  br label %b19

b14:
  %40 = phi i16 [ 1, %10 ], [ %46, %b15 ]
  %41 = phi i16 [ 0, %10 ], [ %45, %b15 ]
  %42 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %40
  %43 = load i8, ptr %42, !tbaa !7
  %44 = icmp eq i8 %43, 0
  br i1 %44, label %b15, label %b16

b15:
  %45 = phi i16 [ %47, %b16 ], [ %41, %b14 ]
  %46 = add nuw nsw i16 %40, 1
  %exitcond2 = icmp eq i16 %46, %8
  br i1 %exitcond2, label %b13, label %b14

b16:
  %47 = add nsw i16 %41, 1
  %48 = getelementptr inbounds i8, ptr @_queue, i16 %41
  store i8 %43, ptr %48, !tbaa !7
  br label %b15

49:
  br label %b18

b18:
  %50 = call cc 1005 addrspace(1) i16 @"_input_read@3"(ptr noundef nonnull %1, i16 noundef 1024)
  %51 = icmp sgt i16 %50, 0
  br i1 %51, label %52, label %b29

52:
  br label %b30

b19:
  %53 = phi i16 [ %36, %39 ], [ %64, %63 ]
  %54 = phi i16 [ 0, %39 ], [ %55, %63 ]
  %55 = add nuw nsw i16 %54, 1
  %56 = getelementptr inbounds nuw i8, ptr @_queue, i16 %54
  %57 = load i8, ptr %56, !tbaa !7
  %58 = zext i8 %57 to i16
  %59 = shl nuw nsw i16 %58, 5
  %60 = getelementptr i8, ptr @_nxt, i16 %59
  %61 = getelementptr inbounds nuw i8, ptr @_fail, i16 %58
  %62 = load i8, ptr %61, !tbaa !7
  br label %b22

63:
  %64 = phi i16 [ %89, %b27 ]
  %65 = icmp slt i16 %55, %64
  br i1 %65, label %b19, label %49

b22:
  %66 = phi i8 [ %62, %b19 ], [ %88, %b27 ]
  %67 = phi i16 [ 0, %b19 ], [ %90, %b27 ]
  %68 = phi i16 [ %53, %b19 ], [ %89, %b27 ]
  %69 = getelementptr i8, ptr %60, i16 %67
  %70 = load i8, ptr %69, !tbaa !7
  %71 = zext i8 %66 to i16
  %72 = shl nuw nsw i16 %71, 5
  %73 = getelementptr i8, ptr @_nxt, i16 %72
  %74 = getelementptr i8, ptr %73, i16 %67
  %75 = load i8, ptr %74, !tbaa !7
  %76 = zext i8 %75 to i16
  %77 = icmp eq i8 %70, 0
  br i1 %77, label %b23, label %b24

b23:
  store i8 %75, ptr %69, !tbaa !7
  br label %b27

b24:
  %78 = zext i8 %70 to i16
  %79 = getelementptr inbounds nuw i8, ptr @_fail, i16 %78
  store i8 %75, ptr %79, !tbaa !7
  %80 = getelementptr inbounds nuw i8, ptr @_hit, i16 %78
  %81 = load i8, ptr %80, !tbaa !7
  %82 = icmp eq i8 %81, 0
  %83 = load i8, ptr %61, !tbaa !7
  br i1 %82, label %b26, label %b25

b25:
  %84 = add nuw nsw i16 %68, 1
  %85 = getelementptr inbounds nuw i8, ptr @_queue, i16 %68
  store i8 %70, ptr %85, !tbaa !7
  br label %b27

b26:
  %86 = getelementptr inbounds nuw i8, ptr @_hit, i16 %76
  %87 = load i8, ptr %86, !tbaa !7
  store i8 %87, ptr %80, !tbaa !7
  br label %b25

b27:
  %88 = phi i8 [ %83, %b25 ], [ %66, %b23 ]
  %89 = phi i16 [ %84, %b25 ], [ %68, %b23 ]
  %90 = add nuw nsw i16 %67, 1
  %exitcond1 = icmp eq i16 %90, %8
  br i1 %exitcond1, label %63, label %b22

91:
  %92 = phi i32 [ %108, %b32 ]
  %93 = phi i16 [ %109, %b32 ]
  %94 = phi i32 [ %110, %b32 ]
  %95 = zext nneg i16 %93 to i32
  %96 = add nsw i32 %94, %95
  br label %b29

b29:
  %97 = phi i32 [ %92, %91 ], [ 0, %b18 ]
  %98 = phi i32 [ %96, %91 ], [ 0, %b18 ]
  store i32 %97, ptr %0, !tbaa !11
  ret i32 %98

b30:
  %99 = phi i16 [ %50, %52 ], [ %111, %b32 ]
  %100 = phi i32 [ 0, %52 ], [ %106, %b32 ]
  %101 = phi i32 [ 0, %52 ], [ %108, %b32 ]
  %102 = phi i32 [ 0, %52 ], [ %110, %b32 ]
  %103 = phi i16 [ 0, %52 ], [ %109, %b32 ]
  %104 = phi i16 [ 0, %52 ], [ %107, %b32 ]
  %105 = zext nneg i16 %99 to i32
  %106 = add i32 %100, %105
  br label %b33

b32:
  %107 = phi i16 [ %129, %b33 ]
  %108 = phi i32 [ %139, %b33 ]
  %109 = phi i16 [ %142, %b33 ]
  %110 = phi i32 [ %144, %b33 ]
  %111 = call cc 1005 addrspace(1) i16 @"_input_read@3"(ptr noundef nonnull %1, i16 noundef 1024)
  %112 = icmp sgt i16 %111, 0
  br i1 %112, label %b30, label %91

b33:
  %113 = phi i32 [ %100, %b30 ], [ %145, %b33 ]
  %114 = phi i32 [ %101, %b30 ], [ %139, %b33 ]
  %115 = phi i32 [ %102, %b30 ], [ %144, %b33 ]
  %116 = phi i16 [ %103, %b30 ], [ %142, %b33 ]
  %117 = phi i16 [ 0, %b30 ], [ %146, %b33 ]
  %118 = phi i16 [ %104, %b30 ], [ %129, %b33 ]
  %119 = getelementptr inbounds nuw i8, ptr %1, i16 %117
  %120 = load i8, ptr %119, !tbaa !7
  %121 = shl nuw nsw i16 %118, 5
  %122 = zext i8 %120 to i16
  %123 = getelementptr inbounds nuw i8, ptr @_classof, i16 %122
  %124 = load i8, ptr %123, !tbaa !7
  %125 = zext i8 %124 to i16
  %126 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %121
  %127 = getelementptr inbounds nuw i8, ptr %126, i16 %125
  %128 = load i8, ptr %127, !tbaa !7
  %129 = zext i8 %128 to i16
  %130 = getelementptr inbounds nuw i8, ptr @_hit, i16 %129
  %131 = load i8, ptr %130, !tbaa !7
  %132 = icmp eq i8 %131, 0
  %133 = shl nuw nsw i32 %114, 1
  %134 = add nsw i32 %133, %113
  %135 = zext i8 %131 to i32
  %136 = add nsw i32 %134, %135
  %137 = and i32 %136, 536870911
  %138 = select i1 %132, i16 %116, i16 1
  %139 = select i1 %132, i32 %114, i32 %137
  %140 = icmp eq i8 %120, 10
  %141 = zext nneg i16 %138 to i32
  %142 = select i1 %140, i16 0, i16 %138
  %143 = select i1 %140, i32 %141, i32 0
  %144 = add nsw i32 %143, %115
  %145 = add nsw i32 %113, 1
  %146 = add nuw nsw i16 %117, 1
  %exitcond = icmp eq i16 %146, %99
  br i1 %exitcond, label %b32, label %b33
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
