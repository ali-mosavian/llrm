// flags: -O0 | -O2
// A byte load widened to a short and then to an int (`(short) owner[key] - 1`): the peephole fused the load with the first
// extension, then matched the two extensions on the instruction it had just consumed and gave the second a read of the load's value,
// which no longer had a definition ("peephole: value#N is read but never defined", QCport keybind, mdl, sb_build, sc at -O0).
extern void report(long value);

signed char owner[4] = { 1, -2, 3, -4 };

static void release(short key)
{
    short action;

    action = (short) owner[key] - 1;
    report(action);
}

int main(void)
{
    short key;

    for (key = 0; key < 4; key++)
        release(key);
    return 0;
}
