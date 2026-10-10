;; name: B$ENRA, B$EXSA
;; desc: BASIC procedure frames, flat
;;
;; args: B$ENRA [in] eax               | bytes of locals, a multiple of 4
;;       B$EXSA [in] ebp               | the frame B$ENRA built
;;                   eax               | the function's result, kept
;; retn: B$ENRA ebp -> the new frame, esp below its locals
;;       B$EXSA ebx, ecx, edx, esi, edi, ebp and b_curframe as the caller had them
;;
;; chng: oct/26 written [ali]
;; obs.: the frame below ebp, a dword each:
;;
;;           ebp+0    previous ebp
;;           ebp-4    previous BASIC frame (b_curframe)
;;           ebp-8    ebx
;;           ebp-12   ecx
;;           ebp-16   edx
;;           ebp-20   esi
;;           ebp-24   edi
;;           ebp-28   bytes of locals
;;           ebp-32   GOSUB count
;;           ...      the locals, zeroed
;;
;;       The procedure's body uses any register, so the frame keeps those a flat caller expects kept.  A
;;       call of either entry takes the frame apart and puts it together again, below the stack the
;;       entry was called with, using no memory below esp: an interrupt may come at any time.

                .386
                .model  flat

                extrn   _cur_level:dword        ;; nhstutil.c

                public  _b_curframe
                public  B$ENRA
                public  B$EXSA

FR_BFRAME       equ     -4
FR_EBX          equ     -8
FR_ECX          equ     -12
FR_EDX          equ     -16
FR_ESI          equ     -20
FR_EDI          equ     -24
FR_LOCALS       equ     -28
FR_GOSUB        equ     -32
FR_SIZE         equ     32                      ;; the header, ebp-32 .. ebp-1

.data
_b_curframe     dd      0                       ;; frame of the procedure running

.code
;;::::::::::::::
;; B$ENRA ()
B$ENRA          proc

                sub     esp, 4
                mov     [esp], eax              ;; the bytes of locals
                mov     eax, [esp+4]            ;; the way back
                mov     [esp+4], ebp            ;; the previous frame, where it was
                lea     ebp, [esp+4]
                sub     esp, FR_SIZE - 4
                mov     [ebp+FR_EBX], ebx
                mov     [ebp+FR_ECX], ecx
                mov     [ebp+FR_EDX], edx
                mov     [ebp+FR_ESI], esi
                mov     [ebp+FR_EDI], edi
                mov     ecx, [ebp-4]
                mov     [ebp+FR_LOCALS], ecx
                mov     edx, _b_curframe        ;; chain this frame to the last
                mov     [ebp+FR_BFRAME], edx
                mov     dword ptr [ebp+FR_GOSUB], 0
                mov     _b_curframe, ebp
                inc     _cur_level

                sub     esp, ecx                ;; the locals, zeroed by dwords
                mov     edx, eax
                xor     eax, eax
                shr     ecx, 2
                mov     edi, esp
                rep     stosd
                jmp     edx
B$ENRA          endp

;;::::::::::::::
;; B$EXSA ()
B$EXSA          proc

                mov     edx, [esp]              ;; the way back
                dec     _cur_level
                mov     ecx, [ebp+FR_BFRAME]
                mov     _b_curframe, ecx
                mov     ecx, [ebp+FR_ECX]
                mov     [ebp-4], ecx            ;; ecx waits where the chain was
                mov     ecx, [ebp]              ;; the previous ebp
                mov     [ebp], edx              ;; the way back, where it was
                mov     ebx, [ebp+FR_EBX]
                mov     esi, [ebp+FR_ESI]
                mov     edi, [ebp+FR_EDI]
                mov     edx, [ebp+FR_EDX]
                lea     esp, [ebp-4]
                mov     ebp, ecx
                pop     ecx
                ret
B$EXSA          endp
                end
