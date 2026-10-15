/* The world d_faces.c runs in, for the executed scoreboard: a seeded scene and recording stand-ins for what the file calls (the
   BASIC-side surface cache, qgl's rasteriser). Built once, by gcc -O0, and shared by every compiler's d_faces.o; see readme.md.

   Everything a stand-in is asked is folded into one hash (integers exactly, floats rounded coarsely enough that the x87's
   extended intermediates and a float rounded at every store agree), and a stand-in answers from its arguments alone, so a
   build of d_faces.c that computes anything differently reports another hash. */
#define pascal
#define far
#define near
#define huge
#include "qcshared.h"

typedef struct { float x, y, z, u, v; } QglVtxView;
extern void report(long value);

enum { NF = 96, NN = 16, NT = 12, NM = 10, NB = 3, ROWS = 4, GEOM_BYTES = 8192 };

static unsigned hash = 2166136261u;
static void fold(unsigned v) { hash = (hash ^ v) * 16777619u; }
static void fold_f(float x, float scale) { fold((unsigned)(int)(x * scale + (x < 0 ? -0.5f : 0.5f))); }

/* Paths the scene must reach, read back by the runner (readme.md lists them). */
unsigned coverage[24];
enum { C_FLAT, C_TEX, C_PTEX, C_WIRE, C_REFUSED, C_SC_HIT, C_SC_MISS, C_ALLOC, C_ALLOC_FAIL, C_BUILD, C_NOZ, C_NODST };
#define COVER(k) (coverage[k]++)

static short frame_z = 1;
static unsigned tick = 0;
static unsigned lcg = 12345;
static unsigned next(void) { lcg = lcg * 1103515245u + 12345u; return lcg >> 8; }

/* ---- what d_faces.c calls ---- */
static short geom[ROWS][GEOM_BYTES / 2];

void qglClRect(short x0, short y0, short x1, short y1) { fold(x0); fold(y0); fold(x1); fold(y1); }
short qglSfZMode(long surf, short mode) { fold((unsigned)surf); fold(mode); return frame_z; }
void qglRsCoverClear(void) { fold(0x77); }
void qglRsCover(short on) { fold(0x78 + on); }
void qglDrLine(long d, short x0, short y0, short x1, short y1, short col) { fold((unsigned)d); fold(x0 + y0 + x1 + y1 + col); }

short qglRsPoly(long dst, void *v, short cnt, short mode, long src)
{
    QglVtxView *p = (QglVtxView *)v;
    short i;
    fold((unsigned)dst); fold(cnt); fold(mode); fold((unsigned)src);
    for (i = 0; i < cnt; i++) {
        fold_f(p[i].x, 8.0f); fold_f(p[i].y, 8.0f); fold_f(p[i].z, 100000.0f);
        fold_f(p[i].u, 16.0f); fold_f(p[i].v, 16.0f);
    }
    COVER(mode == 0 ? C_WIRE : mode == 1 ? C_FLAT : mode == 2 ? C_TEX : C_PTEX);
    if (mode >= 2 && (src & 15) == 7) { COVER(C_REFUSED); return -1; }
    return cnt;
}

long mod_geom_map(void *g, short row) { fold(row); return (long)geom[row & (ROWS - 1)]; }
long mod_tex_raw(void *g, short k, short mip) { fold(k * 8 + mip); return 7000 + k * 4 + mip; }
long mod_tex_shaded(void *g, short k, short mip) { fold(k * 8 + mip + 1); return 9000 + k * 4 + mip; }
short sc_ready(void) { return 1; }
short sc_held(short face) { fold(face); return face % 4 == 0 ? -1 : face % 3; }
long sc_find(short face, short mip, short a, short b, short stag)
{
    fold(face); fold(mip); fold(a); fold(b); fold(stag);
    if (stag == -1 || (face + mip) % 4 == 1) { COVER(C_SC_MISS); return 0; }
    COVER(C_SC_HIT);
    return 100 + face * 8 + mip;
}
long sc_alloc(void *g, short face, short mip, short w, short h, short fw, short fh, short stag)
{
    fold(face); fold(mip); fold(w); fold(h); fold(fw); fold(fh); fold(stag);
    if (face % 7 == 3) { COVER(C_ALLOC_FAIL); return 0; }
    COVER(C_ALLOC);
    return 5000 + face;
}
long sc_view_ofs(void) { return 77; }
short ls_epoch(short style) { fold(style); return style + 1; }
long sys_rdtsc(void) { tick += 977; return (long)tick; }
long qglTmrCycles(void) { tick += 311; return (long)tick; }
void sb_build(void *g, long lm_dc, long tex_dc, short face, short mip, short pw, short ph,
              BASARRAY *tri, BASARRAY *texinf, BASARRAY *gv, BASARRAY *mip_inf, BASARRAY *planes)
{
    fold((unsigned)lm_dc); fold((unsigned)tex_dc); fold(face); fold(mip); fold(pw); fold(ph);
    fold(((short *)gv->farptr)[0]);
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
static short gv_buf[9 + 33 * 3 + 8];
static long tex_ofs[NM * 4];
static float turb_sin[256];
static BASARRAY a_tri, a_texinf, a_gv, a_brush, a_planes, a_nodes, a_mipinf, a_order, a_pflag;
static DrawParams dp;
static char world;

static const short COS[12] = { 1000, 866, 500, 0, -500, -866, -1000, -866, -500, 0, 500, 866 };
static const short SIN[12] = { 0, 500, 866, 1000, 866, 500, 0, -500, -866, -1000, -866, -500 };

static void array(BASARRAY *a, void *data)
{
    a->farptr = data;
    a->dimensions = 1;
}

static void build(void)
{
    int i, k, j;
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
    for (i = 0; i < NB; i++) { brush[i].draw = 1; brush[i].solid = 1; brush[i].node = (short)i; }
    for (i = 0; i < 256; i++) turb_sin[i] = (float)((i * 7) % 33 - 16) * 0.5f;
    for (i = 0; i < NM * 4; i++) tex_ofs[i] = 1000 + i * 3;
    for (i = 0; i < NF; i++) {
        int n = 3 + (int)(next() % 4);                   /* triangles to hexagons */
        int row = i % ROWS, ofs = (i / ROWS) * 64;
        int radius = 40 + (int)(next() % 160);
        int cx = (int)(next() % 900) - 450, cz = (int)(next() % 700) - 350;
        int cy = 60 + (int)(next() % 1700);
        short *r;
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
        r = (short *)((char *)geom[row] + ofs);
        r[0] = (short)(n == 33 && i == 62 ? 40 : n);              /* one record claims more than 33: clamped */
        r[1] = (short)(i % 3 == 0 ? -1 : 0);                      /* no lightmap on every third face */
        r[2] = 0;
        r[3] = (short)(i * 5 % 64); r[4] = (short)(i * 3 % 48);
        r[5] = (short)(2 + i % 4 * 2); r[6] = (short)(2 + (i + 1) % 4 * 2);
        r[7] = (short)(i % 5);
        r[8] = 0;
        for (j = 0; j < n && j < 33; j++) {
            int a = j * 12 / (n < 12 ? n : 12) + (n > 12 ? j / 12 : 0);
            int rr = radius - (n > 12 ? j * 3 : 0);
            if (ofs == GEOM_BYTES - 20 && j > 4) break;
            r[9 + j * 3 + 0] = (short)((cx + rr * COS[a % 12] / 1000) * 8);
            r[9 + j * 3 + 1] = (short)((cy + (j & 1) * 6) * 8);
            r[9 + j * 3 + 2] = (short)((cz + rr * SIN[a % 12] / 1000) * 8);
        }
        planes[i].norm.x = (float)((int)(next() % 200) - 100) * 0.01f;
        planes[i].norm.y = (float)((int)(next() % 200) - 100) * 0.01f;
        planes[i].norm.z = 0.5f;
        planes[i].dist = (float)((int)(next() % 600) - 300);
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
}

typedef void (*DrawFn)(void *, DrawParams *, float *, Vec3f *, BASARRAY *, BASARRAY *, BASARRAY *, BASARRAY *, BASARRAY *,
                       BASARRAY *, BASARRAY *, BASARRAY *, BASARRAY *);
typedef short (*LayoutFn)(long, long);
/* Where the compiler under test put the two functions: the runner writes them in before it starts. */
DrawFn entry_draw;
LayoutFn entry_layout;

static void frame(int which, float *m, Vec3f *campos)
{
    static const short mode[5] = { 0, 1, 2, 0, 0 };
    dp.h_dst_dc = which == 3 ? 0 : 424242;
    dp.tex_ofs_ptr = (long)tex_ofs;
    dp.turb_ptr = (long)turb_sin;
    dp.xresh = 160.0f; dp.yresh = 100.0f;
    dp.z_near = 4.0f; dp.z_far = 1800.0f;
    dp.anim_time = 1.37f + (float)which * 0.31f;
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
    entry_draw((void *)&world, &dp, m, campos, &a_tri, &a_texinf, &a_gv, &a_brush, &a_planes, &a_nodes, &a_mipinf, &a_order, &a_pflag);
    fold(dp.polys); fold(dp.tris); fold(dp.lm_want); fold(dp.lm_fallback); fold(dp.qgl_faces); fold(dp.qgl_drop);
    fold((unsigned)dp.build_us); fold((unsigned)dp.cy_geom);
}

long bench_dfaces(int n)
{
    static float m0[16] = { 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0 };
    static float m1[16] = { 1.3f, 0, 0, 0, 0, 1.1f, 0, 0, 0, 0, 1, 1, 12.5f, -7.25f, 0, 0 };
    Vec3f here = { 0, 0, 0 }, there = { 20, -10, 5 };
    int f;
    if (!world) build();
    hash = 2166136261u;
    fold(entry_layout((long)sizeof(DrawParams), (long)((char *)&dp.qgl_drop - (char *)&dp)));
    for (f = 0; f < 5; f++) frame(f, f == 4 ? m1 : m0, f == 4 ? &there : &here);
    return (long)hash;
}

int main(void)
{
    report(bench_dfaces(0));
    return 0;
}
