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
  %2 = phi i16 [ 1, %b1 ], [ %17, %b7 ]
  %3 = phi i16 [ 1, %b1 ], [ %18, %b7 ]
  %4 = phi i16 [ 1, %b1 ], [ %19, %b7 ]
  %5 = phi i16 [ 0, %b1 ], [ %20, %b7 ]
  %6 = phi ptr [ @_patterns, %b1 ], [ %21, %b7 ]
  %7 = load i8, ptr %6, !tbaa !7
  %8 = icmp eq i8 %7, 0
  br i1 %8, label %b3, label %b4

b3:
  br label %b12

b4:
  %9 = icmp eq i8 %7, 124
  br i1 %9, label %b6, label %b5

b5:
  %10 = zext i8 %7 to i16
  %11 = getelementptr inbounds nuw i8, ptr @_classof, i16 %10
  %12 = load i8, ptr %11, !tbaa !7
  %13 = icmp eq i8 %12, 0
  br i1 %13, label %b9, label %b8

b6:
  %14 = add nsw i16 %4, 1
  %15 = trunc i16 %4 to i8
  %16 = getelementptr inbounds nuw i8, ptr @_hit, i16 %5
  store i8 %15, ptr %16, !tbaa !7
  br label %b7

b7:
  %17 = phi i16 [ %2, %b6 ], [ %22, %b10 ]
  %18 = phi i16 [ %3, %b6 ], [ %32, %b10 ]
  %19 = phi i16 [ %14, %b6 ], [ %4, %b10 ]
  %20 = phi i16 [ 0, %b6 ], [ %38, %b10 ]
  %21 = getelementptr inbounds nuw i8, ptr %6, i16 1
  br label %b2

b8:
  %22 = phi i16 [ %30, %b9 ], [ %2, %b5 ]
  %23 = shl nuw nsw i16 %5, 5
  %24 = load i8, ptr %11, !tbaa !7
  %25 = zext i8 %24 to i16
  %26 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %23
  %27 = getelementptr inbounds nuw i8, ptr %26, i16 %25
  %28 = load i8, ptr %27, !tbaa !7
  %29 = icmp eq i8 %28, 0
  br i1 %29, label %b11, label %b10

b9:
  %30 = add nsw i16 %2, 1
  %31 = trunc i16 %2 to i8
  store i8 %31, ptr %11, !tbaa !7
  br label %b8

b10:
  %32 = phi i16 [ %39, %b11 ], [ %3, %b8 ]
  %33 = load i8, ptr %11, !tbaa !7
  %34 = zext i8 %33 to i16
  %35 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %23
  %36 = getelementptr inbounds nuw i8, ptr %35, i16 %34
  %37 = load i8, ptr %36, !tbaa !7
  %38 = zext i8 %37 to i16
  br label %b7

b11:
  %39 = add nsw i16 %3, 1
  %40 = trunc i16 %3 to i8
  store i8 %40, ptr %27, !tbaa !7
  br label %b10

b12:
  %41 = phi i16 [ 0, %b3 ], [ %47, %b15 ]
  %42 = phi i16 [ 1, %b3 ], [ %48, %b15 ]
  %43 = icmp slt i16 %42, %2
  br i1 %43, label %b14, label %b13

b13:
  br label %b17

b14:
  %44 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %42
  %45 = load i8, ptr %44, !tbaa !7
  %46 = icmp eq i8 %45, 0
  br i1 %46, label %b15, label %b16

b15:
  %47 = phi i16 [ %49, %b16 ], [ %41, %b14 ]
  %48 = add nsw i16 %42, 1
  br label %b12

b16:
  %49 = add nsw i16 %41, 1
  %50 = getelementptr inbounds i8, ptr @_queue, i16 %41
  store i8 %45, ptr %50, !tbaa !7
  br label %b15

b17:
  %51 = phi i16 [ 0, %b13 ], [ %54, %b21 ]
  %52 = phi i16 [ %41, %b13 ], [ %58, %b21 ]
  %53 = icmp slt i16 %51, %52
  br i1 %53, label %b19, label %b18

b18:
  br label %b28

b19:
  %54 = add nsw i16 %51, 1
  %55 = getelementptr inbounds nuw i8, ptr @_queue, i16 %51
  %56 = load i8, ptr %55, !tbaa !7
  %57 = zext i8 %56 to i16
  br label %b20

b20:
  %58 = phi i16 [ %52, %b19 ], [ %83, %b27 ]
  %59 = phi i16 [ 0, %b19 ], [ %84, %b27 ]
  %60 = icmp slt i16 %59, %2
  br i1 %60, label %b22, label %b21

b21:
  br label %b17

b22:
  %61 = shl nuw nsw i16 %57, 5
  %62 = getelementptr i8, ptr @_nxt, i16 %61
  %63 = getelementptr i8, ptr %62, i16 %59
  %64 = load i8, ptr %63, !tbaa !7
  %65 = getelementptr inbounds nuw i8, ptr @_fail, i16 %57
  %66 = load i8, ptr %65, !tbaa !7
  %67 = zext i8 %66 to i16
  %68 = shl nuw nsw i16 %67, 5
  %69 = getelementptr i8, ptr @_nxt, i16 %68
  %70 = getelementptr i8, ptr %69, i16 %59
  %71 = load i8, ptr %70, !tbaa !7
  %72 = zext i8 %71 to i16
  %73 = icmp eq i8 %64, 0
  br i1 %73, label %b23, label %b24

b23:
  store i8 %71, ptr %63, !tbaa !7
  br label %b27

b24:
  %74 = zext i8 %64 to i16
  %75 = getelementptr inbounds nuw i8, ptr @_fail, i16 %74
  store i8 %71, ptr %75, !tbaa !7
  %76 = getelementptr inbounds nuw i8, ptr @_hit, i16 %74
  %77 = load i8, ptr %76, !tbaa !7
  %78 = icmp eq i8 %77, 0
  br i1 %78, label %b26, label %b25

b25:
  %79 = add nsw i16 %58, 1
  %80 = getelementptr inbounds i8, ptr @_queue, i16 %58
  store i8 %64, ptr %80, !tbaa !7
  br label %b27

b26:
  %81 = getelementptr inbounds nuw i8, ptr @_hit, i16 %72
  %82 = load i8, ptr %81, !tbaa !7
  store i8 %82, ptr %76, !tbaa !7
  br label %b25

b27:
  %83 = phi i16 [ %79, %b25 ], [ %58, %b23 ]
  %84 = add nsw i16 %59, 1
  br label %b20

b28:
  %85 = phi i16 [ 0, %b18 ], [ %94, %b32 ]
  %86 = phi i16 [ 0, %b18 ], [ %96, %b32 ]
  %87 = phi i32 [ 0, %b18 ], [ %97, %b32 ]
  %88 = phi i32 [ 0, %b18 ], [ %98, %b32 ]
  %89 = phi i32 [ 0, %b18 ], [ %99, %b32 ]
  %90 = call cc 1005 addrspace(1) i16 @"_input_read@3"(ptr noundef %1, i16 noundef 1024)
  %91 = icmp sgt i16 %90, 0
  br i1 %91, label %b30, label %b29

b29:
  store i32 %88, ptr %0, !tbaa !11
  %92 = zext nneg i16 %86 to i32
  %93 = add nsw i32 %87, %92
  ret i32 %93

b30:
  br label %b31

b31:
  %94 = phi i16 [ %85, %b30 ], [ %111, %b36 ]
  %95 = phi i16 [ 0, %b30 ], [ %126, %b36 ]
  %96 = phi i16 [ %86, %b30 ], [ %123, %b36 ]
  %97 = phi i32 [ %87, %b30 ], [ %124, %b36 ]
  %98 = phi i32 [ %88, %b30 ], [ %116, %b36 ]
  %99 = phi i32 [ %89, %b30 ], [ %125, %b36 ]
  %100 = icmp slt i16 %95, %90
  br i1 %100, label %b33, label %b32

b32:
  br label %b28

b33:
  %101 = getelementptr inbounds nuw i8, ptr %1, i16 %95
  %102 = load i8, ptr %101, !tbaa !7
  %103 = shl nuw nsw i16 %94, 5
  %104 = zext i8 %102 to i16
  %105 = getelementptr inbounds nuw i8, ptr @_classof, i16 %104
  %106 = load i8, ptr %105, !tbaa !7
  %107 = zext i8 %106 to i16
  %108 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %103
  %109 = getelementptr inbounds nuw i8, ptr %108, i16 %107
  %110 = load i8, ptr %109, !tbaa !7
  %111 = zext i8 %110 to i16
  %112 = getelementptr inbounds nuw i8, ptr @_hit, i16 %111
  %113 = load i8, ptr %112, !tbaa !7
  %114 = icmp eq i8 %113, 0
  br i1 %114, label %b34, label %b35

b34:
  %115 = phi i16 [ 1, %b35 ], [ %96, %b33 ]
  %116 = phi i32 [ %122, %b35 ], [ %98, %b33 ]
  %117 = icmp eq i8 %102, 10
  br i1 %117, label %b37, label %b36

b35:
  %118 = shl nuw nsw i32 %98, 1
  %119 = add nsw i32 %118, %99
  %120 = zext i8 %113 to i32
  %121 = add nsw i32 %119, %120
  %122 = and i32 %121, 536870911
  br label %b34

b36:
  %123 = phi i16 [ 0, %b37 ], [ %115, %b34 ]
  %124 = phi i32 [ %128, %b37 ], [ %97, %b34 ]
  %125 = add nsw i32 %99, 1
  %126 = add nsw i16 %95, 1
  br label %b31

b37:
  %127 = zext nneg i16 %115 to i32
  %128 = add nsw i32 %97, %127
  br label %b36
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
