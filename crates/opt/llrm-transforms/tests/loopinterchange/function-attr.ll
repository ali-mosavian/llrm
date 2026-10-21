; ModuleID = '<stdin>'
source_filename = "<stdin>"

@ARR_100 = global [100 x i8] zeroinitializer

define void @call_writeonly(ptr %A) {
entry:
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.next, %outer.latch ]
  br label %inner

inner:                                            ; preds = %inner, %outer.header
  %j = phi i64 [ 0, %outer.header ], [ %j.next, %inner ]
  call void @writeonly(ptr %A)
  %j.next = add i64 %j, 1
  %ec.inner = icmp eq i64 %j.next, 100
  br i1 %ec.inner, label %outer.latch, label %inner

outer.latch:                                      ; preds = %inner
  %i.next = add i64 %i, 1
  %ec.outer = icmp eq i64 %i.next, 100
  br i1 %ec.outer, label %exit, label %outer.header

exit:                                             ; preds = %outer.latch
  ret void
}

define void @call_mem_none(ptr noalias %A, ptr noalias %B) {
entry:
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.next, %outer.latch ]
  br label %inner

inner:                                            ; preds = %inner, %outer.header
  %j = phi i64 [ 0, %outer.header ], [ %j.next, %inner ]
  %gep.A = getelementptr [100 x i32], ptr %A, i64 %j, i64 %i
  store i32 0, ptr %gep.A, align 4
  %val = call i32 @mem_none(ptr %A)
  %gep.B = getelementptr [100 x i32], ptr %B, i64 %j, i64 %i
  store i32 %val, ptr %gep.B, align 4
  %j.next = add i64 %j, 1
  %ec.inner = icmp eq i64 %j.next, 100
  br i1 %ec.inner, label %outer.latch, label %inner

outer.latch:                                      ; preds = %inner
  %i.next = add i64 %i, 1
  %ec.outer = icmp eq i64 %i.next, 100
  br i1 %ec.outer, label %exit, label %outer.header

exit:                                             ; preds = %outer.latch
  ret void
}

define void @call_throw_exception(ptr %A) {
entry:
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.next, %outer.latch ]
  br label %inner.header

inner.header:                                     ; preds = %inner.latch, %outer.header
  %j = phi i64 [ 0, %outer.header ], [ %j.next, %inner.latch ]
  %gep = getelementptr [10 x i8], ptr %A, i64 %j, i64 %i
  store i8 0, ptr %gep, align 1
  %cond = icmp eq i64 %j, 1
  br i1 %cond, label %unwind, label %inner.latch

unwind:                                           ; preds = %inner.header
  call void @throw_exception()
  br label %inner.latch

inner.latch:                                      ; preds = %unwind, %inner.header
  %j.next = add i64 %j, 1
  %ec.inner = icmp eq i64 %j.next, 10
  br i1 %ec.inner, label %outer.latch, label %inner.header

outer.latch:                                      ; preds = %inner.latch
  %i.next = add i64 %i, 1
  %ec.outer = icmp eq i64 %i.next, 10
  br i1 %ec.outer, label %exit, label %outer.header

exit:                                             ; preds = %outer.latch
  ret void
}

define void @call_not_return() {
entry:
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.next, %outer.latch ]
  br label %inner.header

inner.header:                                     ; preds = %inner.latch, %outer.header
  %j = phi i64 [ 0, %outer.header ], [ %j.next, %inner.latch ]
  %gep = getelementptr [10 x i8], ptr @ARR_100, i64 %i, i64 %j
  store i8 0, ptr %gep, align 1
  %cond = icmp eq i64 %j, 1
  br i1 %cond, label %trap, label %inner.latch

trap:                                             ; preds = %inner.header
  call void @not_return()
  br label %inner.latch

inner.latch:                                      ; preds = %trap, %inner.header
  %j.next = add i64 %j, 1
  %ec.inner = icmp eq i64 %j.next, 10
  br i1 %ec.inner, label %outer.latch, label %inner.header

outer.latch:                                      ; preds = %inner.latch
  %i.next = add i64 %i, 1
  %ec.outer = icmp eq i64 %i.next, 1000
  br i1 %ec.outer, label %exit, label %outer.header

exit:                                             ; preds = %outer.latch
  ret void
}

; Function Attrs: nounwind willreturn memory(write)
declare void @writeonly(ptr) #0

; Function Attrs: nounwind willreturn memory(none)
declare void @mem_none(ptr) #1

; Function Attrs: willreturn memory(none)
declare void @throw_exception() #2

; Function Attrs: nounwind memory(none)
declare void @not_return() #3

attributes #0 = { nounwind willreturn memory(write) }
attributes #1 = { nounwind willreturn memory(none) }
attributes #2 = { willreturn memory(none) }
attributes #3 = { nounwind memory(none) }
