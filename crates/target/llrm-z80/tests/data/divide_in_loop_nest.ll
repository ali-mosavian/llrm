target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"

@q = global i64 0
@u = global i64 0
@va = global i64 0
@vb = global i64 0
@sink = global i16 0

define void @run() optsize {
entry:
  br label %outer
outer:
  %i = phi i16 [ 0, %entry ], [ %i.next, %latch ]
  br label %inner
inner:
  %j = phi i16 [ 0, %outer ], [ %j.next, %inner ]
  %a = load volatile i64, ptr @va
  %b = load volatile i64, ptr @vb
  call void @divide(i64 %a, i64 %b)
  %j.next = add i16 %j, 1
  %j.done = icmp eq i16 %j.next, 100
  br i1 %j.done, label %latch, label %inner
latch:
  %i.next = add i16 %i, 1
  %i.done = icmp eq i16 %i.next, 100
  br i1 %i.done, label %exit, label %outer
exit:
  ret void
}

define internal void @divide(i64 %a, i64 %b) noinline optsize {
  %s = sdiv i64 %a, %b
  store i64 %s, ptr @q
  %t = udiv i64 %a, %b
  store i64 %t, ptr @u
  ret void
}
