/* WCC aggregate capture: the raiser uses T25's size, not array shape. */
int fill_cells(int value)
{
    short cells[4];
    int index;

    for (index = 0; index < 4; ++index) cells[index] = value;
    return cells[3];
}
