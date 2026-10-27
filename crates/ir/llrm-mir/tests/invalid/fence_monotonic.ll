; invalid: fence instructions may only have acquire, release, acq_rel, or seq_cst ordering.
define void @f() {
  fence monotonic
  ret void
}
