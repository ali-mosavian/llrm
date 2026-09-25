extern void checkpoint(void);

double separated(double home, double left, double right, double scale, double first, double second)
{
    double difference = home - left;
    double result = difference * difference + difference * home + difference * left;
    checkpoint();
    difference = home - right;
    result += difference * difference;
    first += difference * scale;
    second -= difference * scale;
    return result + first + second;
}
