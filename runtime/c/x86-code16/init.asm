; C's start-up hook on real-mode DOS (c.toml's `init`), which the OS layer's start-up calls before
; `main`: the FPU, which DOS leaves as the last program did.
.model medium
.386

public _llrm_c_init

.code
_llrm_c_init proc far
    fninit
    retf
_llrm_c_init endp

end
