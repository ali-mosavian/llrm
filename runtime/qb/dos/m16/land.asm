;; name: B$OEGA, B$RESN, B$FERR, B$SERR, QB_LAND
;; desc: ON ERROR GOTO, RESUME NEXT, ERR and ERROR: taking an error to the
;;       handler
;;
;; args: B$OEGA [in] erradr:dword            | the landing pad, 0 for none
;;       B$SERR [in] errnum:word             | the error to raise
;;       QB_LAND                             | from C, with the handler's offset
;;                                             in qb_land_to
;; retn: B$FERR ax -> the error number
;;       B$OEGA, B$FERR, B$SERR as BASIC calls them (the callee pops its words)
;;       QB_LAND, B$RESN do not return
;;
;; chng: oct/26 written [ali]
;; obs.: the program registers a landing pad with B$OEGA; the frame of the code
;;       that registered it (its bp, and its sp once the call has returned) and
;;       the call's return offset are kept. An error is taken to the pad by
;;       QB_LAND, which puts that frame back and jumps to the pad. The pad asks
;;       for the number with B$FERR and calls B$RESN, which leaves for the first
;;       row of the module's statement table at or after the registration, the
;;       row the compiler put at the code that runs the user's handler. A
;;       registration made inside a procedure (cur_level not 0) is the handler
;;       re-arming itself and keeps the frame of the main code.

                .model  medium, pascal
                .386
                option  proc:private

                include qb.inc

                extrn   c cur_level:word
                extrn   c b_errnum:word
                extrn   c qb_module_segment:word
                extrn   c qb_land_sp:word
                extrn   c qb_land_bp:word
                extrn   c qb_land_to:word
                extrn   c qb_err_ip:word
                extrn   B$CEND:far



;; error.c: record the pad and mark no error in progress; raise a BASIC error
;; number
on_error        proto   far fastcall :word
raise           proto   far fastcall :word

;; the statement table's offset in the module header
OF_STA          equ     10
ROW_SIZE        equ     4                       ;; an address and a line number
NO_ROW          equ     0FFFFh

.data
landing         dword   0                       ;; where QB_LAND and B$RESN jump

.code
;; B$OEGA - register the landing pad of ON ERROR GOTO.
;;
;; In the main code (cur_level 0) the caller's frame is kept: its bp, its sp
;; once this call has returned, and the call's return offset; a call that
;; disarms (0) keeps nothing.
;;
;; Args:
;;     erradr (bp+6): the landing pad, 0 for none.  The callee pops it.
;;
;; Saves:    none
;; Clobbers: ax, flags
B$OEGA          proc    public\
                        erradr:dword

                cmp     cur_level, 0
                jne     @F
                mov     ax, [bp+6]
                or      ax, [bp+8]
                jz      @F                      ;; disarming: the frame kept is the armed one's
                ;; sp once the caller has the call behind it
                lea     ax, [bp+10]
                mov     qb_land_sp, ax
                mov     ax, [bp]
                mov     qb_land_bp, ax
                mov     ax, [bp+2]
                mov     qb_err_ip, ax
@@:
                invoke  on_error, W erradr
                ret
B$OEGA          endp

;; QB_LAND - take an error to the landing pad.
;;
;; Puts the registered frame back and jumps to the pad.  It does not return.
;;
;; Args:
;;     qb_land_to: the pad's offset (set by C).
;;
;; Saves:    none
;; Clobbers: ax, flags (it does not return)
QB_LAND         proc    public

                mov     ax, qb_land_to
                mov     W landing, ax
                mov     ax, qb_module_segment
                mov     W landing+2, ax
                cli
                mov     sp, qb_land_sp
                mov     bp, qb_land_bp
                sti
                cld
                jmp     D landing
QB_LAND         endp

;; B$RESN - RESUME NEXT: leave the pad for the user's handler.
;;
;; Jumps to the first row of the module's statement table at or after the
;; registration, or to B$CEND when there is none.  It does not return.
;;
;; Args:
;;     none.
;;
;; Saves:    none
;; Clobbers: ax bx cx dx es, flags (it does not return)
B$RESN          proc    public

                ;; the pad's call to us: not coming back
                add     sp, 4
                mov     es, qb_module_segment
                mov     bx, es:[OF_STA]
                mov     cx, qb_err_ip
                mov     dx, NO_ROW
@@next:
                mov     ax, es:[bx]
                test    ax, ax
                jz      @F
                cmp     ax, cx
                jb      @@skip
                cmp     ax, dx
                jae     @@skip
                mov     dx, ax
@@skip:
                add     bx, ROW_SIZE
                jmp     @@next
@@:
                cmp     dx, NO_ROW
                je      @F                      ;; off the bottom: END
                mov     W landing, dx
                mov     W landing+2, es
                jmp     D landing
@@:
                jmp     B$CEND
B$RESN          endp

;; B$FERR - ERR: the number of the error.
;;
;; Args:
;;     none.
;;
;; Returns:
;;     ax: the error number.
;;
;; Saves:    none
;; Clobbers: ax
B$FERR          proc    public

                mov     ax, b_errnum
                ret
B$FERR          endp

;; B$SERR - ERROR: raise a BASIC error.
;;
;; Args:
;;     errnum (bp+6): the error number.  The callee pops it.
;;
;; Saves:    none
;; Clobbers: ax, flags (it does not return)
B$SERR          proc    public\
                        errnum:word

                invoke  raise, errnum
B$SERR          endp
                end
