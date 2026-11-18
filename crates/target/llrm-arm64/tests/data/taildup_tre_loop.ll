; ModuleID = 'fibraw.c'
source_filename = "fibraw.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

; Function Attrs: nounwind uwtable
define dso_local i64 @fib(i32 noundef %0) #0 {
  %2 = alloca i64, align 8
  %3 = alloca i32, align 4
  %4 = alloca i64, align 8
  %5 = alloca i64, align 8
  %6 = alloca i32, align 4
  store i32 %0, ptr %3, align 4, !tbaa !16
  call void @llvm.lifetime.start.p0(ptr %4) #2
  call void @llvm.lifetime.start.p0(ptr %5) #2
  %7 = load i32, ptr %3, align 4, !tbaa !16
  %8 = icmp slt i32 %7, 2
  br i1 %8, label %9, label %12

9:                                                ; preds = %1
  %10 = load i32, ptr %3, align 4, !tbaa !16
  %11 = sext i32 %10 to i64
  store i64 %11, ptr %2, align 8
  store i32 1, ptr %6, align 4
  br label %22

12:                                               ; preds = %1
  %13 = load i32, ptr %3, align 4, !tbaa !16
  %14 = sub nsw i32 %13, 1
  %15 = call i64 @fib(i32 noundef %14) #3
  store i64 %15, ptr %4, align 8, !tbaa !17
  %16 = load i32, ptr %3, align 4, !tbaa !16
  %17 = sub nsw i32 %16, 2
  %18 = call i64 @fib(i32 noundef %17) #3
  store i64 %18, ptr %5, align 8, !tbaa !17
  %19 = load i64, ptr %4, align 8, !tbaa !17
  %20 = load i64, ptr %5, align 8, !tbaa !17
  %21 = add nsw i64 %19, %20
  store i64 %21, ptr %2, align 8
  store i32 1, ptr %6, align 4
  br label %22

22:                                               ; preds = %12, %9
  call void @llvm.lifetime.end.p0(ptr %5) #2
  call void @llvm.lifetime.end.p0(ptr %4) #2
  %23 = load i64, ptr %2, align 8
  ret i64 %23
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

attributes #0 = { nounwind uwtable "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nounwind }
attributes #3 = { nobuiltin "no-builtins" }

!llvm.module.flags = !{!0, !1, !2, !3, !4, !5, !6, !7, !8, !9}
!llvm.ident = !{!10}
!llvm.errno.tbaa = !{!11}

!0 = !{i32 1, !"ptrauth-elf-got", i32 0}
!1 = !{i32 1, !"ptrauth-init-fini", i32 0}
!2 = !{i32 1, !"ptrauth-init-fini-address-discrimination", i32 0}
!3 = !{i32 1, !"ptrauth-sign-personality", i32 0}
!4 = !{i32 1, !"aarch64-elf-pauthabi-platform", i32 268435458}
!5 = !{i32 1, !"aarch64-elf-pauthabi-version", i32 0}
!6 = !{i32 8, !"PIC Level", i32 2}
!7 = !{i32 7, !"PIE Level", i32 2}
!8 = !{i32 7, !"uwtable", i32 2}
!9 = !{i32 7, !"frame-pointer", i32 4}
!10 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!11 = !{!12, !13, i64 0}
!12 = !{!"__libc_errno", !13, i64 0}
!13 = !{!"int", !14, i64 0}
!14 = !{!"omnipotent char", !15, i64 0}
!15 = !{!"Simple C/C++ TBAA"}
!16 = !{!13, !13, i64 0}
!17 = !{!18, !18, i64 0}
!18 = !{!"long", !14, i64 0}
