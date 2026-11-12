; ModuleID = 'bench/recpow/recpow.c'
source_filename = "bench/recpow/recpow.c"
target datalayout = "@m64-datalayout@"

; Function Attrs: nounwind optsize uwtable
define dso_local i64 @bench_recpow(i32 noundef %n) #0 {
entry:
  %n.addr = alloca i32, align 4
  %total = alloca i64, align 8
  %i = alloca i32, align 4
  store i32 %n, ptr %n.addr, align 4, !tbaa !9
  call void @llvm.lifetime.start.p0(ptr %total) #3
  store i64 0, ptr %total, align 8, !tbaa !10
  call void @llvm.lifetime.start.p0(ptr %i) #3
  store i32 1, ptr %i, align 4, !tbaa !9
  br label %for.cond

for.cond:                                         ; preds = %for.inc, %entry
  %0 = load i32, ptr %i, align 4, !tbaa !9
  %1 = load i32, ptr %n.addr, align 4, !tbaa !9
  %cmp = icmp sle i32 %0, %1
  br i1 %cmp, label %for.body, label %for.end

for.body:                                         ; preds = %for.cond
  %2 = load i32, ptr %i, align 4, !tbaa !9
  %rem = srem i32 %2, 97
  %add = add nsw i32 %rem, 2
  %conv = sext i32 %add to i64
  %3 = load i32, ptr %i, align 4, !tbaa !9
  %add1 = add nsw i32 %3, 40
  %call = call i64 @power(i64 noundef %conv, i32 noundef %add1) #4
  %rem2 = srem i64 %call, 1000
  %4 = load i64, ptr %total, align 8, !tbaa !10
  %add3 = add nsw i64 %4, %rem2
  store i64 %add3, ptr %total, align 8, !tbaa !10
  br label %for.inc

for.inc:                                          ; preds = %for.body
  %5 = load i32, ptr %i, align 4, !tbaa !9
  %inc = add nsw i32 %5, 1
  store i32 %inc, ptr %i, align 4, !tbaa !9
  br label %for.cond, !llvm.loop !12

for.end:                                          ; preds = %for.cond
  %6 = load i64, ptr %total, align 8, !tbaa !10
  call void @llvm.lifetime.end.p0(ptr %i) #3
  call void @llvm.lifetime.end.p0(ptr %total) #3
  ret i64 %6
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nounwind optsize uwtable
define internal i64 @power(i64 noundef %b, i32 noundef %e) #0 {
entry:
  %retval = alloca i64, align 8
  %b.addr = alloca i64, align 8
  %e.addr = alloca i32, align 4
  store i64 %b, ptr %b.addr, align 8, !tbaa !10
  store i32 %e, ptr %e.addr, align 4, !tbaa !9
  %0 = load i32, ptr %e.addr, align 4, !tbaa !9
  %cmp = icmp eq i32 %0, 0
  br i1 %cmp, label %if.then, label %if.end

if.then:                                          ; preds = %entry
  store i64 1, ptr %retval, align 8
  br label %return

if.end:                                           ; preds = %entry
  %1 = load i32, ptr %e.addr, align 4, !tbaa !9
  %and = and i32 %1, 1
  %tobool = icmp ne i32 %and, 0
  br i1 %tobool, label %if.then1, label %if.end2

if.then1:                                         ; preds = %if.end
  %2 = load i64, ptr %b.addr, align 8, !tbaa !10
  %3 = load i64, ptr %b.addr, align 8, !tbaa !10
  %4 = load i32, ptr %e.addr, align 4, !tbaa !9
  %sub = sub nsw i32 %4, 1
  %call = call i64 @power(i64 noundef %3, i32 noundef %sub) #4
  %mul = mul nsw i64 %2, %call
  %rem = srem i64 %mul, 30011
  store i64 %rem, ptr %retval, align 8
  br label %return

if.end2:                                          ; preds = %if.end
  %5 = load i64, ptr %b.addr, align 8, !tbaa !10
  %6 = load i64, ptr %b.addr, align 8, !tbaa !10
  %mul3 = mul nsw i64 %5, %6
  %rem4 = srem i64 %mul3, 30011
  %7 = load i32, ptr %e.addr, align 4, !tbaa !9
  %shr = ashr i32 %7, 1
  %call5 = call i64 @power(i64 noundef %rem4, i32 noundef %shr) #4
  store i64 %call5, ptr %retval, align 8
  br label %return

return:                                           ; preds = %if.end2, %if.then1, %if.then
  %8 = load i64, ptr %retval, align 8
  ret i64 %8
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nounwind optsize uwtable
define dso_local i32 @bench_main_() #0 {
entry:
  %call = call i64 @bench_recpow(i32 noundef 400) #4
  call void @report(i64 noundef %call) #4
  ret i32 0
}

; Function Attrs: optsize
declare void @report(i64 noundef) #2

attributes #0 = { nounwind optsize uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" }
attributes #3 = { nounwind }
attributes #4 = { optsize }

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
!9 = !{!6, !6, i64 0}
!10 = !{!11, !11, i64 0}
!11 = !{!"long", !7, i64 0}
!12 = distinct !{!12, !13}
!13 = !{!"llvm.loop.mustprogress"}
