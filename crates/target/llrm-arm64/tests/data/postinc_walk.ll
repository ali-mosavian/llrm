; ModuleID = 'pit.c'
source_filename = "pit.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

@a = dso_local global [64 x [64 x i16]] zeroinitializer, align 2

; Function Attrs: nounwind uwtable
define dso_local i64 @walk(i32 noundef %y) #0 {
entry:
  %y.addr = alloca i32, align 4
  %t = alloca i64, align 8
  %x = alloca i32, align 4
  store i32 %y, ptr %y.addr, align 4, !tbaa !16
  call void @llvm.lifetime.start.p0(ptr %t) #2
  store i64 0, ptr %t, align 8, !tbaa !17
  call void @llvm.lifetime.start.p0(ptr %x) #2
  store i32 0, ptr %x, align 4, !tbaa !16
  br label %for.cond

for.cond:                                         ; preds = %for.inc, %entry
  %0 = load i32, ptr %x, align 4, !tbaa !16
  %cmp = icmp slt i32 %0, 40
  br i1 %cmp, label %for.body, label %for.cond.cleanup

for.cond.cleanup:                                 ; preds = %for.cond
  call void @llvm.lifetime.end.p0(ptr %x) #2
  br label %for.end

for.body:                                         ; preds = %for.cond
  %1 = load i32, ptr %y.addr, align 4, !tbaa !16
  %and = and i32 %1, 63
  %idxprom = sext i32 %and to i64
  %arrayidx = getelementptr inbounds [64 x [64 x i16]], ptr @a, i64 0, i64 %idxprom
  %2 = load i32, ptr %x, align 4, !tbaa !16
  %add = add nsw i32 %2, 7
  %and1 = and i32 %add, 63
  %idxprom2 = sext i32 %and1 to i64
  %arrayidx3 = getelementptr inbounds [64 x i16], ptr %arrayidx, i64 0, i64 %idxprom2
  %3 = load i16, ptr %arrayidx3, align 2, !tbaa !19
  %conv = sext i16 %3 to i64
  %4 = load i64, ptr %t, align 8, !tbaa !17
  %add4 = add nsw i64 %4, %conv
  store i64 %add4, ptr %t, align 8, !tbaa !17
  br label %for.inc

for.inc:                                          ; preds = %for.body
  %5 = load i32, ptr %x, align 4, !tbaa !16
  %inc = add nsw i32 %5, 1
  store i32 %inc, ptr %x, align 4, !tbaa !16
  br label %for.cond, !llvm.loop !21

for.end:                                          ; preds = %for.cond.cleanup
  %6 = load i64, ptr %t, align 8, !tbaa !17
  call void @llvm.lifetime.end.p0(ptr %t) #2
  ret i64 %6
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

attributes #0 = { nounwind uwtable "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nounwind }

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
!19 = !{!20, !20, i64 0}
!20 = !{!"short", !14, i64 0}
!21 = distinct !{!21, !22, !23}
!22 = !{!"llvm.loop.mustprogress"}
!23 = !{!"llvm.loop.unroll.disable"}
