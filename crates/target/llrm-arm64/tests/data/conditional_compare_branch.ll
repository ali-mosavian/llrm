; ModuleID = '/tmp/cb.c'
source_filename = "/tmp/cb.c"
target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx27.2.0"

@view_left = global i32 0, align 4
@view_top = global i32 0, align 4
@view_right = global i32 0, align 4
@view_bottom = global i32 0, align 4

; Function Attrs: nounwind ssp uwtable(sync)
define void @plot(i32 noundef %0, i32 noundef %1, i8 noundef zeroext %2, i8 noundef zeroext %3) #0 {
  %5 = alloca i32, align 4
  %6 = alloca i32, align 4
  %7 = alloca i8, align 1
  %8 = alloca i8, align 1
  store i32 %0, ptr %5, align 4, !tbaa !9
  store i32 %1, ptr %6, align 4, !tbaa !9
  store i8 %2, ptr %7, align 1, !tbaa !10
  store i8 %3, ptr %8, align 1, !tbaa !10
  %9 = load i32, ptr %5, align 4, !tbaa !9
  %10 = load i32, ptr @view_left, align 4, !tbaa !9
  %11 = icmp sge i32 %9, %10
  br i1 %11, label %12, label %29

12:                                               ; preds = %4
  %13 = load i32, ptr %6, align 4, !tbaa !9
  %14 = load i32, ptr @view_top, align 4, !tbaa !9
  %15 = icmp sge i32 %13, %14
  br i1 %15, label %16, label %29

16:                                               ; preds = %12
  %17 = load i32, ptr %5, align 4, !tbaa !9
  %18 = load i32, ptr @view_right, align 4, !tbaa !9
  %19 = icmp sle i32 %17, %18
  br i1 %19, label %20, label %29

20:                                               ; preds = %16
  %21 = load i32, ptr %6, align 4, !tbaa !9
  %22 = load i32, ptr @view_bottom, align 4, !tbaa !9
  %23 = icmp sle i32 %21, %22
  br i1 %23, label %24, label %29

24:                                               ; preds = %20
  %25 = load i32, ptr %5, align 4, !tbaa !9
  %26 = load i32, ptr %6, align 4, !tbaa !9
  %27 = load i8, ptr %7, align 1, !tbaa !10
  %28 = load i8, ptr %8, align 1, !tbaa !10
  call void @draw(i32 noundef %25, i32 noundef %26, i8 noundef zeroext %27, i8 noundef zeroext %28)
  br label %29

29:                                               ; preds = %24, %20, %16, %12, %4
  ret void
}

declare void @draw(i32 noundef, i32 noundef, i8 noundef zeroext, i8 noundef zeroext) #1

attributes #0 = { nounwind ssp uwtable(sync) "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #1 = { "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }

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
!9 = !{!6, !6, i64 0}
!10 = !{!7, !7, i64 0}
