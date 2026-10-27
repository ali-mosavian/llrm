target datalayout = "e-p:16:16-p1:32:16:16:16-i32:16-i64:16"

define i32 @atomics(ptr %p, i32 %v) {
entry:
  %a = load atomic i32, ptr %p monotonic, align 4
  %b = load atomic volatile i32, ptr %p seq_cst, align 4
  store atomic i32 %v, ptr %p release, align 4
  store atomic volatile i32 %v, ptr %p syncscope("singlethread") seq_cst, align 4
  %c = atomicrmw xchg ptr %p, i32 %v acquire
  %d = atomicrmw volatile add ptr %p, i32 %v syncscope("singlethread") seq_cst, align 4
  %e = atomicrmw umax ptr %p, i32 %v monotonic
  %f = cmpxchg ptr %p, i32 %a, i32 %v acq_rel acquire, align 4
  %g = cmpxchg weak volatile ptr %p, i32 %a, i32 %v seq_cst seq_cst
  %h = extractvalue { i32, i1 } %f, 0
  fence acquire
  fence syncscope("singlethread") seq_cst
  %s = add i32 %a, %b
  %t = add i32 %s, %c
  %u = add i32 %t, %d
  %w = add i32 %u, %e
  %x = add i32 %w, %h
  ret i32 %x
}
