; invalid: fence instructions may only have acquire, release, acq_rel, or seq_cst ordering.
target datalayout = "e-p:16:16-i32:16-i64:16"

define void @f() {
entry:
  fence monotonic
  ret void
}
