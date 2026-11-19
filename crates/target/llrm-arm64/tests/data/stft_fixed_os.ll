; ModuleID = 'bench/stft_fixed.c'
source_filename = "bench/stft_fixed.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

@tw_re = internal unnamed_addr global [128 x i64] zeroinitializer, align 8
@tw_im = internal unnamed_addr global [128 x i64] zeroinitializer, align 8
@window = internal unnamed_addr global [256 x i64] zeroinitializer, align 8
@rev = internal unnamed_addr global [256 x i8] zeroinitializer, align 1
@.str = private unnamed_addr constant [7 x i8] c"SIGNAL\00", align 1
@.str.1 = private unnamed_addr constant [8 x i8] c"SPECTRA\00", align 1
@block = internal global [2 x [128 x i16]] zeroinitializer, align 2
@re = internal unnamed_addr global [256 x i64] zeroinitializer, align 8
@im = internal unnamed_addr global [256 x i64] zeroinitializer, align 8
@row = internal global [129 x i16] zeroinitializer, align 2

; Function Attrs: noinline nounwind optsize
define dso_local range(i64 -1, 536870912) i64 @bench_stft_fixed() local_unnamed_addr #0 {
  br label %1

1:                                                ; preds = %0, %1
  %2 = phi i64 [ 0, %0 ], [ %19, %1 ]
  %3 = phi i64 [ 0, %0 ], [ %18, %1 ]
  %4 = phi i64 [ 32768, %0 ], [ %13, %1 ]
  %5 = sdiv i64 %4, 64
  %6 = getelementptr inbounds nuw [8 x i8], ptr @tw_re, i64 %2
  store i64 %5, ptr %6, align 8, !tbaa !15
  %7 = sdiv i64 %3, -64
  %8 = getelementptr inbounds nuw [8 x i8], ptr @tw_im, i64 %2
  store i64 %7, ptr %8, align 8, !tbaa !15
  %9 = mul nsw i64 %4, 32758
  %10 = ashr i64 %9, 15
  %11 = mul nsw i64 %3, 804
  %12 = ashr i64 %11, 15
  %13 = sub nsw i64 %10, %12
  %14 = mul nsw i64 %3, 32758
  %15 = ashr i64 %14, 15
  %16 = mul nsw i64 %4, 804
  %17 = ashr i64 %16, 15
  %18 = add nsw i64 %15, %17
  %19 = add nuw nsw i64 %2, 1
  %20 = icmp eq i64 %19, 128
  br i1 %20, label %21, label %1, !llvm.loop !17

21:                                               ; preds = %1, %21
  %22 = phi i64 [ %30, %21 ], [ 0, %1 ]
  %23 = getelementptr inbounds nuw [8 x i8], ptr @tw_re, i64 %22
  %24 = load i64, ptr %23, align 8, !tbaa !15
  %25 = sub nsw i64 512, %24
  %26 = sdiv i64 %25, 2
  %27 = getelementptr inbounds nuw [8 x i8], ptr @window, i64 %22
  store i64 %26, ptr %27, align 8, !tbaa !15
  %28 = sub nsw i64 512, %26
  %29 = getelementptr inbounds nuw i8, ptr %27, i64 1024
  store i64 %28, ptr %29, align 8, !tbaa !15
  %30 = add nuw nsw i64 %22, 1
  %31 = icmp eq i64 %30, 128
  br i1 %31, label %32, label %21, !llvm.loop !19

32:                                               ; preds = %21, %45
  %33 = phi i64 [ %48, %45 ], [ 0, %21 ]
  %34 = trunc nuw nsw i64 %33 to i32
  br label %35

35:                                               ; preds = %32, %35
  %36 = phi i32 [ %34, %32 ], [ %43, %35 ]
  %37 = phi i32 [ 0, %32 ], [ %41, %35 ]
  %38 = phi i32 [ 0, %32 ], [ %42, %35 ]
  %39 = shl nsw i32 %37, 1
  %40 = and i32 %36, 1
  %41 = or disjoint i32 %40, %39
  %42 = add nuw nsw i32 %38, 1
  %43 = lshr i32 %36, 1
  %44 = icmp eq i32 %42, 8
  br i1 %44, label %45, label %35, !llvm.loop !20

45:                                               ; preds = %35
  %46 = trunc i32 %41 to i8
  %47 = getelementptr inbounds nuw i8, ptr @rev, i64 %33
  store i8 %46, ptr %47, align 1, !tbaa !21
  %48 = add nuw nsw i64 %33, 1
  %49 = icmp eq i64 %48, 256
  br i1 %49, label %50, label %32, !llvm.loop !22

50:                                               ; preds = %45
  %51 = tail call i32 @llrm_os_open(ptr noundef nonnull @.str, i8 noundef 0) #5
  %52 = tail call i32 @llrm_os_create(ptr noundef nonnull @.str.1) #5
  %53 = icmp slt i32 %51, 0
  %54 = icmp slt i32 %52, 0
  %55 = select i1 %53, i1 true, i1 %54
  br i1 %55, label %184, label %56

56:                                               ; preds = %50
  %57 = tail call i32 @llrm_os_read(i32 noundef %51, ptr noundef nonnull @block, i64 noundef 256) #5
  %58 = icmp eq i32 %57, 256
  br i1 %58, label %59, label %184

59:                                               ; preds = %56, %177
  %60 = phi i64 [ %173, %177 ], [ 0, %56 ]
  %61 = phi i64 [ %178, %177 ], [ 0, %56 ]
  %62 = phi i32 [ %63, %177 ], [ 0, %56 ]
  %63 = xor i32 %62, 1
  %64 = zext nneg i32 %63 to i64
  %65 = getelementptr inbounds nuw [256 x i8], ptr @block, i64 %64
  %66 = tail call i32 @llrm_os_read(i32 noundef %51, ptr noundef nonnull %65, i64 noundef 256) #5
  %67 = icmp eq i32 %66, 256
  br i1 %67, label %68, label %181

68:                                               ; preds = %59
  %69 = zext nneg i32 %62 to i64
  %70 = getelementptr inbounds nuw [256 x i8], ptr @block, i64 %69
  br label %71

71:                                               ; preds = %68, %71
  %72 = phi i64 [ 0, %68 ], [ %98, %71 ]
  %73 = getelementptr inbounds nuw [2 x i8], ptr %70, i64 %72
  %74 = load i16, ptr %73, align 2, !tbaa !23
  %75 = sdiv i16 %74, 4
  %76 = sext i16 %75 to i64
  %77 = getelementptr inbounds nuw [8 x i8], ptr @window, i64 %72
  %78 = load i64, ptr %77, align 8, !tbaa !15
  %79 = mul nsw i64 %78, %76
  %80 = getelementptr inbounds nuw i8, ptr @rev, i64 %72
  %81 = load i8, ptr %80, align 1, !tbaa !21
  %82 = zext i8 %81 to i64
  %83 = getelementptr inbounds nuw [8 x i8], ptr @re, i64 %82
  store i64 %79, ptr %83, align 8, !tbaa !15
  %84 = getelementptr inbounds nuw [2 x i8], ptr %65, i64 %72
  %85 = load i16, ptr %84, align 2, !tbaa !23
  %86 = sdiv i16 %85, 4
  %87 = sext i16 %86 to i64
  %88 = or disjoint i64 %72, 128
  %89 = getelementptr inbounds nuw [8 x i8], ptr @window, i64 %88
  %90 = load i64, ptr %89, align 8, !tbaa !15
  %91 = mul nsw i64 %90, %87
  %92 = getelementptr inbounds nuw i8, ptr @rev, i64 %88
  %93 = load i8, ptr %92, align 1, !tbaa !21
  %94 = zext i8 %93 to i64
  %95 = getelementptr inbounds nuw [8 x i8], ptr @re, i64 %94
  store i64 %91, ptr %95, align 8, !tbaa !15
  %96 = getelementptr inbounds nuw [8 x i8], ptr @im, i64 %82
  store i64 0, ptr %96, align 8, !tbaa !15
  %97 = getelementptr inbounds nuw [8 x i8], ptr @im, i64 %94
  store i64 0, ptr %97, align 8, !tbaa !15
  %98 = add nuw nsw i64 %72, 1
  %99 = icmp eq i64 %98, 128
  br i1 %99, label %100, label %71, !llvm.loop !25

100:                                              ; preds = %71, %150
  %101 = phi i32 [ %151, %150 ], [ 2, %71 ]
  %102 = lshr exact i32 %101, 1
  %103 = tail call range(i32 1, 33) i32 @llvm.cttz.i32(i32 %101, i1 true)
  %104 = lshr i32 256, %103
  %105 = zext nneg i32 %102 to i64
  %106 = zext nneg i32 %104 to i64
  %107 = zext nneg i32 %101 to i64
  br label %108

108:                                              ; preds = %100, %147
  %109 = phi i64 [ 0, %100 ], [ %148, %147 ]
  br label %110

110:                                              ; preds = %108, %110
  %111 = phi i64 [ 0, %108 ], [ %145, %110 ]
  %112 = add nuw nsw i64 %111, %109
  %113 = getelementptr inbounds nuw [8 x i8], ptr @re, i64 %112
  %114 = load i64, ptr %113, align 8, !tbaa !15
  %115 = getelementptr inbounds nuw [8 x i8], ptr @im, i64 %112
  %116 = load i64, ptr %115, align 8, !tbaa !15
  %117 = add nuw nsw i64 %112, %105
  %118 = getelementptr inbounds nuw [8 x i8], ptr @re, i64 %117
  %119 = load i64, ptr %118, align 8, !tbaa !15
  %120 = getelementptr inbounds nuw [8 x i8], ptr @im, i64 %117
  %121 = load i64, ptr %120, align 8, !tbaa !15
  %122 = mul nuw nsw i64 %111, %106
  %123 = getelementptr inbounds nuw [8 x i8], ptr @tw_re, i64 %122
  %124 = load i64, ptr %123, align 8, !tbaa !15
  %125 = mul nsw i64 %124, %119
  %126 = ashr i64 %125, 9
  %127 = getelementptr inbounds nuw [8 x i8], ptr @tw_im, i64 %122
  %128 = load i64, ptr %127, align 8, !tbaa !15
  %129 = mul nsw i64 %128, %121
  %130 = ashr i64 %129, 9
  %131 = sub nsw i64 %126, %130
  %132 = mul nsw i64 %124, %121
  %133 = ashr i64 %132, 9
  %134 = mul nsw i64 %128, %119
  %135 = ashr i64 %134, 9
  %136 = add nsw i64 %135, %133
  %137 = add nsw i64 %131, %114
  %138 = sdiv i64 %137, 2
  store i64 %138, ptr %113, align 8, !tbaa !15
  %139 = add nsw i64 %136, %116
  %140 = sdiv i64 %139, 2
  store i64 %140, ptr %115, align 8, !tbaa !15
  %141 = sub nsw i64 %114, %131
  %142 = sdiv i64 %141, 2
  store i64 %142, ptr %118, align 8, !tbaa !15
  %143 = sub nsw i64 %116, %136
  %144 = sdiv i64 %143, 2
  store i64 %144, ptr %120, align 8, !tbaa !15
  %145 = add nuw nsw i64 %111, 1
  %146 = icmp eq i64 %145, %105
  br i1 %146, label %147, label %110, !llvm.loop !26

147:                                              ; preds = %110
  %148 = add nuw nsw i64 %109, %107
  %149 = icmp samesign ult i64 %148, 256
  br i1 %149, label %108, label %150, !llvm.loop !27

150:                                              ; preds = %147
  %151 = shl nuw nsw i32 %101, 1
  %152 = icmp samesign ult i32 %101, 129
  br i1 %152, label %100, label %153, !llvm.loop !28

153:                                              ; preds = %150, %153
  %154 = phi i64 [ %175, %153 ], [ 0, %150 ]
  %155 = phi i64 [ %174, %153 ], [ %61, %150 ]
  %156 = phi i64 [ %173, %153 ], [ %60, %150 ]
  %157 = getelementptr inbounds nuw [8 x i8], ptr @re, i64 %154
  %158 = load i64, ptr %157, align 8, !tbaa !15
  %159 = sdiv i64 %158, 128
  %160 = getelementptr inbounds nuw [8 x i8], ptr @im, i64 %154
  %161 = load i64, ptr %160, align 8, !tbaa !15
  %162 = sdiv i64 %161, 128
  %163 = mul nsw i64 %159, %159
  %164 = mul nsw i64 %162, %162
  %165 = add nuw nsw i64 %164, %163
  %166 = lshr i64 %165, 8
  %167 = tail call i64 @llvm.umin.i64(i64 %166, i64 65535)
  %168 = trunc nuw i64 %167 to i16
  %169 = getelementptr inbounds nuw [2 x i8], ptr @row, i64 %154
  store i16 %168, ptr %169, align 2, !tbaa !23
  %170 = mul nuw nsw i64 %156, 3
  %171 = add i64 %155, %170
  %172 = add i64 %171, %167
  %173 = and i64 %172, 536870911
  %174 = add nsw i64 %155, 1
  %175 = add nuw nsw i64 %154, 1
  %176 = icmp eq i64 %175, 129
  br i1 %176, label %177, label %153, !llvm.loop !29

177:                                              ; preds = %153
  %178 = add i64 %61, 129
  %179 = tail call i32 @llrm_os_write_file(i32 noundef %52, ptr noundef nonnull @row, i64 noundef 258) #5
  %180 = icmp eq i32 %179, 258
  br i1 %180, label %59, label %184, !llvm.loop !30

181:                                              ; preds = %59
  %182 = tail call i64 @llrm_os_close(i32 noundef %51) #5
  %183 = tail call i64 @llrm_os_close(i32 noundef %52) #5
  br label %184

184:                                              ; preds = %177, %56, %50, %181
  %185 = phi i64 [ %60, %181 ], [ -1, %50 ], [ -1, %56 ], [ -1, %177 ]
  ret i64 %185
}

; Function Attrs: optsize
declare i32 @llrm_os_open(ptr noundef, i8 noundef) local_unnamed_addr #1

; Function Attrs: optsize
declare i32 @llrm_os_create(ptr noundef) local_unnamed_addr #1

; Function Attrs: optsize
declare i32 @llrm_os_read(i32 noundef, ptr noundef, i64 noundef) local_unnamed_addr #1

; Function Attrs: optsize
declare i32 @llrm_os_write_file(i32 noundef, ptr noundef, i64 noundef) local_unnamed_addr #1

; Function Attrs: optsize
declare i64 @llrm_os_close(i32 noundef) local_unnamed_addr #1

; Function Attrs: nounwind optsize
define dso_local noundef i32 @main() local_unnamed_addr #2 {
  %1 = tail call i64 @bench_stft_fixed() #6
  tail call void @report(i64 noundef %1) #5
  ret i32 0
}

; Function Attrs: optsize
declare void @report(i64 noundef) local_unnamed_addr #1

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare i64 @llvm.umin.i64(i64, i64) #3

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare i32 @llvm.cttz.i32(i32, i1 immarg) #4

attributes #0 = { noinline nounwind optsize "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #1 = { optsize "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #2 = { nounwind optsize "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #3 = { nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none) }
attributes #4 = { nocallback nofree nosync nounwind speculatable willreturn memory(none) }
attributes #5 = { nobuiltin nounwind optsize "no-builtins" }
attributes #6 = { nobuiltin optsize "no-builtins" }

!llvm.module.flags = !{!0, !1, !2, !3, !4, !5, !6, !7, !8}
!llvm.ident = !{!9}
!llvm.errno.tbaa = !{!10}

!0 = !{i32 1, !"ptrauth-elf-got", i32 0}
!1 = !{i32 1, !"ptrauth-init-fini", i32 0}
!2 = !{i32 1, !"ptrauth-init-fini-address-discrimination", i32 0}
!3 = !{i32 1, !"ptrauth-sign-personality", i32 0}
!4 = !{i32 1, !"aarch64-elf-pauthabi-platform", i32 268435458}
!5 = !{i32 1, !"aarch64-elf-pauthabi-version", i32 0}
!6 = !{i32 8, !"PIC Level", i32 2}
!7 = !{i32 7, !"PIE Level", i32 2}
!8 = !{i32 7, !"frame-pointer", i32 4}
!9 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!10 = !{!11, !12, i64 0}
!11 = !{!"__libc_errno", !12, i64 0}
!12 = !{!"int", !13, i64 0}
!13 = !{!"omnipotent char", !14, i64 0}
!14 = !{!"Simple C/C++ TBAA"}
!15 = !{!16, !16, i64 0}
!16 = !{!"long", !13, i64 0}
!17 = distinct !{!17, !18}
!18 = !{!"llvm.loop.mustprogress"}
!19 = distinct !{!19, !18}
!20 = distinct !{!20, !18}
!21 = !{!13, !13, i64 0}
!22 = distinct !{!22, !18}
!23 = !{!24, !24, i64 0}
!24 = !{!"short", !13, i64 0}
!25 = distinct !{!25, !18}
!26 = distinct !{!26, !18}
!27 = distinct !{!27, !18}
!28 = distinct !{!28, !18}
!29 = distinct !{!29, !18}
!30 = distinct !{!30, !18}
