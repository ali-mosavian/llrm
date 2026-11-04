; ModuleID = 't3.c'
source_filename = "t3.c"
target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx27.2.0"

; Function Attrs: nounwind ssp uwtable(sync)
define double @g(ptr noundef %0, ptr noundef %1, ptr noundef %2, ptr noundef %3, i64 noundef %4) #0 {
  %6 = alloca ptr, align 8
  %7 = alloca ptr, align 8
  %8 = alloca ptr, align 8
  %9 = alloca ptr, align 8
  %10 = alloca i64, align 8
  %11 = alloca i8, align 1
  store ptr %0, ptr %6, align 8, !tbaa !9
  store ptr %1, ptr %7, align 8, !tbaa !13
  store ptr %2, ptr %8, align 8, !tbaa !15
  store ptr %3, ptr %9, align 8, !tbaa !16
  store i64 %4, ptr %10, align 8, !tbaa !18
  call void @llvm.lifetime.start.p0(ptr %11) #3
  %12 = load ptr, ptr %7, align 8, !tbaa !13
  %13 = load ptr, ptr %8, align 8, !tbaa !15
  %14 = call ptr @decimal(ptr noundef %12, ptr noundef %13)
  store ptr %14, ptr %7, align 8, !tbaa !13
  %15 = load ptr, ptr %7, align 8, !tbaa !13
  %16 = load i8, ptr %15, align 1, !tbaa !20
  store i8 %16, ptr %11, align 1, !tbaa !20
  %17 = load i8, ptr %11, align 1, !tbaa !20
  %18 = sext i8 %17 to i32
  %19 = icmp eq i32 %18, 37
  br i1 %19, label %32, label %20

20:                                               ; preds = %5
  %21 = load i8, ptr %11, align 1, !tbaa !20
  %22 = sext i8 %21 to i32
  %23 = icmp eq i32 %22, 38
  br i1 %23, label %32, label %24

24:                                               ; preds = %20
  %25 = load i8, ptr %11, align 1, !tbaa !20
  %26 = sext i8 %25 to i32
  %27 = icmp eq i32 %26, 33
  br i1 %27, label %32, label %28

28:                                               ; preds = %24
  %29 = load i8, ptr %11, align 1, !tbaa !20
  %30 = sext i8 %29 to i32
  %31 = icmp eq i32 %30, 35
  br i1 %31, label %32, label %35

32:                                               ; preds = %28, %24, %20, %5
  %33 = load ptr, ptr %7, align 8, !tbaa !13
  %34 = getelementptr inbounds nuw i8, ptr %33, i32 1
  store ptr %34, ptr %7, align 8, !tbaa !13
  br label %35

35:                                               ; preds = %32, %28
  %36 = load ptr, ptr %7, align 8, !tbaa !13
  %37 = load ptr, ptr %6, align 8, !tbaa !9
  store ptr %36, ptr %37, align 8, !tbaa !13
  %38 = load i8, ptr %11, align 1, !tbaa !20
  %39 = sext i8 %38 to i32
  %40 = icmp eq i32 %39, 37
  br i1 %40, label %41, label %49

41:                                               ; preds = %35
  %42 = load i64, ptr %10, align 8, !tbaa !18
  %43 = icmp slt i64 %42, -32768
  br i1 %43, label %47, label %44

44:                                               ; preds = %41
  %45 = load i64, ptr %10, align 8, !tbaa !18
  %46 = icmp sgt i64 %45, 32767
  br i1 %46, label %47, label %48

47:                                               ; preds = %44, %41
  call void @fail(i32 noundef 13)
  br label %48

48:                                               ; preds = %47, %44
  br label %49

49:                                               ; preds = %48, %35
  %50 = load i8, ptr %11, align 1, !tbaa !20
  %51 = sext i8 %50 to i32
  %52 = load ptr, ptr %9, align 8, !tbaa !16
  store i32 %51, ptr %52, align 4, !tbaa !21
  %53 = load ptr, ptr %8, align 8, !tbaa !15
  %54 = call double @to_real(ptr noundef %53)
  call void @llvm.lifetime.end.p0(ptr %11) #3
  ret double %54
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

declare ptr @decimal(ptr noundef, ptr noundef) #2

declare void @fail(i32 noundef) #2

declare double @to_real(ptr noundef) #2

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

attributes #0 = { nounwind ssp uwtable(sync) "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
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
!9 = !{!10, !10, i64 0}
!10 = !{!"p2 omnipotent char", !11, i64 0}
!11 = !{!"any p2 pointer", !12, i64 0}
!12 = !{!"any pointer", !7, i64 0}
!13 = !{!14, !14, i64 0}
!14 = !{!"p1 omnipotent char", !12, i64 0}
!15 = !{!12, !12, i64 0}
!16 = !{!17, !17, i64 0}
!17 = !{!"p1 int", !12, i64 0}
!18 = !{!19, !19, i64 0}
!19 = !{!"long", !7, i64 0}
!20 = !{!7, !7, i64 0}
!21 = !{!6, !6, i64 0}
