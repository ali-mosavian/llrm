; ModuleID = '/tmp/rows.c'
source_filename = "/tmp/rows.c"
target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx27.2.0"

@cells = global [25 x [40 x i16]] zeroinitializer, align 2
@cursor = global i32 0, align 4

; Function Attrs: nounwind ssp uwtable(sync)
define void @clear() #0 {
  %1 = alloca i32, align 4
  %2 = alloca i32, align 4
  %3 = alloca i32, align 4
  call void @llvm.lifetime.start.p0(ptr %1) #2
  store i32 0, ptr %1, align 4, !tbaa !9
  br label %4

4:                                                ; preds = %24, %0
  %5 = load i32, ptr %1, align 4, !tbaa !9
  %6 = icmp ult i32 %5, 25
  br i1 %6, label %8, label %7

7:                                                ; preds = %4
  store i32 2, ptr %2, align 4
  call void @llvm.lifetime.end.p0(ptr %1) #2
  br label %27

8:                                                ; preds = %4
  call void @llvm.lifetime.start.p0(ptr %3) #2
  store i32 0, ptr %3, align 4, !tbaa !9
  br label %9

9:                                                ; preds = %20, %8
  %10 = load i32, ptr %3, align 4, !tbaa !9
  %11 = icmp ult i32 %10, 40
  br i1 %11, label %13, label %12

12:                                               ; preds = %9
  store i32 5, ptr %2, align 4
  call void @llvm.lifetime.end.p0(ptr %3) #2
  br label %23

13:                                               ; preds = %9
  %14 = load i32, ptr %1, align 4, !tbaa !9
  %15 = zext i32 %14 to i64
  %16 = getelementptr inbounds nuw [25 x [40 x i16]], ptr @cells, i64 0, i64 %15
  %17 = load i32, ptr %3, align 4, !tbaa !9
  %18 = zext i32 %17 to i64
  %19 = getelementptr inbounds nuw [40 x i16], ptr %16, i64 0, i64 %18
  store i16 1824, ptr %19, align 2, !tbaa !10
  br label %20

20:                                               ; preds = %13
  %21 = load i32, ptr %3, align 4, !tbaa !9
  %22 = add i32 %21, 1
  store i32 %22, ptr %3, align 4, !tbaa !9
  br label %9, !llvm.loop !12

23:                                               ; preds = %12
  br label %24

24:                                               ; preds = %23
  %25 = load i32, ptr %1, align 4, !tbaa !9
  %26 = add i32 %25, 1
  store i32 %26, ptr %1, align 4, !tbaa !9
  br label %4, !llvm.loop !14

27:                                               ; preds = %7
  store i32 0, ptr @cursor, align 4, !tbaa !9
  ret void
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

attributes #0 = { nounwind ssp uwtable(sync) "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nounwind }

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
!10 = !{!11, !11, i64 0}
!11 = !{!"short", !7, i64 0}
!12 = distinct !{!12, !13}
!13 = !{!"llvm.loop.mustprogress"}
!14 = distinct !{!14, !13}
