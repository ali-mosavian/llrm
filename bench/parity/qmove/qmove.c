// flags: -O2 -march=i486 | -O2 -march=i486 -m32
extern void report(long value);

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

/* Read through volatile so no build folds the kernel to a constant. */
static volatile float input_vx = 3.0f;
static volatile float input_vy = 4.0f;
static volatile float input_wx = 1.0f;
static volatile float input_speed = 10.0f;
static volatile float input_dt = 0.25f;

long bench_qmove(void)
{
    Vec3 vel;
    Vec3 wishdir;

    vel.x = input_vx;
    vel.y = input_vy;
    vel.z = 5.0f;
    wishdir.x = input_wx;
    wishdir.y = 0.0f;
    wishdir.z = 0.0f;
    pl_ground_accel(&vel, &wishdir, input_speed, input_dt);

    return (long)vel.x * 10000L + (long)vel.y * 100L + (long)vel.z;
}

int main(void)
{
    report(bench_qmove());
    return 0;
}
