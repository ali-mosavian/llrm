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
  %2 = phi i16 [ 1, %b1 ], [ %20, %b7 ]
  %3 = phi i8 [ 1, %b1 ], [ %21, %b7 ]
  %4 = phi i8 [ 1, %b1 ], [ %22, %b7 ]
  %5 = phi i16 [ 0, %b1 ], [ %23, %b7 ]
  %6 = phi ptr [ @_patterns, %b1 ], [ %24, %b7 ]
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
  br i1 %11, label %12, label %b18

12:
  %13 = add i16 %8, -1
  br label %b14

b5:
  %14 = zext i8 %7 to i16
  %15 = getelementptr inbounds nuw i8, ptr @_classof, i16 %14
  %16 = load i8, ptr %15, !tbaa !7
  %17 = icmp eq i8 %16, 0
  br i1 %17, label %b9, label %b8

b6:
  %18 = add i8 %4, 1
  %19 = getelementptr inbounds nuw i8, ptr @_hit, i16 %5
  store i8 %4, ptr %19, !tbaa !7
  br label %b7

b7:
  %20 = phi i16 [ %2, %b6 ], [ %26, %b10 ]
  %21 = phi i8 [ %3, %b6 ], [ %36, %b10 ]
  %22 = phi i8 [ %18, %b6 ], [ %4, %b10 ]
  %23 = phi i16 [ 0, %b6 ], [ %37, %b10 ]
  %24 = getelementptr inbounds nuw i8, ptr %6, i16 1
  br label %b2

b8:
  %25 = phi i8 [ %34, %b9 ], [ %16, %b5 ]
  %26 = phi i16 [ %33, %b9 ], [ %2, %b5 ]
  %27 = shl nuw nsw i16 %5, 5
  %28 = zext i8 %25 to i16
  %29 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %27
  %30 = getelementptr inbounds nuw i8, ptr %29, i16 %28
  %31 = load i8, ptr %30, !tbaa !7
  %32 = icmp eq i8 %31, 0
  br i1 %32, label %b11, label %b10

b9:
  %33 = add nsw i16 %2, 1
  %34 = trunc i16 %2 to i8
  store i8 %34, ptr %15, !tbaa !7
  br label %b8

b10:
  %35 = phi i8 [ %3, %b11 ], [ %31, %b8 ]
  %36 = phi i8 [ %38, %b11 ], [ %3, %b8 ]
  %37 = zext i8 %35 to i16
  br label %b7

b11:
  %38 = add i8 %3, 1
  store i8 %3, ptr %30, !tbaa !7
  br label %b10

b13:
  %39 = phi i16 [ %48, %b15 ]
  %40 = icmp sgt i16 %39, 0
  br i1 %40, label %41, label %b18

41:
  br label %b19

b14:
  %42 = phi i16 [ 0, %12 ], [ %49, %b15 ]
  %43 = phi i16 [ %48, %b15 ], [ 0, %12 ]
  %44 = getelementptr i8, ptr @_nxt, i16 %42
  %45 = getelementptr i8, ptr %44, i16 1
  %46 = load i8, ptr %45, !tbaa !7
  %47 = icmp eq i8 %46, 0
  br i1 %47, label %b15, label %b16

b15:
  %48 = phi i16 [ %50, %b16 ], [ %43, %b14 ]
  %49 = add nuw nsw i16 %42, 1
  %exitcond2 = icmp eq i16 %13, %49
  br i1 %exitcond2, label %b13, label %b14

b16:
  %50 = add nsw i16 %43, 1
  %51 = getelementptr inbounds i8, ptr @_queue, i16 %43
  store i8 %46, ptr %51, !tbaa !7
  br label %b15

52:
  br label %b18

b18:
  %53 = call cc 1005 addrspace(1) i16 @"_input_read@3"(ptr noundef nonnull %1, i16 noundef 1024)
  %54 = icmp sgt i16 %53, 0
  br i1 %54, label %55, label %b29

55:
  br label %b30

b19:
  %56 = phi i16 [ %67, %66 ], [ %39, %41 ]
  %57 = phi i16 [ %58, %66 ], [ 0, %41 ]
  %58 = add nuw nsw i16 %57, 1
  %59 = getelementptr inbounds nuw i8, ptr @_queue, i16 %57
  %60 = load i8, ptr %59, !tbaa !7
  %61 = zext i8 %60 to i16
  %62 = shl nuw nsw i16 %61, 5
  %63 = getelementptr i8, ptr @_nxt, i16 %62
  %64 = getelementptr inbounds nuw i8, ptr @_fail, i16 %61
  %65 = load i8, ptr %64, !tbaa !7
  br label %b22

66:
  %67 = phi i16 [ %92, %b27 ]
  %68 = icmp slt i16 %58, %67
  br i1 %68, label %b19, label %52

b22:
  %69 = phi i8 [ %65, %b19 ], [ %91, %b27 ]
  %70 = phi i16 [ 0, %b19 ], [ %93, %b27 ]
  %71 = phi i16 [ %56, %b19 ], [ %92, %b27 ]
  %72 = getelementptr i8, ptr %63, i16 %70
  %73 = load i8, ptr %72, !tbaa !7
  %74 = zext i8 %69 to i16
  %75 = shl nuw nsw i16 %74, 5
  %76 = add i16 %70, %75
  %77 = getelementptr i8, ptr @_nxt, i16 %76
  %78 = load i8, ptr %77, !tbaa !7
  %79 = zext i8 %78 to i16
  %80 = icmp eq i8 %73, 0
  br i1 %80, label %b23, label %b24

b23:
  store i8 %78, ptr %72, !tbaa !7
  br label %b27

b24:
  %81 = zext i8 %73 to i16
  %82 = getelementptr inbounds nuw i8, ptr @_fail, i16 %81
  store i8 %78, ptr %82, !tbaa !7
  %83 = getelementptr inbounds nuw i8, ptr @_hit, i16 %81
  %84 = load i8, ptr %83, !tbaa !7
  %85 = icmp eq i8 %84, 0
  %86 = load i8, ptr %64, !tbaa !7
  br i1 %85, label %b26, label %b25

b25:
  %87 = add nuw nsw i16 %71, 1
  %88 = getelementptr inbounds nuw i8, ptr @_queue, i16 %71
  store i8 %73, ptr %88, !tbaa !7
  br label %b27

b26:
  %89 = getelementptr inbounds nuw i8, ptr @_hit, i16 %79
  %90 = load i8, ptr %89, !tbaa !7
  store i8 %90, ptr %83, !tbaa !7
  br label %b25

b27:
  %91 = phi i8 [ %86, %b25 ], [ %69, %b23 ]
  %92 = phi i16 [ %87, %b25 ], [ %71, %b23 ]
  %93 = add nuw nsw i16 %70, 1
  %exitcond1 = icmp eq i16 %9, %93
  br i1 %exitcond1, label %66, label %b22

94:
  %95 = phi i32 [ %111, %b32 ]
  %96 = phi i16 [ %112, %b32 ]
  %97 = phi i32 [ %113, %b32 ]
  %98 = zext nneg i16 %96 to i32
  %99 = add nsw i32 %97, %98
  br label %b29

b29:
  %100 = phi i32 [ %95, %94 ], [ 0, %b18 ]
  %101 = phi i32 [ %99, %94 ], [ 0, %b18 ]
  store i32 %100, ptr %0, !tbaa !11
  ret i32 %101

b30:
  %102 = phi i16 [ %114, %b32 ], [ %53, %55 ]
  %103 = phi i32 [ %109, %b32 ], [ 0, %55 ]
  %104 = phi i32 [ %111, %b32 ], [ 0, %55 ]
  %105 = phi i32 [ %113, %b32 ], [ 0, %55 ]
  %106 = phi i16 [ %112, %b32 ], [ 0, %55 ]
  %107 = phi i16 [ %110, %b32 ], [ 0, %55 ]
  %108 = zext nneg i16 %102 to i32
  %109 = add i32 %103, %108
  br label %b33

b32:
  %110 = phi i16 [ %132, %b33 ]
  %111 = phi i32 [ %142, %b33 ]
  %112 = phi i16 [ %145, %b33 ]
  %113 = phi i32 [ %147, %b33 ]
  %114 = call cc 1005 addrspace(1) i16 @"_input_read@3"(ptr noundef nonnull %1, i16 noundef 1024)
  %115 = icmp sgt i16 %114, 0
  br i1 %115, label %b30, label %94

b33:
  %116 = phi i32 [ %103, %b30 ], [ %148, %b33 ]
  %117 = phi i32 [ %104, %b30 ], [ %142, %b33 ]
  %118 = phi i32 [ %105, %b30 ], [ %147, %b33 ]
  %119 = phi i16 [ %106, %b30 ], [ %145, %b33 ]
  %120 = phi i16 [ 0, %b30 ], [ %149, %b33 ]
  %121 = phi i16 [ %107, %b30 ], [ %132, %b33 ]
  %122 = getelementptr i8, ptr %1, i16 %120
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
  %136 = shl i32 %117, 1
  %137 = zext i8 %134 to i32
  %138 = add i32 %136, %137
  %139 = add i32 %116, %138
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
  %exitcond = icmp eq i16 %102, %149
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
