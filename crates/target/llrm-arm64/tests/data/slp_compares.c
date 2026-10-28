long env(int, char *, int);

/* COMSPEC= at the start of a line: eight byte compares. */
int comspec(void) {
    char line[128];
    long length = env(0, line, 127);
    return length > 8 && line[0] == 'C' && line[1] == 'O' && line[2] == 'M' && line[3] == 'S' &&
           line[4] == 'P' && line[5] == 'E' && line[6] == 'C' && line[7] == '=';
}

/* Four bytes: one compare; two or three stay bytes. */
int four(void) {
    char line[16];
    env(0, line, 15);
    return line[0] == 'A' && line[1] == 'B' && line[2] == 'C' && line[3] == 'D';
}

int two(void) {
    char line[16];
    env(0, line, 15);
    return line[0] == 'A' && line[1] == 'B';
}

/* A store between the loads: nothing to merge across it. */
int stored_between(void) {
    char line[16];
    env(0, line, 15);
    int a = line[0] == 'A' && line[1] == 'B';
    env(1, line, 15);
    return a & (line[2] == 'C' && line[3] == 'D');
}

/* Under an or, a leaf is not a conjunct. */
int under_or(void) {
    char line[16];
    env(0, line, 15);
    return (line[0] == 'A' && line[1] == 'B') || (line[2] == 'C' && line[3] == 'D');
}
