; ModuleID = 'fl.c'
source_filename = "fl.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8"

; Function Attrs: nounwind
define dso_local i16 @fletcher(ptr noundef %p, i8 noundef zeroext %n) #0 {
entry:
  %p.addr = alloca ptr, align 2
  %n.addr = alloca i8, align 1
  %a = alloca i8, align 1
  %b = alloca i8, align 1
  %i = alloca i8, align 1
  store ptr %p, ptr %p.addr, align 2, !tbaa !7
  store i8 %n, ptr %n.addr, align 1, !tbaa !10
  call void @llvm.lifetime.start.p0(ptr %a) #2
  store i8 0, ptr %a, align 1, !tbaa !10
  call void @llvm.lifetime.start.p0(ptr %b) #2
  store i8 0, ptr %b, align 1, !tbaa !10
  call void @llvm.lifetime.start.p0(ptr %i) #2
  store i8 0, ptr %i, align 1, !tbaa !10
  br label %for.cond

for.cond:                                         ; preds = %for.inc, %entry
  %0 = load i8, ptr %i, align 1, !tbaa !10
  %conv = zext i8 %0 to i16
  %1 = load i8, ptr %n.addr, align 1, !tbaa !10
  %conv1 = zext i8 %1 to i16
  %cmp = icmp slt i16 %conv, %conv1
  br i1 %cmp, label %for.body, label %for.end

for.body:                                         ; preds = %for.cond
  %2 = load ptr, ptr %p.addr, align 2, !tbaa !7
  %3 = load i8, ptr %i, align 1, !tbaa !10
  %idxprom = zext i8 %3 to i16
  %arrayidx = getelementptr inbounds nuw i8, ptr %2, i16 %idxprom
  %4 = load i8, ptr %arrayidx, align 1, !tbaa !10
  %conv3 = zext i8 %4 to i16
  %5 = load i8, ptr %a, align 1, !tbaa !10
  %conv4 = zext i8 %5 to i16
  %add = add nsw i16 %conv4, %conv3
  %conv5 = trunc i16 %add to i8
  store i8 %conv5, ptr %a, align 1, !tbaa !10
  %6 = load i8, ptr %a, align 1, !tbaa !10
  %conv6 = zext i8 %6 to i16
  %cmp7 = icmp sge i16 %conv6, 255
  br i1 %cmp7, label %if.then, label %if.end

if.then:                                          ; preds = %for.body
  %7 = load i8, ptr %a, align 1, !tbaa !10
  %conv9 = zext i8 %7 to i16
  %sub = sub nsw i16 %conv9, 255
  %conv10 = trunc i16 %sub to i8
  store i8 %conv10, ptr %a, align 1, !tbaa !10
  br label %if.end

if.end:                                           ; preds = %if.then, %for.body
  %8 = load i8, ptr %a, align 1, !tbaa !10
  %conv11 = zext i8 %8 to i16
  %9 = load i8, ptr %b, align 1, !tbaa !10
  %conv12 = zext i8 %9 to i16
  %add13 = add nsw i16 %conv12, %conv11
  %conv14 = trunc i16 %add13 to i8
  store i8 %conv14, ptr %b, align 1, !tbaa !10
  %10 = load i8, ptr %b, align 1, !tbaa !10
  %conv15 = zext i8 %10 to i16
  %cmp16 = icmp sge i16 %conv15, 255
  br i1 %cmp16, label %if.then18, label %if.end22

if.then18:                                        ; preds = %if.end
  %11 = load i8, ptr %b, align 1, !tbaa !10
  %conv19 = zext i8 %11 to i16
  %sub20 = sub nsw i16 %conv19, 255
  %conv21 = trunc i16 %sub20 to i8
  store i8 %conv21, ptr %b, align 1, !tbaa !10
  br label %if.end22

if.end22:                                         ; preds = %if.then18, %if.end
  br label %for.inc

for.inc:                                          ; preds = %if.end22
  %12 = load i8, ptr %i, align 1, !tbaa !10
  %inc = add i8 %12, 1
  store i8 %inc, ptr %i, align 1, !tbaa !10
  br label %for.cond, !llvm.loop !11

for.end:                                          ; preds = %for.cond
  %13 = load i8, ptr %b, align 1, !tbaa !10
  %conv23 = zext i8 %13 to i16
  %shl = shl i16 %conv23, 8
  %14 = load i8, ptr %a, align 1, !tbaa !10
  %conv24 = zext i8 %14 to i16
  %or = or i16 %shl, %conv24
  call void @llvm.lifetime.end.p0(ptr %i) #2
  call void @llvm.lifetime.end.p0(ptr %b) #2
  call void @llvm.lifetime.end.p0(ptr %a) #2
  ret i16 %or
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

attributes #0 = { nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nounwind }

!llvm.module.flags = !{!0}
!llvm.ident = !{!1}
!llvm.errno.tbaa = !{!2}

!0 = !{i32 1, !"wchar_size", i32 2}
!1 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!2 = !{!3, !4, i64 0}
!3 = !{!"__libc_errno", !4, i64 0}
!4 = !{!"int", !5, i64 0}
!5 = !{!"omnipotent char", !6, i64 0}
!6 = !{!"Simple C/C++ TBAA"}
!7 = !{!8, !8, i64 0}
!8 = !{!"p1 omnipotent char", !9, i64 0}
!9 = !{!"any pointer", !5, i64 0}
!10 = !{!5, !5, i64 0}
!11 = distinct !{!11, !12}
!12 = !{!"llvm.loop.mustprogress"}
