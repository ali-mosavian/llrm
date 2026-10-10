;; name: start
;; desc: program start-up of x86-m16: the stack as data, the zeroed bss, the hook, main
;;
;; args: the environment DOS gives an EXE (es: the PSP)
;; retn: never returns: LL$EXIT with main's result
;;
;; chng: oct/26 restyled, symbols renamed to LL$ [ali]
;; obs.: The language's start-up hook runs before `main` when its description names one.

.model medium
.386
;; No .dosseg: its order puts every segment outside DGROUP before it, and a far one the image does
;; not store would then have its zeros stored. The order here is the linker's, by the first
;; appearance of each class: code, DGROUP (data, bss, stack), far data, far bss.
extrn _main:far
extrn BSS_LAST:byte
extrn FBSS_LAST:byte
extrn LL$TOP:word
extrn LL$PSP:word
extrn LL$STACK_LOW:word
extrn LL$EXIT:far
;; The language's start-up hook, run with the stack and the data zeroed and before `main`, when its
;; description names one (`init`): Nib's puts its fault handlers in, C's resets the FPU.
ifdef LANG_INIT
extrn LANG_INIT:far
endif

.code
.data
.data?
;; The near uninitialised data runs from here to dos.asm's BSS_LAST, linked last; the far, class
;; FAR_BSS, from FBSS_FIRST in this object to dos.asm's FBSS_LAST. The EXE stores neither.
BSS_FIRST       label   byte
;; The stack's size is the language's (STACK_BYTES, its description's stack_base): Nib keeps arrays in
;; the frame, C's frames are Open Watcom's.
.stack STACK_BYTES
FARDATA_ORDER   segment para public 'FAR_DATA'
FARDATA_ORDER   ends
FBSS_BEG        segment para public 'FAR_BSS'
public FBSS_FIRST
FBSS_FIRST      label   byte
                db      16 dup (?)
FBSS_BEG        ends

.code
start:
                mov     bx, es                  ;; the PSP, before DS leaves it
                mov     ax, DGROUP
                mov     ds, ax
                mov     es, ax
;; The stack is data (the machine's stack_is_data): SS is DGROUP and SP
;; rebased, so a near pointer to a frame cell reaches it through DS.
                mov     dx, ss
                sub     dx, ax
                shl     dx, 4
                cli
                mov     ss, ax
                add     sp, dx
                sti
;; The stack is the last of DGROUP before the heap, so it starts where the near bss ends.
;; Nothing below the limit but the panic's frames, DOS and an interrupt.
                mov     ax, offset DGROUP:BSS_LAST
                add     ax, STACK_RESERVE
                mov     LL$STACK_LOW, ax
                mov     bp, bx                  ;; the PSP, past the loops' registers
;; Statics without an initializer are in _BSS, which the EXE does not
;; store: they hold whatever the last program left there until zeroed. A test builds with NOZERO
;; to see dirty memory.
ifndef NOZERO
                mov     di, offset DGROUP:BSS_FIRST
                mov     cx, offset DGROUP:BSS_LAST
                sub     cx, di
                xor     al, al
                cld
                rep     stosb
;; The far uninitialised data, a pass of at most 64K at a time. A label's segment is its frame,
;; which the linker shares among these segments: its paragraph is the frame's plus the
;; offset's sixteenths.
                mov     bx, offset FBSS_FIRST
                shr     bx, 4
                add     bx, seg FBSS_FIRST
                mov     dx, offset FBSS_LAST
                shr     dx, 4
                add     dx, seg FBSS_LAST
far_clear:
                cmp     bx, dx
                jae     far_cleared
                mov     ax, dx
                sub     ax, bx
                cmp     ax, 1000h
                jbe     far_pass
                mov     ax, 1000h
far_pass:
                mov     es, bx
                add     bx, ax
                mov     cx, ax
                shl     cx, 3                   ;; words: eight to a paragraph
                xor     di, di
                xor     ax, ax
                rep     stosw
                jmp     far_clear
far_cleared:
endif
                mov     LL$PSP, bp
;; The near heap starts where the stack ends, the image's last byte in DGROUP; the first
;; `more` resizes the program's block to hold it. Start-up releases nothing: far data lies
;; past DGROUP, where DOS would hand it to the next allocation.
                mov     LL$TOP, sp
                push    ds
                pop     es
ifdef LANG_INIT
                call    far ptr LANG_INIT
endif
                call    far ptr _main
                push    ax
                call    far ptr LL$EXIT

end start
