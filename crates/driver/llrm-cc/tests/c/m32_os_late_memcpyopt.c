/* A compound literal stored into an array element, its rect built by a switch:
   clang -Os leaves memset(tmp, 0, 70); ...; memcpy(&kids[i].mid, tmp, 70). */
typedef struct {
    unsigned char row, col, rows, cols;
} Rect;
typedef struct {
    const void *ops;
    Rect rect;
    unsigned char mid[70];
    unsigned char vis, pad;
} W;
typedef struct {
    unsigned char kind, row, col, cols;
    unsigned char rows;
    const char *text;
} Ctl;
extern W kids[8];
extern void add(W *);
extern unsigned slen(const char *);
extern const int fld_ops, lst_ops, txt_ops, press_ops;

static Rect rect_of(const unsigned char *at, const Ctl *c)
{
    Rect r = {.row = at[0] + c->row, .col = at[1] + c->col};
    switch (c->kind) {
    case 1:
        r.rows = 3;
        r.cols = c->cols + 2;
        break;
    case 2:
        r.rows = c->rows + 2;
        r.cols = c->cols + 2;
        break;
    case 5:
        break;
    default:
        r.rows = 1;
        r.cols = slen(c->text) + 4;
    }
    return r;
}

void bind(const unsigned char *at, const Ctl *ctl, unsigned n)
{
    for (unsigned i = 0; i < n; i++) {
        const Ctl *c = &ctl[i];
        const void *ops = c->kind == 1 ? &fld_ops : c->kind == 2 ? &lst_ops : c->kind == 5 ? &txt_ops : &press_ops;
        kids[i] = (W){.ops = ops, .rect = rect_of(at, c), .vis = 1};
        add(&kids[i]);
    }
}
