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
  br i1 %9, label %10, label %b13

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

36:
  %37 = phi i16 [ %47, %b15 ]
  br label %b13

b13:
  %38 = phi i16 [ %37, %36 ], [ 0, %b3 ]
  %39 = icmp sgt i16 %38, 0
  %40 = icmp sgt i16 %8, 0
  %or.cond = and i1 %39, %40
  br i1 %or.cond, label %41, label %b18

41:
  br label %b19

b14:
  %42 = phi i16 [ 1, %10 ], [ %48, %b15 ]
  %43 = phi i16 [ 0, %10 ], [ %47, %b15 ]
  %44 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %42
  %45 = load i8, ptr %44, !tbaa !7
  %46 = icmp eq i8 %45, 0
  br i1 %46, label %b15, label %b16

b15:
  %47 = phi i16 [ %49, %b16 ], [ %43, %b14 ]
  %48 = add nuw nsw i16 %42, 1
  %exitcond2 = icmp ne i16 %48, %8
  br i1 %exitcond2, label %b14, label %36

b16:
  %49 = add nsw i16 %43, 1
  %50 = getelementptr inbounds i8, ptr @_queue, i16 %43
  store i8 %45, ptr %50, !tbaa !7
  br label %b15

51:
  br label %b18

b18:
  %52 = call cc 1005 addrspace(1) i16 @"_input_read@3"(ptr noundef nonnull %1, i16 noundef 1024)
  %53 = icmp sgt i16 %52, 0
  br i1 %53, label %54, label %b29

54:
  br label %b30

b19:
  %55 = phi i16 [ %38, %41 ], [ %66, %65 ]
  %56 = phi i16 [ 0, %41 ], [ %57, %65 ]
  %57 = add nuw nsw i16 %56, 1
  %58 = getelementptr inbounds nuw i8, ptr @_queue, i16 %56
  %59 = load i8, ptr %58, !tbaa !7
  %60 = zext i8 %59 to i16
  %61 = shl nuw nsw i16 %60, 5
  %62 = getelementptr i8, ptr @_nxt, i16 %61
  %63 = getelementptr inbounds nuw i8, ptr @_fail, i16 %60
  %64 = load i8, ptr %63, !tbaa !7
  br label %b22

65:
  %66 = phi i16 [ %91, %b27 ]
  %67 = icmp slt i16 %57, %66
  br i1 %67, label %b19, label %51

b22:
  %68 = phi i8 [ %64, %b19 ], [ %90, %b27 ]
  %69 = phi i16 [ 0, %b19 ], [ %92, %b27 ]
  %70 = phi i16 [ %55, %b19 ], [ %91, %b27 ]
  %71 = getelementptr i8, ptr %62, i16 %69
  %72 = load i8, ptr %71, !tbaa !7
  %73 = zext i8 %68 to i16
  %74 = shl nuw nsw i16 %73, 5
  %75 = getelementptr i8, ptr @_nxt, i16 %74
  %76 = getelementptr i8, ptr %75, i16 %69
  %77 = load i8, ptr %76, !tbaa !7
  %78 = zext i8 %77 to i16
  %79 = icmp eq i8 %72, 0
  br i1 %79, label %b23, label %b24

b23:
  store i8 %77, ptr %71, !tbaa !7
  br label %b27

b24:
  %80 = zext i8 %72 to i16
  %81 = getelementptr inbounds nuw i8, ptr @_fail, i16 %80
  store i8 %77, ptr %81, !tbaa !7
  %82 = getelementptr inbounds nuw i8, ptr @_hit, i16 %80
  %83 = load i8, ptr %82, !tbaa !7
  %84 = icmp eq i8 %83, 0
  %85 = load i8, ptr %63, !tbaa !7
  br i1 %84, label %b26, label %b25

b25:
  %86 = add nsw i16 %70, 1
  %87 = getelementptr inbounds i8, ptr @_queue, i16 %70
  store i8 %72, ptr %87, !tbaa !7
  br label %b27

b26:
  %88 = getelementptr inbounds nuw i8, ptr @_hit, i16 %78
  %89 = load i8, ptr %88, !tbaa !7
  store i8 %89, ptr %82, !tbaa !7
  br label %b25

b27:
  %90 = phi i8 [ %85, %b25 ], [ %68, %b23 ]
  %91 = phi i16 [ %86, %b25 ], [ %70, %b23 ]
  %92 = add nuw nsw i16 %69, 1
  %exitcond1 = icmp ne i16 %92, %8
  br i1 %exitcond1, label %b22, label %65

93:
  %94 = phi i32 [ %111, %b32 ]
  %95 = phi i16 [ %112, %b32 ]
  %96 = phi i32 [ %113, %b32 ]
  %97 = zext nneg i16 %95 to i32
  %98 = add nsw i32 %96, %97
  br label %b29

b29:
  %99 = phi i32 [ %94, %93 ], [ 0, %b18 ]
  %100 = phi i32 [ %98, %93 ], [ 0, %b18 ]
  store i32 %99, ptr %0, !tbaa !11
  ret i32 %100

b30:
  %101 = phi i16 [ %52, %54 ], [ %114, %b32 ]
  %102 = phi i32 [ 0, %54 ], [ %109, %b32 ]
  %103 = phi i32 [ 0, %54 ], [ %111, %b32 ]
  %104 = phi i32 [ 0, %54 ], [ %113, %b32 ]
  %105 = phi i16 [ 0, %54 ], [ %112, %b32 ]
  %106 = phi i16 [ 0, %54 ], [ %110, %b32 ]
  %107 = call i16 @llvm.smax.i16(i16 %101, i16 1)
  %108 = zext nneg i16 %107 to i32
  %109 = add i32 %102, %108
  br label %b33

b32:
  %110 = phi i16 [ %132, %b33 ]
  %111 = phi i32 [ %142, %b33 ]
  %112 = phi i16 [ %145, %b33 ]
  %113 = phi i32 [ %147, %b33 ]
  %114 = call cc 1005 addrspace(1) i16 @"_input_read@3"(ptr noundef nonnull %1, i16 noundef 1024)
  %115 = icmp sgt i16 %114, 0
  br i1 %115, label %b30, label %93

b33:
  %116 = phi i32 [ %102, %b30 ], [ %148, %b33 ]
  %117 = phi i32 [ %103, %b30 ], [ %142, %b33 ]
  %118 = phi i32 [ %104, %b30 ], [ %147, %b33 ]
  %119 = phi i16 [ %105, %b30 ], [ %145, %b33 ]
  %120 = phi i16 [ 0, %b30 ], [ %149, %b33 ]
  %121 = phi i16 [ %106, %b30 ], [ %132, %b33 ]
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
  %exitcond = icmp ne i16 %149, %107
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
