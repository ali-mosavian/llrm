	.386p
	.387
	ifndef	??version
?debug	macro
	endm
publicdll macro	name
	public	name
	endm
$comm	macro	name,dist,size,count
	comm	dist name:BYTE:count*size
	endm
	else
$comm	macro	name,dist,size,count
	comm	dist name[size]:BYTE:count
	endm
	endif
	?debug	V 300h
	?debug	S "cncaller.c"
	?debug	C E992603D5D0A636E63616C6C65722E63
	?debug	C E992603D5D04636E2E68
	?debug	C E992603D5D0863616C6C65722E68
	?debug	C E992603D5D0763617365732E68
CNCALLER_TEXT	segment byte public use16 'CODE'
CNCALLER_TEXT	ends
DGROUP	group	_DATA,_BSS
	assume	cs:CNCALLER_TEXT,ds:DGROUP
_DATA	segment word public use16 'DATA'
d@	label	byte
d@w	label	word
_DATA	ends
_BSS	segment word public use16 'BSS'
b@	label	byte
b@w	label	word
_BSS	ends
CNCALLER_TEXT	segment byte public use16 'CODE'
   ;	
   ;	void far call_scalars(void) {
   ;	
	assume	cs:CNCALLER_TEXT
_call_scalars	proc	far
	push	bp
	mov	bp,sp
   ;	
   ;	    SCALARS(CALL)
   ;	
	push	8738
	mov	al,byte ptr DGROUP:_in_sc
	push	ax
	push	4369
	call	near ptr _p_sc
	add	sp,6
	call	near ptr _r_sc
	mov	byte ptr DGROUP:_out_sc,al
	push	8738
	mov	al,byte ptr DGROUP:_in_uc
	push	ax
	push	4369
	call	near ptr _p_uc
	add	sp,6
	call	near ptr _r_uc
	mov	byte ptr DGROUP:_out_uc,al
	push	8738
	mov	al,byte ptr DGROUP:_in_ch
	push	ax
	push	4369
	call	near ptr _p_ch
	add	sp,6
	call	near ptr _r_ch
	mov	byte ptr DGROUP:_out_ch,al
	push	8738
	push	word ptr DGROUP:_in_i
	push	4369
	call	near ptr _p_i
	add	sp,6
	call	near ptr _r_i
	mov	word ptr DGROUP:_out_i,ax
	push	8738
	push	word ptr DGROUP:_in_u
	push	4369
	call	near ptr _p_u
	add	sp,6
	call	near ptr _r_u
	mov	word ptr DGROUP:_out_u,ax
	push	8738
	push	dword ptr DGROUP:_in_l
	push	4369
	call	near ptr _p_l
	add	sp,8
	call	near ptr _r_l
	shl	eax,16
	shrd	eax,edx,16
	mov	dword ptr DGROUP:_out_l,eax
	push	8738
	push	dword ptr DGROUP:_in_ul
	push	4369
	call	near ptr _p_ul
	add	sp,8
	call	near ptr _r_ul
	shl	eax,16
	shrd	eax,edx,16
	mov	dword ptr DGROUP:_out_ul,eax
	push	8738
	push	word ptr DGROUP:_in_np
	push	4369
	call	near ptr _p_np
	add	sp,6
	call	near ptr _r_np
	mov	word ptr DGROUP:_out_np,ax
	push	8738
	push	dword ptr DGROUP:_in_fp
	push	4369
	call	near ptr _p_fp
	add	sp,8
	call	near ptr _r_fp
	mov	word ptr DGROUP:_out_fp+2,dx
	mov	word ptr DGROUP:_out_fp,ax
	push	8738
	push	dword ptr DGROUP:_in_hp
	push	4369
	call	near ptr _p_hp
	add	sp,8
	call	near ptr _r_hp
	mov	word ptr DGROUP:_out_hp+2,dx
	mov	word ptr DGROUP:_out_hp,ax
	push	8738
	fld	dword ptr DGROUP:_in_f
	sub	sp,4
	fstp	dword ptr [bp-6]
	push	4369
	fwait	
	call	near ptr _p_f
	add	sp,8
	call	near ptr _r_f
	fstp	dword ptr DGROUP:_out_f
	push	8738
	fld	qword ptr DGROUP:_in_d
	sub	sp,8
	fstp	qword ptr [bp-10]
	push	4369
	fwait	
	call	near ptr _p_d
	add	sp,12
	call	near ptr _r_d
	fstp	qword ptr DGROUP:_out_d
	push	8738
	fld	tbyte ptr DGROUP:_in_ld
	sub	sp,10
	fstp	tbyte ptr [bp-12]
	push	4369
	fwait	
	call	near ptr _p_ld
	add	sp,14
	call	near ptr _r_ld
	fstp	tbyte ptr DGROUP:_out_ld
   ;	
   ;	    NARROW(CALL_WIDE)
   ;	
	push	8738
	fwait	
	mov	al,byte ptr DGROUP:_in_sc
	push	ax
	push	4369
	call	near ptr _w_sc
	add	sp,6
	call	near ptr _r_sc
	cbw	
	movsx	eax,ax
	mov	dword ptr DGROUP:_ow_sc,eax
	push	8738
	mov	al,byte ptr DGROUP:_in_uc
	push	ax
	push	4369
	call	near ptr _w_uc
	add	sp,6
	call	near ptr _r_uc
	mov	ah,0
	movsx	eax,ax
	mov	dword ptr DGROUP:_ow_uc,eax
	push	8738
	mov	al,byte ptr DGROUP:_in_ch
	push	ax
	push	4369
	call	near ptr _w_ch
	add	sp,6
	call	near ptr _r_ch
	cbw	
	movsx	eax,ax
	mov	dword ptr DGROUP:_ow_ch,eax
	push	8738
	push	word ptr DGROUP:_in_i
	push	4369
	call	near ptr _w_i
	add	sp,6
	call	near ptr _r_i
	movsx	eax,ax
	mov	dword ptr DGROUP:_ow_i,eax
	push	8738
	push	word ptr DGROUP:_in_u
	push	4369
	call	near ptr _w_u
	add	sp,6
	call	near ptr _r_u
	movzx	eax,ax
	mov	dword ptr DGROUP:_ow_u,eax
   ;	
   ;	    ARM(); r_void(); VERIFY("r_void");
   ;	
	call	near ptr _r_void
   ;	
   ;	    ARM(); p_mixed(in_sc, in_l, in_d, in_i, in_f, in_fp, in_ld); VERIFY("p_mixed");
   ;	
	fld	tbyte ptr DGROUP:_in_ld
	sub	sp,10
	fstp	tbyte ptr [bp-10]
	fwait	
	push	dword ptr DGROUP:_in_fp
	fld	dword ptr DGROUP:_in_f
	sub	sp,4
	fstp	dword ptr [bp-18]
	fwait	
	push	word ptr DGROUP:_in_i
	fld	qword ptr DGROUP:_in_d
	sub	sp,8
	fstp	qword ptr [bp-28]
	fwait	
	push	dword ptr DGROUP:_in_l
	mov	al,byte ptr DGROUP:_in_sc
	push	ax
	call	near ptr _p_mixed
	add	sp,34
   ;	
   ;	#ifdef VARIADIC
   ;	#define VARG(n, T, P) , in_##n
   ;	    ARM(); p_variadic(0x1111 PROMOTED(VARG)); VERIFY("p_variadic");
   ;	
	mov	ax,offset DGROUP:_in_s5
	mov	dx,ds
	mov	cx,6
	call	far ptr F_SPUSH@
	mov	ax,offset DGROUP:_in_s3
	mov	dx,ds
	mov	cx,4
	call	far ptr F_SPUSH@
	fld	tbyte ptr DGROUP:_in_ld
	sub	sp,10
	fstp	tbyte ptr [bp-20]
	fld	qword ptr DGROUP:_in_d
	sub	sp,8
	fstp	qword ptr [bp-28]
	fld	dword ptr DGROUP:_in_f
	sub	sp,8
	fstp	qword ptr [bp-36]
	fwait	
	push	dword ptr DGROUP:_in_fp
	push	word ptr DGROUP:_in_np
	push	dword ptr DGROUP:_in_ul
	push	dword ptr DGROUP:_in_l
	push	word ptr DGROUP:_in_u
	push	word ptr DGROUP:_in_i
	mov	al,byte ptr DGROUP:_in_ch
	cbw	
	push	ax
	mov	al,byte ptr DGROUP:_in_uc
	mov	ah,0
	push	ax
	mov	al,byte ptr DGROUP:_in_sc
	cbw	
	push	ax
	push	4369
	call	near ptr _p_variadic
	add	sp,62
   ;	
   ;	#undef VARG
   ;	#endif
   ;	}
   ;	
	pop	bp
	ret	
_call_scalars	endp
CNCALLER_TEXT	ends
_BSS	segment word public use16 'BSS'
_ow_u	label	word
	db	4 dup (?)
_ow_i	label	word
	db	4 dup (?)
_ow_ch	label	word
	db	4 dup (?)
_ow_uc	label	word
	db	4 dup (?)
_ow_sc	label	word
	db	4 dup (?)
_out_ld	label	qword
	db	10 dup (?)
_out_d	label	qword
	db	8 dup (?)
_out_f	label	dword
	db	4 dup (?)
_out_hp	label	dword
	db	4 dup (?)
_out_fp	label	dword
	db	4 dup (?)
_out_np	label	word
	db	2 dup (?)
_out_ul	label	word
	db	4 dup (?)
_out_l	label	word
	db	4 dup (?)
_out_u	label	word
	db	2 dup (?)
_out_i	label	word
	db	2 dup (?)
_out_ch	label	byte
	db	1 dup (?)
_out_uc	label	byte
	db	1 dup (?)
_out_sc	label	byte
	db	1 dup (?)
	?debug	C E9
	?debug	C FA00000000
_BSS	ends
_DATA	segment word public use16 'DATA'
s@	label	byte
_DATA	ends
CNCALLER_TEXT	segment byte public use16 'CODE'
CNCALLER_TEXT	ends
	extrn	F_SPUSH@:far
	public	_call_scalars
	extrn	_p_variadic:near
	extrn	_p_mixed:near
	extrn	_r_void:near
	public	_ow_u
	extrn	_w_u:near
	public	_ow_i
	extrn	_w_i:near
	public	_ow_ch
	extrn	_w_ch:near
	public	_ow_uc
	extrn	_w_uc:near
	public	_ow_sc
	extrn	_w_sc:near
	public	_out_ld
	extrn	_r_ld:near
	extrn	_p_ld:near
	public	_out_d
	extrn	_r_d:near
	extrn	_p_d:near
	public	_out_f
	extrn	_r_f:near
	extrn	_p_f:near
	public	_out_hp
	extrn	_r_hp:near
	extrn	_p_hp:near
	public	_out_fp
	extrn	_r_fp:near
	extrn	_p_fp:near
	public	_out_np
	extrn	_r_np:near
	extrn	_p_np:near
	public	_out_ul
	extrn	_r_ul:near
	extrn	_p_ul:near
	public	_out_l
	extrn	_r_l:near
	extrn	_p_l:near
	public	_out_u
	extrn	_r_u:near
	extrn	_p_u:near
	public	_out_i
	extrn	_r_i:near
	extrn	_p_i:near
	public	_out_ch
	extrn	_r_ch:near
	extrn	_p_ch:near
	public	_out_uc
	extrn	_r_uc:near
	extrn	_p_uc:near
	public	_out_sc
	extrn	_r_sc:near
	extrn	_p_sc:near
	extrn	_in_s5:word
	extrn	_in_s3:word
	extrn	_in_ld:qword
	extrn	_in_d:qword
	extrn	_in_f:dword
	extrn	_in_hp:dword
	extrn	_in_fp:dword
	extrn	_in_np:word
	extrn	_in_ul:word
	extrn	_in_l:word
	extrn	_in_u:word
	extrn	_in_i:word
	extrn	_in_ch:byte
	extrn	_in_uc:byte
	extrn	_in_sc:byte
_s@	equ	s@
	end
