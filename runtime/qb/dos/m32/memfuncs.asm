; memcpy_, memmove_ and memset_ (and the stack-convention _memcpy, _memmove, _memset) for the QB runtime: its own C sources are compiled
; with the C runtime's `libcalls`. The one copy is shared with ext.asm.
.386
.model flat

include ../../../shared/dos/m32/memfuncs.inc

end
