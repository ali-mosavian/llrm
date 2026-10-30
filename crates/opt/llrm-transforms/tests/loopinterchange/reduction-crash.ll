; ModuleID = '<stdin>'
source_filename = "<stdin>"

define void @reduction_smax_store(ptr %A) {
entry:
  br label %for.i.header

for.i.header:                                     ; preds = %for.i.latch, %entry
  %i = phi i32 [ 0, %entry ], [ %i.inc, %for.i.latch ]
  %max.i = phi i32 [ 0, %entry ], [ %max.i.lcssa, %for.i.latch ]
  br label %for.j

for.j:                                            ; preds = %for.j, %for.i.header
  %j = phi i32 [ 0, %for.i.header ], [ %j.inc, %for.j ]
  %max.j = phi i32 [ %max.i, %for.i.header ], [ %max.j.next, %for.j ]
  %idx = getelementptr inbounds [2 x [2 x i32]], ptr %A, i32 0, i32 %j, i32 %i
  %a = load i32, ptr %idx, align 4
  %max.j.next = tail call i32 @llvm.smax.i32(i32 %max.j, i32 %a)
  store i32 %max.j.next, ptr %idx, align 4
  %j.inc = add i32 %j, 1
  %cmp.j = icmp slt i32 %j.inc, 2
  br i1 %cmp.j, label %for.j, label %for.i.latch

for.i.latch:                                      ; preds = %for.j
  %max.i.lcssa = phi i32 [ %max.j.next, %for.j ]
  %i.inc = add i32 %i, 1
  %cmp.i = icmp slt i32 %i.inc, 2
  br i1 %cmp.i, label %for.i.header, label %exit

exit:                                             ; preds = %for.i.latch
  ret void
}

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare i32 @llvm.smax.i32(i32, i32) #0

attributes #0 = { nocallback nofree nosync nounwind speculatable willreturn memory(none) }
