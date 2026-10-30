; ModuleID = '<stdin>'
source_filename = "<stdin>"

define void @f(ptr %A) {
entry:
  br label %for.i.header

for.i.header:                                     ; preds = %for.i.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.next, %for.i.latch ]
  %gep = getelementptr double, ptr %A, i64 %i
  br label %for.j

for.j:                                            ; preds = %for.j, %for.i.header
  %j = phi i64 [ 0, %for.i.header ], [ %j.next, %for.j ]
  %red = phi double [ 0.000000e+00, %for.i.header ], [ %red.next, %for.j ]
  %j.next = add i64 %j, 1
  %red.next = fadd reassoc double 1.000000e+00, %red
  %ec.j = icmp eq i64 %j.next, 10
  br i1 %ec.j, label %for.i.latch, label %for.j

for.i.latch:                                      ; preds = %for.j
  %red.next.lcssa = phi double [ %red.next, %for.j ]
  store double %red.next.lcssa, ptr %gep, align 8
  %i.next = add i64 %i, 1
  %ec.i = icmp eq i64 %i.next, 10
  br i1 %ec.i, label %exit, label %for.i.header

exit:                                             ; preds = %for.i.latch
  ret void
}

!llvm.module.flags = !{!0}

!0 = !{i32 2, !"Debug Info Version", i32 3}
