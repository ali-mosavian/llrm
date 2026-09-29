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
	?debug	S "cfcallee.c"
	?debug	C E9996A3D5D0A636663616C6C65652E63
	?debug	C E9996A3D5D0463662E68
	?debug	C E9996A3D5D0863616C6C65652E68
	?debug	C E9996A3D5D0763617365732E68
CFCALLEE_TEXT	segment byte public use16 'CODE'
CFCALLEE_TEXT	ends
DGROUP	group	_DATA,_BSS
	assume	cs:CFCALLEE_TEXT,ds:DGROUP
_DATA	segment word public use16 'DATA'
d@	label	byte
d@w	label	word
_DATA	ends
_BSS	segment word public use16 'BSS'
b@	label	byte
b@w	label	word
_BSS	ends
CFCALLEE_TEXT	segment byte public use16 'CODE'
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:CFCALLEE_TEXT
_p_sc	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	mov	ax,word ptr [bp+6]
	mov	word ptr DGROUP:_gh_sc,ax
	mov	al,byte ptr [bp+8]
	mov	byte ptr DGROUP:_gv_sc,al
	mov	ax,word ptr [bp+10]
	mov	word ptr DGROUP:_gt_sc,ax
	pop	bp
	ret	
_p_sc	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:CFCALLEE_TEXT
_p_uc	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	mov	ax,word ptr [bp+6]
	mov	word ptr DGROUP:_gh_uc,ax
	mov	al,byte ptr [bp+8]
	mov	byte ptr DGROUP:_gv_uc,al
	mov	ax,word ptr [bp+10]
	mov	word ptr DGROUP:_gt_uc,ax
	pop	bp
	ret	
_p_uc	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:CFCALLEE_TEXT
_p_ch	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	mov	ax,word ptr [bp+6]
	mov	word ptr DGROUP:_gh_ch,ax
	mov	al,byte ptr [bp+8]
	mov	byte ptr DGROUP:_gv_ch,al
	mov	ax,word ptr [bp+10]
	mov	word ptr DGROUP:_gt_ch,ax
	pop	bp
	ret	
_p_ch	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:CFCALLEE_TEXT
_p_i	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	mov	ax,word ptr [bp+6]
	mov	word ptr DGROUP:_gh_i,ax
	mov	ax,word ptr [bp+8]
	mov	word ptr DGROUP:_gv_i,ax
	mov	ax,word ptr [bp+10]
	mov	word ptr DGROUP:_gt_i,ax
	pop	bp
	ret	
_p_i	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:CFCALLEE_TEXT
_p_u	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	mov	ax,word ptr [bp+6]
	mov	word ptr DGROUP:_gh_u,ax
	mov	ax,word ptr [bp+8]
	mov	word ptr DGROUP:_gv_u,ax
	mov	ax,word ptr [bp+10]
	mov	word ptr DGROUP:_gt_u,ax
	pop	bp
	ret	
_p_u	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:CFCALLEE_TEXT
_p_l	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	mov	ax,word ptr [bp+6]
	mov	word ptr DGROUP:_gh_l,ax
	mov	eax,dword ptr [bp+8]
	mov	dword ptr DGROUP:_gv_l,eax
	mov	ax,word ptr [bp+12]
	mov	word ptr DGROUP:_gt_l,ax
	pop	bp
	ret	
_p_l	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:CFCALLEE_TEXT
_p_ul	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	mov	ax,word ptr [bp+6]
	mov	word ptr DGROUP:_gh_ul,ax
	mov	eax,dword ptr [bp+8]
	mov	dword ptr DGROUP:_gv_ul,eax
	mov	ax,word ptr [bp+12]
	mov	word ptr DGROUP:_gt_ul,ax
	pop	bp
	ret	
_p_ul	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:CFCALLEE_TEXT
_p_np	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	mov	ax,word ptr [bp+6]
	mov	word ptr DGROUP:_gh_np,ax
	mov	ax,word ptr [bp+8]
	mov	word ptr DGROUP:_gv_np,ax
	mov	ax,word ptr [bp+10]
	mov	word ptr DGROUP:_gt_np,ax
	pop	bp
	ret	
_p_np	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:CFCALLEE_TEXT
_p_fp	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	mov	ax,word ptr [bp+6]
	mov	word ptr DGROUP:_gh_fp,ax
	mov	eax,dword ptr [bp+8]
	mov	dword ptr DGROUP:_gv_fp,eax
	mov	ax,word ptr [bp+12]
	mov	word ptr DGROUP:_gt_fp,ax
	pop	bp
	ret	
_p_fp	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:CFCALLEE_TEXT
_p_hp	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	mov	ax,word ptr [bp+6]
	mov	word ptr DGROUP:_gh_hp,ax
	mov	eax,dword ptr [bp+8]
	mov	dword ptr DGROUP:_gv_hp,eax
	mov	ax,word ptr [bp+12]
	mov	word ptr DGROUP:_gt_hp,ax
	pop	bp
	ret	
_p_hp	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:CFCALLEE_TEXT
_p_f	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	mov	ax,word ptr [bp+6]
	mov	word ptr DGROUP:_gh_f,ax
	mov	eax,dword ptr [bp+8]
	mov	dword ptr DGROUP:_gv_f,eax
	mov	ax,word ptr [bp+12]
	mov	word ptr DGROUP:_gt_f,ax
	pop	bp
	ret	
_p_f	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:CFCALLEE_TEXT
_p_d	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	mov	ax,word ptr [bp+6]
	mov	word ptr DGROUP:_gh_d,ax
	fld	qword ptr [bp+8]
	fstp	qword ptr DGROUP:_gv_d
	fwait	
	mov	ax,word ptr [bp+16]
	mov	word ptr DGROUP:_gt_d,ax
	pop	bp
	ret	
_p_d	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:CFCALLEE_TEXT
_p_ld	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	mov	ax,word ptr [bp+6]
	mov	word ptr DGROUP:_gh_ld,ax
	fld	tbyte ptr [bp+8]
	fstp	tbyte ptr DGROUP:_gv_ld
	fwait	
	mov	ax,word ptr [bp+18]
	mov	word ptr DGROUP:_gt_ld,ax
	pop	bp
	ret	
_p_ld	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:CFCALLEE_TEXT
_r_sc	proc	far
	call	far ptr _trash
	mov	al,byte ptr DGROUP:_in_sc
	ret	
_r_sc	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:CFCALLEE_TEXT
_r_uc	proc	far
	call	far ptr _trash
	mov	al,byte ptr DGROUP:_in_uc
	ret	
_r_uc	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:CFCALLEE_TEXT
_r_ch	proc	far
	call	far ptr _trash
	mov	al,byte ptr DGROUP:_in_ch
	ret	
_r_ch	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:CFCALLEE_TEXT
_r_i	proc	far
	call	far ptr _trash
	mov	ax,word ptr DGROUP:_in_i
	ret	
_r_i	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:CFCALLEE_TEXT
_r_u	proc	far
	call	far ptr _trash
	mov	ax,word ptr DGROUP:_in_u
	ret	
_r_u	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:CFCALLEE_TEXT
_r_l	proc	far
	call	far ptr _trash
	mov	eax,dword ptr DGROUP:_in_l
	shld	edx,eax,16
	ret	
_r_l	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:CFCALLEE_TEXT
_r_ul	proc	far
	call	far ptr _trash
	mov	eax,dword ptr DGROUP:_in_ul
	shld	edx,eax,16
	ret	
_r_ul	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:CFCALLEE_TEXT
_r_np	proc	far
	call	far ptr _trash
	mov	ax,word ptr DGROUP:_in_np
	ret	
_r_np	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:CFCALLEE_TEXT
_r_fp	proc	far
	call	far ptr _trash
	mov	dx,word ptr DGROUP:_in_fp+2
	mov	ax,word ptr DGROUP:_in_fp
	ret	
_r_fp	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:CFCALLEE_TEXT
_r_hp	proc	far
	call	far ptr _trash
	mov	dx,word ptr DGROUP:_in_hp+2
	mov	ax,word ptr DGROUP:_in_hp
	ret	
_r_hp	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:CFCALLEE_TEXT
_r_f	proc	far
	call	far ptr _trash
	fld	dword ptr DGROUP:_in_f
	ret	
_r_f	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:CFCALLEE_TEXT
_r_d	proc	far
	call	far ptr _trash
	fld	qword ptr DGROUP:_in_d
	ret	
_r_d	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:CFCALLEE_TEXT
_r_ld	proc	far
	call	far ptr _trash
	fld	tbyte ptr DGROUP:_in_ld
	ret	
_r_ld	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:CFCALLEE_TEXT
_w_sc	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	mov	al,byte ptr [bp+8]
	cbw	
	movsx	eax,ax
	mov	dword ptr DGROUP:_gw_sc,eax
	mov	ax,word ptr [bp+6]
	mov	word ptr DGROUP:_gh_sc,ax
	mov	ax,word ptr [bp+10]
	mov	word ptr DGROUP:_gt_sc,ax
	pop	bp
	ret	
_w_sc	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:CFCALLEE_TEXT
_w_uc	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	mov	al,byte ptr [bp+8]
	mov	ah,0
	movsx	eax,ax
	mov	dword ptr DGROUP:_gw_uc,eax
	mov	ax,word ptr [bp+6]
	mov	word ptr DGROUP:_gh_uc,ax
	mov	ax,word ptr [bp+10]
	mov	word ptr DGROUP:_gt_uc,ax
	pop	bp
	ret	
_w_uc	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:CFCALLEE_TEXT
_w_ch	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	mov	al,byte ptr [bp+8]
	cbw	
	movsx	eax,ax
	mov	dword ptr DGROUP:_gw_ch,eax
	mov	ax,word ptr [bp+6]
	mov	word ptr DGROUP:_gh_ch,ax
	mov	ax,word ptr [bp+10]
	mov	word ptr DGROUP:_gt_ch,ax
	pop	bp
	ret	
_w_ch	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:CFCALLEE_TEXT
_w_i	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	movsx	eax,word ptr [bp+8]
	mov	dword ptr DGROUP:_gw_i,eax
	mov	ax,word ptr [bp+6]
	mov	word ptr DGROUP:_gh_i,ax
	mov	ax,word ptr [bp+10]
	mov	word ptr DGROUP:_gt_i,ax
	pop	bp
	ret	
_w_i	endp
   ;	
   ;	SCALARS(PARAM) SCALARS(RESULT) NARROW(WIDEN)
   ;	
	assume	cs:CFCALLEE_TEXT
_w_u	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	movzx	eax,word ptr [bp+8]
	mov	dword ptr DGROUP:_gw_u,eax
	mov	ax,word ptr [bp+6]
	mov	word ptr DGROUP:_gh_u,ax
	mov	ax,word ptr [bp+10]
	mov	word ptr DGROUP:_gt_u,ax
	pop	bp
	ret	
_w_u	endp
   ;	
   ;	void CONV DIST r_void(void) { trash(); }
   ;	
	assume	cs:CFCALLEE_TEXT
_r_void	proc	far
	call	far ptr _trash
	ret	
_r_void	endp
   ;	
   ;	void CONV DIST p_mixed(signed char a, long b, double c, int d, float e, char far *f, long double h) {
   ;	
	assume	cs:CFCALLEE_TEXT
_p_mixed	proc	far
	push	bp
	mov	bp,sp
   ;	
   ;	    trash(); gm_a = a; gm_b = b; gm_c = c; gm_d = d; gm_e = e; gm_f = f; gm_h = h;
   ;	
	call	far ptr _trash
	mov	al,byte ptr [bp+6]
	mov	byte ptr DGROUP:_gm_a,al
	mov	eax,dword ptr [bp+8]
	mov	dword ptr DGROUP:_gm_b,eax
	fld	qword ptr [bp+12]
	fstp	qword ptr DGROUP:_gm_c
	fwait	
	mov	ax,word ptr [bp+20]
	mov	word ptr DGROUP:_gm_d,ax
	mov	eax,dword ptr [bp+22]
	mov	dword ptr DGROUP:_gm_e,eax
	mov	eax,dword ptr [bp+26]
	mov	dword ptr DGROUP:_gm_f,eax
	fld	tbyte ptr [bp+30]
	fstp	tbyte ptr DGROUP:_gm_h
	fwait	
   ;	
   ;	}
   ;	
	pop	bp
	ret	
_p_mixed	endp
   ;	
   ;	void interrupt i_regs(unsigned bp, unsigned di, unsigned si, unsigned ds, unsigned es,
   ;	
	assume	cs:CFCALLEE_TEXT
_i_regs	proc	far
	push	ax
	push	bx
	push	cx
	push	dx
	push	es
	push	ds
	push	si
	push	di
	push	bp
	mov	bp,DGROUP
	mov	ds,bp
	mov	bp,sp
   ;	
   ;	    unsigned dx, unsigned cx, unsigned bx, unsigned ax) {
   ;	    trash();
   ;	
	call	far ptr _trash
   ;	
   ;	    gi_regs[0] = bp; gi_regs[1] = di; gi_regs[2] = si; gi_regs[3] = ds; gi_regs[4] = es;
   ;	
	mov	ax,word ptr [bp]
	mov	word ptr DGROUP:_gi_regs,ax
	mov	ax,word ptr [bp+2]
	mov	word ptr DGROUP:_gi_regs+2,ax
	mov	ax,word ptr [bp+4]
	mov	word ptr DGROUP:_gi_regs+4,ax
	mov	ax,word ptr [bp+6]
	mov	word ptr DGROUP:_gi_regs+6,ax
	mov	ax,word ptr [bp+8]
	mov	word ptr DGROUP:_gi_regs+8,ax
   ;	
   ;	    gi_regs[5] = dx; gi_regs[6] = cx; gi_regs[7] = bx; gi_regs[8] = ax;
   ;	
	mov	ax,word ptr [bp+10]
	mov	word ptr DGROUP:_gi_regs+10,ax
	mov	ax,word ptr [bp+12]
	mov	word ptr DGROUP:_gi_regs+12,ax
	mov	ax,word ptr [bp+14]
	mov	word ptr DGROUP:_gi_regs+14,ax
	mov	ax,word ptr [bp+16]
	mov	word ptr DGROUP:_gi_regs+16,ax
   ;	
   ;	    ax = 0x7777;
   ;	
	mov	word ptr [bp+16],30583
   ;	
   ;	    bx = ~bx;
   ;	
	mov	ax,word ptr [bp+14]
	not	ax
	mov	word ptr [bp+14],ax
   ;	
   ;	}
   ;	
	pop	bp
	pop	di
	pop	si
	pop	ds
	pop	es
	pop	dx
	pop	cx
	pop	bx
	pop	ax
	iret	
_i_regs	endp
   ;	
   ;	void interrupt i_plain(void) { trash(); gi_plain = 0x1234; }
   ;	
	assume	cs:CFCALLEE_TEXT
_i_plain	proc	far
	push	ax
	push	bx
	push	cx
	push	dx
	push	es
	push	ds
	push	si
	push	di
	push	bp
	mov	bp,DGROUP
	mov	ds,bp
	call	far ptr _trash
	mov	word ptr DGROUP:_gi_plain,4660
	pop	bp
	pop	di
	pop	si
	pop	ds
	pop	es
	pop	dx
	pop	cx
	pop	bx
	pop	ax
	iret	
_i_plain	endp
   ;	
   ;	void CONV DIST p_variadic(int n, ...) {
   ;	
	assume	cs:CFCALLEE_TEXT
_p_variadic	proc	far
	push	bp
	mov	bp,sp
	sub	sp,2
   ;	
   ;	    va_list ap;
   ;	    trash();
   ;	
	call	far ptr _trash
   ;	
   ;	    gva_n = n;
   ;	
	mov	ax,word ptr [bp+6]
	mov	word ptr DGROUP:_gva_n,ax
   ;	
   ;	    va_start(ap, n);
   ;	
	lea	ax,word ptr [bp+8]
	mov	word ptr [bp-2],ax
   ;	
   ;	#define VREAD(n, T, P) gva_##n = va_arg(ap, P);
   ;	    PROMOTED(VREAD)
   ;	
	add	word ptr [bp-2],2
	mov	bx,word ptr [bp-2]
	mov	ax,word ptr [bx-2]
	mov	word ptr DGROUP:_gva_sc,ax
	add	word ptr [bp-2],2
	mov	bx,word ptr [bp-2]
	mov	ax,word ptr [bx-2]
	mov	word ptr DGROUP:_gva_uc,ax
	add	word ptr [bp-2],2
	mov	bx,word ptr [bp-2]
	mov	ax,word ptr [bx-2]
	mov	word ptr DGROUP:_gva_ch,ax
	add	word ptr [bp-2],2
	mov	bx,word ptr [bp-2]
	mov	ax,word ptr [bx-2]
	mov	word ptr DGROUP:_gva_i,ax
	add	word ptr [bp-2],2
	mov	bx,word ptr [bp-2]
	mov	ax,word ptr [bx-2]
	mov	word ptr DGROUP:_gva_u,ax
	add	word ptr [bp-2],4
	mov	bx,word ptr [bp-2]
	mov	eax,dword ptr [bx-4]
	mov	dword ptr DGROUP:_gva_l,eax
	add	word ptr [bp-2],4
	mov	bx,word ptr [bp-2]
	mov	eax,dword ptr [bx-4]
	mov	dword ptr DGROUP:_gva_ul,eax
	add	word ptr [bp-2],2
	mov	bx,word ptr [bp-2]
	mov	ax,word ptr [bx-2]
	mov	word ptr DGROUP:_gva_np,ax
	add	word ptr [bp-2],4
	mov	bx,word ptr [bp-2]
	mov	eax,dword ptr [bx-4]
	mov	dword ptr DGROUP:_gva_fp,eax
	add	word ptr [bp-2],8
	mov	bx,word ptr [bp-2]
	fld	qword ptr [bx-8]
	fstp	qword ptr DGROUP:_gva_f
	fwait	
	add	word ptr [bp-2],8
	mov	bx,word ptr [bp-2]
	fld	qword ptr [bx-8]
	fstp	qword ptr DGROUP:_gva_d
	fwait	
	add	word ptr [bp-2],10
	mov	bx,word ptr [bp-2]
	fld	tbyte ptr [bx-10]
	fstp	tbyte ptr DGROUP:_gva_ld
	fwait	
	add	word ptr [bp-2],4
	mov	bx,word ptr [bp-2]
	mov	ax,word ptr [bx-4]
	mov	word ptr DGROUP:_gva_s3,ax
	mov	al,byte ptr [bx-2]
	mov	byte ptr DGROUP:_gva_s3+2,al
	add	word ptr [bp-2],6
	mov	bx,word ptr [bp-2]
	mov	ax,word ptr [bx-6]
	mov	word ptr DGROUP:_gva_s5,ax
	mov	ax,word ptr [bx-4]
	mov	word ptr DGROUP:_gva_s5+2,ax
	mov	al,byte ptr [bx-2]
	mov	byte ptr DGROUP:_gva_s5+4,al
   ;	
   ;	#undef VREAD
   ;	}
   ;	
	leave	
	ret	
_p_variadic	endp
CFCALLEE_TEXT	ends
_BSS	segment word public use16 'BSS'
_gva_n	label	word
	db	2 dup (?)
_gva_s5	label	word
	db	5 dup (?)
_gva_s3	label	word
	db	3 dup (?)
_gva_ld	label	qword
	db	10 dup (?)
_gva_d	label	qword
	db	8 dup (?)
_gva_f	label	qword
	db	8 dup (?)
_gva_fp	label	dword
	db	4 dup (?)
_gva_np	label	word
	db	2 dup (?)
_gva_ul	label	word
	db	4 dup (?)
_gva_l	label	word
	db	4 dup (?)
_gva_u	label	word
	db	2 dup (?)
_gva_i	label	word
	db	2 dup (?)
_gva_ch	label	word
	db	2 dup (?)
_gva_uc	label	word
	db	2 dup (?)
_gva_sc	label	word
	db	2 dup (?)
_gi_plain	label	word
	db	2 dup (?)
_gi_regs	label	word
	db	18 dup (?)
_gm_h	label	qword
	db	10 dup (?)
_gm_f	label	dword
	db	4 dup (?)
_gm_e	label	dword
	db	4 dup (?)
_gm_d	label	word
	db	2 dup (?)
_gm_c	label	qword
	db	8 dup (?)
_gm_b	label	word
	db	4 dup (?)
_gm_a	label	byte
	db	1 dup (?)
_gw_u	label	word
	db	4 dup (?)
_gw_i	label	word
	db	4 dup (?)
_gw_ch	label	word
	db	4 dup (?)
_gw_uc	label	word
	db	4 dup (?)
_gw_sc	label	word
	db	4 dup (?)
_gt_ld	label	word
	db	2 dup (?)
_gh_ld	label	word
	db	2 dup (?)
_gv_ld	label	qword
	db	10 dup (?)
_gt_d	label	word
	db	2 dup (?)
_gh_d	label	word
	db	2 dup (?)
_gv_d	label	qword
	db	8 dup (?)
_gt_f	label	word
	db	2 dup (?)
_gh_f	label	word
	db	2 dup (?)
_gv_f	label	dword
	db	4 dup (?)
_gt_hp	label	word
	db	2 dup (?)
_gh_hp	label	word
	db	2 dup (?)
_gv_hp	label	dword
	db	4 dup (?)
_gt_fp	label	word
	db	2 dup (?)
_gh_fp	label	word
	db	2 dup (?)
_gv_fp	label	dword
	db	4 dup (?)
_gt_np	label	word
	db	2 dup (?)
_gh_np	label	word
	db	2 dup (?)
_gv_np	label	word
	db	2 dup (?)
_gt_ul	label	word
	db	2 dup (?)
_gh_ul	label	word
	db	2 dup (?)
_gv_ul	label	word
	db	4 dup (?)
_gt_l	label	word
	db	2 dup (?)
_gh_l	label	word
	db	2 dup (?)
_gv_l	label	word
	db	4 dup (?)
_gt_u	label	word
	db	2 dup (?)
_gh_u	label	word
	db	2 dup (?)
_gv_u	label	word
	db	2 dup (?)
_gt_i	label	word
	db	2 dup (?)
_gh_i	label	word
	db	2 dup (?)
_gv_i	label	word
	db	2 dup (?)
_gt_ch	label	word
	db	2 dup (?)
_gh_ch	label	word
	db	2 dup (?)
_gv_ch	label	byte
	db	1 dup (?)
_gt_uc	label	word
	db	2 dup (?)
_gh_uc	label	word
	db	2 dup (?)
_gv_uc	label	byte
	db	1 dup (?)
_gt_sc	label	word
	db	2 dup (?)
_gh_sc	label	word
	db	2 dup (?)
_gv_sc	label	byte
	db	1 dup (?)
	?debug	C E9
	?debug	C FA00000000
_BSS	ends
_DATA	segment word public use16 'DATA'
s@	label	byte
_DATA	ends
CFCALLEE_TEXT	segment byte public use16 'CODE'
CFCALLEE_TEXT	ends
	public	_p_variadic
	public	_gva_n
	public	_gva_s5
	public	_gva_s3
	public	_gva_ld
	public	_gva_d
	public	_gva_f
	public	_gva_fp
	public	_gva_np
	public	_gva_ul
	public	_gva_l
	public	_gva_u
	public	_gva_i
	public	_gva_ch
	public	_gva_uc
	public	_gva_sc
	public	_i_plain
	public	_i_regs
	public	_gi_plain
	public	_gi_regs
	public	_p_mixed
	public	_gm_h
	public	_gm_f
	public	_gm_e
	public	_gm_d
	public	_gm_c
	public	_gm_b
	public	_gm_a
	public	_r_void
	public	_w_u
	public	_gw_u
	public	_w_i
	public	_gw_i
	public	_w_ch
	public	_gw_ch
	public	_w_uc
	public	_gw_uc
	public	_w_sc
	public	_gw_sc
	public	_r_ld
	public	_r_d
	public	_r_f
	public	_r_hp
	public	_r_fp
	public	_r_np
	public	_r_ul
	public	_r_l
	public	_r_u
	public	_r_i
	public	_r_ch
	public	_r_uc
	public	_r_sc
	public	_p_ld
	public	_gt_ld
	public	_gh_ld
	public	_gv_ld
	public	_p_d
	public	_gt_d
	public	_gh_d
	public	_gv_d
	public	_p_f
	public	_gt_f
	public	_gh_f
	public	_gv_f
	public	_p_hp
	public	_gt_hp
	public	_gh_hp
	public	_gv_hp
	public	_p_fp
	public	_gt_fp
	public	_gh_fp
	public	_gv_fp
	public	_p_np
	public	_gt_np
	public	_gh_np
	public	_gv_np
	public	_p_ul
	public	_gt_ul
	public	_gh_ul
	public	_gv_ul
	public	_p_l
	public	_gt_l
	public	_gh_l
	public	_gv_l
	public	_p_u
	public	_gt_u
	public	_gh_u
	public	_gv_u
	public	_p_i
	public	_gt_i
	public	_gh_i
	public	_gv_i
	public	_p_ch
	public	_gt_ch
	public	_gh_ch
	public	_gv_ch
	public	_p_uc
	public	_gt_uc
	public	_gh_uc
	public	_gv_uc
	public	_p_sc
	public	_gt_sc
	public	_gh_sc
	public	_gv_sc
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
	extrn	_trash:far
_s@	equ	s@
	end
