; ModuleID = 'bench/grep/grep.c'
source_filename = "bench/grep/grep.c"
target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-linux-gnu"

@patterns = internal unnamed_addr constant [81 x i8] c"Oliver|Scrooge|Copperfield|Dombey|Marley|Micawber|Heep|ghost|gentleman|Tiny Tim|\00", align 1
@hit = internal unnamed_addr global [80 x i8] zeroinitializer, align 1
@classof = internal unnamed_addr global [256 x i8] zeroinitializer, align 1
@nxt = internal unnamed_addr global [2560 x i8] zeroinitializer, align 1
@queue = internal unnamed_addr global [80 x i8] zeroinitializer, align 1
@fail = internal unnamed_addr global [80 x i8] zeroinitializer, align 1

; Function Attrs: nounwind
define dso_local i32 @bench_grep(ptr nofree noundef writeonly captures(none) %0) local_unnamed_addr #0 {
  %2 = alloca [1024 x i8], align 1
  call void @llvm.lifetime.start.p0(ptr nonnull %2) #3
  br label %3

3:                                                ; preds = %40, %1
  %4 = phi ptr [ @patterns, %1 ], [ %45, %40 ]
  %5 = phi i32 [ 1, %1 ], [ %41, %40 ]
  %6 = phi i32 [ 1, %1 ], [ %42, %40 ]
  %7 = phi i32 [ 1, %1 ], [ %43, %40 ]
  %8 = phi i32 [ 0, %1 ], [ %44, %40 ]
  %9 = load i8, ptr %4, align 1, !tbaa !8
  switch i8 %9, label %16 [
    i8 0, label %10
    i8 124, label %12
  ]

10:                                               ; preds = %3
  %11 = icmp sgt i32 %5, 1
  br i1 %11, label %51, label %46

12:                                               ; preds = %3
  %13 = add nsw i32 %7, 1
  %14 = trunc i32 %7 to i8
  %15 = getelementptr inbounds nuw i8, ptr @hit, i32 %8
  store i8 %14, ptr %15, align 1, !tbaa !8
  br label %40

16:                                               ; preds = %3
  %17 = zext i8 %9 to i32
  %18 = getelementptr inbounds nuw i8, ptr @classof, i32 %17
  %19 = load i8, ptr %18, align 1, !tbaa !8
  %20 = icmp eq i8 %19, 0
  br i1 %20, label %21, label %24

21:                                               ; preds = %16
  %22 = add nsw i32 %5, 1
  %23 = trunc i32 %5 to i8
  store i8 %23, ptr %18, align 1, !tbaa !8
  br label %24

24:                                               ; preds = %21, %16
  %25 = phi i32 [ %5, %16 ], [ %22, %21 ]
  %26 = shl nuw nsw i32 %8, 5
  %27 = load i8, ptr %18, align 1, !tbaa !8
  %28 = zext i8 %27 to i32
  %29 = getelementptr inbounds nuw i8, ptr @nxt, i32 %26
  %30 = getelementptr inbounds nuw i8, ptr %29, i32 %28
  %31 = load i8, ptr %30, align 1, !tbaa !8
  %32 = icmp eq i8 %31, 0
  br i1 %32, label %33, label %36

33:                                               ; preds = %24
  %34 = add nsw i32 %6, 1
  %35 = trunc i32 %6 to i8
  store i8 %35, ptr %30, align 1, !tbaa !8
  br label %36

36:                                               ; preds = %33, %24
  %37 = phi i32 [ %6, %24 ], [ %34, %33 ]
  %38 = load i8, ptr %30, align 1, !tbaa !8
  %39 = zext i8 %38 to i32
  br label %40

40:                                               ; preds = %36, %12
  %41 = phi i32 [ %5, %12 ], [ %25, %36 ]
  %42 = phi i32 [ %6, %12 ], [ %37, %36 ]
  %43 = phi i32 [ %13, %12 ], [ %7, %36 ]
  %44 = phi i32 [ 0, %12 ], [ %39, %36 ]
  %45 = getelementptr inbounds nuw i8, ptr %4, i32 1
  br label %3, !llvm.loop !9

46:                                               ; preds = %60, %10
  %47 = phi i32 [ 0, %10 ], [ %61, %60 ]
  %48 = icmp sgt i32 %47, 0
  %49 = icmp sgt i32 %5, 0
  %50 = and i1 %48, %49
  br i1 %50, label %70, label %67

51:                                               ; preds = %10, %60
  %52 = phi i32 [ %62, %60 ], [ 1, %10 ]
  %53 = phi i32 [ %61, %60 ], [ 0, %10 ]
  %54 = getelementptr inbounds nuw i8, ptr @nxt, i32 %52
  %55 = load i8, ptr %54, align 1, !tbaa !8
  %56 = icmp eq i8 %55, 0
  br i1 %56, label %60, label %57

57:                                               ; preds = %51
  %58 = add nsw i32 %53, 1
  %59 = getelementptr inbounds i8, ptr @queue, i32 %53
  store i8 %55, ptr %59, align 1, !tbaa !8
  br label %60

60:                                               ; preds = %51, %57
  %61 = phi i32 [ %58, %57 ], [ %53, %51 ]
  %62 = add nuw nsw i32 %52, 1
  %63 = icmp eq i32 %62, %5
  br i1 %63, label %46, label %51, !llvm.loop !12

64:                                               ; preds = %105
  %65 = add nuw nsw i32 %72, 1
  %66 = icmp slt i32 %65, %106
  br i1 %66, label %70, label %67, !llvm.loop !13

67:                                               ; preds = %64, %46
  %68 = call i32 @input_read(ptr noundef nonnull %2, i32 noundef 1024) #4
  %69 = icmp sgt i32 %68, 0
  br i1 %69, label %113, label %159

70:                                               ; preds = %46, %64
  %71 = phi i32 [ %106, %64 ], [ %47, %46 ]
  %72 = phi i32 [ %65, %64 ], [ 0, %46 ]
  %73 = getelementptr inbounds nuw i8, ptr @queue, i32 %72
  %74 = load i8, ptr %73, align 1, !tbaa !8
  %75 = zext i8 %74 to i32
  %76 = shl nuw nsw i32 %75, 5
  %77 = getelementptr inbounds nuw i8, ptr @nxt, i32 %76
  %78 = getelementptr inbounds nuw i8, ptr @fail, i32 %75
  br label %79

79:                                               ; preds = %70, %105
  %80 = phi i32 [ 0, %70 ], [ %107, %105 ]
  %81 = phi i32 [ %71, %70 ], [ %106, %105 ]
  %82 = getelementptr inbounds nuw i8, ptr %77, i32 %80
  %83 = load i8, ptr %82, align 1, !tbaa !8
  %84 = load i8, ptr %78, align 1, !tbaa !8
  %85 = zext i8 %84 to i32
  %86 = shl nuw nsw i32 %85, 5
  %87 = getelementptr inbounds nuw i8, ptr @nxt, i32 %86
  %88 = getelementptr inbounds nuw i8, ptr %87, i32 %80
  %89 = load i8, ptr %88, align 1, !tbaa !8
  %90 = zext i8 %89 to i32
  %91 = icmp eq i8 %83, 0
  br i1 %91, label %104, label %92

92:                                               ; preds = %79
  %93 = zext i8 %83 to i32
  %94 = getelementptr inbounds nuw i8, ptr @fail, i32 %93
  store i8 %89, ptr %94, align 1, !tbaa !8
  %95 = getelementptr inbounds nuw i8, ptr @hit, i32 %93
  %96 = load i8, ptr %95, align 1, !tbaa !8
  %97 = icmp eq i8 %96, 0
  br i1 %97, label %98, label %101

98:                                               ; preds = %92
  %99 = getelementptr inbounds nuw i8, ptr @hit, i32 %90
  %100 = load i8, ptr %99, align 1, !tbaa !8
  store i8 %100, ptr %95, align 1, !tbaa !8
  br label %101

101:                                              ; preds = %98, %92
  %102 = add nuw nsw i32 %81, 1
  %103 = getelementptr inbounds nuw i8, ptr @queue, i32 %81
  store i8 %83, ptr %103, align 1, !tbaa !8
  br label %105

104:                                              ; preds = %79
  store i8 %89, ptr %82, align 1, !tbaa !8
  br label %105

105:                                              ; preds = %104, %101
  %106 = phi i32 [ %102, %101 ], [ %81, %104 ]
  %107 = add nuw nsw i32 %80, 1
  %108 = icmp eq i32 %107, %5
  br i1 %108, label %64, label %79, !llvm.loop !14

109:                                              ; preds = %147
  %110 = add i32 %115, %114
  %111 = call i32 @input_read(ptr noundef nonnull %2, i32 noundef 1024) #4
  %112 = icmp sgt i32 %111, 0
  br i1 %112, label %113, label %157, !llvm.loop !15

113:                                              ; preds = %67, %109
  %114 = phi i32 [ %111, %109 ], [ %68, %67 ]
  %115 = phi i32 [ %110, %109 ], [ 0, %67 ]
  %116 = phi i32 [ %149, %109 ], [ 0, %67 ]
  %117 = phi i32 [ %153, %109 ], [ 0, %67 ]
  %118 = phi i32 [ %151, %109 ], [ 0, %67 ]
  %119 = phi i32 [ %137, %109 ], [ 0, %67 ]
  br label %120

120:                                              ; preds = %113, %147
  %121 = phi i32 [ %115, %113 ], [ %154, %147 ]
  %122 = phi i32 [ %116, %113 ], [ %149, %147 ]
  %123 = phi i32 [ %117, %113 ], [ %153, %147 ]
  %124 = phi i32 [ %118, %113 ], [ %151, %147 ]
  %125 = phi i32 [ 0, %113 ], [ %155, %147 ]
  %126 = phi i32 [ %119, %113 ], [ %137, %147 ]
  %127 = getelementptr inbounds nuw i8, ptr %2, i32 %125
  %128 = load i8, ptr %127, align 1, !tbaa !8
  %129 = shl nuw nsw i32 %126, 5
  %130 = zext i8 %128 to i32
  %131 = getelementptr inbounds nuw i8, ptr @classof, i32 %130
  %132 = load i8, ptr %131, align 1, !tbaa !8
  %133 = zext i8 %132 to i32
  %134 = getelementptr inbounds nuw i8, ptr @nxt, i32 %129
  %135 = getelementptr inbounds nuw i8, ptr %134, i32 %133
  %136 = load i8, ptr %135, align 1, !tbaa !8
  %137 = zext i8 %136 to i32
  %138 = getelementptr inbounds nuw i8, ptr @hit, i32 %137
  %139 = load i8, ptr %138, align 1, !tbaa !8
  %140 = icmp eq i8 %139, 0
  br i1 %140, label %147, label %141

141:                                              ; preds = %120
  %142 = shl nuw nsw i32 %122, 1
  %143 = add nsw i32 %142, %121
  %144 = zext i8 %139 to i32
  %145 = add nsw i32 %143, %144
  %146 = and i32 %145, 536870911
  br label %147

147:                                              ; preds = %141, %120
  %148 = phi i32 [ 1, %141 ], [ %124, %120 ]
  %149 = phi i32 [ %146, %141 ], [ %122, %120 ]
  %150 = icmp eq i8 %128, 10
  %151 = select i1 %150, i32 0, i32 %148
  %152 = select i1 %150, i32 %148, i32 0
  %153 = add nsw i32 %152, %123
  %154 = add nsw i32 %121, 1
  %155 = add nuw nsw i32 %125, 1
  %156 = icmp eq i32 %155, %114
  br i1 %156, label %109, label %120, !llvm.loop !16

157:                                              ; preds = %109
  %158 = add nsw i32 %153, %151
  br label %159

159:                                              ; preds = %157, %67
  %160 = phi i32 [ 0, %67 ], [ %149, %157 ]
  %161 = phi i32 [ 0, %67 ], [ %158, %157 ]
  store i32 %160, ptr %0, align 2, !tbaa !17
  call void @llvm.lifetime.end.p0(ptr nonnull %2) #3
  ret i32 %161
}

; Function Attrs: mustprogress nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: mustprogress nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

declare i32 @input_read(ptr noundef, i32 noundef) local_unnamed_addr #2

; Function Attrs: nounwind
define dso_local noundef i32 @main() local_unnamed_addr #0 {
  %1 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr nonnull %1) #3
  %2 = call i32 @bench_grep(ptr noundef nonnull %1) #5
  tail call void @report(i32 noundef %2) #4
  %3 = load i32, ptr %1, align 2, !tbaa !17
  tail call void @report(i32 noundef %3) #4
  call void @llvm.lifetime.end.p0(ptr nonnull %1) #3
  ret i32 0
}

declare void @report(i32 noundef) local_unnamed_addr #2

attributes #0 = { nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" }
attributes #1 = { mustprogress nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" }
attributes #3 = { nounwind }
attributes #4 = { nobuiltin nounwind "no-builtins" }
attributes #5 = { nobuiltin "no-builtins" }

!llvm.module.flags = !{!0, !1}
!llvm.ident = !{!2}
!llvm.errno.tbaa = !{!3}

!0 = !{i32 8, !"PIC Level", i32 2}
!1 = !{i32 7, !"PIE Level", i32 2}
!2 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!3 = !{!4, !5, i64 0}
!4 = !{!"__libc_errno", !5, i64 0}
!5 = !{!"int", !6, i64 0}
!6 = !{!"omnipotent char", !7, i64 0}
!7 = !{!"Simple C/C++ TBAA"}
!8 = !{!6, !6, i64 0}
!9 = distinct !{!9, !10, !11}
!10 = !{!"llvm.loop.mustprogress"}
!11 = !{!"llvm.loop.unroll.disable"}
!12 = distinct !{!12, !10, !11}
!13 = distinct !{!13, !10, !11}
!14 = distinct !{!14, !10, !11}
!15 = distinct !{!15, !10, !11}
!16 = distinct !{!16, !10, !11}
!17 = !{!18, !18, i64 0}
!18 = !{!"long", !6, i64 0}
