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
  %8 = zext i8 %7 to i16
  %9 = icmp eq i8 %7, 124
  br i1 %9, label %b6, label %10

10:
  %11 = icmp eq i8 %7, 0
  br i1 %11, label %b3, label %b5

b3:
  %12 = icmp sgt i16 %2, 1
  br i1 %12, label %13, label %b18

13:
  %14 = add i16 %2, -1
  br label %b14

b5:
  %15 = getelementptr inbounds nuw i8, ptr @_classof, i16 %8
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
  %21 = phi i8 [ %3, %b6 ], [ %37, %b10 ]
  %22 = phi i8 [ %18, %b6 ], [ %4, %b10 ]
  %23 = phi i16 [ 0, %b6 ], [ %38, %b10 ]
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
  %35 = getelementptr i8, ptr @_classof, i16 %8
  store i8 %34, ptr %35, !tbaa !7
  br label %b8

b10:
  %36 = phi i8 [ %3, %b11 ], [ %31, %b8 ]
  %37 = phi i8 [ %39, %b11 ], [ %3, %b8 ]
  %38 = zext i8 %36 to i16
  br label %b7

b11:
  %39 = add i8 %3, 1
  %40 = getelementptr i8, ptr @_nxt, i16 %27
  %41 = getelementptr i8, ptr %40, i16 %28
  store i8 %3, ptr %41, !tbaa !7
  br label %b10

b13:
  %42 = icmp sgt i16 %50, 0
  br i1 %42, label %43, label %b18

43:
  br label %b19

b14:
  %44 = phi i16 [ 0, %13 ], [ %51, %b15 ]
  %45 = phi i16 [ %50, %b15 ], [ 0, %13 ]
  %46 = getelementptr i8, ptr @_nxt, i16 %44
  %47 = getelementptr i8, ptr %46, i16 1
  %48 = load i8, ptr %47, !tbaa !7
  %49 = icmp eq i8 %48, 0
  br i1 %49, label %b15, label %b16

b15:
  %50 = phi i16 [ %52, %b16 ], [ %45, %b14 ]
  %51 = add nuw nsw i16 %44, 1
  %exitcond2 = icmp eq i16 %14, %51
  br i1 %exitcond2, label %b13, label %b14

b16:
  %52 = add nsw i16 %45, 1
  %53 = getelementptr inbounds i8, ptr @_queue, i16 %45
  store i8 %48, ptr %53, !tbaa !7
  br label %b15

b18:
  %54 = call cc 1005 addrspace(1) i16 @"_input_read@3"(ptr noundef nonnull %1, i16 noundef 1024)
  %55 = icmp sgt i16 %54, 0
  br i1 %55, label %56, label %b29

56:
  br label %b30

b19:
  %57 = phi i16 [ %96, %66 ], [ %50, %43 ]
  %58 = phi i16 [ %59, %66 ], [ 0, %43 ]
  %59 = add nuw nsw i16 %58, 1
  %60 = getelementptr inbounds nuw i8, ptr @_queue, i16 %58
  %61 = load i8, ptr %60, !tbaa !7
  %62 = zext i8 %61 to i16
  %63 = shl nuw nsw i16 %62, 5
  %64 = getelementptr inbounds nuw i8, ptr @_fail, i16 %62
  %65 = load i8, ptr %64, !tbaa !7
  br label %b22

66:
  %67 = icmp slt i16 %59, %96
  br i1 %67, label %b19, label %b18

b22:
  %68 = phi i8 [ %65, %b19 ], [ %95, %b27 ]
  %69 = phi i16 [ 0, %b19 ], [ %97, %b27 ]
  %70 = phi i16 [ %57, %b19 ], [ %96, %b27 ]
  %71 = getelementptr i8, ptr @_nxt, i16 %63
  %72 = getelementptr i8, ptr %71, i16 %69
  %73 = load i8, ptr %72, !tbaa !7
  %74 = zext i8 %73 to i16
  %75 = zext i8 %68 to i16
  %76 = shl nuw nsw i16 %75, 5
  %77 = add i16 %69, %76
  %78 = getelementptr i8, ptr @_nxt, i16 %77
  %79 = load i8, ptr %78, !tbaa !7
  %80 = zext i8 %79 to i16
  %81 = icmp eq i8 %73, 0
  br i1 %81, label %b23, label %b24

b23:
  %82 = getelementptr i8, ptr @_nxt, i16 %63
  %83 = getelementptr i8, ptr %82, i16 %69
  store i8 %79, ptr %83, !tbaa !7
  br label %b27

b24:
  %84 = getelementptr inbounds nuw i8, ptr @_fail, i16 %74
  store i8 %79, ptr %84, !tbaa !7
  %85 = getelementptr inbounds nuw i8, ptr @_hit, i16 %74
  %86 = load i8, ptr %85, !tbaa !7
  %87 = icmp eq i8 %86, 0
  %88 = getelementptr i8, ptr @_fail, i16 %62
  %89 = load i8, ptr %88, !tbaa !7
  br i1 %87, label %b26, label %b25

b25:
  %90 = add nuw nsw i16 %70, 1
  %91 = getelementptr inbounds nuw i8, ptr @_queue, i16 %70
  store i8 %73, ptr %91, !tbaa !7
  br label %b27

b26:
  %92 = getelementptr inbounds nuw i8, ptr @_hit, i16 %80
  %93 = load i8, ptr %92, !tbaa !7
  %94 = getelementptr i8, ptr @_hit, i16 %74
  store i8 %93, ptr %94, !tbaa !7
  br label %b25

b27:
  %95 = phi i8 [ %89, %b25 ], [ %68, %b23 ]
  %96 = phi i16 [ %90, %b25 ], [ %70, %b23 ]
  %97 = add nuw nsw i16 %69, 1
  %exitcond1 = icmp eq i16 %2, %97
  br i1 %exitcond1, label %66, label %b22

98:
  %99 = zext nneg i16 %142 to i32
  %100 = add nsw i32 %144, %99
  br label %b29

b29:
  %101 = phi i32 [ %139, %98 ], [ 0, %b18 ]
  %102 = phi i32 [ %100, %98 ], [ 0, %b18 ]
  store i32 %101, ptr %0, !tbaa !11
  ret i32 %102

b30:
  %103 = phi i16 [ %111, %b32 ], [ %54, %56 ]
  %104 = phi i32 [ %110, %b32 ], [ 0, %56 ]
  %105 = phi i32 [ %139, %b32 ], [ 0, %56 ]
  %106 = phi i32 [ %144, %b32 ], [ 0, %56 ]
  %107 = phi i16 [ %142, %b32 ], [ 0, %56 ]
  %108 = phi i16 [ %129, %b32 ], [ 0, %56 ]
  %109 = zext nneg i16 %103 to i32
  %110 = add i32 %104, %109
  br label %b33

b32:
  %111 = call cc 1005 addrspace(1) i16 @"_input_read@3"(ptr noundef nonnull %1, i16 noundef 1024)
  %112 = icmp sgt i16 %111, 0
  br i1 %112, label %b30, label %98

b33:
  %113 = phi i32 [ %104, %b30 ], [ %145, %b33 ]
  %114 = phi i32 [ %105, %b30 ], [ %139, %b33 ]
  %115 = phi i32 [ %106, %b30 ], [ %144, %b33 ]
  %116 = phi i16 [ %107, %b30 ], [ %142, %b33 ]
  %117 = phi i16 [ 0, %b30 ], [ %146, %b33 ]
  %118 = phi i16 [ %108, %b30 ], [ %129, %b33 ]
  %119 = getelementptr i8, ptr %1, i16 %117
  %120 = load i8, ptr %119, !tbaa !7
  %121 = zext i8 %120 to i16
  %122 = shl nuw nsw i16 %118, 5
  %123 = getelementptr inbounds nuw i8, ptr @_classof, i16 %121
  %124 = load i8, ptr %123, !tbaa !7
  %125 = zext i8 %124 to i16
  %126 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %122
  %127 = getelementptr inbounds nuw i8, ptr %126, i16 %125
  %128 = load i8, ptr %127, !tbaa !7
  %129 = zext i8 %128 to i16
  %130 = getelementptr inbounds nuw i8, ptr @_hit, i16 %129
  %131 = load i8, ptr %130, !tbaa !7
  %132 = zext i8 %131 to i32
  %133 = icmp eq i32 %132, 0
  %134 = shl i32 %114, 1
  %135 = add i32 %134, %132
  %136 = add i32 %113, %135
  %137 = and i32 %136, 536870911
  %138 = select i1 %133, i16 %116, i16 1
  %139 = select i1 %133, i32 %114, i32 %137
  %140 = icmp eq i16 %121, 10
  %141 = zext nneg i16 %138 to i32
  %142 = select i1 %140, i16 0, i16 %138
  %143 = select i1 %140, i32 %141, i32 0
  %144 = add nsw i32 %143, %115
  %145 = add nsw i32 %113, 1
  %146 = add nuw nsw i16 %117, 1
  %exitcond = icmp eq i16 %103, %146
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
