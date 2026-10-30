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
  %2 = phi i16 [ 1, %b1 ], [ %15, %b7 ]
  %3 = phi i8 [ 1, %b1 ], [ %16, %b7 ]
  %4 = phi i8 [ 1, %b1 ], [ %17, %b7 ]
  %5 = phi i16 [ 0, %b1 ], [ %18, %b7 ]
  %6 = phi ptr [ @_patterns, %b1 ], [ %19, %b7 ]
  %7 = load i8, ptr %6, !tbaa !7
  switch i8 %7, label %b5 [
    i8 0, label %b3
    i8 124, label %b6
  ]

b3:
  %8 = phi i16 [ %2, %b2 ]
  br label %b12

b5:
  %9 = zext i8 %7 to i16
  %10 = getelementptr inbounds nuw i8, ptr @_classof, i16 %9
  %11 = load i8, ptr %10, !tbaa !7
  %12 = icmp eq i8 %11, 0
  br i1 %12, label %b9, label %b8

b6:
  %13 = add i8 %4, 1
  %14 = getelementptr inbounds nuw i8, ptr @_hit, i16 %5
  store i8 %4, ptr %14, !tbaa !7
  br label %b7

b7:
  %15 = phi i16 [ %2, %b6 ], [ %20, %b10 ]
  %16 = phi i8 [ %3, %b6 ], [ %30, %b10 ]
  %17 = phi i8 [ %13, %b6 ], [ %4, %b10 ]
  %18 = phi i16 [ 0, %b6 ], [ %32, %b10 ]
  %19 = getelementptr inbounds nuw i8, ptr %6, i16 1
  br label %b2

b8:
  %20 = phi i16 [ %28, %b9 ], [ %2, %b5 ]
  %21 = shl nuw nsw i16 %5, 5
  %22 = load i8, ptr %10, !tbaa !7
  %23 = zext i8 %22 to i16
  %24 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %21
  %25 = getelementptr inbounds nuw i8, ptr %24, i16 %23
  %26 = load i8, ptr %25, !tbaa !7
  %27 = icmp eq i8 %26, 0
  br i1 %27, label %b11, label %b10

b9:
  %28 = add nsw i16 %2, 1
  %29 = trunc i16 %2 to i8
  store i8 %29, ptr %10, !tbaa !7
  br label %b8

b10:
  %30 = phi i8 [ %33, %b11 ], [ %3, %b8 ]
  %31 = load i8, ptr %25, !tbaa !7
  %32 = zext i8 %31 to i16
  br label %b7

b11:
  %33 = add i8 %3, 1
  store i8 %3, ptr %25, !tbaa !7
  br label %b10

b12:
  %34 = phi i16 [ 0, %b3 ], [ %41, %b15 ]
  %35 = phi i16 [ 1, %b3 ], [ %42, %b15 ]
  %36 = icmp slt i16 %35, %8
  br i1 %36, label %b14, label %b13

b13:
  %37 = phi i16 [ %34, %b12 ]
  br label %b17

b14:
  %38 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %35
  %39 = load i8, ptr %38, !tbaa !7
  %40 = icmp eq i8 %39, 0
  br i1 %40, label %b15, label %b16

b15:
  %41 = phi i16 [ %43, %b16 ], [ %34, %b14 ]
  %42 = add nuw nsw i16 %35, 1
  br label %b12

b16:
  %43 = add nsw i16 %34, 1
  %44 = getelementptr inbounds i8, ptr @_queue, i16 %34
  store i8 %39, ptr %44, !tbaa !7
  br label %b15

b17:
  %45 = phi i16 [ 0, %b13 ], [ %48, %b21 ]
  %46 = phi i16 [ %37, %b13 ], [ %55, %b21 ]
  %47 = icmp slt i16 %45, %46
  br i1 %47, label %b19, label %b18

b18:
  br label %b28

b19:
  %48 = add nuw nsw i16 %45, 1
  %49 = getelementptr inbounds nuw i8, ptr @_queue, i16 %45
  %50 = load i8, ptr %49, !tbaa !7
  %51 = zext i8 %50 to i16
  br label %b20

b20:
  %52 = phi i16 [ %46, %b19 ], [ %78, %b27 ]
  %53 = phi i16 [ 0, %b19 ], [ %79, %b27 ]
  %54 = icmp slt i16 %53, %8
  br i1 %54, label %b22, label %b21

b21:
  %55 = phi i16 [ %52, %b20 ]
  br label %b17

b22:
  %56 = shl nuw nsw i16 %51, 5
  %57 = getelementptr i8, ptr @_nxt, i16 %56
  %58 = getelementptr i8, ptr %57, i16 %53
  %59 = load i8, ptr %58, !tbaa !7
  %60 = getelementptr inbounds nuw i8, ptr @_fail, i16 %51
  %61 = load i8, ptr %60, !tbaa !7
  %62 = zext i8 %61 to i16
  %63 = shl nuw nsw i16 %62, 5
  %64 = getelementptr i8, ptr @_nxt, i16 %63
  %65 = getelementptr i8, ptr %64, i16 %53
  %66 = load i8, ptr %65, !tbaa !7
  %67 = zext i8 %66 to i16
  %68 = icmp eq i8 %59, 0
  br i1 %68, label %b23, label %b24

b23:
  store i8 %66, ptr %58, !tbaa !7
  br label %b27

b24:
  %69 = zext i8 %59 to i16
  %70 = getelementptr inbounds nuw i8, ptr @_fail, i16 %69
  store i8 %66, ptr %70, !tbaa !7
  %71 = getelementptr inbounds nuw i8, ptr @_hit, i16 %69
  %72 = load i8, ptr %71, !tbaa !7
  %73 = icmp eq i8 %72, 0
  br i1 %73, label %b26, label %b25

b25:
  %74 = add nsw i16 %52, 1
  %75 = getelementptr inbounds i8, ptr @_queue, i16 %52
  store i8 %59, ptr %75, !tbaa !7
  br label %b27

b26:
  %76 = getelementptr inbounds nuw i8, ptr @_hit, i16 %67
  %77 = load i8, ptr %76, !tbaa !7
  store i8 %77, ptr %71, !tbaa !7
  br label %b25

b27:
  %78 = phi i16 [ %74, %b25 ], [ %52, %b23 ]
  %79 = add nuw nsw i16 %53, 1
  br label %b20

b28:
  %80 = phi i16 [ 0, %b18 ], [ %103, %b32 ]
  %81 = phi i16 [ 0, %b18 ], [ %102, %b32 ]
  %82 = phi i32 [ 0, %b18 ], [ %101, %b32 ]
  %83 = phi i32 [ 0, %b18 ], [ %100, %b32 ]
  %84 = phi i32 [ 0, %b18 ], [ %99, %b32 ]
  %85 = call cc 1005 addrspace(1) i16 @"_input_read@3"(ptr noundef nonnull %1, i16 noundef 1024)
  %86 = icmp sgt i16 %85, 0
  br i1 %86, label %b30, label %b29

b29:
  %87 = phi i32 [ %83, %b28 ]
  %88 = phi i32 [ %82, %b28 ]
  %89 = phi i16 [ %81, %b28 ]
  store i32 %87, ptr %0, !tbaa !11
  %90 = zext nneg i16 %89 to i32
  %91 = add nsw i32 %88, %90
  ret i32 %91

b30:
  br label %b31

b31:
  %92 = phi i16 [ %80, %b30 ], [ %114, %b33 ]
  %93 = phi i16 [ 0, %b30 ], [ %131, %b33 ]
  %94 = phi i16 [ %81, %b30 ], [ %127, %b33 ]
  %95 = phi i32 [ %82, %b30 ], [ %129, %b33 ]
  %96 = phi i32 [ %83, %b30 ], [ %124, %b33 ]
  %97 = phi i32 [ %84, %b30 ], [ %130, %b33 ]
  %98 = icmp slt i16 %93, %85
  br i1 %98, label %b33, label %b32

b32:
  %99 = phi i32 [ %97, %b31 ]
  %100 = phi i32 [ %96, %b31 ]
  %101 = phi i32 [ %95, %b31 ]
  %102 = phi i16 [ %94, %b31 ]
  %103 = phi i16 [ %92, %b31 ]
  br label %b28

b33:
  %104 = getelementptr inbounds nuw i8, ptr %1, i16 %93
  %105 = load i8, ptr %104, !tbaa !7
  %106 = shl nuw nsw i16 %92, 5
  %107 = zext i8 %105 to i16
  %108 = getelementptr inbounds nuw i8, ptr @_classof, i16 %107
  %109 = load i8, ptr %108, !tbaa !7
  %110 = zext i8 %109 to i16
  %111 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %106
  %112 = getelementptr inbounds nuw i8, ptr %111, i16 %110
  %113 = load i8, ptr %112, !tbaa !7
  %114 = zext i8 %113 to i16
  %115 = getelementptr inbounds nuw i8, ptr @_hit, i16 %114
  %116 = load i8, ptr %115, !tbaa !7
  %117 = icmp eq i8 %116, 0
  %118 = mul nuw nsw i32 %96, 2
  %119 = add nsw i32 %118, %97
  %120 = zext i8 %116 to i32
  %121 = add nsw i32 %119, %120
  %122 = and i32 %121, 536870911
  %123 = select i1 %117, i16 %94, i16 1
  %124 = select i1 %117, i32 %96, i32 %122
  %125 = icmp eq i8 %105, 10
  %126 = zext nneg i16 %123 to i32
  %127 = select i1 %125, i16 0, i16 %123
  %128 = select i1 %125, i32 %126, i32 0
  %129 = add nsw i32 %128, %95
  %130 = add nsw i32 %97, 1
  %131 = add nuw nsw i16 %93, 1
  br label %b31
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
