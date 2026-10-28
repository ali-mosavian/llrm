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
  %2 = phi i16 [ 1, %b1 ], [ %19, %b7 ]
  %3 = phi i16 [ 1, %b1 ], [ %20, %b7 ]
  %4 = phi i16 [ 1, %b1 ], [ %21, %b7 ]
  %5 = phi i16 [ 0, %b1 ], [ %22, %b7 ]
  %6 = phi ptr [ @_patterns, %b1 ], [ %23, %b7 ]
  %7 = load i8, ptr %6, !tbaa !7
  %8 = sext i8 %7 to i16
  %9 = icmp ne i16 %8, 0
  br i1 %9, label %b4, label %b3

b3:
  br label %b12

b4:
  %10 = zext i8 %7 to i16
  %11 = icmp eq i16 %10, 124
  br i1 %11, label %b6, label %b5

b5:
  %12 = getelementptr inbounds i8, ptr @_classof, i16 %10
  %13 = load i8, ptr %12, !tbaa !7
  %14 = zext i8 %13 to i16
  %15 = icmp eq i16 %14, 0
  br i1 %15, label %b9, label %b8

b6:
  %16 = add nsw i16 %4, 1
  %17 = trunc i16 %4 to i8
  %18 = getelementptr inbounds i8, ptr @_hit, i16 %5
  store i8 %17, ptr %18, !tbaa !7
  br label %b7

b7:
  %19 = phi i16 [ %2, %b6 ], [ %24, %b10 ]
  %20 = phi i16 [ %3, %b6 ], [ %35, %b10 ]
  %21 = phi i16 [ %16, %b6 ], [ %4, %b10 ]
  %22 = phi i16 [ 0, %b6 ], [ %41, %b10 ]
  %23 = getelementptr inbounds i8, ptr %6, i16 1
  br label %b2

b8:
  %24 = phi i16 [ %33, %b9 ], [ %2, %b5 ]
  %25 = mul nsw i16 %5, 32
  %26 = load i8, ptr %12, !tbaa !7
  %27 = zext i8 %26 to i16
  %28 = add nsw i16 %25, %27
  %29 = getelementptr inbounds i8, ptr @_nxt, i16 %28
  %30 = load i8, ptr %29, !tbaa !7
  %31 = zext i8 %30 to i16
  %32 = icmp eq i16 %31, 0
  br i1 %32, label %b11, label %b10

b9:
  %33 = add nsw i16 %2, 1
  %34 = trunc i16 %2 to i8
  store i8 %34, ptr %12, !tbaa !7
  br label %b8

b10:
  %35 = phi i16 [ %42, %b11 ], [ %3, %b8 ]
  %36 = load i8, ptr %12, !tbaa !7
  %37 = zext i8 %36 to i16
  %38 = add nsw i16 %25, %37
  %39 = getelementptr inbounds i8, ptr @_nxt, i16 %38
  %40 = load i8, ptr %39, !tbaa !7
  %41 = zext i8 %40 to i16
  br label %b7

b11:
  %42 = add nsw i16 %3, 1
  %43 = trunc i16 %3 to i8
  store i8 %43, ptr %29, !tbaa !7
  br label %b10

b12:
  %44 = phi i16 [ 0, %b3 ], [ %51, %b15 ]
  %45 = phi i16 [ 1, %b3 ], [ %52, %b15 ]
  %46 = icmp slt i16 %45, %2
  br i1 %46, label %b14, label %b13

b13:
  br label %b17

b14:
  %47 = getelementptr inbounds i8, ptr @_nxt, i16 %45
  %48 = load i8, ptr %47, !tbaa !7
  %49 = zext i8 %48 to i16
  %50 = icmp ne i16 %49, 0
  br i1 %50, label %b16, label %b15

b15:
  %51 = phi i16 [ %53, %b16 ], [ %44, %b14 ]
  %52 = add nsw i16 %45, 1
  br label %b12

b16:
  %53 = add nsw i16 %44, 1
  %54 = getelementptr inbounds i8, ptr @_queue, i16 %44
  store i8 %48, ptr %54, !tbaa !7
  br label %b15

b17:
  %55 = phi i16 [ 0, %b13 ], [ %58, %b21 ]
  %56 = phi i16 [ %44, %b13 ], [ %62, %b21 ]
  %57 = icmp slt i16 %55, %56
  br i1 %57, label %b19, label %b18

b18:
  br label %b28

b19:
  %58 = add nsw i16 %55, 1
  %59 = getelementptr inbounds i8, ptr @_queue, i16 %55
  %60 = load i8, ptr %59, !tbaa !7
  %61 = zext i8 %60 to i16
  br label %b20

b20:
  %62 = phi i16 [ %56, %b19 ], [ %88, %b27 ]
  %63 = phi i16 [ 0, %b19 ], [ %89, %b27 ]
  %64 = icmp slt i16 %63, %2
  br i1 %64, label %b22, label %b21

b21:
  br label %b17

b22:
  %65 = mul nsw i16 %61, 32
  %66 = add nsw i16 %65, %63
  %67 = getelementptr inbounds i8, ptr @_nxt, i16 %66
  %68 = load i8, ptr %67, !tbaa !7
  %69 = zext i8 %68 to i16
  %70 = getelementptr inbounds i8, ptr @_fail, i16 %61
  %71 = load i8, ptr %70, !tbaa !7
  %72 = zext i8 %71 to i16
  %73 = mul nsw i16 %72, 32
  %74 = add nsw i16 %73, %63
  %75 = getelementptr inbounds i8, ptr @_nxt, i16 %74
  %76 = load i8, ptr %75, !tbaa !7
  %77 = zext i8 %76 to i16
  %78 = icmp ne i16 %69, 0
  br i1 %78, label %b24, label %b23

b23:
  store i8 %76, ptr %67, !tbaa !7
  br label %b27

b24:
  %79 = getelementptr inbounds i8, ptr @_fail, i16 %69
  store i8 %76, ptr %79, !tbaa !7
  %80 = getelementptr inbounds i8, ptr @_hit, i16 %69
  %81 = load i8, ptr %80, !tbaa !7
  %82 = zext i8 %81 to i16
  %83 = icmp eq i16 %82, 0
  br i1 %83, label %b26, label %b25

b25:
  %84 = add nsw i16 %62, 1
  %85 = getelementptr inbounds i8, ptr @_queue, i16 %62
  store i8 %68, ptr %85, !tbaa !7
  br label %b27

b26:
  %86 = getelementptr inbounds i8, ptr @_hit, i16 %77
  %87 = load i8, ptr %86, !tbaa !7
  store i8 %87, ptr %80, !tbaa !7
  br label %b25

b27:
  %88 = phi i16 [ %84, %b25 ], [ %62, %b23 ]
  %89 = add nsw i16 %63, 1
  br label %b20

b28:
  %90 = phi i16 [ 0, %b18 ], [ %99, %b32 ]
  %91 = phi i16 [ 0, %b18 ], [ %101, %b32 ]
  %92 = phi i32 [ 0, %b18 ], [ %102, %b32 ]
  %93 = phi i32 [ 0, %b18 ], [ %103, %b32 ]
  %94 = phi i32 [ 0, %b18 ], [ %104, %b32 ]
  %95 = call cc 1005 addrspace(1) i16 @"_input_read@3"(ptr noundef %1, i16 noundef 1024)
  %96 = icmp sgt i16 %95, 0
  br i1 %96, label %b30, label %b29

b29:
  store i32 %93, ptr %0, !tbaa !11
  %97 = sext i16 %91 to i32
  %98 = add nsw i32 %92, %97
  ret i32 %98

b30:
  br label %b31

b31:
  %99 = phi i16 [ %90, %b30 ], [ %116, %b36 ]
  %100 = phi i16 [ 0, %b30 ], [ %132, %b36 ]
  %101 = phi i16 [ %91, %b30 ], [ %129, %b36 ]
  %102 = phi i32 [ %92, %b30 ], [ %130, %b36 ]
  %103 = phi i32 [ %93, %b30 ], [ %122, %b36 ]
  %104 = phi i32 [ %94, %b30 ], [ %131, %b36 ]
  %105 = icmp slt i16 %100, %95
  br i1 %105, label %b33, label %b32

b32:
  br label %b28

b33:
  %106 = getelementptr inbounds i8, ptr %1, i16 %100
  %107 = load i8, ptr %106, !tbaa !7
  %108 = mul nsw i16 %99, 32
  %109 = zext i8 %107 to i16
  %110 = getelementptr inbounds i8, ptr @_classof, i16 %109
  %111 = load i8, ptr %110, !tbaa !7
  %112 = zext i8 %111 to i16
  %113 = add nsw i16 %108, %112
  %114 = getelementptr inbounds i8, ptr @_nxt, i16 %113
  %115 = load i8, ptr %114, !tbaa !7
  %116 = zext i8 %115 to i16
  %117 = getelementptr inbounds i8, ptr @_hit, i16 %116
  %118 = load i8, ptr %117, !tbaa !7
  %119 = zext i8 %118 to i16
  %120 = icmp ne i16 %119, 0
  br i1 %120, label %b35, label %b34

b34:
  %121 = phi i16 [ 1, %b35 ], [ %101, %b33 ]
  %122 = phi i32 [ %128, %b35 ], [ %103, %b33 ]
  %123 = icmp eq i16 %109, 10
  br i1 %123, label %b37, label %b36

b35:
  %124 = shl i32 %103, 1
  %125 = add nsw i32 %124, %104
  %126 = zext i8 %118 to i32
  %127 = add nsw i32 %125, %126
  %128 = and i32 %127, 536870911
  br label %b34

b36:
  %129 = phi i16 [ 0, %b37 ], [ %121, %b34 ]
  %130 = phi i32 [ %134, %b37 ], [ %102, %b34 ]
  %131 = add nsw i32 %104, 1
  %132 = add nsw i16 %100, 1
  br label %b31

b37:
  %133 = sext i16 %121 to i32
  %134 = add nsw i32 %102, %133
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
