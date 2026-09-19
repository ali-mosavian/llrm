typedef struct {
    float x;
    float y;
    float z;
} Vec3;

typedef struct {
    Vec3 norm;
    float dist;
} Plane;

typedef struct {
    short plane_id;
    short child0;
    short child1;
} Node;

/* From qb-qrender r_bsp.bas and qcport render/r_bsp.c. */
float r_plane_dist(Vec3 *p, Plane *pl)
{
    return p->x * pl->norm.x + p->y * pl->norm.y + p->z * pl->norm.z - pl->dist;
}

short r_point_leaf(Vec3 *p, Node *nodes, Plane *planes)
{
    short nodenr;

    nodenr = 0;
    while (!(nodenr & 0x8000)) {
        Node *node = &nodes[nodenr];
        if (r_plane_dist(p, &planes[node->plane_id]) >= 0.0f)
            nodenr = node->child0;
        else
            nodenr = node->child1;
    }

    return (short)~nodenr;
}

long quake_bsp_demo(void)
{
    Vec3 p;
    Node nodes[2];
    Plane planes[2];
    short a;
    short b;
    short c;

    planes[0].norm.x = 1.0f;
    planes[0].norm.y = 0.0f;
    planes[0].norm.z = 0.0f;
    planes[0].dist = 0.0f;
    planes[1].norm.x = 0.0f;
    planes[1].norm.y = 1.0f;
    planes[1].norm.z = 0.0f;
    planes[1].dist = 0.0f;
    nodes[0].plane_id = 0;
    nodes[0].child0 = 1;
    nodes[0].child1 = -1;
    nodes[1].plane_id = 1;
    nodes[1].child0 = -2;
    nodes[1].child1 = -3;

    p.x = 2.0f;
    p.y = 3.0f;
    p.z = 0.0f;
    a = r_point_leaf(&p, nodes, planes);
    p.y = -3.0f;
    b = r_point_leaf(&p, nodes, planes);
    p.x = -2.0f;
    c = r_point_leaf(&p, nodes, planes);

    return (long)a * 100L + (long)b * 10L + c;
}
