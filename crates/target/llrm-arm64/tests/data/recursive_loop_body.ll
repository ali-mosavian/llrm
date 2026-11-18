target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

@values = internal global [64 x i32] zeroinitializer, align 4

; Function Attrs: nounwind uwtable
define dso_local i64 @bench_recchop(i32 noundef %n) #0 {
entry:
  %n.addr = alloca i32, align 4
  %i = alloca i32, align 4
  store i32 %n, ptr %n.addr, align 4, !tbaa !6
  call void @llvm.lifetime.start.p0(i64 4, ptr %i) #3
  store i32 0, ptr %i, align 4, !tbaa !6
  br label %for.cond

for.cond:                                         ; preds = %for.inc, %entry
  %0 = load i32, ptr %i, align 4, !tbaa !6
  %cmp = icmp slt i32 %0, 64
  br i1 %cmp, label %for.body, label %for.end

for.body:                                         ; preds = %for.cond
  %1 = load i32, ptr %i, align 4, !tbaa !6
  %mul = mul nsw i32 %1, 37
  %2 = load i32, ptr %n.addr, align 4, !tbaa !6
  %add = add nsw i32 %mul, %2
  %and = and i32 %add, 127
  %3 = load i32, ptr %i, align 4, !tbaa !6
  %idxprom = sext i32 %3 to i64
  %arrayidx = getelementptr inbounds [64 x i32], ptr @values, i64 0, i64 %idxprom
  store i32 %and, ptr %arrayidx, align 4, !tbaa !6
  br label %for.inc

for.inc:                                          ; preds = %for.body
  %4 = load i32, ptr %i, align 4, !tbaa !6
  %inc = add nsw i32 %4, 1
  store i32 %inc, ptr %i, align 4, !tbaa !6
  br label %for.cond, !llvm.loop !10

for.end:                                          ; preds = %for.cond
  %call = call i64 @chop(i32 noundef 0, i32 noundef 64)
  call void @llvm.lifetime.end.p0(i64 4, ptr %i) #3
  ret i64 %call
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) #1

; Function Attrs: nounwind uwtable
define internal i64 @chop(i32 noundef %lo, i32 noundef %hi) #0 {
entry:
  %retval = alloca i64, align 8
  %lo.addr = alloca i32, align 4
  %hi.addr = alloca i32, align 4
  %sum = alloca i64, align 8
  %i = alloca i32, align 4
  %mid = alloca i32, align 4
  %cleanup.dest.slot = alloca i32, align 4
  store i32 %lo, ptr %lo.addr, align 4, !tbaa !6
  store i32 %hi, ptr %hi.addr, align 4, !tbaa !6
  call void @llvm.lifetime.start.p0(i64 8, ptr %sum) #3
  store i64 0, ptr %sum, align 8, !tbaa !13
  call void @llvm.lifetime.start.p0(i64 4, ptr %i) #3
  call void @llvm.lifetime.start.p0(i64 4, ptr %mid) #3
  %0 = load i32, ptr %hi.addr, align 4, !tbaa !6
  %1 = load i32, ptr %lo.addr, align 4, !tbaa !6
  %sub = sub nsw i32 %0, %1
  %cmp = icmp slt i32 %sub, 2
  br i1 %cmp, label %if.then, label %if.end

if.then:                                          ; preds = %entry
  store i64 0, ptr %retval, align 8
  store i32 1, ptr %cleanup.dest.slot, align 4
  br label %cleanup

if.end:                                           ; preds = %entry
  %2 = load i32, ptr %lo.addr, align 4, !tbaa !6
  %3 = load i32, ptr %hi.addr, align 4, !tbaa !6
  %add = add nsw i32 %2, %3
  %div = sdiv i32 %add, 2
  store i32 %div, ptr %mid, align 4, !tbaa !6
  %4 = load i32, ptr %lo.addr, align 4, !tbaa !6
  store i32 %4, ptr %i, align 4, !tbaa !6
  br label %for.cond

for.cond:                                         ; preds = %for.inc, %if.end
  %5 = load i32, ptr %i, align 4, !tbaa !6
  %6 = load i32, ptr %hi.addr, align 4, !tbaa !6
  %cmp1 = icmp slt i32 %5, %6
  br i1 %cmp1, label %for.body, label %for.end

for.body:                                         ; preds = %for.cond
  %7 = load i32, ptr %i, align 4, !tbaa !6
  %idxprom = sext i32 %7 to i64
  %arrayidx = getelementptr inbounds [64 x i32], ptr @values, i64 0, i64 %idxprom
  %8 = load i32, ptr %arrayidx, align 4, !tbaa !6
  %9 = load i32, ptr %mid, align 4, !tbaa !6
  %and = and i32 %8, %9
  %conv = sext i32 %and to i64
  %10 = load i64, ptr %sum, align 8, !tbaa !13
  %add2 = add nsw i64 %10, %conv
  store i64 %add2, ptr %sum, align 8, !tbaa !13
  br label %for.inc

for.inc:                                          ; preds = %for.body
  %11 = load i32, ptr %i, align 4, !tbaa !6
  %inc = add nsw i32 %11, 1
  store i32 %inc, ptr %i, align 4, !tbaa !6
  br label %for.cond, !llvm.loop !15

for.end:                                          ; preds = %for.cond
  %12 = load i64, ptr %sum, align 8, !tbaa !13
  %13 = load i32, ptr %lo.addr, align 4, !tbaa !6
  %14 = load i32, ptr %mid, align 4, !tbaa !6
  %call = call i64 @chop(i32 noundef %13, i32 noundef %14)
  %add3 = add nsw i64 %12, %call
  %15 = load i32, ptr %mid, align 4, !tbaa !6
  %16 = load i32, ptr %hi.addr, align 4, !tbaa !6
  %call4 = call i64 @chop(i32 noundef %15, i32 noundef %16)
  %add5 = add nsw i64 %add3, %call4
  store i64 %add5, ptr %retval, align 8
  store i32 1, ptr %cleanup.dest.slot, align 4
  br label %cleanup

cleanup:                                          ; preds = %for.end, %if.then
  call void @llvm.lifetime.end.p0(i64 4, ptr %mid) #3
  call void @llvm.lifetime.end.p0(i64 4, ptr %i) #3
  call void @llvm.lifetime.end.p0(i64 8, ptr %sum) #3
  %17 = load i64, ptr %retval, align 8
  ret i64 %17
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) #1

; Function Attrs: nounwind uwtable
define dso_local i32 @main() #0 {
entry:
  %retval = alloca i32, align 4
  store i32 0, ptr %retval, align 4
  %call = call i64 @bench_recchop(i32 noundef 5)
  call void @report(i64 noundef %call)
  ret i32 0
}

declare void @report(i64 noundef) #2

attributes #0 = { nounwind uwtable "frame-pointer"="non-leaf" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { "frame-pointer"="non-leaf" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #3 = { nounwind }

!llvm.module.flags = !{!0, !1, !2, !3, !4}
!llvm.ident = !{!5}

!0 = !{i32 1, !"wchar_size", i32 4}
!1 = !{i32 8, !"PIC Level", i32 2}
!2 = !{i32 7, !"PIE Level", i32 2}
!3 = !{i32 7, !"uwtable", i32 2}
!4 = !{i32 7, !"frame-pointer", i32 1}
!5 = !{!"Ubuntu clang version 20.1.8 (0ubuntu4)"}
!6 = !{!7, !7, i64 0}
!7 = !{!"int", !8, i64 0}
!8 = !{!"omnipotent char", !9, i64 0}
!9 = !{!"Simple C/C++ TBAA"}
!10 = distinct !{!10, !11, !12}
!11 = !{!"llvm.loop.mustprogress"}
!12 = !{!"llvm.loop.unroll.disable"}
!13 = !{!14, !14, i64 0}
!14 = !{!"long", !8, i64 0}
!15 = distinct !{!15, !11, !12}
