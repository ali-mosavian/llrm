; ModuleID = '<stdin>'
source_filename = "<stdin>"

define void @test_padding_nontrivial_direction(ptr noalias %S, ptr noalias %D) {
entry:
  br label %loop.i.header

loop.i.header:                                    ; preds = %loop.i.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.next, %loop.i.latch ]
  br label %loop.j.header

loop.j.header:                                    ; preds = %loop.j.latch, %loop.i.header
  %j = phi i64 [ 0, %loop.i.header ], [ %j.next, %loop.j.latch ]
  %s.ptr = getelementptr i32, ptr %S, i64 %j
  br label %loop.k.header

loop.k.header:                                    ; preds = %loop.k.latch, %loop.j.header
  %k = phi i64 [ 0, %loop.j.header ], [ %k.next, %loop.k.latch ]
  %sum = phi i32 [ 0, %loop.j.header ], [ %sum.next, %loop.k.latch ]
  %k.trunc = trunc i64 %k to i32
  %sum.next = add nsw i32 %sum, %k.trunc
  br label %loop.k.latch

loop.k.latch:                                     ; preds = %loop.k.header
  %k.next = add nuw nsw i64 %k, 1
  %k.done = icmp eq i64 %k.next, 32
  br i1 %k.done, label %loop.j.latch, label %loop.k.header

loop.j.latch:                                     ; preds = %loop.k.latch
  %sum.lcssa = phi i32 [ %sum.next, %loop.k.latch ]
  %jm1 = add i64 %j, -1
  %s.prev = getelementptr i32, ptr %S, i64 %jm1
  %s.load = load i32, ptr %s.prev, align 4
  %val = add nsw i32 %sum.lcssa, %s.load
  store i32 %val, ptr %s.ptr, align 4
  %j.next = add nuw nsw i64 %j, 1
  %j.done = icmp eq i64 %j.next, 32
  br i1 %j.done, label %loop.x.header.preheader, label %loop.j.header

loop.x.header.preheader:                          ; preds = %loop.j.latch
  br label %loop.x.header

loop.x.header:                                    ; preds = %loop.x.header.preheader, %loop.x.latch
  %x = phi i64 [ %x.next, %loop.x.latch ], [ 0, %loop.x.header.preheader ]
  %d.ptr = getelementptr i8, ptr %D, i64 %x
  store i8 0, ptr %d.ptr, align 1
  br label %loop.x.latch

loop.x.latch:                                     ; preds = %loop.x.header
  %x.next = add nuw nsw i64 %x, 1
  %x.done = icmp eq i64 %x.next, 32
  br i1 %x.done, label %loop.i.latch, label %loop.x.header

loop.i.latch:                                     ; preds = %loop.x.latch
  %i.next = add nuw nsw i64 %i, 1
  %i.done = icmp eq i64 %i.next, 32
  br i1 %i.done, label %exit, label %loop.i.header

exit:                                             ; preds = %loop.i.latch
  ret void
}
