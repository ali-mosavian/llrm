; ModuleID = 'pin2.c'
source_filename = "pin2.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

; Function Attrs: nounwind uwtable
define dso_local i64 @bench_nbody(i16 noundef %steps) #0 {
entry:
  %steps.addr = alloca i16, align 2
  %x = alloca [4 x double], align 8
  %y = alloca [4 x double], align 8
  %vx = alloca [4 x double], align 8
  %vy = alloca [4 x double], align 8
  %step = alloca i16, align 2
  %i = alloca i16, align 2
  %j = alloca i16, align 2
  %dx = alloca double, align 8
  %dy = alloca double, align 8
  %scale = alloca double, align 8
  store i16 %steps, ptr %steps.addr, align 2, !tbaa !16
  call void @llvm.lifetime.start.p0(ptr %x) #3
  call void @llvm.lifetime.start.p0(ptr %y) #3
  call void @llvm.lifetime.start.p0(ptr %vx) #3
  call void @llvm.lifetime.start.p0(ptr %vy) #3
  call void @llvm.lifetime.start.p0(ptr %step) #3
  call void @llvm.lifetime.start.p0(ptr %i) #3
  call void @llvm.lifetime.start.p0(ptr %j) #3
  %arrayidx = getelementptr inbounds [4 x double], ptr %x, i64 0, i64 0
  store double -1.000000e+00, ptr %arrayidx, align 8, !tbaa !18
  %arrayidx1 = getelementptr inbounds [4 x double], ptr %x, i64 0, i64 1
  store double 1.000000e+00, ptr %arrayidx1, align 8, !tbaa !18
  %arrayidx2 = getelementptr inbounds [4 x double], ptr %x, i64 0, i64 2
  store double 0.000000e+00, ptr %arrayidx2, align 8, !tbaa !18
  %arrayidx3 = getelementptr inbounds [4 x double], ptr %x, i64 0, i64 3
  store double 0.000000e+00, ptr %arrayidx3, align 8, !tbaa !18
  %arrayidx4 = getelementptr inbounds [4 x double], ptr %y, i64 0, i64 0
  store double 0.000000e+00, ptr %arrayidx4, align 8, !tbaa !18
  %arrayidx5 = getelementptr inbounds [4 x double], ptr %y, i64 0, i64 1
  store double 0.000000e+00, ptr %arrayidx5, align 8, !tbaa !18
  %arrayidx6 = getelementptr inbounds [4 x double], ptr %y, i64 0, i64 2
  store double -1.000000e+00, ptr %arrayidx6, align 8, !tbaa !18
  %arrayidx7 = getelementptr inbounds [4 x double], ptr %y, i64 0, i64 3
  store double 1.000000e+00, ptr %arrayidx7, align 8, !tbaa !18
  %arrayidx8 = getelementptr inbounds [4 x double], ptr %vx, i64 0, i64 0
  store double 0.000000e+00, ptr %arrayidx8, align 8, !tbaa !18
  %arrayidx9 = getelementptr inbounds [4 x double], ptr %vx, i64 0, i64 1
  store double 0.000000e+00, ptr %arrayidx9, align 8, !tbaa !18
  %arrayidx10 = getelementptr inbounds [4 x double], ptr %vx, i64 0, i64 2
  store double 1.250000e-02, ptr %arrayidx10, align 8, !tbaa !18
  %arrayidx11 = getelementptr inbounds [4 x double], ptr %vx, i64 0, i64 3
  store double -1.250000e-02, ptr %arrayidx11, align 8, !tbaa !18
  %arrayidx12 = getelementptr inbounds [4 x double], ptr %vy, i64 0, i64 0
  store double -1.250000e-02, ptr %arrayidx12, align 8, !tbaa !18
  %arrayidx13 = getelementptr inbounds [4 x double], ptr %vy, i64 0, i64 1
  store double 1.250000e-02, ptr %arrayidx13, align 8, !tbaa !18
  %arrayidx14 = getelementptr inbounds [4 x double], ptr %vy, i64 0, i64 2
  store double 0.000000e+00, ptr %arrayidx14, align 8, !tbaa !18
  %arrayidx15 = getelementptr inbounds [4 x double], ptr %vy, i64 0, i64 3
  store double 0.000000e+00, ptr %arrayidx15, align 8, !tbaa !18
  store i16 0, ptr %step, align 2, !tbaa !16
  br label %for.cond

for.cond:                                         ; preds = %for.inc70, %entry
  %0 = load i16, ptr %step, align 2, !tbaa !16
  %conv = zext i16 %0 to i32
  %1 = load i16, ptr %steps.addr, align 2, !tbaa !16
  %conv16 = zext i16 %1 to i32
  %cmp = icmp slt i32 %conv, %conv16
  br i1 %cmp, label %for.body, label %for.end72

for.body:                                         ; preds = %for.cond
  store i16 0, ptr %i, align 2, !tbaa !16
  br label %for.cond18

for.cond18:                                       ; preds = %for.inc49, %for.body
  %2 = load i16, ptr %i, align 2, !tbaa !16
  %conv19 = zext i16 %2 to i32
  %cmp20 = icmp slt i32 %conv19, 4
  br i1 %cmp20, label %for.body22, label %for.end51

for.body22:                                       ; preds = %for.cond18
  %3 = load i16, ptr %i, align 2, !tbaa !16
  %conv23 = zext i16 %3 to i32
  %add = add nsw i32 %conv23, 1
  %conv24 = trunc i32 %add to i16
  store i16 %conv24, ptr %j, align 2, !tbaa !16
  br label %for.cond25

for.cond25:                                       ; preds = %for.inc, %for.body22
  %4 = load i16, ptr %j, align 2, !tbaa !16
  %conv26 = zext i16 %4 to i32
  %cmp27 = icmp slt i32 %conv26, 4
  br i1 %cmp27, label %for.body29, label %for.end

for.body29:                                       ; preds = %for.cond25
  call void @llvm.lifetime.start.p0(ptr %dx) #3
  %5 = load i16, ptr %j, align 2, !tbaa !16
  %idxprom = zext i16 %5 to i64
  %arrayidx30 = getelementptr inbounds nuw [4 x double], ptr %x, i64 0, i64 %idxprom
  %6 = load double, ptr %arrayidx30, align 8, !tbaa !18
  %7 = load i16, ptr %i, align 2, !tbaa !16
  %idxprom31 = zext i16 %7 to i64
  %arrayidx32 = getelementptr inbounds nuw [4 x double], ptr %x, i64 0, i64 %idxprom31
  %8 = load double, ptr %arrayidx32, align 8, !tbaa !18
  %sub = fsub double %6, %8
  store double %sub, ptr %dx, align 8, !tbaa !18
  call void @llvm.lifetime.start.p0(ptr %dy) #3
  %9 = load i16, ptr %j, align 2, !tbaa !16
  %idxprom33 = zext i16 %9 to i64
  %arrayidx34 = getelementptr inbounds nuw [4 x double], ptr %y, i64 0, i64 %idxprom33
  %10 = load double, ptr %arrayidx34, align 8, !tbaa !18
  %11 = load i16, ptr %i, align 2, !tbaa !16
  %idxprom35 = zext i16 %11 to i64
  %arrayidx36 = getelementptr inbounds nuw [4 x double], ptr %y, i64 0, i64 %idxprom35
  %12 = load double, ptr %arrayidx36, align 8, !tbaa !18
  %sub37 = fsub double %10, %12
  store double %sub37, ptr %dy, align 8, !tbaa !18
  call void @llvm.lifetime.start.p0(ptr %scale) #3
  %13 = load double, ptr %dx, align 8, !tbaa !18
  %14 = load double, ptr %dx, align 8, !tbaa !18
  %15 = load double, ptr %dy, align 8, !tbaa !18
  %16 = load double, ptr %dy, align 8, !tbaa !18
  %mul38 = fmul double %15, %16
  %17 = call double @llvm.fmuladd.f64(double %13, double %14, double %mul38)
  %add39 = fadd double %17, 1.250000e-01
  %div = fdiv double 1.000000e-05, %add39
  store double %div, ptr %scale, align 8, !tbaa !18
  %18 = load double, ptr %dx, align 8, !tbaa !18
  %19 = load double, ptr %scale, align 8, !tbaa !18
  %20 = load i16, ptr %i, align 2, !tbaa !16
  %idxprom40 = zext i16 %20 to i64
  %arrayidx41 = getelementptr inbounds nuw [4 x double], ptr %vx, i64 0, i64 %idxprom40
  %21 = load double, ptr %arrayidx41, align 8, !tbaa !18
  %22 = call double @llvm.fmuladd.f64(double %18, double %19, double %21)
  store double %22, ptr %arrayidx41, align 8, !tbaa !18
  %23 = load double, ptr %dy, align 8, !tbaa !18
  %24 = load double, ptr %scale, align 8, !tbaa !18
  %25 = load i16, ptr %i, align 2, !tbaa !16
  %idxprom42 = zext i16 %25 to i64
  %arrayidx43 = getelementptr inbounds nuw [4 x double], ptr %vy, i64 0, i64 %idxprom42
  %26 = load double, ptr %arrayidx43, align 8, !tbaa !18
  %27 = call double @llvm.fmuladd.f64(double %23, double %24, double %26)
  store double %27, ptr %arrayidx43, align 8, !tbaa !18
  %28 = load double, ptr %dx, align 8, !tbaa !18
  %29 = load double, ptr %scale, align 8, !tbaa !18
  %30 = load i16, ptr %j, align 2, !tbaa !16
  %idxprom44 = zext i16 %30 to i64
  %arrayidx45 = getelementptr inbounds nuw [4 x double], ptr %vx, i64 0, i64 %idxprom44
  %31 = load double, ptr %arrayidx45, align 8, !tbaa !18
  %neg = fneg double %28
  %32 = call double @llvm.fmuladd.f64(double %neg, double %29, double %31)
  store double %32, ptr %arrayidx45, align 8, !tbaa !18
  %33 = load double, ptr %dy, align 8, !tbaa !18
  %34 = load double, ptr %scale, align 8, !tbaa !18
  %35 = load i16, ptr %j, align 2, !tbaa !16
  %idxprom46 = zext i16 %35 to i64
  %arrayidx47 = getelementptr inbounds nuw [4 x double], ptr %vy, i64 0, i64 %idxprom46
  %36 = load double, ptr %arrayidx47, align 8, !tbaa !18
  %neg48 = fneg double %33
  %37 = call double @llvm.fmuladd.f64(double %neg48, double %34, double %36)
  store double %37, ptr %arrayidx47, align 8, !tbaa !18
  call void @llvm.lifetime.end.p0(ptr %scale) #3
  call void @llvm.lifetime.end.p0(ptr %dy) #3
  call void @llvm.lifetime.end.p0(ptr %dx) #3
  br label %for.inc

for.inc:                                          ; preds = %for.body29
  %38 = load i16, ptr %j, align 2, !tbaa !16
  %inc = add i16 %38, 1
  store i16 %inc, ptr %j, align 2, !tbaa !16
  br label %for.cond25, !llvm.loop !20

for.end:                                          ; preds = %for.cond25
  br label %for.inc49

for.inc49:                                        ; preds = %for.end
  %39 = load i16, ptr %i, align 2, !tbaa !16
  %inc50 = add i16 %39, 1
  store i16 %inc50, ptr %i, align 2, !tbaa !16
  br label %for.cond18, !llvm.loop !23

for.end51:                                        ; preds = %for.cond18
  store i16 0, ptr %i, align 2, !tbaa !16
  br label %for.cond52

for.cond52:                                       ; preds = %for.inc67, %for.end51
  %40 = load i16, ptr %i, align 2, !tbaa !16
  %conv53 = zext i16 %40 to i32
  %cmp54 = icmp slt i32 %conv53, 4
  br i1 %cmp54, label %for.body56, label %for.end69

for.body56:                                       ; preds = %for.cond52
  %41 = load i16, ptr %i, align 2, !tbaa !16
  %idxprom57 = zext i16 %41 to i64
  %arrayidx58 = getelementptr inbounds nuw [4 x double], ptr %vx, i64 0, i64 %idxprom57
  %42 = load double, ptr %arrayidx58, align 8, !tbaa !18
  %43 = load i16, ptr %i, align 2, !tbaa !16
  %idxprom59 = zext i16 %43 to i64
  %arrayidx60 = getelementptr inbounds nuw [4 x double], ptr %x, i64 0, i64 %idxprom59
  %44 = load double, ptr %arrayidx60, align 8, !tbaa !18
  %add61 = fadd double %44, %42
  store double %add61, ptr %arrayidx60, align 8, !tbaa !18
  %45 = load i16, ptr %i, align 2, !tbaa !16
  %idxprom62 = zext i16 %45 to i64
  %arrayidx63 = getelementptr inbounds nuw [4 x double], ptr %vy, i64 0, i64 %idxprom62
  %46 = load double, ptr %arrayidx63, align 8, !tbaa !18
  %47 = load i16, ptr %i, align 2, !tbaa !16
  %idxprom64 = zext i16 %47 to i64
  %arrayidx65 = getelementptr inbounds nuw [4 x double], ptr %y, i64 0, i64 %idxprom64
  %48 = load double, ptr %arrayidx65, align 8, !tbaa !18
  %add66 = fadd double %48, %46
  store double %add66, ptr %arrayidx65, align 8, !tbaa !18
  br label %for.inc67

for.inc67:                                        ; preds = %for.body56
  %49 = load i16, ptr %i, align 2, !tbaa !16
  %inc68 = add i16 %49, 1
  store i16 %inc68, ptr %i, align 2, !tbaa !16
  br label %for.cond52, !llvm.loop !24

for.end69:                                        ; preds = %for.cond52
  br label %for.inc70

for.inc70:                                        ; preds = %for.end69
  %50 = load i16, ptr %step, align 2, !tbaa !16
  %inc71 = add i16 %50, 1
  store i16 %inc71, ptr %step, align 2, !tbaa !16
  br label %for.cond, !llvm.loop !25

for.end72:                                        ; preds = %for.cond
  %arrayidx73 = getelementptr inbounds [4 x double], ptr %x, i64 0, i64 0
  %51 = load double, ptr %arrayidx73, align 8, !tbaa !18
  %arrayidx74 = getelementptr inbounds [4 x double], ptr %y, i64 0, i64 1
  %52 = load double, ptr %arrayidx74, align 8, !tbaa !18
  %add75 = fadd double %51, %52
  %arrayidx76 = getelementptr inbounds [4 x double], ptr %x, i64 0, i64 2
  %53 = load double, ptr %arrayidx76, align 8, !tbaa !18
  %add77 = fadd double %add75, %53
  %arrayidx78 = getelementptr inbounds [4 x double], ptr %y, i64 0, i64 3
  %54 = load double, ptr %arrayidx78, align 8, !tbaa !18
  %add79 = fadd double %add77, %54
  %mul = fmul double %add79, 1.000000e+06
  %conv80 = fptosi double %mul to i64
  call void @llvm.lifetime.end.p0(ptr %j) #3
  call void @llvm.lifetime.end.p0(ptr %i) #3
  call void @llvm.lifetime.end.p0(ptr %step) #3
  call void @llvm.lifetime.end.p0(ptr %vy) #3
  call void @llvm.lifetime.end.p0(ptr %vx) #3
  call void @llvm.lifetime.end.p0(ptr %y) #3
  call void @llvm.lifetime.end.p0(ptr %x) #3
  ret i64 %conv80
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare double @llvm.fmuladd.f64(double, double, double) #2

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

attributes #0 = { nounwind uwtable "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none) }
attributes #3 = { nounwind }

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
!16 = !{!17, !17, i64 0}
!17 = !{!"short", !14, i64 0}
!18 = !{!19, !19, i64 0}
!19 = !{!"double", !14, i64 0}
!20 = distinct !{!20, !21, !22}
!21 = !{!"llvm.loop.mustprogress"}
!22 = !{!"llvm.loop.unroll.disable"}
!23 = distinct !{!23, !21, !22}
!24 = distinct !{!24, !21, !22}
!25 = distinct !{!25, !21, !22}
