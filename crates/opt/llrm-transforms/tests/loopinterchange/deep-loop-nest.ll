; ModuleID = '<stdin>'
source_filename = "<stdin>"
target datalayout = "e-m:e-i64:64-f80:128-n8:16:32:64-S128"

define void @big_loop_nest() {
entry:
  br label %for1.header

for1.header.loopexit:                             ; preds = %for1.inc
  %j.next.lcssa = phi i64 [ %j.next, %for1.inc ]
  br label %for1.header

for1.header:                                      ; preds = %for1.header.loopexit, %entry
  %j = phi i64 [ 0, %entry ], [ %j.next.lcssa, %for1.header.loopexit ]
  br label %for2.header

for2.header.loopexit:                             ; preds = %for2.inc
  %k.next.lcssa = phi i64 [ %k.next, %for2.inc ]
  br label %for2.header

for2.header:                                      ; preds = %for2.header.loopexit, %for1.header
  %k = phi i64 [ 0, %for1.header ], [ %k.next.lcssa, %for2.header.loopexit ]
  br label %for3.header

for3.header.loopexit:                             ; preds = %for3.inc
  %l.next.lcssa = phi i64 [ %l.next, %for3.inc ]
  br label %for3.header

for3.header:                                      ; preds = %for3.header.loopexit, %for2.header
  %l = phi i64 [ 0, %for2.header ], [ %l.next.lcssa, %for3.header.loopexit ]
  br label %for4.header

for4.header.loopexit:                             ; preds = %for4.inc
  %m.next.lcssa = phi i64 [ %m.next, %for4.inc ]
  br label %for4.header

for4.header:                                      ; preds = %for4.header.loopexit, %for3.header
  %m = phi i64 [ 0, %for3.header ], [ %m.next.lcssa, %for4.header.loopexit ]
  br label %for5.header

for5.header.loopexit:                             ; preds = %for5.inc
  %n.next.lcssa = phi i64 [ %n.next, %for5.inc ]
  br label %for5.header

for5.header:                                      ; preds = %for5.header.loopexit, %for4.header
  %n = phi i64 [ 0, %for4.header ], [ %n.next.lcssa, %for5.header.loopexit ]
  br label %for6.header

for6.header.loopexit:                             ; preds = %for6.inc
  %o.next.lcssa = phi i64 [ %o.next, %for6.inc ]
  br label %for6.header

for6.header:                                      ; preds = %for6.header.loopexit, %for5.header
  %o = phi i64 [ 0, %for5.header ], [ %o.next.lcssa, %for6.header.loopexit ]
  br label %for7.header

for7.header.loopexit:                             ; preds = %for7.inc
  %p.next.lcssa = phi i64 [ %p.next, %for7.inc ]
  br label %for7.header

for7.header:                                      ; preds = %for7.header.loopexit, %for6.header
  %p = phi i64 [ 0, %for6.header ], [ %p.next.lcssa, %for7.header.loopexit ]
  br label %for8.header

for8.header.loopexit:                             ; preds = %for8.inc
  %q.next.lcssa = phi i64 [ %q.next, %for8.inc ]
  br label %for8.header

for8.header:                                      ; preds = %for8.header.loopexit, %for7.header
  %q = phi i64 [ 0, %for7.header ], [ %q.next.lcssa, %for8.header.loopexit ]
  br label %for9.header

for9.header.loopexit:                             ; preds = %for9.inc
  %r.next.lcssa = phi i64 [ %r.next, %for9.inc ]
  br label %for9.header

for9.header:                                      ; preds = %for9.header.loopexit, %for8.header
  %r = phi i64 [ 0, %for8.header ], [ %r.next.lcssa, %for9.header.loopexit ]
  br label %for10.header

for10.header:                                     ; preds = %for10.inc, %for9.header
  %s = phi i64 [ 0, %for9.header ], [ %s.next, %for10.inc ]
  br label %for11

for11:                                            ; preds = %for11, %for10.header
  %t = phi i64 [ %t.next, %for11 ], [ 0, %for10.header ]
  %t.next = add nuw nsw i64 %t, 1
  %exitcond = icmp eq i64 %t.next, 99
  br i1 %exitcond, label %for1.inc, label %for11

for1.inc:                                         ; preds = %for11
  %j.next = add nuw nsw i64 %j, 1
  %exitcond26 = icmp eq i64 %j.next, 99
  br i1 %exitcond26, label %for2.inc, label %for1.header.loopexit

for2.inc:                                         ; preds = %for1.inc
  %k.next = add nuw nsw i64 %k, 1
  %exitcond27 = icmp eq i64 %j.next, 99
  br i1 %exitcond27, label %for3.inc, label %for2.header.loopexit

for3.inc:                                         ; preds = %for2.inc
  %l.next = add nuw nsw i64 %l, 1
  %exitcond28 = icmp eq i64 %l.next, 99
  br i1 %exitcond28, label %for4.inc, label %for3.header.loopexit

for4.inc:                                         ; preds = %for3.inc
  %m.next = add nuw nsw i64 %m, 1
  %exitcond29 = icmp eq i64 %m.next, 99
  br i1 %exitcond29, label %for5.inc, label %for4.header.loopexit

for5.inc:                                         ; preds = %for4.inc
  %n.next = add nuw nsw i64 %n, 1
  %exitcond30 = icmp eq i64 %n.next, 99
  br i1 %exitcond30, label %for6.inc, label %for5.header.loopexit

for6.inc:                                         ; preds = %for5.inc
  %o.next = add nuw nsw i64 %o, 1
  %exitcond31 = icmp eq i64 %o.next, 99
  br i1 %exitcond31, label %for7.inc, label %for6.header.loopexit

for7.inc:                                         ; preds = %for6.inc
  %p.next = add nuw nsw i64 %p, 1
  %exitcond32 = icmp eq i64 %p.next, 99
  br i1 %exitcond32, label %for8.inc, label %for7.header.loopexit

for8.inc:                                         ; preds = %for7.inc
  %q.next = add nuw nsw i64 %q, 1
  %exitcond33 = icmp eq i64 %q.next, 99
  br i1 %exitcond33, label %for9.inc, label %for8.header.loopexit

for9.inc:                                         ; preds = %for8.inc
  %r.next = add nuw nsw i64 %r, 1
  %exitcond34 = icmp eq i64 %q.next, 99
  br i1 %exitcond34, label %for10.inc, label %for9.header.loopexit

for10.inc:                                        ; preds = %for9.inc
  %s.next = add nuw nsw i64 %s, 1
  %exitcond35 = icmp eq i64 %s.next, 99
  br i1 %exitcond35, label %for.end, label %for10.header

for.end:                                          ; preds = %for10.inc
  ret void
}
