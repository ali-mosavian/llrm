; ModuleID = '/home/alim/scratch/w15-bar/out/src/rectwo.c'
source_filename = "/home/alim/scratch/w15-bar/out/src/rectwo.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

@opq0 = internal global i32 0, align 4

; Function Attrs: noinline nounwind
define dso_local i64 @bench_rectwo(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !15
  %3 = load i32, ptr %2, align 4, !tbaa !15
  %4 = load i32, ptr %2, align 4, !tbaa !15
  %5 = call i64 @paths(i32 noundef %3, i32 noundef %4) #3
  ret i64 %5
}

; Function Attrs: nounwind
define internal i64 @paths(i32 noundef %0, i32 noundef %1) #1 {
  %3 = alloca i64, align 8
  %4 = alloca i32, align 4
  %5 = alloca i32, align 4
  store i32 %0, ptr %4, align 4, !tbaa !15
  store i32 %1, ptr %5, align 4, !tbaa !15
  %6 = load i32, ptr %4, align 4, !tbaa !15
  %7 = icmp eq i32 %6, 0
  br i1 %7, label %11, label %8

8:                                                ; preds = %2
  %9 = load i32, ptr %5, align 4, !tbaa !15
  %10 = icmp eq i32 %9, 0
  br i1 %10, label %11, label %12

11:                                               ; preds = %8, %2
  store i64 1, ptr %3, align 8
  br label %22

12:                                               ; preds = %8
  %13 = load i32, ptr %4, align 4, !tbaa !15
  %14 = sub nsw i32 %13, 1
  %15 = load i32, ptr %5, align 4, !tbaa !15
  %16 = call i64 @paths(i32 noundef %14, i32 noundef %15) #3
  %17 = load i32, ptr %4, align 4, !tbaa !15
  %18 = load i32, ptr %5, align 4, !tbaa !15
  %19 = sub nsw i32 %18, 1
  %20 = call i64 @paths(i32 noundef %17, i32 noundef %19) #3
  %21 = add nsw i64 %16, %20
  store i64 %21, ptr %3, align 8
  br label %22

22:                                               ; preds = %12, %11
  %23 = load i64, ptr %3, align 8
  ret i64 %23
}

; Function Attrs: nounwind
define dso_local i32 @main() #1 {
  %1 = load volatile i32, ptr @opq0, align 4, !tbaa !15
  %2 = add nsw i32 9, %1
  %3 = call i64 @bench_rectwo(i32 noundef %2) #3
  call void @report(i64 noundef %3) #3
  ret i32 0
}

declare void @report(i64 noundef) #2

attributes #0 = { noinline nounwind "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #1 = { nounwind "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #2 = { "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #3 = { nobuiltin "no-builtins" }

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
!15 = !{!12, !12, i64 0}
