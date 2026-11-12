; ModuleID = 'bench/sieve/sieve.c'
source_filename = "bench/sieve/sieve.c"
target datalayout = "@m64-datalayout@"

; Function Attrs: nounwind uwtable
define dso_local i64 @bench_sieve(i16 noundef zeroext %limit) #0 {
entry:
  %limit.addr = alloca i16, align 2
  %composite = alloca [1024 x i8], align 16
  %i = alloca i16, align 2
  %multiple = alloca i16, align 2
  %count = alloca i16, align 2
  %sum = alloca i64, align 8
  store i16 %limit, ptr %limit.addr, align 2, !tbaa !9
  call void @llvm.lifetime.start.p0(ptr %composite) #3
  call void @llvm.lifetime.start.p0(ptr %i) #3
  call void @llvm.lifetime.start.p0(ptr %multiple) #3
  call void @llvm.lifetime.start.p0(ptr %count) #3
  store i16 0, ptr %count, align 2, !tbaa !9
  call void @llvm.lifetime.start.p0(ptr %sum) #3
  store i64 0, ptr %sum, align 8, !tbaa !11
  store i16 0, ptr %i, align 2, !tbaa !9
  br label %for.cond

for.cond:                                         ; preds = %for.inc, %entry
  %0 = load i16, ptr %i, align 2, !tbaa !9
  %conv = zext i16 %0 to i32
  %1 = load i16, ptr %limit.addr, align 2, !tbaa !9
  %conv1 = zext i16 %1 to i32
  %cmp = icmp slt i32 %conv, %conv1
  br i1 %cmp, label %for.body, label %for.end

for.body:                                         ; preds = %for.cond
  %2 = load i16, ptr %i, align 2, !tbaa !9
  %idxprom = zext i16 %2 to i64
  %arrayidx = getelementptr inbounds nuw [1024 x i8], ptr %composite, i64 0, i64 %idxprom
  store i8 0, ptr %arrayidx, align 1, !tbaa !13
  br label %for.inc

for.inc:                                          ; preds = %for.body
  %3 = load i16, ptr %i, align 2, !tbaa !9
  %inc = add i16 %3, 1
  store i16 %inc, ptr %i, align 2, !tbaa !9
  br label %for.cond, !llvm.loop !14

for.end:                                          ; preds = %for.cond
  store i16 2, ptr %i, align 2, !tbaa !9
  br label %for.cond3

for.cond3:                                        ; preds = %for.inc35, %for.end
  %4 = load i16, ptr %i, align 2, !tbaa !9
  %conv4 = zext i16 %4 to i32
  %5 = load i16, ptr %limit.addr, align 2, !tbaa !9
  %conv5 = zext i16 %5 to i32
  %cmp6 = icmp slt i32 %conv4, %conv5
  br i1 %cmp6, label %for.body8, label %for.end37

for.body8:                                        ; preds = %for.cond3
  %6 = load i16, ptr %i, align 2, !tbaa !9
  %idxprom9 = zext i16 %6 to i64
  %arrayidx10 = getelementptr inbounds nuw [1024 x i8], ptr %composite, i64 0, i64 %idxprom9
  %7 = load i8, ptr %arrayidx10, align 1, !tbaa !13
  %tobool = icmp ne i8 %7, 0
  br i1 %tobool, label %if.then, label %if.end

if.then:                                          ; preds = %for.body8
  br label %for.inc35

if.end:                                           ; preds = %for.body8
  %8 = load i16, ptr %count, align 2, !tbaa !9
  %inc11 = add i16 %8, 1
  store i16 %inc11, ptr %count, align 2, !tbaa !9
  %9 = load i16, ptr %i, align 2, !tbaa !9
  %conv12 = zext i16 %9 to i64
  %10 = load i64, ptr %sum, align 8, !tbaa !11
  %add = add i64 %10, %conv12
  store i64 %add, ptr %sum, align 8, !tbaa !11
  %11 = load i16, ptr %i, align 2, !tbaa !9
  %conv13 = zext i16 %11 to i32
  %cmp14 = icmp sle i32 %conv13, 31
  br i1 %cmp14, label %if.then16, label %if.end34

if.then16:                                        ; preds = %if.end
  %12 = load i16, ptr %i, align 2, !tbaa !9
  %conv17 = zext i16 %12 to i32
  %13 = load i16, ptr %i, align 2, !tbaa !9
  %conv18 = zext i16 %13 to i32
  %mul = mul nsw i32 %conv17, %conv18
  %conv19 = trunc i32 %mul to i16
  store i16 %conv19, ptr %multiple, align 2, !tbaa !9
  br label %for.cond20

for.cond20:                                       ; preds = %for.inc28, %if.then16
  %14 = load i16, ptr %multiple, align 2, !tbaa !9
  %conv21 = zext i16 %14 to i32
  %15 = load i16, ptr %limit.addr, align 2, !tbaa !9
  %conv22 = zext i16 %15 to i32
  %cmp23 = icmp slt i32 %conv21, %conv22
  br i1 %cmp23, label %for.body25, label %for.end33

for.body25:                                       ; preds = %for.cond20
  %16 = load i16, ptr %multiple, align 2, !tbaa !9
  %idxprom26 = zext i16 %16 to i64
  %arrayidx27 = getelementptr inbounds nuw [1024 x i8], ptr %composite, i64 0, i64 %idxprom26
  store i8 1, ptr %arrayidx27, align 1, !tbaa !13
  br label %for.inc28

for.inc28:                                        ; preds = %for.body25
  %17 = load i16, ptr %multiple, align 2, !tbaa !9
  %conv29 = zext i16 %17 to i32
  %18 = load i16, ptr %i, align 2, !tbaa !9
  %conv30 = zext i16 %18 to i32
  %add31 = add nsw i32 %conv29, %conv30
  %conv32 = trunc i32 %add31 to i16
  store i16 %conv32, ptr %multiple, align 2, !tbaa !9
  br label %for.cond20, !llvm.loop !16

for.end33:                                        ; preds = %for.cond20
  br label %if.end34

if.end34:                                         ; preds = %for.end33, %if.end
  br label %for.inc35

for.inc35:                                        ; preds = %if.end34, %if.then
  %19 = load i16, ptr %i, align 2, !tbaa !9
  %inc36 = add i16 %19, 1
  store i16 %inc36, ptr %i, align 2, !tbaa !9
  br label %for.cond3, !llvm.loop !17

for.end37:                                        ; preds = %for.cond3
  %20 = load i16, ptr %count, align 2, !tbaa !9
  %conv38 = zext i16 %20 to i64
  %shl = shl i64 %conv38, 16
  %21 = load i64, ptr %sum, align 8, !tbaa !11
  %xor = xor i64 %shl, %21
  call void @llvm.lifetime.end.p0(ptr %sum) #3
  call void @llvm.lifetime.end.p0(ptr %count) #3
  call void @llvm.lifetime.end.p0(ptr %multiple) #3
  call void @llvm.lifetime.end.p0(ptr %i) #3
  call void @llvm.lifetime.end.p0(ptr %composite) #3
  ret i64 %xor
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nounwind uwtable
define dso_local i32 @bench_main_() #0 {
entry:
  %call = call i64 @bench_sieve(i16 noundef zeroext 1024)
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
!13 = !{!7, !7, i64 0}
!14 = distinct !{!14, !15}
!15 = !{!"llvm.loop.mustprogress"}
!16 = distinct !{!16, !15}
!17 = distinct !{!17, !15}
