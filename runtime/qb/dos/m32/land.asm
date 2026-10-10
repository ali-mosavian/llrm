;; name: B$OEGA, B$RESN, B$FERR, B$SERR, QB_LAND
;; desc: ON ERROR GOTO, RESUME NEXT, ERR and ERROR: taking an error to the handler, flat
;;
;; args: B$OEGA [in] erradr:dword            | the landing pad, 0 for none
;;       B$SERR [in] errnum:dword            | the error to raise
;;       QB_LAND                             | from C, with the handler's address in qb_land_to
;; retn: B$FERR eax -> the error number
;;       B$OEGA, B$SERR as BASIC calls them (the callee pops its dword)
;;       QB_LAND, B$RESN do not return
;;
;; chng: oct/26 written [ali]
;; obs.: as the real-mode entries: the program registers a landing pad with B$OEGA; the frame of the code that
;;       registered it (its ebp, and its esp once the call has returned) and the call's return address are kept.
;;       An error is taken to the pad by QB_LAND, which puts that frame back and jumps to the pad. The pad asks for
;;       the number with B$FERR and calls B$RESN, which leaves for the first row of the module's statement table
;;       at or after the registration, the row the compiler put at the code that runs the user's handler. A
;;       registration made inside a procedure (cur_level not 0) is the handler re-arming itself and keeps the
;;       frame of the main code.  The table is rows of an address and a line, a dword each, and a row of 0.

                .386
                .model  flat

                extrn   _cur_level:dword
                extrn   _b_errnum:dword
                extrn   qb_module_header:dword
                extrn   _qb_land_sp:dword
                extrn   _qb_land_bp:dword
                extrn   _qb_land_to:dword
                extrn   _qb_err_ip:dword
                extrn   B$CEND:near
                extrn   @on_error@2:near
                extrn   @raise@2:near

                public  B$OEGA
                public  B$RESN
                public  B$FERR
                public  B$SERR
                public  QB_LAND

OF_STA          equ     12                      ;; the statement table's address in the module header
ROW_SIZE        equ     8                       ;; an address and a line number
NO_ROW          equ     0FFFFFFFFh

.code
;;::::::::::::::
;; B$OEGA (erradr:dword)
B$OEGA          proc

                cmp     _cur_level, 0
                jne     @F
                lea     eax, [esp+8]            ;; esp once the caller has the call behind it
                mov     _qb_land_sp, eax
                mov     _qb_land_bp, ebp
                mov     eax, [esp]
                mov     _qb_err_ip, eax
@@:
                mov     eax, [esp+4]
                call    @on_error@2
                ret     4
B$OEGA          endp

;;::::::::::::::
;; QB_LAND ()
QB_LAND         proc

                mov     eax, _qb_land_to
                cli
                mov     esp, _qb_land_sp
                mov     ebp, _qb_land_bp
                sti
                cld
                jmp     eax
QB_LAND         endp

;;::::::::::::::
;; B$RESN ()
B$RESN          proc

                add     esp, 4                  ;; the pad's call to us: not coming back
                mov     ebx, qb_module_header
                mov     ebx, [ebx+OF_STA]
                mov     ecx, _qb_err_ip
                mov     edx, NO_ROW
@@next:
                mov     eax, [ebx]
                test    eax, eax
                jz      @F
                cmp     eax, ecx
                jb      @@skip
                cmp     eax, edx
                jae     @@skip
                mov     edx, eax
@@skip:
                add     ebx, ROW_SIZE
                jmp     @@next
@@:
                cmp     edx, NO_ROW
                je      @F                      ;; off the bottom: END
                jmp     edx
@@:
                jmp     B$CEND
B$RESN          endp

;;::::::::::::::
;; B$FERR ()
B$FERR          proc

                mov     eax, _b_errnum
                ret
B$FERR          endp

;;::::::::::::::
;; B$SERR (errnum:dword)
B$SERR          proc

                movzx   eax, word ptr [esp+4]
                call    @raise@2
B$SERR          endp
                end
