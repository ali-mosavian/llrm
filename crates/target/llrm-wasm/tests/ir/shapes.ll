; ModuleID = 's0.ll'
source_filename = "shapes.c"
target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"

; Function Attrs: noinline nounwind
define i32 @tri(i32 noundef %a) #0 {
entry:
  %cmp = icmp sgt i32 %a, 3
  br i1 %cmp, label %if.then, label %if.end

if.then:                                          ; preds = %entry
  call void @side(i32 noundef %a)
  br label %if.end

if.end:                                           ; preds = %if.then, %entry
  %add = add nsw i32 %a, 1
  ret i32 %add
}

declare void @side(i32 noundef) #1

; Function Attrs: noinline nounwind
define i32 @ifelse(i32 noundef %a, i32 noundef %b) #0 {
entry:
  %cmp = icmp slt i32 %a, %b
  br i1 %cmp, label %if.then, label %if.else

if.then:                                          ; preds = %entry
  %mul = mul nsw i32 %a, 2
  br label %if.end

if.else:                                          ; preds = %entry
  %sub = sub nsw i32 %b, 7
  br label %if.end

if.end:                                           ; preds = %if.else, %if.then
  %r.0 = phi i32 [ %mul, %if.then ], [ %sub, %if.else ]
  %add = add nsw i32 %r.0, 1
  ret i32 %add
}

; Function Attrs: noinline nounwind
define i32 @chain(i32 noundef %a) #0 {
entry:
  %cmp = icmp eq i32 %a, 1
  br i1 %cmp, label %if.then, label %if.end

if.then:                                          ; preds = %entry
  br label %return

if.end:                                           ; preds = %entry
  %cmp1 = icmp eq i32 %a, 2
  br i1 %cmp1, label %if.then2, label %if.end3

if.then2:                                         ; preds = %if.end
  br label %return

if.end3:                                          ; preds = %if.end
  %cmp4 = icmp eq i32 %a, 3
  br i1 %cmp4, label %if.then5, label %if.end6

if.then5:                                         ; preds = %if.end3
  br label %return

if.end6:                                          ; preds = %if.end3
  br label %return

return:                                           ; preds = %if.end6, %if.then5, %if.then2, %if.then
  %retval.0 = phi i32 [ 10, %if.then ], [ 20, %if.then2 ], [ 30, %if.then5 ], [ 0, %if.end6 ]
  ret i32 %retval.0
}

; Function Attrs: noinline nounwind
define i32 @andor(i32 noundef %a, i32 noundef %b, i32 noundef %c) #0 {
entry:
  %tobool = icmp ne i32 %a, 0
  br i1 %tobool, label %land.lhs.true, label %lor.lhs.false

land.lhs.true:                                    ; preds = %entry
  %tobool1 = icmp ne i32 %b, 0
  br i1 %tobool1, label %if.then, label %lor.lhs.false

lor.lhs.false:                                    ; preds = %land.lhs.true, %entry
  %tobool2 = icmp ne i32 %c, 0
  br i1 %tobool2, label %if.then, label %if.end

if.then:                                          ; preds = %lor.lhs.false, %land.lhs.true
  br label %return

if.end:                                           ; preds = %lor.lhs.false
  br label %return

return:                                           ; preds = %if.end, %if.then
  %retval.0 = phi i32 [ 1, %if.then ], [ 2, %if.end ]
  ret i32 %retval.0
}

; Function Attrs: noinline nounwind
define i32 @nested_if(i32 noundef %a, i32 noundef %b) #0 {
entry:
  %tobool = icmp ne i32 %a, 0
  br i1 %tobool, label %if.then, label %if.else3

if.then:                                          ; preds = %entry
  %tobool1 = icmp ne i32 %b, 0
  br i1 %tobool1, label %if.then2, label %if.else

if.then2:                                         ; preds = %if.then
  br label %return

if.else:                                          ; preds = %if.then
  br label %return

if.else3:                                         ; preds = %entry
  %tobool4 = icmp ne i32 %b, 0
  br i1 %tobool4, label %if.then5, label %if.end

if.then5:                                         ; preds = %if.else3
  br label %return

if.end:                                           ; preds = %if.else3
  br label %if.end6

if.end6:                                          ; preds = %if.end
  br label %return

return:                                           ; preds = %if.end6, %if.then5, %if.else, %if.then2
  %retval.0 = phi i32 [ 1, %if.then2 ], [ 2, %if.else ], [ 3, %if.then5 ], [ 4, %if.end6 ]
  ret i32 %retval.0
}

; Function Attrs: noinline nounwind
define i32 @whiles(i32 noundef %n) #0 {
entry:
  br label %while.cond

while.cond:                                       ; preds = %while.body, %entry
  %s.0 = phi i32 [ 0, %entry ], [ %add, %while.body ]
  %n.addr.0 = phi i32 [ %n, %entry ], [ %dec, %while.body ]
  %cmp = icmp sgt i32 %n.addr.0, 0
  br i1 %cmp, label %while.body, label %while.end

while.body:                                       ; preds = %while.cond
  %add = add nsw i32 %s.0, %n.addr.0
  %dec = add nsw i32 %n.addr.0, -1
  br label %while.cond, !llvm.loop !2

while.end:                                        ; preds = %while.cond
  ret i32 %s.0
}

; Function Attrs: noinline nounwind
define i32 @dowhile(i32 noundef %n) #0 {
entry:
  br label %do.body

do.body:                                          ; preds = %do.cond, %entry
  %s.0 = phi i32 [ 0, %entry ], [ %add, %do.cond ]
  %n.addr.0 = phi i32 [ %n, %entry ], [ %dec, %do.cond ]
  %call = call i32 @get()
  %add = add nsw i32 %s.0, %call
  %dec = add nsw i32 %n.addr.0, -1
  br label %do.cond

do.cond:                                          ; preds = %do.body
  %cmp = icmp sgt i32 %dec, 0
  br i1 %cmp, label %do.body, label %do.end, !llvm.loop !4

do.end:                                           ; preds = %do.cond
  ret i32 %add
}

declare i32 @get() #1

; Function Attrs: noinline nounwind
define i32 @forloop(i32 noundef %n) #0 {
entry:
  br label %for.cond

for.cond:                                         ; preds = %for.inc, %entry
  %s.0 = phi i32 [ 0, %entry ], [ %add, %for.inc ]
  %i.0 = phi i32 [ 0, %entry ], [ %inc, %for.inc ]
  %cmp = icmp slt i32 %i.0, %n
  br i1 %cmp, label %for.body, label %for.end

for.body:                                         ; preds = %for.cond
  %mul = mul nsw i32 %i.0, %i.0
  %add = add nsw i32 %s.0, %mul
  br label %for.inc

for.inc:                                          ; preds = %for.body
  %inc = add nsw i32 %i.0, 1
  br label %for.cond, !llvm.loop !5

for.end:                                          ; preds = %for.cond
  ret i32 %s.0
}

; Function Attrs: noinline nounwind
define i32 @breaks(i32 noundef %n) #0 {
entry:
  br label %for.cond

for.cond:                                         ; preds = %for.inc, %entry
  %s.0 = phi i32 [ 0, %entry ], [ %add, %for.inc ]
  %i.0 = phi i32 [ 0, %entry ], [ %inc, %for.inc ]
  %cmp = icmp slt i32 %i.0, %n
  br i1 %cmp, label %for.body, label %for.end

for.body:                                         ; preds = %for.cond
  %call = call i32 @get()
  %cmp1 = icmp eq i32 %call, 5
  br i1 %cmp1, label %if.then, label %if.end

if.then:                                          ; preds = %for.body
  br label %for.end

if.end:                                           ; preds = %for.body
  %add = add nsw i32 %s.0, %i.0
  br label %for.inc

for.inc:                                          ; preds = %if.end
  %inc = add nsw i32 %i.0, 1
  br label %for.cond, !llvm.loop !6

for.end:                                          ; preds = %if.then, %for.cond
  ret i32 %s.0
}

; Function Attrs: noinline nounwind
define i32 @conts(i32 noundef %n) #0 {
entry:
  br label %for.cond

for.cond:                                         ; preds = %for.inc, %entry
  %s.0 = phi i32 [ 0, %entry ], [ %s.1, %for.inc ]
  %i.0 = phi i32 [ 0, %entry ], [ %inc, %for.inc ]
  %cmp = icmp slt i32 %i.0, %n
  br i1 %cmp, label %for.body, label %for.end

for.body:                                         ; preds = %for.cond
  %and = and i32 %i.0, 1
  %tobool = icmp ne i32 %and, 0
  br i1 %tobool, label %if.then, label %if.end

if.then:                                          ; preds = %for.body
  br label %for.inc

if.end:                                           ; preds = %for.body
  %add = add nsw i32 %s.0, %i.0
  br label %for.inc

for.inc:                                          ; preds = %if.end, %if.then
  %s.1 = phi i32 [ %s.0, %if.then ], [ %add, %if.end ]
  %inc = add nsw i32 %i.0, 1
  br label %for.cond, !llvm.loop !7

for.end:                                          ; preds = %for.cond
  ret i32 %s.0
}

; Function Attrs: noinline nounwind
define i32 @nested_loops(i32 noundef %n, i32 noundef %m) #0 {
entry:
  br label %for.cond

for.cond:                                         ; preds = %for.inc4, %entry
  %s.0 = phi i32 [ 0, %entry ], [ %s.1, %for.inc4 ]
  %i.0 = phi i32 [ 0, %entry ], [ %inc5, %for.inc4 ]
  %cmp = icmp slt i32 %i.0, %n
  br i1 %cmp, label %for.body, label %for.end6

for.body:                                         ; preds = %for.cond
  br label %for.cond1

for.cond1:                                        ; preds = %for.inc, %for.body
  %s.1 = phi i32 [ %s.0, %for.body ], [ %add, %for.inc ]
  %j.0 = phi i32 [ 0, %for.body ], [ %inc, %for.inc ]
  %cmp2 = icmp slt i32 %j.0, %m
  br i1 %cmp2, label %for.body3, label %for.end

for.body3:                                        ; preds = %for.cond1
  %xor = xor i32 %i.0, %j.0
  %add = add nsw i32 %s.1, %xor
  br label %for.inc

for.inc:                                          ; preds = %for.body3
  %inc = add nsw i32 %j.0, 1
  br label %for.cond1, !llvm.loop !8

for.end:                                          ; preds = %for.cond1
  br label %for.inc4

for.inc4:                                         ; preds = %for.end
  %inc5 = add nsw i32 %i.0, 1
  br label %for.cond, !llvm.loop !9

for.end6:                                         ; preds = %for.cond
  ret i32 %s.0
}

; Function Attrs: noinline nounwind
define i32 @loop_in_if(i32 noundef %a, i32 noundef %n) #0 {
entry:
  %tobool = icmp ne i32 %a, 0
  br i1 %tobool, label %if.then, label %if.end

if.then:                                          ; preds = %entry
  br label %for.cond

for.cond:                                         ; preds = %for.inc, %if.then
  %s.1 = phi i32 [ 0, %if.then ], [ %add, %for.inc ]
  %i.0 = phi i32 [ 0, %if.then ], [ %inc, %for.inc ]
  %cmp = icmp slt i32 %i.0, %n
  br i1 %cmp, label %for.body, label %for.end

for.body:                                         ; preds = %for.cond
  %call = call i32 @get()
  %add = add nsw i32 %s.1, %call
  br label %for.inc

for.inc:                                          ; preds = %for.body
  %inc = add nsw i32 %i.0, 1
  br label %for.cond, !llvm.loop !10

for.end:                                          ; preds = %for.cond
  br label %if.end

if.end:                                           ; preds = %for.end, %entry
  %s.0 = phi i32 [ %s.1, %for.end ], [ 0, %entry ]
  ret i32 %s.0
}

; Function Attrs: noinline nounwind
define i32 @if_in_loop(i32 noundef %n) #0 {
entry:
  br label %for.cond

for.cond:                                         ; preds = %for.inc, %entry
  %s.0 = phi i32 [ 0, %entry ], [ %s.1, %for.inc ]
  %i.0 = phi i32 [ 0, %entry ], [ %inc1, %for.inc ]
  %cmp = icmp slt i32 %i.0, %n
  br i1 %cmp, label %for.body, label %for.end

for.body:                                         ; preds = %for.cond
  %call = call i32 @get()
  %tobool = icmp ne i32 %call, 0
  br i1 %tobool, label %if.then, label %if.else

if.then:                                          ; preds = %for.body
  %inc = add nsw i32 %s.0, 1
  br label %if.end

if.else:                                          ; preds = %for.body
  %add = add nsw i32 %s.0, 2
  br label %if.end

if.end:                                           ; preds = %if.else, %if.then
  %s.1 = phi i32 [ %inc, %if.then ], [ %add, %if.else ]
  br label %for.inc

for.inc:                                          ; preds = %if.end
  %inc1 = add nsw i32 %i.0, 1
  br label %for.cond, !llvm.loop !11

for.end:                                          ; preds = %for.cond
  ret i32 %s.0
}

; Function Attrs: noinline nounwind
define void @calls_in_loop(i32 noundef %n) #0 {
entry:
  br label %while.cond

while.cond:                                       ; preds = %while.body, %entry
  %n.addr.0 = phi i32 [ %n, %entry ], [ %dec, %while.body ]
  %dec = add nsw i32 %n.addr.0, -1
  %tobool = icmp ne i32 %n.addr.0, 0
  br i1 %tobool, label %while.body, label %while.end

while.body:                                       ; preds = %while.cond
  call void @side(i32 noundef %dec)
  br label %while.cond, !llvm.loop !12

while.end:                                        ; preds = %while.cond
  ret void
}

; Function Attrs: noinline nounwind
define i32 @early(i32 noundef %a) #0 {
entry:
  br label %while.body

while.body:                                       ; preds = %if.end4, %entry
  %call = call i32 @get()
  %cmp = icmp eq i32 %call, %a
  br i1 %cmp, label %if.then, label %if.end

if.then:                                          ; preds = %while.body
  br label %return

if.end:                                           ; preds = %while.body
  %call1 = call i32 @get()
  %cmp2 = icmp sgt i32 %call1, 99
  br i1 %cmp2, label %if.then3, label %if.end4

if.then3:                                         ; preds = %if.end
  br label %return

if.end4:                                          ; preds = %if.end
  br label %while.body

return:                                           ; preds = %if.then3, %if.then
  %retval.0 = phi i32 [ 1, %if.then ], [ 2, %if.then3 ]
  ret i32 %retval.0
}

attributes #0 = { noinline nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+bulk-memory,+bulk-memory-opt,+call-indirect-overlong,+multivalue,+mutable-globals,+nontrapping-fptoint,+reference-types,+sign-ext" }
attributes #1 = { "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+bulk-memory,+bulk-memory-opt,+call-indirect-overlong,+multivalue,+mutable-globals,+nontrapping-fptoint,+reference-types,+sign-ext" }

!llvm.module.flags = !{!0}
!llvm.ident = !{!1}

!0 = !{i32 1, !"wchar_size", i32 4}
!1 = !{!"Ubuntu clang version 20.1.8 (0ubuntu4)"}
!2 = distinct !{!2, !3}
!3 = !{!"llvm.loop.mustprogress"}
!4 = distinct !{!4, !3}
!5 = distinct !{!5, !3}
!6 = distinct !{!6, !3}
!7 = distinct !{!7, !3}
!8 = distinct !{!8, !3}
!9 = distinct !{!9, !3}
!10 = distinct !{!10, !3}
!11 = distinct !{!11, !3}
!12 = distinct !{!12, !3}
