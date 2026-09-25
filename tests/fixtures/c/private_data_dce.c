/* A private static object unused after whole-module optimization must not be emitted. */
static short unusedValue = 17;
static short retainedValue = 23;
static short *retainedPointer = &retainedValue;

short readUsedValue(short value)
{
    return (short)(value + *retainedPointer);
}
