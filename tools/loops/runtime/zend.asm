; Where the uninitialised data ends, near and far, which crt.asm zeroes: this object is linked last.
.model medium
.386
public BSS_LAST
public FBSS_LAST
_BSS segment word public 'BSS'
BSS_LAST label byte
_BSS ends
FBSS_END segment para public 'FAR_BSS'
FBSS_LAST label byte
        db 16 dup (?)
FBSS_END ends
end
