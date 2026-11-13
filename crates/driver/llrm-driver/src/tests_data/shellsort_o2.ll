; ModuleID = 'bench/shellsort/shellsort.c'
source_filename = "bench/shellsort/shellsort.c"
target datalayout = "@m64-datalayout@"

; Function Attrs: nounwind uwtable
define dso_local i64 @bench_shellsort(i16 noundef zeroext %seed) #0 {
entry:
  %seed.addr = alloca i16, align 2
  %values = alloca [64 x i16], align 16
  %i = alloca i16, align 2
  %gap = alloca i16, align 2
  %checksum = alloca i64, align 8
  %at = alloca i16, align 2
  %value = alloca i16, align 2
  store i16 %seed, ptr %seed.addr, align 2, !tbaa !9
  call void @llvm.lifetime.start.p0(ptr %values) #3
  call void @llvm.lifetime.start.p0(ptr %i) #3
  call void @llvm.lifetime.start.p0(ptr %gap) #3
  call void @llvm.lifetime.start.p0(ptr %checksum) #3
  store i64 0, ptr %checksum, align 8, !tbaa !11
  store i16 0, ptr %i, align 2, !tbaa !9
  br label %for.cond

for.cond:                                         ; preds = %for.inc, %entry
  %0 = load i16, ptr %i, align 2, !tbaa !9
  %conv = zext i16 %0 to i32
  %cmp = icmp slt i32 %conv, 64
  br i1 %cmp, label %for.body, label %for.end

for.body:                                         ; preds = %for.cond
  %1 = load i16, ptr %i, align 2, !tbaa !9
  %conv2 = zext i16 %1 to i64
  %mul = mul i64 %conv2, 109
  %add = add i64 %mul, 37
  %2 = load i16, ptr %i, align 2, !tbaa !9
  %conv3 = zext i16 %2 to i32
  %shl = shl i32 %conv3, 7
  %conv4 = sext i32 %shl to i64
  %xor = xor i64 %add, %conv4
  %3 = load i16, ptr %seed.addr, align 2, !tbaa !9
  %conv5 = zext i16 %3 to i64
  %xor6 = xor i64 %xor, %conv5
  %conv7 = trunc i64 %xor6 to i16
  %4 = load i16, ptr %i, align 2, !tbaa !9
  %idxprom = zext i16 %4 to i64
  %arrayidx = getelementptr inbounds nuw [64 x i16], ptr %values, i64 0, i64 %idxprom
  store i16 %conv7, ptr %arrayidx, align 2, !tbaa !9
  br label %for.inc

for.inc:                                          ; preds = %for.body
  %5 = load i16, ptr %i, align 2, !tbaa !9
  %inc = add i16 %5, 1
  store i16 %inc, ptr %i, align 2, !tbaa !9
  br label %for.cond, !llvm.loop !13

for.end:                                          ; preds = %for.cond
  store i16 32, ptr %gap, align 2, !tbaa !9
  br label %for.cond8

for.cond8:                                        ; preds = %for.inc45, %for.end
  %6 = load i16, ptr %gap, align 2, !tbaa !9
  %tobool = icmp ne i16 %6, 0
  br i1 %tobool, label %for.body9, label %for.end48

for.body9:                                        ; preds = %for.cond8
  %7 = load i16, ptr %gap, align 2, !tbaa !9
  store i16 %7, ptr %i, align 2, !tbaa !9
  br label %for.cond10

for.cond10:                                       ; preds = %for.inc42, %for.body9
  %8 = load i16, ptr %i, align 2, !tbaa !9
  %conv11 = zext i16 %8 to i32
  %cmp12 = icmp slt i32 %conv11, 64
  br i1 %cmp12, label %for.body14, label %for.end44

for.body14:                                       ; preds = %for.cond10
  call void @llvm.lifetime.start.p0(ptr %at) #3
  %9 = load i16, ptr %i, align 2, !tbaa !9
  store i16 %9, ptr %at, align 2, !tbaa !9
  call void @llvm.lifetime.start.p0(ptr %value) #3
  %10 = load i16, ptr %i, align 2, !tbaa !9
  %idxprom15 = zext i16 %10 to i64
  %arrayidx16 = getelementptr inbounds nuw [64 x i16], ptr %values, i64 0, i64 %idxprom15
  %11 = load i16, ptr %arrayidx16, align 2, !tbaa !9
  store i16 %11, ptr %value, align 2, !tbaa !9
  br label %while.cond

while.cond:                                       ; preds = %while.body, %for.body14
  %12 = load i16, ptr %at, align 2, !tbaa !9
  %conv17 = zext i16 %12 to i32
  %13 = load i16, ptr %gap, align 2, !tbaa !9
  %conv18 = zext i16 %13 to i32
  %cmp19 = icmp sge i32 %conv17, %conv18
  br i1 %cmp19, label %land.rhs, label %land.end

land.rhs:                                         ; preds = %while.cond
  %14 = load i16, ptr %at, align 2, !tbaa !9
  %conv21 = zext i16 %14 to i32
  %15 = load i16, ptr %gap, align 2, !tbaa !9
  %conv22 = zext i16 %15 to i32
  %sub = sub nsw i32 %conv21, %conv22
  %idxprom23 = sext i32 %sub to i64
  %arrayidx24 = getelementptr inbounds [64 x i16], ptr %values, i64 0, i64 %idxprom23
  %16 = load i16, ptr %arrayidx24, align 2, !tbaa !9
  %conv25 = zext i16 %16 to i32
  %17 = load i16, ptr %value, align 2, !tbaa !9
  %conv26 = zext i16 %17 to i32
  %cmp27 = icmp sgt i32 %conv25, %conv26
  br label %land.end

land.end:                                         ; preds = %land.rhs, %while.cond
  %18 = phi i1 [ false, %while.cond ], [ %cmp27, %land.rhs ]
  br i1 %18, label %while.body, label %while.end

while.body:                                       ; preds = %land.end
  %19 = load i16, ptr %at, align 2, !tbaa !9
  %conv29 = zext i16 %19 to i32
  %20 = load i16, ptr %gap, align 2, !tbaa !9
  %conv30 = zext i16 %20 to i32
  %sub31 = sub nsw i32 %conv29, %conv30
  %idxprom32 = sext i32 %sub31 to i64
  %arrayidx33 = getelementptr inbounds [64 x i16], ptr %values, i64 0, i64 %idxprom32
  %21 = load i16, ptr %arrayidx33, align 2, !tbaa !9
  %22 = load i16, ptr %at, align 2, !tbaa !9
  %idxprom34 = zext i16 %22 to i64
  %arrayidx35 = getelementptr inbounds nuw [64 x i16], ptr %values, i64 0, i64 %idxprom34
  store i16 %21, ptr %arrayidx35, align 2, !tbaa !9
  %23 = load i16, ptr %at, align 2, !tbaa !9
  %conv36 = zext i16 %23 to i32
  %24 = load i16, ptr %gap, align 2, !tbaa !9
  %conv37 = zext i16 %24 to i32
  %sub38 = sub nsw i32 %conv36, %conv37
  %conv39 = trunc i32 %sub38 to i16
  store i16 %conv39, ptr %at, align 2, !tbaa !9
  br label %while.cond, !llvm.loop !15

while.end:                                        ; preds = %land.end
  %25 = load i16, ptr %value, align 2, !tbaa !9
  %26 = load i16, ptr %at, align 2, !tbaa !9
  %idxprom40 = zext i16 %26 to i64
  %arrayidx41 = getelementptr inbounds nuw [64 x i16], ptr %values, i64 0, i64 %idxprom40
  store i16 %25, ptr %arrayidx41, align 2, !tbaa !9
  call void @llvm.lifetime.end.p0(ptr %value) #3
  call void @llvm.lifetime.end.p0(ptr %at) #3
  br label %for.inc42

for.inc42:                                        ; preds = %while.end
  %27 = load i16, ptr %i, align 2, !tbaa !9
  %inc43 = add i16 %27, 1
  store i16 %inc43, ptr %i, align 2, !tbaa !9
  br label %for.cond10, !llvm.loop !16

for.end44:                                        ; preds = %for.cond10
  br label %for.inc45

for.inc45:                                        ; preds = %for.end44
  %28 = load i16, ptr %gap, align 2, !tbaa !9
  %conv46 = zext i16 %28 to i32
  %shr = ashr i32 %conv46, 1
  %conv47 = trunc i32 %shr to i16
  store i16 %conv47, ptr %gap, align 2, !tbaa !9
  br label %for.cond8, !llvm.loop !17

for.end48:                                        ; preds = %for.cond8
  store i16 0, ptr %i, align 2, !tbaa !9
  br label %for.cond49

for.cond49:                                       ; preds = %for.inc62, %for.end48
  %29 = load i16, ptr %i, align 2, !tbaa !9
  %conv50 = zext i16 %29 to i32
  %cmp51 = icmp slt i32 %conv50, 64
  br i1 %cmp51, label %for.body53, label %for.end64

for.body53:                                       ; preds = %for.cond49
  %30 = load i16, ptr %i, align 2, !tbaa !9
  %idxprom54 = zext i16 %30 to i64
  %arrayidx55 = getelementptr inbounds nuw [64 x i16], ptr %values, i64 0, i64 %idxprom54
  %31 = load i16, ptr %arrayidx55, align 2, !tbaa !9
  %conv56 = zext i16 %31 to i64
  %32 = load i16, ptr %i, align 2, !tbaa !9
  %conv57 = zext i16 %32 to i32
  %add58 = add nsw i32 %conv57, 1
  %conv59 = sext i32 %add58 to i64
  %mul60 = mul i64 %conv56, %conv59
  %33 = load i64, ptr %checksum, align 8, !tbaa !11
  %add61 = add i64 %33, %mul60
  store i64 %add61, ptr %checksum, align 8, !tbaa !11
  br label %for.inc62

for.inc62:                                        ; preds = %for.body53
  %34 = load i16, ptr %i, align 2, !tbaa !9
  %inc63 = add i16 %34, 1
  store i16 %inc63, ptr %i, align 2, !tbaa !9
  br label %for.cond49, !llvm.loop !18

for.end64:                                        ; preds = %for.cond49
  %35 = load i64, ptr %checksum, align 8, !tbaa !11
  call void @llvm.lifetime.end.p0(ptr %checksum) #3
  call void @llvm.lifetime.end.p0(ptr %gap) #3
  call void @llvm.lifetime.end.p0(ptr %i) #3
  call void @llvm.lifetime.end.p0(ptr %values) #3
  ret i64 %35
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nounwind uwtable
define dso_local i32 @bench_main_() #0 {
entry:
  %call = call i64 @bench_shellsort(i16 noundef zeroext 0)
  call void @report(i64 noundef %call)
  ret i32 0
}

declare void @report(i64 noundef) #2

attributes #0 = { nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" }
attributes #3 = { nounwind }

!llvm.module.flags = !{!0, !1, !2}
!llvm.ident = !{!3}
!llvm.errno.tbaa = !{!4}

!0 = !{i32 8, !"PIC Level", i32 2}
!1 = !{i32 7, !"PIE Level", i32 2}
!2 = !{i32 7, !"uwtable", i32 2}
!3 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!4 = !{!5, !6, i64 0}
!5 = !{!"__libc_errno", !6, i64 0}
!6 = !{!"int", !7, i64 0}
!7 = !{!"omnipotent char", !8, i64 0}
!8 = !{!"Simple C/C++ TBAA"}
!9 = !{!10, !10, i64 0}
!10 = !{!"short", !7, i64 0}
!11 = !{!12, !12, i64 0}
!12 = !{!"long", !7, i64 0}
!13 = distinct !{!13, !14}
!14 = !{!"llvm.loop.mustprogress"}
!15 = distinct !{!15, !14}
!16 = distinct !{!16, !14}
!17 = distinct !{!17, !14}
!18 = distinct !{!18, !14}
