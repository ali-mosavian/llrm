; Segments are pointers in address space 2: a cast from a far pointer gives
; its segment, one back gives segment:0, and a far pointer's low word is its
; offset. msp430 defines no such casts, and llrm's interpreter keeps one
; flat space, so neither runs it.
; expect: none
; msp430: no
target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@payload = internal addrspace(1) constant [4 x i8] c"\02\00HI"
@segment = internal global ptr addrspace(2) addrspacecast (ptr addrspace(1) @payload to ptr addrspace(2))

define ptr addrspace(1) @join(ptr addrspace(2) %segment, i16 %offset) {
  %base = addrspacecast ptr addrspace(2) %segment to ptr addrspace(1)
  %far = getelementptr i8, ptr addrspace(1) %base, i16 %offset
  ret ptr addrspace(1) %far
}

define i16 @offset(ptr addrspace(1) %far) {
  %offset = ptrtoint ptr addrspace(1) %far to i16
  ret i16 %offset
}
