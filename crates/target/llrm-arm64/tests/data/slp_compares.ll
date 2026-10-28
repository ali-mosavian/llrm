; ModuleID = '/tmp/slp.c'
source_filename = "/tmp/slp.c"
target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx27.2.0"

; Function Attrs: nounwind uwtable(sync)
define range(i32 0, 2) i32 @comspec() local_unnamed_addr #0 {
  %1 = alloca [128 x i8], align 1
  call void @llvm.lifetime.start.p0(ptr nonnull %1) #3
  %2 = call i64 @env(i32 noundef 0, ptr noundef nonnull %1, i32 noundef 127) #3
  %3 = icmp sgt i64 %2, 8
  %4 = load i8, ptr %1, align 1
  %5 = icmp eq i8 %4, 67
  %6 = select i1 %3, i1 %5, i1 false
  %7 = getelementptr inbounds nuw i8, ptr %1, i64 1
  %8 = load i8, ptr %7, align 1
  %9 = icmp eq i8 %8, 79
  %10 = select i1 %6, i1 %9, i1 false
  %11 = getelementptr inbounds nuw i8, ptr %1, i64 2
  %12 = load i8, ptr %11, align 1
  %13 = icmp eq i8 %12, 77
  %14 = select i1 %10, i1 %13, i1 false
  %15 = getelementptr inbounds nuw i8, ptr %1, i64 3
  %16 = load i8, ptr %15, align 1
  %17 = icmp eq i8 %16, 83
  %18 = select i1 %14, i1 %17, i1 false
  %19 = getelementptr inbounds nuw i8, ptr %1, i64 4
  %20 = load i8, ptr %19, align 1
  %21 = icmp eq i8 %20, 80
  %22 = select i1 %18, i1 %21, i1 false
  %23 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %24 = load i8, ptr %23, align 1
  %25 = icmp eq i8 %24, 69
  %26 = select i1 %22, i1 %25, i1 false
  %27 = getelementptr inbounds nuw i8, ptr %1, i64 6
  %28 = load i8, ptr %27, align 1
  %29 = icmp eq i8 %28, 67
  %30 = select i1 %26, i1 %29, i1 false
  %31 = getelementptr inbounds nuw i8, ptr %1, i64 7
  %32 = load i8, ptr %31, align 1
  %33 = icmp eq i8 %32, 61
  %34 = select i1 %30, i1 %33, i1 false
  %35 = zext i1 %34 to i32
  call void @llvm.lifetime.end.p0(ptr nonnull %1) #3
  ret i32 %35
}

; Function Attrs: mustprogress nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

declare i64 @env(i32 noundef, ptr noundef, i32 noundef) local_unnamed_addr #2

; Function Attrs: mustprogress nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nounwind uwtable(sync)
define range(i32 0, 2) i32 @four() local_unnamed_addr #0 {
  %1 = alloca [16 x i8], align 1
  call void @llvm.lifetime.start.p0(ptr nonnull %1) #3
  %2 = call i64 @env(i32 noundef 0, ptr noundef nonnull %1, i32 noundef 15) #3
  %3 = load i8, ptr %1, align 1, !tbaa !9
  %4 = icmp eq i8 %3, 65
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 1
  %6 = load i8, ptr %5, align 1
  %7 = icmp eq i8 %6, 66
  %8 = select i1 %4, i1 %7, i1 false
  %9 = getelementptr inbounds nuw i8, ptr %1, i64 2
  %10 = load i8, ptr %9, align 1
  %11 = icmp eq i8 %10, 67
  %12 = select i1 %8, i1 %11, i1 false
  %13 = getelementptr inbounds nuw i8, ptr %1, i64 3
  %14 = load i8, ptr %13, align 1
  %15 = icmp eq i8 %14, 68
  %16 = select i1 %12, i1 %15, i1 false
  %17 = zext i1 %16 to i32
  call void @llvm.lifetime.end.p0(ptr nonnull %1) #3
  ret i32 %17
}

; Function Attrs: nounwind uwtable(sync)
define range(i32 0, 2) i32 @two() local_unnamed_addr #0 {
  %1 = alloca [16 x i8], align 1
  call void @llvm.lifetime.start.p0(ptr nonnull %1) #3
  %2 = call i64 @env(i32 noundef 0, ptr noundef nonnull %1, i32 noundef 15) #3
  %3 = load i8, ptr %1, align 1, !tbaa !9
  %4 = icmp eq i8 %3, 65
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 1
  %6 = load i8, ptr %5, align 1
  %7 = icmp eq i8 %6, 66
  %8 = select i1 %4, i1 %7, i1 false
  %9 = zext i1 %8 to i32
  call void @llvm.lifetime.end.p0(ptr nonnull %1) #3
  ret i32 %9
}

; Function Attrs: nounwind uwtable(sync)
define range(i32 0, 2) i32 @stored_between() local_unnamed_addr #0 {
  %1 = alloca [16 x i8], align 1
  call void @llvm.lifetime.start.p0(ptr nonnull %1) #3
  %2 = call i64 @env(i32 noundef 0, ptr noundef nonnull %1, i32 noundef 15) #3
  %3 = load i8, ptr %1, align 1, !tbaa !9
  %4 = icmp eq i8 %3, 65
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 1
  %6 = load i8, ptr %5, align 1
  %7 = icmp eq i8 %6, 66
  %8 = select i1 %4, i1 %7, i1 false
  %9 = call i64 @env(i32 noundef 1, ptr noundef nonnull %1, i32 noundef 15) #3
  %10 = getelementptr inbounds nuw i8, ptr %1, i64 2
  %11 = load i8, ptr %10, align 1, !tbaa !9
  %12 = icmp eq i8 %11, 67
  %13 = getelementptr inbounds nuw i8, ptr %1, i64 3
  %14 = load i8, ptr %13, align 1
  %15 = icmp eq i8 %14, 68
  %16 = select i1 %12, i1 %15, i1 false
  %17 = and i1 %8, %16
  %18 = zext i1 %17 to i32
  call void @llvm.lifetime.end.p0(ptr nonnull %1) #3
  ret i32 %18
}

; Function Attrs: nounwind uwtable(sync)
define range(i32 0, 2) i32 @under_or() local_unnamed_addr #0 {
  %1 = alloca [16 x i8], align 1
  call void @llvm.lifetime.start.p0(ptr nonnull %1) #3
  %2 = call i64 @env(i32 noundef 0, ptr noundef nonnull %1, i32 noundef 15) #3
  %3 = load i8, ptr %1, align 1, !tbaa !9
  %4 = icmp eq i8 %3, 65
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 1
  %6 = load i8, ptr %5, align 1
  %7 = icmp eq i8 %6, 66
  %8 = select i1 %4, i1 %7, i1 false
  br i1 %8, label %18, label %9

9:                                                ; preds = %0
  %10 = getelementptr inbounds nuw i8, ptr %1, i64 2
  %11 = load i8, ptr %10, align 1, !tbaa !9
  %12 = icmp eq i8 %11, 67
  %13 = getelementptr inbounds nuw i8, ptr %1, i64 3
  %14 = load i8, ptr %13, align 1
  %15 = icmp eq i8 %14, 68
  %16 = select i1 %12, i1 %15, i1 false
  %17 = zext i1 %16 to i32
  br label %18

18:                                               ; preds = %0, %9
  %19 = phi i32 [ 1, %0 ], [ %17, %9 ]
  call void @llvm.lifetime.end.p0(ptr nonnull %1) #3
  ret i32 %19
}

attributes #0 = { nounwind uwtable(sync) "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #1 = { mustprogress nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #3 = { nounwind }

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
