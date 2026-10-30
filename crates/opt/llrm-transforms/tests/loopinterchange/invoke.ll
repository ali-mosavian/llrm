; ModuleID = '<stdin>'
source_filename = "<stdin>"

@g = global i64 0

declare void @rd(ptr)

declare i32 @__gxx_personality_v0(...)

define void @invoke_may_rw() personality ptr @__gxx_personality_v0 {
entry:
  br label %i.header

i.header:                                         ; preds = %i.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.next, %i.latch ]
  br label %j.header

j.header:                                         ; preds = %j.latch, %i.header
  %j = phi i64 [ 0, %i.header ], [ %j.next, %j.latch ]
  invoke void @rd(ptr @g)
          to label %j.latch unwind label %lp

lp:                                               ; preds = %j.header
  %l = landingpad { ptr, i32 }
          cleanup
  br label %j.latch

j.latch:                                          ; preds = %lp, %j.header
  %j.next = add i64 %j, 1
  %ec.j = icmp eq i64 %j.next, 8
  br i1 %ec.j, label %i.latch, label %j.header

i.latch:                                          ; preds = %j.latch
  %i.next = add i64 %i, 1
  %ec.i = icmp eq i64 %i.next, 8
  br i1 %ec.i, label %exit, label %i.header

exit:                                             ; preds = %i.latch
  ret void
}
