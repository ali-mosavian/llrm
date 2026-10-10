;; name: B$OEGA, B$RESN, B$FERR, B$SERR, QB_LAND
;; desc: ON ERROR GOTO, RESUME NEXT, ERR and ERROR: taking an error to the handler
;;
;; args: B$OEGA [in] erradr:dword            | the landing pad, 0 for none
;;       B$SERR [in] errnum:word             | the error to raise
;;       QB_LAND                             | from C, with the handler's offset in qb_land_to
;; retn: B$FERR ax -> the error number
;;       B$OEGA, B$FERR, B$SERR as BASIC calls them (the callee pops its words)
;;       QB_LAND, B$RESN do not return
;;
;; chng: oct/26 written [ali]
;; obs.: the program registers a landing pad with B$OEGA; the frame of the code that
;;       registered it (its bp, and its sp once the call has returned) and the
;;       call's return offset are kept. An error is taken to the pad by QB_LAND,
;;       which puts that frame back and jumps to the pad. The pad asks for the
;;       number with B$FERR and calls B$RESN, which leaves for the first row of
;;       the module's statement table at or after the registration, the row the
;;       compiler put at the code that runs the user's handler. A registration
;;       made inside a procedure (cur_level not 0) is the handler re-arming
;;       itself and keeps the frame of the main code.

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



;; error.c: record the pad and mark no error in progress; raise a BASIC error number
on_error        proto   far fastcall :word
raise           proto   far fastcall :word

OF_STA          equ     10                      ;; the statement table's offset in the module header
ROW_SIZE        equ     4                       ;; an address and a line number
NO_ROW          equ     0FFFFh

.data
landing         dword   0                       ;; where QB_LAND and B$RESN jump

.code
;;::::::::::::::
;; B$OEGA (erradr:dword)
B$OEGA          proc    public\
                        erradr:dword

                cmp     cur_level, 0
                jne     @F
                lea     ax, [bp+10]             ;; sp once the caller has the call behind it
                mov     qb_land_sp, ax
                mov     ax, [bp]
                mov     qb_land_bp, ax
                mov     ax, [bp+2]
                mov     qb_err_ip, ax
@@:
                invoke  on_error, W erradr
                ret
B$OEGA          endp

;;::::::::::::::
;; QB_LAND ()
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

;;::::::::::::::
;; B$RESN ()
B$RESN          proc    public

                add     sp, 4                   ;; the pad's call to us: not coming back
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

;;::::::::::::::
;; B$FERR ()
B$FERR          proc    public

                mov     ax, b_errnum
                ret
B$FERR          endp

;;::::::::::::::
;; B$SERR (errnum:word)
B$SERR          proc    public\
                        errnum:word

                invoke  raise, errnum
B$SERR          endp
                end
