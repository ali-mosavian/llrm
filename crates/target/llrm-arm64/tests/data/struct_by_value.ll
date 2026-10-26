; ModuleID = 'tests/llc/structs.c'
source_filename = "tests/llc/structs.c"
target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx27.2.0"

%struct.P = type { i32, i32, i64 }

@.str = private unnamed_addr constant [8 x i8] c"%ld %d\0A\00", align 1

; Function Attrs: nounwind ssp
define i32 @main() #0 {
  %1 = alloca %struct.P, align 8
  %2 = alloca %struct.P, align 8
  %3 = call [2 x i64] @mk(i32 noundef 3, i32 noundef 4) #2
  store [2 x i64] %3, ptr %1, align 8
  %4 = call [2 x i64] @mk(i32 noundef 5, i32 noundef 6) #2
  store [2 x i64] %4, ptr %2, align 8
  %5 = call i64 @dot(ptr noundef %1, ptr noundef %2) #2
  %6 = getelementptr inbounds nuw %struct.P, ptr %1, i32 0, i32 1
  %7 = load i32, ptr %6, align 4, !tbaa !9
  %8 = call i32 (ptr, ...) @printf(ptr noundef @.str, i64 noundef %5, i32 noundef %7) #2
  ret i32 0
}

; Function Attrs: nounwind ssp
define internal [2 x i64] @mk(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca %struct.P, align 8
  %4 = alloca i32, align 4
  %5 = alloca i32, align 4
  store i32 %0, ptr %4, align 4, !tbaa !12
  store i32 %1, ptr %5, align 4, !tbaa !12
  %6 = getelementptr inbounds nuw %struct.P, ptr %3, i32 0, i32 0
  %7 = load i32, ptr %4, align 4, !tbaa !12
  store i32 %7, ptr %6, align 8, !tbaa !13
  %8 = getelementptr inbounds nuw %struct.P, ptr %3, i32 0, i32 1
  %9 = load i32, ptr %5, align 4, !tbaa !12
  store i32 %9, ptr %8, align 4, !tbaa !9
  %10 = getelementptr inbounds nuw %struct.P, ptr %3, i32 0, i32 2
  %11 = load i32, ptr %4, align 4, !tbaa !12
  %12 = sext i32 %11 to i64
  %13 = load i32, ptr %5, align 4, !tbaa !12
  %14 = sext i32 %13 to i64
  %15 = mul nsw i64 %12, %14
  store i64 %15, ptr %10, align 8, !tbaa !14
  %16 = load [2 x i64], ptr %3, align 8
  ret [2 x i64] %16
}

declare i32 @printf(ptr noundef, ...) #1

; Function Attrs: nounwind ssp
define internal i64 @dot(ptr noundef %0, ptr noundef %1) #0 {
  %3 = alloca ptr, align 8
  %4 = alloca ptr, align 8
  store ptr %0, ptr %3, align 8, !tbaa !15
  store ptr %1, ptr %4, align 8, !tbaa !15
  %5 = load ptr, ptr %3, align 8, !tbaa !15
  %6 = getelementptr inbounds nuw %struct.P, ptr %5, i32 0, i32 0
  %7 = load i32, ptr %6, align 8, !tbaa !13
  %8 = sext i32 %7 to i64
  %9 = load ptr, ptr %4, align 8, !tbaa !15
  %10 = getelementptr inbounds nuw %struct.P, ptr %9, i32 0, i32 0
  %11 = load i32, ptr %10, align 8, !tbaa !13
  %12 = sext i32 %11 to i64
  %13 = mul nsw i64 %8, %12
  %14 = load ptr, ptr %3, align 8, !tbaa !15
  %15 = getelementptr inbounds nuw %struct.P, ptr %14, i32 0, i32 1
  %16 = load i32, ptr %15, align 4, !tbaa !9
  %17 = sext i32 %16 to i64
  %18 = load ptr, ptr %4, align 8, !tbaa !15
  %19 = getelementptr inbounds nuw %struct.P, ptr %18, i32 0, i32 1
  %20 = load i32, ptr %19, align 4, !tbaa !9
  %21 = sext i32 %20 to i64
  %22 = mul nsw i64 %17, %21
  %23 = add nsw i64 %13, %22
  %24 = load ptr, ptr %3, align 8, !tbaa !15
  %25 = getelementptr inbounds nuw %struct.P, ptr %24, i32 0, i32 2
  %26 = load i64, ptr %25, align 8, !tbaa !14
  %27 = load ptr, ptr %4, align 8, !tbaa !15
  %28 = getelementptr inbounds nuw %struct.P, ptr %27, i32 0, i32 2
  %29 = load i64, ptr %28, align 8, !tbaa !14
  %30 = mul nsw i64 %26, %29
  %31 = add nsw i64 %23, %30
  ret i64 %31
}

attributes #0 = { nounwind ssp "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #1 = { "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #2 = { nobuiltin "no-builtins" }

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
!9 = !{!10, !6, i64 4}
!10 = !{!"P", !6, i64 0, !6, i64 4, !11, i64 8}
!11 = !{!"long", !7, i64 0}
!12 = !{!6, !6, i64 0}
!13 = !{!10, !6, i64 0}
!14 = !{!10, !11, i64 8}
!15 = !{!16, !16, i64 0}
!16 = !{!"p1 _ZTS1P", !17, i64 0}
!17 = !{!"any pointer", !7, i64 0}
