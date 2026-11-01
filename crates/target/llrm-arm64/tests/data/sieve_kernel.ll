; ModuleID = 'bench/sieve/sieve.c'
source_filename = "bench/sieve/sieve.c"
target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx10.4.0"

; Function Attrs: nofree noinline norecurse nosync nounwind ssp memory(none) uwtable(sync)
define i64 @bench_sieve(i16 noundef zeroext %0) local_unnamed_addr #0 {
  %2 = alloca [1024 x i8], align 1
  call void @llvm.lifetime.start.p0(ptr nonnull %2) #5
  %3 = icmp eq i16 %0, 0
  br i1 %3, label %41, label %4

4:                                                ; preds = %1
  %5 = zext i16 %0 to i64
  call void @llvm.memset.p0.i64(ptr nonnull align 1 %2, i8 0, i64 %5, i1 false), !tbaa !9
  %6 = icmp ugt i16 %0, 2
  br i1 %6, label %7, label %41

7:                                                ; preds = %4
  %8 = zext i16 %0 to i64
  br label %9

9:                                                ; preds = %7, %32
  %10 = phi i64 [ 2, %7 ], [ %35, %32 ]
  %11 = phi i64 [ 0, %7 ], [ %34, %32 ]
  %12 = phi i16 [ 0, %7 ], [ %33, %32 ]
  %13 = getelementptr inbounds nuw i8, ptr %2, i64 %10
  %14 = load i8, ptr %13, align 1, !tbaa !9
  %15 = icmp eq i8 %14, 0
  br i1 %15, label %16, label %32

16:                                               ; preds = %9
  %17 = add i16 %12, 1
  %18 = add i64 %11, %10
  %19 = icmp samesign ult i64 %10, 32
  %20 = mul nuw nsw i64 %10, %10
  %21 = icmp samesign ult i64 %20, %8
  %22 = select i1 %19, i1 %21, i1 false
  br i1 %22, label %23, label %32

23:                                               ; preds = %16
  %24 = trunc nuw i64 %20 to i16
  %25 = trunc nuw nsw i64 %10 to i16
  br label %26

26:                                               ; preds = %23, %26
  %27 = phi i16 [ %30, %26 ], [ %24, %23 ]
  %28 = zext i16 %27 to i64
  %29 = getelementptr inbounds nuw i8, ptr %2, i64 %28
  store i8 1, ptr %29, align 1, !tbaa !9
  %30 = add i16 %27, %25
  %31 = icmp ult i16 %30, %0
  br i1 %31, label %26, label %32, !llvm.loop !10

32:                                               ; preds = %26, %16, %9
  %33 = phi i16 [ %12, %9 ], [ %17, %16 ], [ %17, %26 ]
  %34 = phi i64 [ %11, %9 ], [ %18, %16 ], [ %18, %26 ]
  %35 = add nuw nsw i64 %10, 1
  %36 = icmp eq i64 %35, %8
  br i1 %36, label %37, label %9, !llvm.loop !12

37:                                               ; preds = %32
  %38 = zext i16 %33 to i64
  %39 = shl nuw nsw i64 %38, 16
  %40 = xor i64 %39, %34
  br label %41

41:                                               ; preds = %1, %37, %4
  %42 = phi i64 [ 0, %4 ], [ %40, %37 ], [ 0, %1 ]
  call void @llvm.lifetime.end.p0(ptr nonnull %2) #5
  ret i64 %42
}

; Function Attrs: mustprogress nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: mustprogress nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: noinline nounwind ssp uwtable(sync)
define noundef i32 @main() local_unnamed_addr #2 {
  %1 = tail call i64 @bench_sieve(i16 noundef zeroext 1024)
  tail call void @report(i64 noundef %1) #5
  ret i32 0
}

declare void @report(i64 noundef) local_unnamed_addr #3

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: write)
declare void @llvm.memset.p0.i64(ptr writeonly captures(none), i8, i64, i1 immarg) #4

attributes #0 = { nofree noinline norecurse nosync nounwind ssp memory(none) uwtable(sync) "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #1 = { mustprogress nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { noinline nounwind ssp uwtable(sync) "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #3 = { "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #4 = { nocallback nofree nosync nounwind willreturn memory(argmem: write) }
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
!9 = !{!7, !7, i64 0}
!10 = distinct !{!10, !11}
!11 = !{!"llvm.loop.mustprogress"}
!12 = distinct !{!12, !11}
