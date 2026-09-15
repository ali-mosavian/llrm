/* qcport's mod_link_anims: four reads of t[j] through twin counters, which strength reduced one a round and never finished. */
typedef struct { char name[16]; long wdth, hght; long offset[4]; } DiskMipTex;
typedef struct { short w, h, anim_base, anim_count; long pad; } MipTex;
typedef struct { MipTex far *miptex; short far *anim_tab; } World;

void far *qglMemAlloc(long size);
void modtex_fatal(const char *why);
int _fstrncmp(const char far *a, const char far *b, unsigned n);

static void link_anims(World *world, DiskMipTex far *t_mip_inf, long texture_count)
{
    long i, j, k;
    short frame[10], nf, d, best, bi;
    char far *suffix;
    short used = 0;
    {
        long want = 0;
        for (i = 0; i < texture_count; i++)
            if (t_mip_inf[i].name[0] == '+' && t_mip_inf[i].name[1] >= '0' && t_mip_inf[i].name[1] <= '9')
                want++;
        if (want < 2) return;
        world->anim_tab = (short far *) qglMemAlloc(want * (long) sizeof(short));
        if (!world->anim_tab) modtex_fatal("out of memory");
    }
    for (i = 0; i < texture_count; i++) {
        if (t_mip_inf[i].name[0] != '+') continue;
        if (world->miptex[i].anim_count > 1) continue;
        if (t_mip_inf[i].name[1] < '0' || t_mip_inf[i].name[1] > '9') continue;
        suffix = t_mip_inf[i].name + 2;
        nf = 0;
        for (j = i; j < texture_count && nf < 10; j++) {
            if (t_mip_inf[j].name[0] != '+') continue;
            if (t_mip_inf[j].name[1] < '0' || t_mip_inf[j].name[1] > '9') continue;
            if (_fstrncmp(t_mip_inf[j].name + 2, suffix, 14) != 0) continue;
            frame[nf++] = (short) j;
        }
        if (nf < 2) continue;
        for (k = 0; k < nf - 1; k++) {
            bi = (short) k;
            best = (short)(t_mip_inf[frame[k]].name[1]);
            for (j = k + 1; j < nf; j++) {
                d = (short)(t_mip_inf[frame[j]].name[1]);
                if (d < best) { best = d; bi = (short) j; }
            }
            if (bi != k) { d = frame[k]; frame[k] = frame[bi]; frame[bi] = d; }
        }
        for (k = 0; k < nf; k++) {
            world->anim_tab[used + k] = frame[k];
            world->miptex[frame[k]].anim_base = used;
            world->miptex[frame[k]].anim_count = nf;
        }
        used = (short)(used + nf);
    }
}

void anims(World *world, DiskMipTex far *t, long n) { link_anims(world, t, n); }
