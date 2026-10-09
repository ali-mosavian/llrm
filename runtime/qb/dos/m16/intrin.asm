;; name: QB$INT10, QB$INT21, QB$OUTB, QB$OUTW, QB$INB, QB$MOVE, QB$FILL, QB$ATAN2, QB$SINCOS
;; desc: what C cannot do on this target: software interrupts, I/O ports, string moves, the FPU
;;
;; args: QB$INT10, QB$INT21 [in] ax:ptr Regs     | the interrupt, with the registers from Regs
;;       QB$OUTB  [in] ax:word port, dx:word value        | a byte to the port
;;       QB$OUTW  [in] ax:word port, dx:word value        | a word: the index in the low byte
;;                                                       | and the datum in the high one, as
;;                                                       | the VGA's index/data pairs take it
;;       QB$INB   [in] ax:word port
;;       QB$MOVE  [in] ax:ptr BlockOp                     | dst, src: bytes forward; by dwords,
;;                                                       | or by bytes if `value` is not 0 (a latch
;;                                                       | copy in the EGA holds one byte)
;;       QB$FILL  [in] ax:ptr BlockOp                     | dst: words of a 16-bit value
;;       QB$ATAN2 [in] ax:ptr double y, dx:ptr double x, cx:ptr double out
;;       QB$SINCOS [in] ax:ptr double angle, dx:ptr double sine, cx:ptr double cosine
;; retn: QB$INT10, QB$INT21 Regs -> the registers and the flags as the interrupt left them
;;       QB$INB ax -> the byte
;;
;; chng: oct/26 written [ali]
;; obs.: the registers are C's (regparm3: ax, dx, cx), so no thunk is needed; the QB
;;       runtime's own device code (device.c, gfxdev.c) is C around these. Target 386+
;;       with an FPU: the moves are dwords, the arctangent and sine are the FPU's.

                .model  medium, pascal
                .386
                .387
                option  proc:private

                include qb.inc

Regs            struc
                rax             word    ?
                rbx             word    ?
                rcx             word    ?
                rdx             word    ?
                rbp             word    ?
                res             word    ?
                rflags          word    ?
Regs            ends

BlockOp         struc
                dst             dword   ?
                src             dword   ?
                count           word    ?               ;; bytes, or words for QB$FILL
                value           word    ?               ;; for QB$FILL
BlockOp         ends

.code
;;::::::::::::::
;; software interrupt, with the registers of the Regs at si
CALLINT         macro   vector:req

                push    bx
                push    si
                push    bp
                push    es
                mov     si, ax
                push    si
                mov     ax, [si].Regs.rax
                mov     bx, [si].Regs.rbx
                mov     cx, [si].Regs.rcx
                mov     dx, [si].Regs.rdx
                int     vector
                pop     si
                mov     [si].Regs.rax, ax
                mov     [si].Regs.rbx, bx
                mov     [si].Regs.rcx, cx
                mov     [si].Regs.rdx, dx
                mov     [si].Regs.rbp, bp
                mov     ax, es
                mov     [si].Regs.res, ax
                pushf
                pop     W [si].Regs.rflags
                pop     es
                pop     bp
                pop     si
                pop     bx
                ret
                endm

;;::::::::::::::
;; QB$INT10 (ax: regs)
QB$INT10         proc    public

                CALLINT 10h
QB$INT10         endp

;;::::::::::::::
;; QB$INT21 (ax: regs)
QB$INT21         proc    public

                CALLINT 21h
QB$INT21         endp

;;::::::::::::::
;; QB$OUTB (ax: port, dx: value)
QB$OUTB          proc    public

                xchg    ax, dx                  ;; dx the port, al the byte
                out     dx, al
                ret
QB$OUTB          endp

;;::::::::::::::
;; QB$OUTW (ax: port, dx: value)
QB$OUTW          proc    public

                xchg    ax, dx                  ;; dx the port, ax the word
                out     dx, ax
                ret
QB$OUTW          endp

;;::::::::::::::
;; QB$INB (ax: port)
QB$INB           proc    public

                mov     dx, ax
                in      al, dx
                xor     ah, ah
                ret
QB$INB           endp

;;::::::::::::::
;; QB$MOVE (ax: op)
QB$MOVE          proc    public

                push    si
                push    di
                push    ds
                push    es
                mov     bx, ax
                les     di, [bx].BlockOp.dst
                mov     cx, [bx].BlockOp.count
                mov     dx, [bx].BlockOp.value
                lds     si, [bx].BlockOp.src
                cld
                test    dx, dx
                jnz     @F
                mov     ax, cx
                shr     cx, 2
                rep     movsd
                mov     cx, ax
                and     cx, 3
@@:
                rep     movsb
                pop     es
                pop     ds
                pop     di
                pop     si
                ret
QB$MOVE          endp

;;::::::::::::::
;; QB$FILL (ax: op)
QB$FILL          proc    public

                push    di
                push    es
                mov     bx, ax
                les     di, [bx].BlockOp.dst
                movzx   eax, [bx].BlockOp.value
                mov     edx, eax
                shl     edx, 16
                or      eax, edx                ;; the word twice
                mov     cx, [bx].BlockOp.count
                cld
                shr     cx, 1
                rep     stosd
                jnc     @F
                stosw
@@:
                pop     es
                pop     di
                ret
QB$FILL          endp

;;::::::::::::::
;; QB$ATAN2 (ax: y, dx: x, cx: out)
QB$ATAN2         proc    public

                push    si
                mov     si, cx
                mov     bx, ax
                fld     qword ptr [bx]
                mov     bx, dx
                fld     qword ptr [bx]
                fpatan                          ;; atan(st1 / st0), in (-pi, pi]
                fstp    qword ptr [si]
                fwait
                pop     si
                ret
QB$ATAN2         endp

;;::::::::::::::
;; QB$SINCOS (ax: angle, dx: sine, cx: cosine)
QB$SINCOS        proc    public

                push    si
                mov     si, cx
                mov     bx, ax
                fld     qword ptr [bx]
                fsincos                         ;; st0 the cosine, st1 the sine
                fstp    qword ptr [si]
                mov     bx, dx
                fstp    qword ptr [bx]
                fwait
                pop     si
                ret
QB$SINCOS        endp
                end
