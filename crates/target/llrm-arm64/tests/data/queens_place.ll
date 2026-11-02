; ModuleID = 'bench/queens/queens.c'
source_filename = "bench/queens/queens.c"
target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx10.4.0"

; Function Attrs: nofree nosync nounwind ssp memory(none) uwtable(sync)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1
declare void @report(i64 noundef) local_unnamed_addr #4
define internal fastcc i32 @place(ptr nofree noundef nonnull captures(none) %0, i32 noundef %1, i32 noundef %2) unnamed_addr #2 {
  %4 = icmp eq i32 %1, %2
  br i1 %4, label %40, label %5

5:                                                ; preds = %3
  %6 = icmp sgt i32 %2, 0
  br i1 %6, label %7, label %40

7:                                                ; preds = %5
  %8 = icmp sgt i32 %1, 0
  %9 = zext nneg i32 %1 to i64
  %10 = sext i32 %1 to i64
  %11 = getelementptr inbounds [4 x i8], ptr %0, i64 %10
  %12 = add nsw i32 %1, 1
  br label %13

13:                                               ; preds = %7, %36
  %14 = phi i32 [ 0, %7 ], [ %37, %36 ]
  %15 = phi i32 [ 0, %7 ], [ %38, %36 ]
  br i1 %8, label %19, label %33

16:                                               ; preds = %19
  %17 = add nuw nsw i64 %20, 1
  %18 = icmp eq i64 %17, %9
  br i1 %18, label %33, label %19, !llvm.loop !9

19:                                               ; preds = %13, %16
  %20 = phi i64 [ %17, %16 ], [ 0, %13 ]
  %21 = getelementptr inbounds nuw [4 x i8], ptr %0, i64 %20
  %22 = load i32, ptr %21, align 4, !tbaa !11
  %23 = sub nsw i32 %22, %15
  %24 = icmp eq i32 %22, %15
  %25 = sub nuw nsw i64 %9, %20
  %26 = zext i32 %23 to i64
  %27 = icmp eq i64 %25, %26
  %28 = trunc i64 %20 to i32
  %29 = sub i32 %28, %1
  %30 = icmp eq i32 %23, %29
  %31 = or i1 %30, %27
  %32 = select i1 %24, i1 true, i1 %31
  br i1 %32, label %36, label %16

33:                                               ; preds = %16, %13
  store i32 %15, ptr %11, align 4, !tbaa !11
  %34 = tail call fastcc i32 @place(ptr noundef %0, i32 noundef %12, i32 noundef %2)
  %35 = add nsw i32 %34, %14
  br label %36

36:                                               ; preds = %19, %33
  %37 = phi i32 [ %35, %33 ], [ %14, %19 ]
  %38 = add nuw nsw i32 %15, 1
  %39 = icmp eq i32 %38, %2
  br i1 %39, label %40, label %13, !llvm.loop !12

40:                                               ; preds = %36, %5, %3
  %41 = phi i32 [ 1, %3 ], [ 0, %5 ], [ %37, %36 ]
  ret i32 %41
}

attributes #0 = { nofree nosync nounwind ssp memory(none) uwtable(sync) "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #1 = { mustprogress nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nofree nosync nounwind ssp memory(argmem: readwrite) uwtable(sync) "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #3 = { nounwind ssp uwtable(sync) "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #4 = { "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #5 = { nounwind }

!llvm.module.flags = !{!0, !1, !2}
!llvm.ident = !{!3}
!llvm.errno.tbaa = !{!4}

!0 = !{i32 8, !"PIC Level", i32 2}
!1 = !{i32 7, !"uwtable", i32 1}
!2 = !{i32 7, !"frame-pointer", i32 4}
!3 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!4 = !{!5, !6, i64 0}
!5 = !{!"__libc_errno", !6, i64 0}
!6 = !{!"int", !7, i64 0}
!7 = !{!"omnipotent char", !8, i64 0}
!8 = !{!"Simple C/C++ TBAA"}
!9 = distinct !{!9, !10}
!10 = !{!"llvm.loop.mustprogress"}
!11 = !{!6, !6, i64 0}
!12 = distinct !{!12, !10}
