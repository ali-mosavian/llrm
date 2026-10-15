/* The world d_faces.c runs in, for the executed scoreboard: a seeded scene and recording stand-ins for what the file calls (the
   BASIC-side surface cache, qgl's rasteriser). Built once, by gcc -O0, and shared by every compiler's d_faces.o; see readme.md.

   Everything a stand-in is asked is folded into one hash (integers exactly, floats rounded coarsely enough that the x87's
   extended intermediates and a float rounded at every store agree), and a stand-in answers from its arguments alone, so a
   build of d_faces.c that computes anything differently reports another hash. */
/* FLAT (m32 for gcc, clang and llrm): the segmented keywords mean nothing. Otherwise (llrm -m16) they are the file's own, and so is
   the calling convention d_faces.c is written to. Integers are `short` and `long` throughout: an `int` is 16 bits in one and 32 in
   the other, and the hash must come out the same. */
#ifdef FLAT
#define pascal
#define far
#define near
#define huge
#define PF
#define FAR(p) ((long)(p))
#else
#define PF pascal far
#define FAR(p) ((long)(void far *)(p))
#endif
#include "qcshared.h"

typedef struct { float x, y, z, u, v; } QglVtxView;
extern void report(long value);

enum { NF = 96, NN = 16, NT = 12, NM = 10, NB = 4, ROWS = 4, GEOM_BYTES = 8192 };

static unsigned long hash = 2166136261UL;
static void fold(unsigned long v)
{
#ifdef TRACE_ALL
    report((long)v);
#endif
    hash = (hash ^ v) * 16777619UL;
}
#ifdef TRACE_ALL
static void mark(long k) { report(-1000 - k); }
#else
#define mark(k) ((void)0)
#endif
/* The floats the rasteriser is handed are summed, not hashed: a float that lands on a rounding boundary differs between a compiler
   that keeps extended precision and one that rounds at every store, and a hash would call that a miscompile. The runner compares
   the sums with a tolerance, and the structure (what was drawn, in what order, with what ints) exactly. */
static double fsum[5];
static void accumulate(float x, float y, float z, float u, float v)
{
    fsum[0] += x; fsum[1] += y; fsum[2] += z; fsum[3] += u; fsum[4] += v;
}

/* Paths the scene must reach, read back by the runner (readme.md lists them). */
unsigned long coverage[24];
enum { C_FLAT, C_TEX, C_PTEX, C_WIRE, C_REFUSED, C_SC_HIT, C_SC_MISS, C_ALLOC, C_ALLOC_FAIL, C_BUILD, C_NOZ, C_NODST };
#define COVER(k) (coverage[k]++)

static short frame_z = 1;
static unsigned long tick = 0;
static unsigned long lcg = 12345UL;
static unsigned long next(void) { lcg = lcg * 1103515245UL + 12345UL; return lcg >> 8; }

/* ---- what d_faces.c calls ---- */
static short gv_buf[9 + 33 * 3 + 8];
#ifdef TRACE_ALL
#define DUMPGV do { short q; report(-6000); for (q = 0; q < 116; q++) report(gv_buf[q]); } while (0)
#else
#define DUMPGV ((void)0)
#endif
static short geom[ROWS][GEOM_BYTES / 2];

void PF qglClRect(short x0, short y0, short x1, short y1) { mark(0); fold((unsigned long)x0); fold((unsigned long)y0); fold((unsigned long)x1); fold((unsigned long)y1); }
short PF qglSfZMode(long surf, short mode) { mark(1); fold((unsigned long)surf); fold(mode); return frame_z; }
void PF qglRsCoverClear(void) { mark(2); fold(0x77); }
void PF qglRsCover(short on) { mark(3); fold((unsigned long)(0x78 + on)); }
void PF qglDrLine(long d, short x0, short y0, short x1, short y1, short col) { mark(4); fold((unsigned long)d); fold((unsigned long)(x0 + y0 + x1 + y1 + col)); }

short PF qglRsPoly(long dst, void far *v, short cnt, short mode, long src)
{
    QglVtxView far *p = (QglVtxView far *)v;
    short i;
    mark(5);
    fold((unsigned long)dst); fold((unsigned long)cnt); fold((unsigned long)mode); fold((unsigned long)src);
    for (i = 0; i < cnt; i++) {
        accumulate(p[i].x, p[i].y, p[i].z * 100000.0f, p[i].u, p[i].v);
#ifdef TRACE_ALL
        report(-4000); report((long)(p[i].x * 8)); report((long)(p[i].y * 8)); report((long)(p[i].z * 100000.0f)); report((long)(p[i].u * 16)); report((long)(p[i].v * 16));
#endif
    }
#ifdef TRACE_ALL
    report(-2000); report((long)fsum[0]); report((long)fsum[1]); report((long)fsum[3]);
#endif
    COVER(mode == 0 ? C_WIRE : mode == 1 ? C_FLAT : mode == 2 ? C_TEX : C_PTEX);
    if (mode >= 2 && (src & 15) == 7) { COVER(C_REFUSED); return -1; }
    return cnt;
}

long PF mod_geom_map(void *g, short row) { mark(6); fold((unsigned long)row); return FAR(geom[row & (ROWS - 1)]); }
long PF mod_tex_raw(void *g, short k, short mip) { mark(7); fold((unsigned long)(k * 8 + mip)); return 7000 + k * 4 + mip; }
long PF mod_tex_shaded(void *g, short k, short mip) { mark(8); DUMPGV; fold((unsigned long)(k * 8 + mip + 1)); return 9000 + k * 4 + mip; }
short PF sc_ready(void) { mark(9); return 1; }
short PF sc_held(short face) { mark(10); fold((unsigned long)face); return face % 4 == 0 ? -1 : face % 3; }
long PF sc_find(short face, short mip, short a, short b, short stag)
{ mark(11);
    fold((unsigned long)face); fold((unsigned long)mip); fold((unsigned long)a); fold((unsigned long)b); fold((unsigned long)stag);
    if (stag == -1 || (face + mip) % 4 == 1) { COVER(C_SC_MISS); return 0; }
    COVER(C_SC_HIT);
    return 100 + face * 8 + mip;
}
long PF sc_alloc(void *g, short face, short mip, short w, short h, short fw, short fh, short stag)
{ mark(12);
    fold((unsigned long)face); fold((unsigned long)mip); fold((unsigned long)w); fold((unsigned long)h); fold((unsigned long)fw); fold((unsigned long)fh); fold((unsigned long)stag);
    if (face % 7 == 3) { COVER(C_ALLOC_FAIL); return 0; }
    COVER(C_ALLOC);
    return 5000 + face;
}
long PF sc_view_ofs(void) { mark(13); return 77; }
short PF ls_epoch(short style) { mark(14); fold((unsigned long)style); return style + 1; }
long PF sys_rdtsc(void) { mark(15); tick += 977; return (long)tick; }
long PF qglTmrCycles(void) { mark(16); tick += 311; return (long)tick; }
void PF sb_build(void *g, long lm_dc, long tex_dc, short face, short mip, short pw, short ph,
              BASARRAY *tri, BASARRAY *texinf, BASARRAY *gv, BASARRAY *mip_inf, BASARRAY *planes)
{ mark(17);
    fold((unsigned long)lm_dc); fold((unsigned long)tex_dc); fold((unsigned long)face); fold((unsigned long)mip); fold((unsigned long)pw); fold((unsigned long)ph);
    fold((unsigned long)((short far *)gv->farptr)[0]);
    COVER(C_BUILD);
}

/* ---- the scene ---- */
static Face tri[NF];
static TexInfo texinf[NT];
static BrushModel brush[NB];
static Plane planes[NF];
static Node nodes[NN];
static MipTex mipinf[NM];
static short order[NN];
static short pflag[(NF + 15) / 16];
static long tex_ofs[NM * 4];
static float turb_sin[256];
static BASARRAY a_tri, a_texinf, a_gv, a_brush, a_planes, a_nodes, a_mipinf, a_order, a_pflag;
static DrawParams dp;
static char world;

static const short COS[12] = { 1000, 866, 500, 0, -500, -866, -1000, -866, -500, 0, 500, 866 };
static const short SIN[12] = { 0, 500, 866, 1000, 866, 500, 0, -500, -866, -1000, -866, -500 };

/* Word `k` of the record at byte `ofs` of a row, if the row holds it: a record that meets the end of its row is cut there (d_faces
   clamps its copy to the row, and what lies past the row is not the scene's to overwrite). */
static void put(long row, long ofs, long k, long v)
{
    long at = ofs + 2 * k;
    if (at + 2 <= GEOM_BYTES) *(short *)((char *)geom[row] + at) = (short)v;
}

static void array(BASARRAY *a, void *data)
{
    a->farptr = data;
    a->dimensions = 1;
}

static void build(void)
{
    long i, k, j;
    for (i = 0; i < NM; i++) {
        mipinf[i].wdth = (float)(16 << (i % 4));
        mipinf[i].hght = (float)(16 << ((i + 1) % 4));
        mipinf[i].liquid = (i % 7 == 3);
        mipinf[i].anim_count = 1; mipinf[i].anim_pos = 0; mipinf[i].anim_next = (short)i;
    }
    /* a ring of three animated frames: mips 4, 5, 6 */
    for (k = 0; k < 3; k++) {
        mipinf[4 + k].anim_count = 3; mipinf[4 + k].anim_pos = (short)k; mipinf[4 + k].anim_next = (short)(4 + (k + 1) % 3);
    }
    for (i = 0; i < NT; i++) {
        texinf[i].vecs[0] = 0.25f; texinf[i].vecs[1] = (float)(i % 3) * 0.125f; texinf[i].vecs[2] = 0.0f; texinf[i].vecs[3] = (float)(i * 3);
        texinf[i].vect[0] = 0.0f; texinf[i].vect[1] = 0.125f; texinf[i].vect[2] = 0.25f; texinf[i].vect[3] = (float)(i * 2);
        texinf[i].mip_tex = (short)(i % NM);
    }
    brush[0].ofs.x = 0; brush[0].ofs.y = 0; brush[0].ofs.z = 0;
    brush[1].ofs.x = 50; brush[1].ofs.y = 0; brush[1].ofs.z = -20;
    brush[2].ofs.x = -30; brush[2].ofs.y = 100; brush[2].ofs.z = 10;
    brush[3].ofs.x = 10; brush[3].ofs.y = -5; brush[3].ofs.z = 15;      /* side >> 1 reaches 3 */
    for (i = 0; i < NB; i++) { brush[i].draw = 1; brush[i].solid = 1; brush[i].node = (short)i; }
    /* Four steps, not a wave: the index is floor(coordinate * 326 + phase), and a rounding the compilers disagree on, 1 in 10^7,
       would move a vertex to another entry. Steps move it only at the 64th. */
    for (i = 0; i < 256; i++) turb_sin[i] = (float)(i >> 6) * 0.25f;
    for (i = 0; i < NM * 4; i++) tex_ofs[i] = 1000 + i * 3;
    for (i = 0; i < NF; i++) {
        long n = 3 + (long)(next() % 4);                   /* triangles to hexagons */
        long row = i % ROWS, ofs = (i / ROWS) * 64;
        long radius = 40 + (long)(next() % 160);
        long cx = (long)(next() % 900) - 450, cz = (long)(next() % 700) - 350;
        long cy = 60 + (long)(next() % 1700);
        if (i % 19 == 0) cy = -150 + (int)(next() % 200);         /* across or behind the near plane */
        if (i % 23 == 0) cy = 1650 + (int)(next() % 500);         /* across or beyond the far plane */
        if (i % 29 == 0) n = 2;                                   /* too few corners: clipped away */
        if (i % 31 == 0) n = 33;                                  /* the most the store allows */
        if (i == 7) ofs = GEOM_BYTES - 20;                        /* a record that meets the end of its row */
        tri[i].plane_id = (short)i;
        tri[i].side = (short)((i & 1) | (i % 11 == 0 ? 2 : 0) | (i % 17 == 0 ? 4 : 0));
        tri[i].geom_row = (short)row;
        tri[i].geom_ofs = (short)ofs;
        tri[i].tex_info_id = (short)(i % NT);
        put(row, ofs, 0, n == 33 && i == 62 ? 40 : n);             /* one record claims more than 33: clamped */
        put(row, ofs, 1, i % 3 == 0 ? -1 : 0);                     /* no lightmap on every third face */
        put(row, ofs, 2, 0);
        put(row, ofs, 3, i * 5 % 64); put(row, ofs, 4, i * 3 % 48);
        put(row, ofs, 5, 2 + i % 4 * 2); put(row, ofs, 6, 2 + (i + 1) % 4 * 2);
        put(row, ofs, 7, i % 5);
        put(row, ofs, 8, 0);
        for (j = 0; j < n && j < 33; j++) {
            long a = j * 12 / (n < 12 ? n : 12) + (n > 12 ? j / 12 : 0);
            long rr = radius - (n > 12 ? j * 3 : 0);
            put(row, ofs, 9 + j * 3 + 0, (cx + rr * COS[a % 12] / 1000) * 8);
            put(row, ofs, 9 + j * 3 + 1, (cy + (j & 1) * 6) * 8);
            put(row, ofs, 9 + j * 3 + 2, (cz + rr * SIN[a % 12] / 1000) * 8);
        }
        planes[i].norm.x = (float)((long)(next() % 200) - 100) * 0.01f;
        planes[i].norm.y = (float)((long)(next() % 200) - 100) * 0.01f;
        planes[i].norm.z = 0.5f;
        planes[i].dist = (float)((long)(next() % 600) - 300);
    }
    for (i = 0; i < NN; i++) {
        nodes[i].plane_id = (short)i;
        nodes[i].child0 = nodes[i].child1 = 0;
        nodes[i].lface_id = (short)(i * 6);
        nodes[i].lface_num = 6;
        order[i] = (short)((i * 5 + 3) % NN);
    }
    for (i = 0; i < NF; i++) if (i % 13 != 5) pflag[i >> 4] |= (short)(1 << (i & 15));
    array(&a_tri, tri); array(&a_texinf, texinf); array(&a_gv, gv_buf); array(&a_brush, brush); array(&a_planes, planes);
    array(&a_nodes, nodes); array(&a_mipinf, mipinf); array(&a_order, order); array(&a_pflag, pflag);
    world = 1;
#ifdef TRACE_ALL
    {
        long sum = 0, r2;
        for (r2 = 0; r2 < ROWS; r2++) for (i = 0; i < GEOM_BYTES / 2; i++) sum += geom[r2][i] * (i % 7 + 1);
        report(-3000); report(sum);
        sum = 0;
        for (i = 0; i < NF; i++) sum += (long)(planes[i].norm.x * 1000) + (long)(planes[i].norm.y * 1000) + (long)planes[i].dist;
        report(sum);
        sum = 0;
        for (i = 0; i < NF; i++) sum += tri[i].geom_row * 3 + tri[i].geom_ofs + tri[i].side * 5 + tri[i].tex_info_id * 7;
        report(sum);
    }
#endif
}

#ifdef FLAT
typedef void (*DrawFn)(void *, DrawParams *, float *, Vec3f *, BASARRAY *, BASARRAY *, BASARRAY *, BASARRAY *, BASARRAY *,
                       BASARRAY *, BASARRAY *, BASARRAY *, BASARRAY *);
typedef short (*LayoutFn)(long, long);
#endif
#ifdef FLAT
/* Where the compiler under test put the two functions: the runner writes them in before it starts. */
DrawFn entry_draw;
LayoutFn entry_layout;
#define DRAW entry_draw
#define LAYOUT entry_layout
#else
extern void PF d_draw_faces(void *, DrawParams *, float *, Vec3f *, BASARRAY *, BASARRAY *, BASARRAY *, BASARRAY *, BASARRAY *,
                            BASARRAY *, BASARRAY *, BASARRAY *, BASARRAY *);
extern short PF d_faces_layout_ok(long, long);
#define DRAW d_draw_faces
#define LAYOUT d_faces_layout_ok
#endif

static void frame(short which, float *m, Vec3f *campos)
{
    static const short mode[5] = { 0, 1, 2, 0, 0 };
    dp.h_dst_dc = which == 3 ? 0 : 424242;
    dp.tex_ofs_ptr = FAR(tex_ofs);
    dp.turb_ptr = FAR(turb_sin);
    dp.xresh = 160.0f; dp.yresh = 100.0f;
    dp.z_near = 4.0f; dp.z_far = 1800.0f;
    /* floor(time * 10) picks the animation frame: a time whose tenths sit on a whole number (2.30 * 10 is 22.999998 or 23.000002) picks
       another frame by the compiler. */
    dp.anim_time = 1.373f + (float)which * 0.317f;
    dp.dl_x = 100.0f; dp.dl_y = 700.0f; dp.dl_z = 20.0f; dp.dl_radius = which == 4 ? 600.0f : 90.0f;
    dp.ord_count = NN;
    dp.use_lm = (short)(which != 3);
    dp.lightmap = 1;
    dp.backface = (short)(which != 2);
    dp.rend_mode = mode[which];
    dp.use_mips = (short)(which != 3);
    dp.x_res = 320; dp.y_res = 200;
    dp.prof = (short)(which == 1);
    frame_z = (short)(which != 1);
    if (!frame_z) COVER(C_NOZ);
    if (!dp.h_dst_dc) COVER(C_NODST);
    DRAW((void *)&world, &dp, m, campos, &a_tri, &a_texinf, &a_gv, &a_brush, &a_planes, &a_nodes, &a_mipinf, &a_order, &a_pflag);
    fold((unsigned long)dp.polys); fold((unsigned long)dp.tris); fold((unsigned long)dp.lm_want); fold((unsigned long)dp.lm_fallback);
    fold((unsigned long)dp.qgl_faces); fold((unsigned long)dp.qgl_drop);
    fold((unsigned long)dp.build_us); fold((unsigned long)dp.cy_geom);
#ifdef TRACE
    report((long)hash);
#endif
}

long bench_dfaces(short n)
{
    static float m0[16] = { 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0 };
    static float m1[16] = { 1.3f, 0, 0, 0, 0, 1.1f, 0, 0, 0, 0, 1, 1, 12.5f, -7.25f, 0, 0 };
    Vec3f here = { 0, 0, 0 }, there = { 20, -10, 5 };
    short f;
    if (!world) build();
#ifdef TRACE_ALL
    for (f = 0; f < NF; f++) { report(-5000 - f); report(tri[f].geom_row); report(tri[f].geom_ofs); report(tri[f].side); report(tri[f].tex_info_id); }
#endif
    hash = 2166136261UL;
    for (f = 0; f < 5; f++) fsum[f] = 0;
    fold((unsigned long)LAYOUT((long)sizeof(DrawParams), (long)((char *)&dp.qgl_drop - (char *)&dp)));
    for (f = 0; f < 5; f++) frame(f, f == 4 ? m1 : m0, f == 4 ? &there : &here);
    return (long)hash;
}

int main(void)
{
    short k;
    report(bench_dfaces(0));
    for (k = 0; k < 5; k++) report((long)(fsum[k] + 0.5));
    return 0;
}
