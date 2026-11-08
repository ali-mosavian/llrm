target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@_patterns = internal constant [81 x i8] c"Oliver|Scrooge|Copperfield|Dombey|Marley|Micawber|Heep|ghost|gentleman|Tiny Tim|\00", align 1
@_hit = internal unnamed_addr global [80 x i8] zeroinitializer, align 1
@_classof = internal unnamed_addr global [256 x i8] zeroinitializer, align 1
@_nxt = internal unnamed_addr global [2560 x i8] zeroinitializer, align 1
@_queue = internal unnamed_addr global [80 x i8] zeroinitializer, align 1
@_fail = internal unnamed_addr global [80 x i8] zeroinitializer, align 1

define cc 1005 i32 @"_bench_grep@3"(ptr noundef nocapture nofree writeonly "llrm.reads"="" "llrm.writes"="0:4" %0) addrspace(1) noinline nounwind "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i386" {
  %2 = alloca [1024 x i8], align 1
  call addrspace(1) void @llvm.lifetime.start.p0(i64 -1, ptr %2) nounwind
  br label %3

3:
  %4 = phi ptr [ @_patterns, %1 ], [ %46, %41 ]
  %5 = phi i16 [ 1, %1 ], [ %42, %41 ]
  %6 = phi i16 [ 1, %1 ], [ %43, %41 ]
  %7 = phi i16 [ 1, %1 ], [ %44, %41 ]
  %8 = phi i16 [ 0, %1 ], [ %45, %41 ]
  %9 = load i8, ptr %4, align 1, !tbaa !16
  switch i8 %9, label %17 [
    i8 0, label %10
    i8 124, label %13
  ]

10:
  %11 = icmp sgt i16 %5, 1
  br i1 %11, label %12, label %48

12:
  br label %53

13:
  %14 = add nsw i16 %7, 1
  %15 = trunc i16 %7 to i8
  %16 = getelementptr inbounds nuw i8, ptr @_hit, i16 %8
  store i8 %15, ptr %16, align 1, !tbaa !16
  br label %41

17:
  %18 = zext i8 %9 to i16
  %19 = getelementptr inbounds nuw i8, ptr @_classof, i16 %18
  %20 = load i8, ptr %19, align 1, !tbaa !16
  %21 = icmp eq i8 %20, 0
  br i1 %21, label %22, label %25

22:
  %23 = add nsw i16 %5, 1
  %24 = trunc i16 %5 to i8
  store i8 %24, ptr %19, align 1, !tbaa !16
  br label %25

25:
  %26 = phi i16 [ %5, %17 ], [ %23, %22 ]
  %27 = shl nuw nsw i16 %8, 5
  %28 = load i8, ptr %19, align 1, !tbaa !16
  %29 = zext i8 %28 to i16
  %30 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %27
  %31 = getelementptr inbounds nuw i8, ptr %30, i16 %29
  %32 = load i8, ptr %31, align 1, !tbaa !16
  %33 = icmp eq i8 %32, 0
  br i1 %33, label %34, label %37

34:
  %35 = add nsw i16 %6, 1
  %36 = trunc i16 %6 to i8
  store i8 %36, ptr %31, align 1, !tbaa !16
  br label %37

37:
  %38 = phi i16 [ %6, %25 ], [ %35, %34 ]
  %39 = load i8, ptr %31, align 1, !tbaa !16
  %40 = zext i8 %39 to i16
  br label %41

41:
  %42 = phi i16 [ %5, %13 ], [ %26, %37 ]
  %43 = phi i16 [ %6, %13 ], [ %38, %37 ]
  %44 = phi i16 [ %14, %13 ], [ %7, %37 ]
  %45 = phi i16 [ 0, %13 ], [ %40, %37 ]
  %46 = getelementptr inbounds nuw i8, ptr %4, i16 1
  br label %3, !llvm.loop !17

47:
  br label %48

48:
  %49 = phi i16 [ 0, %10 ], [ %63, %47 ]
  %50 = icmp sgt i16 %49, 0
  %51 = icmp sgt i16 %5, 0
  %or.cond = and i1 %50, %51
  br i1 %or.cond, label %52, label %68, !llvm.loop !21

52:
  br label %72

53:
  %54 = phi i16 [ %64, %62 ], [ 1, %12 ]
  %55 = phi i16 [ %63, %62 ], [ 0, %12 ]
  %56 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %54
  %57 = load i8, ptr %56, align 1, !tbaa !16
  %58 = icmp eq i8 %57, 0
  br i1 %58, label %62, label %59

59:
  %60 = add nsw i16 %55, 1
  %61 = getelementptr inbounds i8, ptr @_queue, i16 %55
  store i8 %57, ptr %61, align 1, !tbaa !16
  br label %62

62:
  %63 = phi i16 [ %60, %59 ], [ %55, %53 ]
  %64 = add nuw nsw i16 %54, 1
  %exitcond = icmp eq i16 %64, %5
  br i1 %exitcond, label %47, label %53, !llvm.loop !19

65:
  %66 = icmp slt i16 %75, %109
  br i1 %66, label %72, label %67

67:
  br label %68

68:
  %69 = call cc 1005 addrspace(1) i16 @"_input_read@3"(ptr noundef %2, i16 noundef 1024)
  %70 = icmp sgt i16 %69, 0
  br i1 %70, label %71, label %161

71:
  br label %114

72:
  %73 = phi i16 [ %109, %65 ], [ %49, %52 ]
  %74 = phi i16 [ %75, %65 ], [ 0, %52 ]
  %75 = add nuw nsw i16 %74, 1
  %76 = getelementptr inbounds nuw i8, ptr @_queue, i16 %74
  %77 = load i8, ptr %76, align 1, !tbaa !16
  %78 = zext i8 %77 to i16
  %79 = shl nuw nsw i16 %78, 5
  %80 = getelementptr i8, ptr @_nxt, i16 %79
  %81 = getelementptr inbounds nuw i8, ptr @_fail, i16 %78
  br label %82

82:
  %83 = phi i16 [ 0, %72 ], [ %110, %108 ]
  %84 = phi i16 [ %73, %72 ], [ %109, %108 ]
  %85 = getelementptr i8, ptr %80, i16 %83
  %86 = load i8, ptr %85, align 1, !tbaa !16
  %87 = load i8, ptr %81, align 1, !tbaa !16
  %88 = zext i8 %87 to i16
  %89 = shl nuw nsw i16 %88, 5
  %90 = getelementptr i8, ptr @_nxt, i16 %89
  %91 = getelementptr i8, ptr %90, i16 %83
  %92 = load i8, ptr %91, align 1, !tbaa !16
  %93 = zext i8 %92 to i16
  %94 = icmp eq i8 %86, 0
  br i1 %94, label %107, label %95

95:
  %96 = zext i8 %86 to i16
  %97 = getelementptr inbounds nuw i8, ptr @_fail, i16 %96
  store i8 %92, ptr %97, align 1, !tbaa !16
  %98 = getelementptr inbounds nuw i8, ptr @_hit, i16 %96
  %99 = load i8, ptr %98, align 1, !tbaa !16
  %100 = icmp eq i8 %99, 0
  br i1 %100, label %101, label %104

101:
  %102 = getelementptr inbounds nuw i8, ptr @_hit, i16 %93
  %103 = load i8, ptr %102, align 1, !tbaa !16
  store i8 %103, ptr %98, align 1, !tbaa !16
  br label %104

104:
  %105 = add nuw nsw i16 %84, 1
  %106 = getelementptr inbounds nuw i8, ptr @_queue, i16 %84
  store i8 %86, ptr %106, align 1, !tbaa !16
  br label %108

107:
  store i8 %92, ptr %85, align 1, !tbaa !16
  br label %108

108:
  %109 = phi i16 [ %105, %104 ], [ %84, %107 ]
  %110 = add nuw nsw i16 %83, 1
  %exitcond1 = icmp eq i16 %110, %5
  br i1 %exitcond1, label %65, label %82, !llvm.loop !20

111:
  %112 = call cc 1005 addrspace(1) i16 @"_input_read@3"(ptr noundef %2, i16 noundef 1024)
  %113 = icmp sgt i16 %112, 0
  br i1 %113, label %114, label %158

114:
  %115 = phi i16 [ %112, %111 ], [ %69, %71 ]
  %116 = phi i32 [ %156, %111 ], [ 0, %71 ]
  %117 = phi i32 [ %150, %111 ], [ 0, %71 ]
  %118 = phi i32 [ %155, %111 ], [ 0, %71 ]
  %119 = phi i16 [ %153, %111 ], [ 0, %71 ]
  %120 = phi i16 [ %138, %111 ], [ 0, %71 ]
  br label %121

121:
  %122 = phi i32 [ %116, %114 ], [ %156, %148 ]
  %123 = phi i32 [ %117, %114 ], [ %150, %148 ]
  %124 = phi i32 [ %118, %114 ], [ %155, %148 ]
  %125 = phi i16 [ %119, %114 ], [ %153, %148 ]
  %126 = phi i16 [ 0, %114 ], [ %157, %148 ]
  %127 = phi i16 [ %120, %114 ], [ %138, %148 ]
  %128 = getelementptr inbounds nuw i8, ptr %2, i16 %126
  %129 = load i8, ptr %128, align 1, !tbaa !16
  %130 = shl nuw nsw i16 %127, 5
  %131 = zext i8 %129 to i16
  %132 = getelementptr inbounds nuw i8, ptr @_classof, i16 %131
  %133 = load i8, ptr %132, align 1, !tbaa !16
  %134 = zext i8 %133 to i16
  %135 = getelementptr inbounds nuw i8, ptr @_nxt, i16 %130
  %136 = getelementptr inbounds nuw i8, ptr %135, i16 %134
  %137 = load i8, ptr %136, align 1, !tbaa !16
  %138 = zext i8 %137 to i16
  %139 = getelementptr inbounds nuw i8, ptr @_hit, i16 %138
  %140 = load i8, ptr %139, align 1, !tbaa !16
  %141 = icmp eq i8 %140, 0
  br i1 %141, label %148, label %142

142:
  %143 = shl nuw nsw i32 %123, 1
  %144 = add nsw i32 %143, %122
  %145 = zext i8 %140 to i32
  %146 = add nsw i32 %144, %145
  %147 = and i32 %146, 536870911
  br label %148

148:
  %149 = phi i16 [ 1, %142 ], [ %125, %121 ]
  %150 = phi i32 [ %147, %142 ], [ %123, %121 ]
  %151 = icmp eq i8 %129, 10
  %152 = zext nneg i16 %149 to i32
  %153 = select i1 %151, i16 0, i16 %149
  %154 = select i1 %151, i32 %152, i32 0
  %155 = add nsw i32 %154, %124
  %156 = add nsw i32 %122, 1
  %157 = add nuw nsw i16 %126, 1
  %exitcond2 = icmp eq i16 %157, %115
  br i1 %exitcond2, label %111, label %121, !llvm.loop !22

158:
  %159 = zext nneg i16 %153 to i32
  %160 = add nsw i32 %155, %159
  br label %161

161:
  %162 = phi i32 [ 0, %68 ], [ %150, %158 ]
  %163 = phi i32 [ %160, %158 ], [ 0, %68 ]
  store i32 %162, ptr %0, align 2, !tbaa !12
  call addrspace(1) void @llvm.lifetime.end.p0(i64 -1, ptr %2) nounwind
  ret i32 %163
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) addrspace(1) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) addrspace(1) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare cc 1005 i16 @"_input_read@3"(ptr noundef, i16 noundef) addrspace(1) "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i386"

define cc 1002 i16 @main_() addrspace(1) noinline nounwind "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i386" {
  %1 = alloca i32, align 2
  call addrspace(1) void @llvm.lifetime.start.p0(i64 -1, ptr %1) nounwind
  %2 = call cc 1005 addrspace(1) i32 @"_bench_grep@3"(ptr noundef %1)
  tail call cc 1005 addrspace(1) void @"_report@3"(i32 noundef %2)
  %3 = load i32, ptr %1, align 2, !tbaa !12
  tail call cc 1005 addrspace(1) void @"_report@3"(i32 noundef %3)
  call addrspace(1) void @llvm.lifetime.end.p0(i64 -1, ptr %1) nounwind
  ret i16 0
}

declare cc 1005 void @"_report@3"(i32 noundef) addrspace(1) "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i386"

declare i16 @llvm.umax.i16(i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)

!llvm.module.flags = !{!0, !1}
!llvm.ident = !{!2}
!llvm.errno.tbaa = !{!3}

!0 = !{i32 1, !"NumRegisterParameters", i32 0}
!1 = !{i32 1, !"wchar_size", i32 2}
!2 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git 4b787ddb5b7f8e894de33d3c18ef1a78cd0a094a)"}
!3 = !{!4, !5, i64 0}
!4 = !{!"__libc_errno", !5, i64 0}
!5 = !{!"int", !6, i64 0}
!6 = !{!"omnipotent char", !7, i64 0}
!7 = !{!"Simple C/C++ TBAA"}
!8 = !{!9, !9, i64 0}
!9 = !{!"p1 long", !10, i64 0}
!10 = !{!"any pointer", !6, i64 0}
!11 = !{!5, !5, i64 0}
!12 = !{!13, !13, i64 0}
!13 = !{!"long", !6, i64 0}
!14 = !{!15, !15, i64 0}
!15 = !{!"p1 omnipotent char", !10, i64 0}
!16 = !{!6, !6, i64 0}
!17 = distinct !{!17, !18}
!18 = !{!"llvm.loop.unroll.disable"}
!19 = distinct !{!19, !18}
!20 = distinct !{!20, !18}
!21 = distinct !{!21, !18}
!22 = distinct !{!22, !18}
!23 = distinct !{!23, !18}
