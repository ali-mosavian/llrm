; ModuleID = 'tests/run/c/wide_frame.c'
source_filename = "tests/run/c/wide_frame.c"
target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx27.2.0"

; Function Attrs: nounwind ssp
define i32 @main() #0 {
  call void @frame(float noundef 1.500000e+00) #3
  ret i32 0
}

; Function Attrs: nounwind ssp
define internal void @frame(float noundef %0) #0 {
  %2 = alloca float, align 4
  %3 = alloca float, align 4
  %4 = alloca [16400 x i32], align 4
  %5 = alloca i32, align 4
  %6 = alloca i64, align 8
  store float %0, ptr %2, align 4, !tbaa !9
  call void @llvm.lifetime.start.p0(ptr %3) #4
  call void @llvm.lifetime.start.p0(ptr %4) #4
  call void @llvm.lifetime.start.p0(ptr %5) #4
  call void @llvm.lifetime.start.p0(ptr %6) #4
  store i64 0, ptr %6, align 8, !tbaa !11
  %7 = getelementptr inbounds [16400 x i32], ptr %4, i64 0, i64 0
  store i32 7, ptr %7, align 4, !tbaa !13
  store i32 16380, ptr %5, align 4, !tbaa !13
  br label %8

8:                                                ; preds = %17, %1
  %9 = load i32, ptr %5, align 4, !tbaa !13
  %10 = icmp slt i32 %9, 16400
  br i1 %10, label %11, label %20

11:                                               ; preds = %8
  %12 = load i32, ptr %5, align 4, !tbaa !13
  %13 = sub nsw i32 %12, 16370
  %14 = load i32, ptr %5, align 4, !tbaa !13
  %15 = sext i32 %14 to i64
  %16 = getelementptr inbounds [16400 x i32], ptr %4, i64 0, i64 %15
  store i32 %13, ptr %16, align 4, !tbaa !13
  br label %17

17:                                               ; preds = %11
  %18 = load i32, ptr %5, align 4, !tbaa !13
  %19 = add nsw i32 %18, 1
  store i32 %19, ptr %5, align 4, !tbaa !13
  br label %8, !llvm.loop !14

20:                                               ; preds = %8
  %21 = load float, ptr %2, align 4, !tbaa !9
  %22 = getelementptr inbounds [16400 x i32], ptr %4, i64 0, i64 0
  %23 = load i32, ptr %22, align 4, !tbaa !13
  %24 = sitofp i32 %23 to float
  %25 = fadd float %21, %24
  %26 = getelementptr inbounds [16400 x i32], ptr %4, i64 0, i64 16399
  %27 = load i32, ptr %26, align 4, !tbaa !13
  %28 = sitofp i32 %27 to float
  %29 = fadd float %25, %28
  store float %29, ptr %3, align 4, !tbaa !9
  %30 = load float, ptr %3, align 4, !tbaa !9
  call void @note(float noundef %30) #3
  store i32 16380, ptr %5, align 4, !tbaa !13
  br label %31

31:                                               ; preds = %42, %20
  %32 = load i32, ptr %5, align 4, !tbaa !13
  %33 = icmp slt i32 %32, 16400
  br i1 %33, label %34, label %45

34:                                               ; preds = %31
  %35 = load i32, ptr %5, align 4, !tbaa !13
  %36 = sext i32 %35 to i64
  %37 = getelementptr inbounds [16400 x i32], ptr %4, i64 0, i64 %36
  %38 = load i32, ptr %37, align 4, !tbaa !13
  %39 = sext i32 %38 to i64
  %40 = load i64, ptr %6, align 8, !tbaa !11
  %41 = add nsw i64 %40, %39
  store i64 %41, ptr %6, align 8, !tbaa !11
  br label %42

42:                                               ; preds = %34
  %43 = load i32, ptr %5, align 4, !tbaa !13
  %44 = add nsw i32 %43, 1
  store i32 %44, ptr %5, align 4, !tbaa !13
  br label %31, !llvm.loop !16

45:                                               ; preds = %31
  %46 = load i64, ptr %6, align 8, !tbaa !11
  call void @report(i64 noundef %46) #3
  call void @llvm.lifetime.end.p0(ptr %6) #4
  call void @llvm.lifetime.end.p0(ptr %5) #4
  call void @llvm.lifetime.end.p0(ptr %4) #4
  call void @llvm.lifetime.end.p0(ptr %3) #4
  ret void
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nounwind ssp
define internal void @note(float noundef %0) #0 {
  %2 = alloca float, align 4
  store float %0, ptr %2, align 4, !tbaa !9
  %3 = load float, ptr %2, align 4, !tbaa !9
  %4 = fmul float %3, 2.000000e+00
  %5 = fptosi float %4 to i64
  call void @report(i64 noundef %5) #3
  ret void
}

declare void @report(i64 noundef) #2

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

attributes #0 = { nounwind ssp "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #3 = { nobuiltin "no-builtins" }
attributes #4 = { nounwind }

!llvm.module.flags = !{!0, !1, !2}
!llvm.ident = !{!3}
!llvm.errno.tbaa = !{!4}

!0 = !{i32 2, !"SDK Version", [2 x i32] [i32 27, i32 0]}
!1 = !{i32 8, !"PIC Level", i32 2}
!2 = !{i32 7, !"frame-pointer", i32 4}
!3 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!4 = !{!5, !6, i64 0}
!5 = !{!"__libc_errno", !6, i64 0}
!6 = !{!"int", !7, i64 0}
!7 = !{!"omnipotent char", !8, i64 0}
!8 = !{!"Simple C/C++ TBAA"}
!9 = !{!10, !10, i64 0}
!10 = !{!"float", !7, i64 0}
!11 = !{!12, !12, i64 0}
!12 = !{!"long", !7, i64 0}
!13 = !{!6, !6, i64 0}
!14 = distinct !{!14, !15}
!15 = !{!"llvm.loop.mustprogress"}
!16 = distinct !{!16, !15}
