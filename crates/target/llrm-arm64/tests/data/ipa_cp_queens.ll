target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

; Function Attrs: nounwind uwtable
define dso_local i64 @bench_queens(i32 noundef %n) #0 {
entry:
  %n.addr = alloca i32, align 4
  %q = alloca [12 x i32], align 4
  store i32 %n, ptr %n.addr, align 4, !tbaa !6
  call void @llvm.lifetime.start.p0(i64 48, ptr %q) #3
  %arraydecay = getelementptr inbounds [12 x i32], ptr %q, i64 0, i64 0
  %0 = load i32, ptr %n.addr, align 4, !tbaa !6
  %call = call i32 @place(ptr noundef %arraydecay, i32 noundef 0, i32 noundef %0)
  %conv = sext i32 %call to i64
  call void @llvm.lifetime.end.p0(i64 48, ptr %q) #3
  ret i64 %conv
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) #1

; Function Attrs: nounwind uwtable
define internal i32 @place(ptr noundef %q, i32 noundef %row, i32 noundef %n) #0 {
entry:
  %retval = alloca i32, align 4
  %q.addr = alloca ptr, align 8
  %row.addr = alloca i32, align 4
  %n.addr = alloca i32, align 4
  %col = alloca i32, align 4
  %found = alloca i32, align 4
  %cleanup.dest.slot = alloca i32, align 4
  store ptr %q, ptr %q.addr, align 8, !tbaa !10
  store i32 %row, ptr %row.addr, align 4, !tbaa !6
  store i32 %n, ptr %n.addr, align 4, !tbaa !6
  call void @llvm.lifetime.start.p0(i64 4, ptr %col) #3
  call void @llvm.lifetime.start.p0(i64 4, ptr %found) #3
  store i32 0, ptr %found, align 4, !tbaa !6
  %0 = load i32, ptr %row.addr, align 4, !tbaa !6
  %1 = load i32, ptr %n.addr, align 4, !tbaa !6
  %cmp = icmp eq i32 %0, %1
  br i1 %cmp, label %if.then, label %if.end

if.then:                                          ; preds = %entry
  store i32 1, ptr %retval, align 4
  store i32 1, ptr %cleanup.dest.slot, align 4
  br label %cleanup

if.end:                                           ; preds = %entry
  store i32 0, ptr %col, align 4, !tbaa !6
  br label %for.cond

for.cond:                                         ; preds = %for.inc, %if.end
  %2 = load i32, ptr %col, align 4, !tbaa !6
  %3 = load i32, ptr %n.addr, align 4, !tbaa !6
  %cmp1 = icmp slt i32 %2, %3
  br i1 %cmp1, label %for.body, label %for.end

for.body:                                         ; preds = %for.cond
  %4 = load ptr, ptr %q.addr, align 8, !tbaa !10
  %5 = load i32, ptr %row.addr, align 4, !tbaa !6
  %6 = load i32, ptr %col, align 4, !tbaa !6
  %call = call i32 @safe(ptr noundef %4, i32 noundef %5, i32 noundef %6)
  %tobool = icmp ne i32 %call, 0
  br i1 %tobool, label %if.then2, label %if.end5

if.then2:                                         ; preds = %for.body
  %7 = load i32, ptr %col, align 4, !tbaa !6
  %8 = load ptr, ptr %q.addr, align 8, !tbaa !10
  %9 = load i32, ptr %row.addr, align 4, !tbaa !6
  %idxprom = sext i32 %9 to i64
  %arrayidx = getelementptr inbounds i32, ptr %8, i64 %idxprom
  store i32 %7, ptr %arrayidx, align 4, !tbaa !6
  %10 = load ptr, ptr %q.addr, align 8, !tbaa !10
  %11 = load i32, ptr %row.addr, align 4, !tbaa !6
  %add = add nsw i32 %11, 1
  %12 = load i32, ptr %n.addr, align 4, !tbaa !6
  %call3 = call i32 @place(ptr noundef %10, i32 noundef %add, i32 noundef %12)
  %13 = load i32, ptr %found, align 4, !tbaa !6
  %add4 = add nsw i32 %13, %call3
  store i32 %add4, ptr %found, align 4, !tbaa !6
  br label %if.end5

if.end5:                                          ; preds = %if.then2, %for.body
  br label %for.inc

for.inc:                                          ; preds = %if.end5
  %14 = load i32, ptr %col, align 4, !tbaa !6
  %inc = add nsw i32 %14, 1
  store i32 %inc, ptr %col, align 4, !tbaa !6
  br label %for.cond, !llvm.loop !13

for.end:                                          ; preds = %for.cond
  %15 = load i32, ptr %found, align 4, !tbaa !6
  store i32 %15, ptr %retval, align 4
  store i32 1, ptr %cleanup.dest.slot, align 4
  br label %cleanup

cleanup:                                          ; preds = %for.end, %if.then
  call void @llvm.lifetime.end.p0(i64 4, ptr %found) #3
  call void @llvm.lifetime.end.p0(i64 4, ptr %col) #3
  %16 = load i32, ptr %retval, align 4
  ret i32 %16
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) #1

; Function Attrs: nounwind uwtable
define dso_local i32 @main() #0 {
entry:
  %retval = alloca i32, align 4
  store i32 0, ptr %retval, align 4
  %call = call i64 @bench_queens(i32 noundef 7)
  call void @report(i64 noundef %call)
  ret i32 0
}

declare void @report(i64 noundef) #2

; Function Attrs: nounwind uwtable
define internal i32 @safe(ptr noundef %q, i32 noundef %row, i32 noundef %col) #0 {
entry:
  %retval = alloca i32, align 4
  %q.addr = alloca ptr, align 8
  %row.addr = alloca i32, align 4
  %col.addr = alloca i32, align 4
  %r = alloca i32, align 4
  %d = alloca i32, align 4
  %cleanup.dest.slot = alloca i32, align 4
  store ptr %q, ptr %q.addr, align 8, !tbaa !10
  store i32 %row, ptr %row.addr, align 4, !tbaa !6
  store i32 %col, ptr %col.addr, align 4, !tbaa !6
  call void @llvm.lifetime.start.p0(i64 4, ptr %r) #3
  store i32 0, ptr %r, align 4, !tbaa !6
  br label %for.cond

for.cond:                                         ; preds = %for.inc, %entry
  %0 = load i32, ptr %r, align 4, !tbaa !6
  %1 = load i32, ptr %row.addr, align 4, !tbaa !6
  %cmp = icmp slt i32 %0, %1
  br i1 %cmp, label %for.body, label %for.end

for.body:                                         ; preds = %for.cond
  call void @llvm.lifetime.start.p0(i64 4, ptr %d) #3
  %2 = load ptr, ptr %q.addr, align 8, !tbaa !10
  %3 = load i32, ptr %r, align 4, !tbaa !6
  %idxprom = sext i32 %3 to i64
  %arrayidx = getelementptr inbounds i32, ptr %2, i64 %idxprom
  %4 = load i32, ptr %arrayidx, align 4, !tbaa !6
  %5 = load i32, ptr %col.addr, align 4, !tbaa !6
  %sub = sub nsw i32 %4, %5
  store i32 %sub, ptr %d, align 4, !tbaa !6
  %6 = load i32, ptr %d, align 4, !tbaa !6
  %cmp1 = icmp eq i32 %6, 0
  br i1 %cmp1, label %if.then, label %lor.lhs.false

lor.lhs.false:                                    ; preds = %for.body
  %7 = load i32, ptr %d, align 4, !tbaa !6
  %8 = load i32, ptr %row.addr, align 4, !tbaa !6
  %9 = load i32, ptr %r, align 4, !tbaa !6
  %sub2 = sub nsw i32 %8, %9
  %cmp3 = icmp eq i32 %7, %sub2
  br i1 %cmp3, label %if.then, label %lor.lhs.false4

lor.lhs.false4:                                   ; preds = %lor.lhs.false
  %10 = load i32, ptr %d, align 4, !tbaa !6
  %11 = load i32, ptr %r, align 4, !tbaa !6
  %12 = load i32, ptr %row.addr, align 4, !tbaa !6
  %sub5 = sub nsw i32 %11, %12
  %cmp6 = icmp eq i32 %10, %sub5
  br i1 %cmp6, label %if.then, label %if.end

if.then:                                          ; preds = %lor.lhs.false4, %lor.lhs.false, %for.body
  store i32 0, ptr %retval, align 4
  store i32 1, ptr %cleanup.dest.slot, align 4
  br label %cleanup

if.end:                                           ; preds = %lor.lhs.false4
  store i32 0, ptr %cleanup.dest.slot, align 4
  br label %cleanup

cleanup:                                          ; preds = %if.end, %if.then
  call void @llvm.lifetime.end.p0(i64 4, ptr %d) #3
  %cleanup.dest = load i32, ptr %cleanup.dest.slot, align 4
  switch i32 %cleanup.dest, label %cleanup7 [
    i32 0, label %cleanup.cont
  ]

cleanup.cont:                                     ; preds = %cleanup
  br label %for.inc

for.inc:                                          ; preds = %cleanup.cont
  %13 = load i32, ptr %r, align 4, !tbaa !6
  %inc = add nsw i32 %13, 1
  store i32 %inc, ptr %r, align 4, !tbaa !6
  br label %for.cond, !llvm.loop !16

for.end:                                          ; preds = %for.cond
  store i32 1, ptr %retval, align 4
  store i32 1, ptr %cleanup.dest.slot, align 4
  br label %cleanup7

cleanup7:                                         ; preds = %for.end, %cleanup
  call void @llvm.lifetime.end.p0(i64 4, ptr %r) #3
  %14 = load i32, ptr %retval, align 4
  ret i32 %14
}

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
!10 = !{!11, !11, i64 0}
!11 = !{!"p1 int", !12, i64 0}
!12 = !{!"any pointer", !8, i64 0}
!13 = distinct !{!13, !14, !15}
!14 = !{!"llvm.loop.mustprogress"}
!15 = !{!"llvm.loop.unroll.disable"}
!16 = distinct !{!16, !14, !15}
