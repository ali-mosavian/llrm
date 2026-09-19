#define PL_ACCELERATE 10.0f

typedef struct {
    float x;
    float y;
    float z;
} Vec3;

/* From qb-qrender pl_move.bas and qcport game/pl_move.c. */
void pl_ground_accel(Vec3 *vel, Vec3 *wishdir, float wishspeed, float dt)
{
    float currentspeed;
    float addspeed;
    float accelspeed;

    currentspeed = vel->x * wishdir->x + vel->y * wishdir->y;
    addspeed = wishspeed - currentspeed;
    if (addspeed <= 0.0f) return;

    accelspeed = PL_ACCELERATE * wishspeed * dt;
    if (accelspeed > addspeed) accelspeed = addspeed;

    vel->x += accelspeed * wishdir->x;
    vel->y += accelspeed * wishdir->y;
}

long quake_move_demo(void)
{
    Vec3 vel;
    Vec3 wishdir;

    vel.x = 3.0f;
    vel.y = 4.0f;
    vel.z = 5.0f;
    wishdir.x = 1.0f;
    wishdir.y = 0.0f;
    wishdir.z = 0.0f;
    pl_ground_accel(&vel, &wishdir, 10.0f, 0.25f);

    return (long)vel.x * 10000L + (long)vel.y * 100L + (long)vel.z;
}
