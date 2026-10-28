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
  %2 = phi i16 [ 1, %b1 ], [ %19, %b7 ]
  %3 = phi i8 [ 1, %b1 ], [ %20, %b7 ]
  %4 = phi i8 [ 1, %b1 ], [ %21, %b7 ]
  %5 = phi i16 [ 0, %b1 ], [ %22, %b7 ]
  %6 = phi ptr [ @_patterns, %b1 ], [ %23, %b7 ]
  %7 = load i8, ptr %6, !tbaa !7
  switch i8 %7, label %b5 [
    i8 0, label %b3
    i8 124, label %b6
  ]

b3:
  %8 = phi i16 [ %2, %b2 ]
  %9 = phi i16 [ %2, %b2 ]
  %10 = phi i16 [ %2, %b2 ]
  %11 = icmp sgt i16 %10, 1
  br i1 %11, label %12, label %b13

12:
  br label %b14

b5:
  %13 = zext i8 %7 to i16
  %14 = getelementptr inbounds nuw i8, ptr @_classof, i16 %13
  %15 = load i8, ptr %14, !tbaa !7
  %16 = icmp eq i8 %15, 0
  br i1 %16, label %b9, label %b8

b6:
  %17 = add i8 %4, 1
  %18 = getelementptr inbounds nuw i8, ptr @_hit, i16 %5
  store i8 %4, ptr %18, !tbaa !7
  br label %b7

b7:
  %19 = phi i16 [ %2, %b6 ], [ %24, %b10 ]
  %20 = phi i8 [ %3, %b6 ], [ %34, %b10 ]
  %21 = phi i8 [ %17, %b6 ], [ %4, %b10 ]
  %22 = phi i16 [ 0, %b6 ], [ %36, %b10 ]
  %23 = getelementptr inbounds nuw i8, ptr %6, i16 1
  br label %b2

b8:
  %24 = phi i16 [ %32, %b9 ], [ %2, %b5 ]
  %25 = shl nuw nsw i16 %5, 5
  %26 = load i8, ptr %14, !tbaa !7
  %27 = zext i8 %26 to i16
  %28 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %25
  %29 = getelementptr inbounds nuw i8, ptr %28, i16 %27
  %30 = load i8, ptr %29, !tbaa !7
  %31 = icmp eq i8 %30, 0
  br i1 %31, label %b11, label %b10

b9:
  %32 = add nsw i16 %2, 1
  %33 = trunc i16 %2 to i8
  store i8 %33, ptr %14, !tbaa !7
  br label %b8

b10:
  %34 = phi i8 [ %37, %b11 ], [ %3, %b8 ]
  %35 = load i8, ptr %29, !tbaa !7
  %36 = zext i8 %35 to i16
  br label %b7

b11:
  %37 = add i8 %3, 1
  store i8 %3, ptr %29, !tbaa !7
  br label %b10

38:
  %39 = phi i16 [ %49, %b15 ]
  br label %b13

b13:
  %40 = phi i16 [ %39, %38 ], [ 0, %b3 ]
  %41 = icmp sgt i16 %40, 0
  %42 = icmp sgt i16 %10, 0
  %or.cond = and i1 %41, %42
  br i1 %or.cond, label %43, label %b18

43:
  br label %b19

b14:
  %44 = phi i16 [ 1, %12 ], [ %50, %b15 ]
  %45 = phi i16 [ 0, %12 ], [ %49, %b15 ]
  %46 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %44
  %47 = load i8, ptr %46, !tbaa !7
  %48 = icmp eq i8 %47, 0
  br i1 %48, label %b15, label %b16

b15:
  %49 = phi i16 [ %51, %b16 ], [ %45, %b14 ]
  %50 = add nuw nsw i16 %44, 1
  %exitcond2 = icmp ne i16 %50, %8
  br i1 %exitcond2, label %b14, label %38

b16:
  %51 = add nsw i16 %45, 1
  %52 = getelementptr inbounds i8, ptr @_queue, i16 %45
  store i8 %47, ptr %52, !tbaa !7
  br label %b15

53:
  br label %b18

b18:
  %54 = call cc 1005 addrspace(1) i16 @"_input_read@3"(ptr noundef nonnull %1, i16 noundef 1024)
  %55 = icmp sgt i16 %54, 0
  br i1 %55, label %56, label %b29

56:
  br label %b30

b19:
  %57 = phi i16 [ %40, %43 ], [ %67, %66 ]
  %58 = phi i16 [ 0, %43 ], [ %59, %66 ]
  %59 = add nuw nsw i16 %58, 1
  %60 = getelementptr inbounds nuw i8, ptr @_queue, i16 %58
  %61 = load i8, ptr %60, !tbaa !7
  %62 = zext i8 %61 to i16
  %63 = shl nuw nsw i16 %62, 5
  %64 = getelementptr i8, ptr @_nxt, i16 %63
  %65 = getelementptr inbounds nuw i8, ptr @_fail, i16 %62
  br label %b22

66:
  %67 = phi i16 [ %90, %b27 ]
  %68 = icmp slt i16 %59, %67
  br i1 %68, label %b19, label %53

b22:
  %69 = phi i16 [ 0, %b19 ], [ %91, %b27 ]
  %70 = phi i16 [ %57, %b19 ], [ %90, %b27 ]
  %71 = getelementptr i8, ptr %64, i16 %69
  %72 = load i8, ptr %71, !tbaa !7
  %73 = load i8, ptr %65, !tbaa !7
  %74 = zext i8 %73 to i16
  %75 = shl nuw nsw i16 %74, 5
  %76 = getelementptr i8, ptr @_nxt, i16 %75
  %77 = getelementptr i8, ptr %76, i16 %69
  %78 = load i8, ptr %77, !tbaa !7
  %79 = zext i8 %78 to i16
  %80 = icmp eq i8 %72, 0
  br i1 %80, label %b23, label %b24

b23:
  store i8 %78, ptr %71, !tbaa !7
  br label %b27

b24:
  %81 = zext i8 %72 to i16
  %82 = getelementptr inbounds nuw i8, ptr @_fail, i16 %81
  store i8 %78, ptr %82, !tbaa !7
  %83 = getelementptr inbounds nuw i8, ptr @_hit, i16 %81
  %84 = load i8, ptr %83, !tbaa !7
  %85 = icmp eq i8 %84, 0
  br i1 %85, label %b26, label %b25

b25:
  %86 = add nsw i16 %70, 1
  %87 = getelementptr inbounds i8, ptr @_queue, i16 %70
  store i8 %72, ptr %87, !tbaa !7
  br label %b27

b26:
  %88 = getelementptr inbounds nuw i8, ptr @_hit, i16 %79
  %89 = load i8, ptr %88, !tbaa !7
  store i8 %89, ptr %83, !tbaa !7
  br label %b25

b27:
  %90 = phi i16 [ %86, %b25 ], [ %70, %b23 ]
  %91 = add nuw nsw i16 %69, 1
  %exitcond1 = icmp ne i16 %91, %9
  br i1 %exitcond1, label %b22, label %66

92:
  %93 = phi i32 [ %111, %b32 ]
  %94 = phi i16 [ %112, %b32 ]
  %95 = phi i32 [ %113, %b32 ]
  %96 = zext nneg i16 %94 to i32
  %97 = add nsw i32 %95, %96
  br label %b29

b29:
  %98 = phi i32 [ %93, %92 ], [ 0, %b18 ]
  %99 = phi i32 [ %97, %92 ], [ 0, %b18 ]
  store i32 %98, ptr %0, !tbaa !11
  ret i32 %99

b30:
  %100 = phi i16 [ %54, %56 ], [ %114, %b32 ]
  %101 = phi i32 [ 0, %56 ], [ %108, %b32 ]
  %102 = phi i32 [ 0, %56 ], [ %111, %b32 ]
  %103 = phi i32 [ 0, %56 ], [ %113, %b32 ]
  %104 = phi i16 [ 0, %56 ], [ %112, %b32 ]
  %105 = phi i16 [ 0, %56 ], [ %110, %b32 ]
  %106 = call i16 @llvm.smax.i16(i16 %100, i16 1)
  %107 = zext nneg i16 %106 to i32
  %108 = add i32 %101, %107
  %109 = call i16 @llvm.smax.i16(i16 %100, i16 1)
  br label %b33

b32:
  %110 = phi i16 [ %132, %b33 ]
  %111 = phi i32 [ %142, %b33 ]
  %112 = phi i16 [ %145, %b33 ]
  %113 = phi i32 [ %147, %b33 ]
  %114 = call cc 1005 addrspace(1) i16 @"_input_read@3"(ptr noundef nonnull %1, i16 noundef 1024)
  %115 = icmp sgt i16 %114, 0
  br i1 %115, label %b30, label %92

b33:
  %116 = phi i32 [ %101, %b30 ], [ %148, %b33 ]
  %117 = phi i32 [ %102, %b30 ], [ %142, %b33 ]
  %118 = phi i32 [ %103, %b30 ], [ %147, %b33 ]
  %119 = phi i16 [ %104, %b30 ], [ %145, %b33 ]
  %120 = phi i16 [ 0, %b30 ], [ %149, %b33 ]
  %121 = phi i16 [ %105, %b30 ], [ %132, %b33 ]
  %122 = getelementptr inbounds nuw i8, ptr %1, i16 %120
  %123 = load i8, ptr %122, !tbaa !7
  %124 = shl nuw nsw i16 %121, 5
  %125 = zext i8 %123 to i16
  %126 = getelementptr inbounds nuw i8, ptr @_classof, i16 %125
  %127 = load i8, ptr %126, !tbaa !7
  %128 = zext i8 %127 to i16
  %129 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %124
  %130 = getelementptr inbounds nuw i8, ptr %129, i16 %128
  %131 = load i8, ptr %130, !tbaa !7
  %132 = zext i8 %131 to i16
  %133 = getelementptr inbounds nuw i8, ptr @_hit, i16 %132
  %134 = load i8, ptr %133, !tbaa !7
  %135 = icmp eq i8 %134, 0
  %136 = shl nuw nsw i32 %117, 1
  %137 = add nsw i32 %136, %116
  %138 = zext i8 %134 to i32
  %139 = add nsw i32 %137, %138
  %140 = and i32 %139, 536870911
  %141 = select i1 %135, i16 %119, i16 1
  %142 = select i1 %135, i32 %117, i32 %140
  %143 = icmp eq i8 %123, 10
  %144 = zext nneg i16 %141 to i32
  %145 = select i1 %143, i16 0, i16 %141
  %146 = select i1 %143, i32 %144, i32 0
  %147 = add nsw i32 %146, %118
  %148 = add nsw i32 %116, 1
  %149 = add nuw nsw i16 %120, 1
  %exitcond = icmp ne i16 %149, %109
  br i1 %exitcond, label %b33, label %b32
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
