;; name: _main, qb_xi_begin, qb_xi_end, qb_module_header
;; desc: the QB module's start under DOS/32A: the initializers, then the module's body
;;
;; args: the OS layer's start-up (runtime/shared/dos/m32/start.asm) calls _main with the stack set
;; retn: _main does not return: the module ends through B$CENP
;;
;; chng: oct/26 written [ali]
;; obs.: Every module with an initializer declares XIB, XI and XIE in that order (xi.h), so the first the
;;       linker meets fixes the order and the bounds below are those segments' bases.  The module's data
;;       segment BC_SA holds the address of its header; its code starts 30h bytes after that header.

                .386
                .model  flat

                public  _main
                public  qb_xi_begin
                public  qb_xi_end
                public  qb_module_header

HEADER_CODE     equ     30h                     ;; where the module's code starts
                extrn   _qb_start:near

XIB             segment dword public 'DATA'
XIB             ends
XI              segment dword public 'DATA'
XI              ends
XIE             segment dword public 'DATA'
XIE             ends

;; the module's BC_SA starts with the address of its header: a label here is that address's place
BC_SA           segment dword public 'DATA'
bc_sa           label   dword
BC_SA           ends

.data
qb_xi_begin     dd      offset XIB
qb_xi_end       dd      offset XIE
qb_module_header dd     0

.code
;;::::::::::::::
;; _main ()
_main           proc

                call    _qb_start
                mov     eax, dword ptr bc_sa
                mov     qb_module_header, eax
                add     eax, HEADER_CODE
                call    eax
                ret                             ;; a module ends through B$CENP
_main           endp
                end
