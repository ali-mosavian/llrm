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
  %2 = phi i16 [ 1, %b1 ], [ %14, %b7 ]
  %3 = phi i8 [ 1, %b1 ], [ %15, %b7 ]
  %4 = phi i8 [ 1, %b1 ], [ %16, %b7 ]
  %5 = phi i16 [ 0, %b1 ], [ %17, %b7 ]
  %6 = phi ptr [ @_patterns, %b1 ], [ %18, %b7 ]
  %7 = load i8, ptr %6, !tbaa !7
  switch i8 %7, label %b5 [
    i8 0, label %b3
    i8 124, label %b6
  ]

b3:
  br label %b12

b5:
  %8 = zext i8 %7 to i16
  %9 = getelementptr inbounds nuw i8, ptr @_classof, i16 %8
  %10 = load i8, ptr %9, !tbaa !7
  %11 = icmp eq i8 %10, 0
  br i1 %11, label %b9, label %b8

b6:
  %12 = add i8 %4, 1
  %13 = getelementptr inbounds nuw i8, ptr @_hit, i16 %5
  store i8 %4, ptr %13, !tbaa !7
  br label %b7

b7:
  %14 = phi i16 [ %2, %b6 ], [ %19, %b10 ]
  %15 = phi i8 [ %3, %b6 ], [ %29, %b10 ]
  %16 = phi i8 [ %12, %b6 ], [ %4, %b10 ]
  %17 = phi i16 [ 0, %b6 ], [ %31, %b10 ]
  %18 = getelementptr inbounds nuw i8, ptr %6, i16 1
  br label %b2

b8:
  %19 = phi i16 [ %27, %b9 ], [ %2, %b5 ]
  %20 = shl nuw nsw i16 %5, 5
  %21 = load i8, ptr %9, !tbaa !7
  %22 = zext i8 %21 to i16
  %23 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %20
  %24 = getelementptr inbounds nuw i8, ptr %23, i16 %22
  %25 = load i8, ptr %24, !tbaa !7
  %26 = icmp eq i8 %25, 0
  br i1 %26, label %b11, label %b10

b9:
  %27 = add nsw i16 %2, 1
  %28 = trunc i16 %2 to i8
  store i8 %28, ptr %9, !tbaa !7
  br label %b8

b10:
  %29 = phi i8 [ %32, %b11 ], [ %3, %b8 ]
  %30 = load i8, ptr %24, !tbaa !7
  %31 = zext i8 %30 to i16
  br label %b7

b11:
  %32 = add i8 %3, 1
  store i8 %3, ptr %24, !tbaa !7
  br label %b10

b12:
  %33 = phi i16 [ 0, %b3 ], [ %39, %b15 ]
  %34 = phi i16 [ 1, %b3 ], [ %40, %b15 ]
  %35 = icmp slt i16 %34, %2
  br i1 %35, label %b14, label %b13

b13:
  br label %b17

b14:
  %36 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %34
  %37 = load i8, ptr %36, !tbaa !7
  %38 = icmp eq i8 %37, 0
  br i1 %38, label %b15, label %b16

b15:
  %39 = phi i16 [ %41, %b16 ], [ %33, %b14 ]
  %40 = add nuw nsw i16 %34, 1
  br label %b12

b16:
  %41 = add nsw i16 %33, 1
  %42 = getelementptr inbounds i8, ptr @_queue, i16 %33
  store i8 %37, ptr %42, !tbaa !7
  br label %b15

b17:
  %43 = phi i16 [ 0, %b13 ], [ %46, %b21 ]
  %44 = phi i16 [ %33, %b13 ], [ %50, %b21 ]
  %45 = icmp slt i16 %43, %44
  br i1 %45, label %b19, label %b18

b18:
  br label %b28

b19:
  %46 = add nuw nsw i16 %43, 1
  %47 = getelementptr inbounds nuw i8, ptr @_queue, i16 %43
  %48 = load i8, ptr %47, !tbaa !7
  %49 = zext i8 %48 to i16
  br label %b20

b20:
  %50 = phi i16 [ %44, %b19 ], [ %75, %b27 ]
  %51 = phi i16 [ 0, %b19 ], [ %76, %b27 ]
  %52 = icmp slt i16 %51, %2
  br i1 %52, label %b22, label %b21

b21:
  br label %b17

b22:
  %53 = shl nuw nsw i16 %49, 5
  %54 = getelementptr i8, ptr @_nxt, i16 %53
  %55 = getelementptr i8, ptr %54, i16 %51
  %56 = load i8, ptr %55, !tbaa !7
  %57 = getelementptr inbounds nuw i8, ptr @_fail, i16 %49
  %58 = load i8, ptr %57, !tbaa !7
  %59 = zext i8 %58 to i16
  %60 = shl nuw nsw i16 %59, 5
  %61 = getelementptr i8, ptr @_nxt, i16 %60
  %62 = getelementptr i8, ptr %61, i16 %51
  %63 = load i8, ptr %62, !tbaa !7
  %64 = zext i8 %63 to i16
  %65 = icmp eq i8 %56, 0
  br i1 %65, label %b23, label %b24

b23:
  store i8 %63, ptr %55, !tbaa !7
  br label %b27

b24:
  %66 = zext i8 %56 to i16
  %67 = getelementptr inbounds nuw i8, ptr @_fail, i16 %66
  store i8 %63, ptr %67, !tbaa !7
  %68 = getelementptr inbounds nuw i8, ptr @_hit, i16 %66
  %69 = load i8, ptr %68, !tbaa !7
  %70 = icmp eq i8 %69, 0
  br i1 %70, label %b26, label %b25

b25:
  %71 = add nsw i16 %50, 1
  %72 = getelementptr inbounds i8, ptr @_queue, i16 %50
  store i8 %56, ptr %72, !tbaa !7
  br label %b27

b26:
  %73 = getelementptr inbounds nuw i8, ptr @_hit, i16 %64
  %74 = load i8, ptr %73, !tbaa !7
  store i8 %74, ptr %68, !tbaa !7
  br label %b25

b27:
  %75 = phi i16 [ %71, %b25 ], [ %50, %b23 ]
  %76 = add nuw nsw i16 %51, 1
  br label %b20

b28:
  %77 = phi i16 [ 0, %b18 ], [ %86, %b32 ]
  %78 = phi i16 [ 0, %b18 ], [ %88, %b32 ]
  %79 = phi i32 [ 0, %b18 ], [ %89, %b32 ]
  %80 = phi i32 [ 0, %b18 ], [ %90, %b32 ]
  %81 = phi i32 [ 0, %b18 ], [ %91, %b32 ]
  %82 = call cc 1005 addrspace(1) i16 @"_input_read@3"(ptr noundef nonnull %1, i16 noundef 1024)
  %83 = icmp sgt i16 %82, 0
  br i1 %83, label %b30, label %b29

b29:
  store i32 %80, ptr %0, !tbaa !11
  %84 = zext nneg i16 %78 to i32
  %85 = add nsw i32 %79, %84
  ret i32 %85

b30:
  br label %b31

b31:
  %86 = phi i16 [ %77, %b30 ], [ %103, %b33 ]
  %87 = phi i16 [ 0, %b30 ], [ %120, %b33 ]
  %88 = phi i16 [ %78, %b30 ], [ %116, %b33 ]
  %89 = phi i32 [ %79, %b30 ], [ %118, %b33 ]
  %90 = phi i32 [ %80, %b30 ], [ %113, %b33 ]
  %91 = phi i32 [ %81, %b30 ], [ %119, %b33 ]
  %92 = icmp slt i16 %87, %82
  br i1 %92, label %b33, label %b32

b32:
  br label %b28

b33:
  %93 = getelementptr inbounds nuw i8, ptr %1, i16 %87
  %94 = load i8, ptr %93, !tbaa !7
  %95 = shl nuw nsw i16 %86, 5
  %96 = zext i8 %94 to i16
  %97 = getelementptr inbounds nuw i8, ptr @_classof, i16 %96
  %98 = load i8, ptr %97, !tbaa !7
  %99 = zext i8 %98 to i16
  %100 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %95
  %101 = getelementptr inbounds nuw i8, ptr %100, i16 %99
  %102 = load i8, ptr %101, !tbaa !7
  %103 = zext i8 %102 to i16
  %104 = getelementptr inbounds nuw i8, ptr @_hit, i16 %103
  %105 = load i8, ptr %104, !tbaa !7
  %106 = icmp eq i8 %105, 0
  %107 = mul nuw nsw i32 %90, 2
  %108 = add nsw i32 %107, %91
  %109 = zext i8 %105 to i32
  %110 = add nsw i32 %108, %109
  %111 = and i32 %110, 536870911
  %112 = select i1 %106, i16 %88, i16 1
  %113 = select i1 %106, i32 %90, i32 %111
  %114 = icmp eq i8 %94, 10
  %115 = zext nneg i16 %112 to i32
  %116 = select i1 %114, i16 0, i16 %112
  %117 = select i1 %114, i32 %115, i32 0
  %118 = add nsw i32 %117, %89
  %119 = add nsw i32 %91, 1
  %120 = add nuw nsw i16 %87, 1
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
