; ModuleID = '<stdin>'
source_filename = "<stdin>"

define void @reduction_or(ptr %A) {
entry:
  br label %for.i.header

for.i.header:                                     ; preds = %for.i.latch, %entry
  %i = phi i32 [ 0, %entry ], [ %i.inc, %for.i.latch ]
  %or.i = phi i32 [ 0, %entry ], [ %or.i.lcssa, %for.i.latch ]
  br label %for.j

for.j:                                            ; preds = %for.j, %for.i.header
  %j = phi i32 [ 0, %for.i.header ], [ %j.inc, %for.j ]
  %or.j = phi i32 [ %or.i, %for.i.header ], [ %or.j.next, %for.j ]
  %idx = getelementptr inbounds [2 x [2 x i32]], ptr %A, i32 0, i32 %j, i32 %i
  %a = load i32, ptr %idx, align 4
  %or.j.next = or i32 %or.j, %a
  %j.inc = add i32 %j, 1
  %cmp.j = icmp slt i32 %j.inc, 2
  br i1 %cmp.j, label %for.j, label %for.i.latch

for.i.latch:                                      ; preds = %for.j
  %or.i.lcssa = phi i32 [ %or.j.next, %for.j ]
  %i.inc = add i32 %i, 1
  %cmp.i = icmp slt i32 %i.inc, 2
  br i1 %cmp.i, label %for.i.header, label %exit

exit:                                             ; preds = %for.i.latch
  ret void
}

define void @reduction_and(ptr %A) {
entry:
  br label %for.i.header

for.i.header:                                     ; preds = %for.i.latch, %entry
  %i = phi i32 [ 0, %entry ], [ %i.inc, %for.i.latch ]
  %and.i = phi i32 [ -1, %entry ], [ %and.i.lcssa, %for.i.latch ]
  br label %for.j

for.j:                                            ; preds = %for.j, %for.i.header
  %j = phi i32 [ 0, %for.i.header ], [ %j.inc, %for.j ]
  %and.j = phi i32 [ %and.i, %for.i.header ], [ %and.j.next, %for.j ]
  %idx = getelementptr inbounds [2 x [2 x i32]], ptr %A, i32 0, i32 %j, i32 %i
  %a = load i32, ptr %idx, align 4
  %and.j.next = and i32 %and.j, %a
  %j.inc = add i32 %j, 1
  %cmp.j = icmp slt i32 %j.inc, 2
  br i1 %cmp.j, label %for.j, label %for.i.latch

for.i.latch:                                      ; preds = %for.j
  %and.i.lcssa = phi i32 [ %and.j.next, %for.j ]
  %i.inc = add i32 %i, 1
  %cmp.i = icmp slt i32 %i.inc, 2
  br i1 %cmp.i, label %for.i.header, label %exit

exit:                                             ; preds = %for.i.latch
  ret void
}

define void @reduction_xor(ptr %A) {
entry:
  br label %for.i.header

for.i.header:                                     ; preds = %for.i.latch, %entry
  %i = phi i32 [ 0, %entry ], [ %i.inc, %for.i.latch ]
  %xor.i = phi i32 [ 0, %entry ], [ %xor.i.lcssa, %for.i.latch ]
  br label %for.j

for.j:                                            ; preds = %for.j, %for.i.header
  %j = phi i32 [ 0, %for.i.header ], [ %j.inc, %for.j ]
  %xor.j = phi i32 [ %xor.i, %for.i.header ], [ %xor.j.next, %for.j ]
  %idx = getelementptr inbounds [2 x [2 x i32]], ptr %A, i32 0, i32 %j, i32 %i
  %a = load i32, ptr %idx, align 4
  %xor.j.next = xor i32 %xor.j, %a
  %j.inc = add i32 %j, 1
  %cmp.j = icmp slt i32 %j.inc, 2
  br i1 %cmp.j, label %for.j, label %for.i.latch

for.i.latch:                                      ; preds = %for.j
  %xor.i.lcssa = phi i32 [ %xor.j.next, %for.j ]
  %i.inc = add i32 %i, 1
  %cmp.i = icmp slt i32 %i.inc, 2
  br i1 %cmp.i, label %for.i.header, label %exit

exit:                                             ; preds = %for.i.latch
  ret void
}

define void @reduction_smin(ptr %A, i32 %init) {
entry:
  br label %for.i.header

for.i.header:                                     ; preds = %for.i.latch, %entry
  %i = phi i32 [ 0, %entry ], [ %i.inc, %for.i.latch ]
  %smin.i = phi i32 [ %init, %entry ], [ %smin.i.lcssa, %for.i.latch ]
  br label %for.j

for.j:                                            ; preds = %for.j, %for.i.header
  %j = phi i32 [ 0, %for.i.header ], [ %j.inc, %for.j ]
  %smin.j = phi i32 [ %smin.i, %for.i.header ], [ %smin.j.next, %for.j ]
  %idx = getelementptr inbounds [2 x [2 x i32]], ptr %A, i32 0, i32 %j, i32 %i
  %a = load i32, ptr %idx, align 4
  %smin.j.next = call i32 @llvm.smin.i32(i32 %a, i32 %smin.j)
  %j.inc = add i32 %j, 1
  %cmp.j = icmp slt i32 %j.inc, 2
  br i1 %cmp.j, label %for.j, label %for.i.latch

for.i.latch:                                      ; preds = %for.j
  %smin.i.lcssa = phi i32 [ %smin.j.next, %for.j ]
  %i.inc = add i32 %i, 1
  %cmp.i = icmp slt i32 %i.inc, 2
  br i1 %cmp.i, label %for.i.header, label %exit

exit:                                             ; preds = %for.i.latch
  ret void
}

define void @reduction_smax(ptr %A, i32 %init) {
entry:
  br label %for.i.header

for.i.header:                                     ; preds = %for.i.latch, %entry
  %i = phi i32 [ 0, %entry ], [ %i.inc, %for.i.latch ]
  %smax.i = phi i32 [ %init, %entry ], [ %smax.i.lcssa, %for.i.latch ]
  br label %for.j

for.j:                                            ; preds = %for.j, %for.i.header
  %j = phi i32 [ 0, %for.i.header ], [ %j.inc, %for.j ]
  %smax.j = phi i32 [ %smax.i, %for.i.header ], [ %smax.j.next, %for.j ]
  %idx = getelementptr inbounds [2 x [2 x i32]], ptr %A, i32 0, i32 %j, i32 %i
  %a = load i32, ptr %idx, align 4
  %smax.j.next = call i32 @llvm.smax.i32(i32 %a, i32 %smax.j)
  %j.inc = add i32 %j, 1
  %cmp.j = icmp slt i32 %j.inc, 2
  br i1 %cmp.j, label %for.j, label %for.i.latch

for.i.latch:                                      ; preds = %for.j
  %smax.i.lcssa = phi i32 [ %smax.j.next, %for.j ]
  %i.inc = add i32 %i, 1
  %cmp.i = icmp slt i32 %i.inc, 2
  br i1 %cmp.i, label %for.i.header, label %exit

exit:                                             ; preds = %for.i.latch
  ret void
}

define void @reduction_umin(ptr %A, i32 %init) {
entry:
  br label %for.i.header

for.i.header:                                     ; preds = %for.i.latch, %entry
  %i = phi i32 [ 0, %entry ], [ %i.inc, %for.i.latch ]
  %umin.i = phi i32 [ %init, %entry ], [ %umin.i.lcssa, %for.i.latch ]
  br label %for.j

for.j:                                            ; preds = %for.j, %for.i.header
  %j = phi i32 [ 0, %for.i.header ], [ %j.inc, %for.j ]
  %umin.j = phi i32 [ %umin.i, %for.i.header ], [ %umin.j.next, %for.j ]
  %idx = getelementptr inbounds [2 x [2 x i32]], ptr %A, i32 0, i32 %j, i32 %i
  %a = load i32, ptr %idx, align 4
  %umin.j.next = call i32 @llvm.umin.i32(i32 %a, i32 %umin.j)
  %j.inc = add i32 %j, 1
  %cmp.j = icmp slt i32 %j.inc, 2
  br i1 %cmp.j, label %for.j, label %for.i.latch

for.i.latch:                                      ; preds = %for.j
  %umin.i.lcssa = phi i32 [ %umin.j.next, %for.j ]
  %i.inc = add i32 %i, 1
  %cmp.i = icmp slt i32 %i.inc, 2
  br i1 %cmp.i, label %for.i.header, label %exit

exit:                                             ; preds = %for.i.latch
  ret void
}

define void @reduction_umax(ptr %A) {
entry:
  br label %for.i.header

for.i.header:                                     ; preds = %for.i.latch, %entry
  %i = phi i32 [ 0, %entry ], [ %i.inc, %for.i.latch ]
  %umax.i = phi i32 [ 0, %entry ], [ %umax.i.lcssa, %for.i.latch ]
  br label %for.j

for.j:                                            ; preds = %for.j, %for.i.header
  %j = phi i32 [ 0, %for.i.header ], [ %j.inc, %for.j ]
  %umax.j = phi i32 [ %umax.i, %for.i.header ], [ %umax.j.next, %for.j ]
  %idx = getelementptr inbounds [2 x [2 x i32]], ptr %A, i32 0, i32 %j, i32 %i
  %a = load i32, ptr %idx, align 4
  %umax.j.next = call i32 @llvm.umax.i32(i32 %a, i32 %umax.j)
  %j.inc = add i32 %j, 1
  %cmp.j = icmp slt i32 %j.inc, 2
  br i1 %cmp.j, label %for.j, label %for.i.latch

for.i.latch:                                      ; preds = %for.j
  %umax.i.lcssa = phi i32 [ %umax.j.next, %for.j ]
  %i.inc = add i32 %i, 1
  %cmp.i = icmp slt i32 %i.inc, 2
  br i1 %cmp.i, label %for.i.header, label %exit

exit:                                             ; preds = %for.i.latch
  ret void
}

define void @reduction_anyof(ptr %A) {
entry:
  br label %for.i.header

for.i.header:                                     ; preds = %for.i.latch, %entry
  %i = phi i32 [ 0, %entry ], [ %i.inc, %for.i.latch ]
  %anyof.i = phi i32 [ 0, %entry ], [ %anyof.i.lcssa, %for.i.latch ]
  br label %for.j

for.j:                                            ; preds = %for.j, %for.i.header
  %j = phi i32 [ 0, %for.i.header ], [ %j.inc, %for.j ]
  %anyof.j = phi i32 [ %anyof.i, %for.i.header ], [ %anyof.j.next, %for.j ]
  %idx = getelementptr inbounds [2 x [2 x i32]], ptr %A, i32 0, i32 %j, i32 %i
  %a = load i32, ptr %idx, align 4
  %cmp = icmp eq i32 %a, 42
  %anyof.j.next = select i1 %cmp, i32 1, i32 %anyof.j
  %j.inc = add i32 %j, 1
  %cmp.j = icmp slt i32 %j.inc, 2
  br i1 %cmp.j, label %for.j, label %for.i.latch

for.i.latch:                                      ; preds = %for.j
  %anyof.i.lcssa = phi i32 [ %anyof.j.next, %for.j ]
  %i.inc = add i32 %i, 1
  %cmp.i = icmp slt i32 %i.inc, 2
  br i1 %cmp.i, label %for.i.header, label %exit

exit:                                             ; preds = %for.i.latch
  ret void
}

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare i32 @llvm.smax.i32(i32, i32) #0

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare i32 @llvm.smin.i32(i32, i32) #0

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare i32 @llvm.umax.i32(i32, i32) #0

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare i32 @llvm.umin.i32(i32, i32) #0

attributes #0 = { nocallback nofree nosync nounwind speculatable willreturn memory(none) }
