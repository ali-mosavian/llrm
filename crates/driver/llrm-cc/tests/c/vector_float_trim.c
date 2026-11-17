/* Four doubles from consecutive counters. The pair's tree costs +2 as written (the conversions and their splat cost more
   than the scalars); the pin's BoUpSLP cuts the conversions out (gathers them) and the multiply and stores stay packed:
   -2. Ours had no cut and kept every lane scalar (`mulsd` x4). */
double t[4];

void fill(unsigned i)
{
    t[0] = (i + 0) * 0.5;
    t[1] = (i + 1) * 0.5;
    t[2] = (i + 2) * 0.5;
    t[3] = (i + 3) * 0.5;
}
