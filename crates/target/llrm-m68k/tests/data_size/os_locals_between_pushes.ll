target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

define i32 @main() nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" {
  %1 = tail call fastcc i32 @probe(i32 noundef 1) nobuiltin "no-builtins"
  tail call void @report(i32 noundef %1) nobuiltin "no-builtins"
  %2 = tail call fastcc i32 @probe(i32 noundef 2) nobuiltin "no-builtins"
  tail call void @report(i32 noundef %2) nobuiltin "no-builtins"
  ret i32 0
}

declare void @report(i32 noundef) "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000"

define internal fastcc i32 @probe(i32 noundef range(i32 1, 3) %0) noinline nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" nofree nosync norecurse memory(inaccessiblemem: readwrite) willreturn {
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  %7 = alloca i32, align 2
  %8 = alloca i32, align 2
  %9 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(i64 4, ptr nonnull %2) nounwind
  call void @llvm.lifetime.start.p0(i64 4, ptr nonnull %3) nounwind
  call void @llvm.lifetime.start.p0(i64 4, ptr nonnull %4) nounwind
  call void @llvm.lifetime.start.p0(i64 4, ptr nonnull %5) nounwind
  call void @llvm.lifetime.start.p0(i64 4, ptr nonnull %6) nounwind
  call void @llvm.lifetime.start.p0(i64 4, ptr nonnull %7) nounwind
  call void @llvm.lifetime.start.p0(i64 4, ptr nonnull %8) nounwind
  call void @llvm.lifetime.start.p0(i64 4, ptr nonnull %9) nounwind
  store i32 %0, ptr %2, align 2
  %10 = add nuw nsw i32 %0, 1
  store i32 %10, ptr %3, align 2
  %11 = add nuw nsw i32 %0, 2
  store i32 %11, ptr %4, align 2
  %12 = add nuw nsw i32 %0, 3
  store i32 %12, ptr %5, align 2
  %13 = or disjoint i32 %0, 4
  store i32 %13, ptr %6, align 2
  %14 = add nuw nsw i32 %0, 5
  store i32 %14, ptr %7, align 2
  %15 = add nuw nsw i32 %0, 6
  store i32 %15, ptr %8, align 2
  %16 = add nuw nsw i32 %0, 7
  store i32 %16, ptr %9, align 2
  %17 = load volatile i32, ptr %9, align 2, !tbaa !8
  %18 = load volatile i32, ptr %8, align 2, !tbaa !8
  %19 = load volatile i32, ptr %7, align 2, !tbaa !8
  %20 = load volatile i32, ptr %6, align 2, !tbaa !8
  %21 = load volatile i32, ptr %5, align 2, !tbaa !8
  %22 = load volatile i32, ptr %4, align 2, !tbaa !8
  %23 = load volatile i32, ptr %3, align 2, !tbaa !8
  %24 = load volatile i32, ptr %2, align 2, !tbaa !8
  %25 = tail call fastcc i32 @pack(i32 noundef %17, i32 noundef %18, i32 noundef %19, i32 noundef %20, i32 noundef %21, i32 noundef %22, i32 noundef %23, i32 noundef %24) nobuiltin "no-builtins"
  call void @llvm.lifetime.end.p0(i64 4, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 4, ptr nonnull %3) nounwind
  call void @llvm.lifetime.end.p0(i64 4, ptr nonnull %4) nounwind
  call void @llvm.lifetime.end.p0(i64 4, ptr nonnull %5) nounwind
  call void @llvm.lifetime.end.p0(i64 4, ptr nonnull %6) nounwind
  call void @llvm.lifetime.end.p0(i64 4, ptr nonnull %7) nounwind
  call void @llvm.lifetime.end.p0(i64 4, ptr nonnull %8) nounwind
  call void @llvm.lifetime.end.p0(i64 4, ptr nonnull %9) nounwind
  ret i32 %25
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

define internal fastcc i32 @pack(i32 noundef %0, i32 noundef %1, i32 noundef %2, i32 noundef %3, i32 noundef %4, i32 noundef %5, i32 noundef %6, i32 noundef %7) noinline nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" memory(none) willreturn nofree nosync norecurse {
  %9 = mul nsw i32 %0, 10
  %10 = add nsw i32 %9, %1
  %11 = mul nsw i32 %10, 10
  %12 = add nsw i32 %11, %2
  %13 = mul nsw i32 %12, 10
  %14 = add nsw i32 %13, %3
  %15 = mul nsw i32 %14, 10
  %16 = add nsw i32 %15, %4
  %17 = mul nsw i32 %16, 10
  %18 = add nsw i32 %17, %5
  %19 = mul nsw i32 %18, 100
  %20 = mul nsw i32 %6, 10
  %21 = add i32 %7, %20
  %22 = add i32 %21, %19
  ret i32 %22
}

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

!llvm.module.flags = !{!0, !1}
!llvm.ident = !{!2}
!llvm.errno.tbaa = !{!3}

!0 = !{i32 8, !"PIC Level", i32 2}
!1 = !{i32 7, !"PIE Level", i32 2}
!2 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!3 = !{!4, !5, i64 0}
!4 = !{!"__libc_errno", !5, i64 0}
!5 = !{!"int", !6, i64 0}
!6 = !{!"omnipotent char", !7, i64 0}
!7 = !{!"Simple C/C++ TBAA"}
!8 = !{!9, !9, i64 0}
!9 = !{!"long", !6, i64 0}
!10 = !{!11, !11, i64 0}
!11 = !{!"p1 long", !12, i64 0}
!12 = !{!"any pointer", !6, i64 0}
!13 = !{!5, !5, i64 0}
!14 = distinct !{!14, !15}
!15 = !{!"llvm.loop.mustprogress"}
