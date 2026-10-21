; ModuleID = '<stdin>'
source_filename = "<stdin>"

define void @bar(i32 %m, i32 %n) {
entry:
  %m_temp = add i32 %n, 1
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %entry
  %m_temp1 = phi i32 [ %m, %entry ], [ %m_temp, %outer.latch ]
  br label %inner.header

inner.header:                                     ; preds = %inner.latch, %outer.header
  %n_temp1 = phi i32 [ %n, %outer.header ], [ %n_temp, %inner.latch ]
  br label %body

body:                                             ; preds = %inner.header
  br label %inner.latch

inner.latch:                                      ; preds = %body
  %n_temp = add i32 %n_temp1, 1
  %cmp2 = icmp eq i32 %n_temp, 1
  br i1 %cmp2, label %outer.latch, label %inner.header

outer.latch:                                      ; preds = %inner.latch
  %cmp3 = icmp eq i32 %m_temp, 1
  br i1 %cmp3, label %exit, label %outer.header

exit:                                             ; preds = %outer.latch
  ret void
}
