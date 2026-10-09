/* QB dynamic-array descriptors and their reusable near allocation arena. */

typedef unsigned char byte;
typedef unsigned short word;
typedef unsigned long dword;
typedef short sword;

typedef struct {
    word count;
    word lower;
} Dimension;

typedef struct {
    word data;
    word segment;
    word next;
    word bytes;
    byte dimensions;
    byte features;
    word adjusted;
    word element;
    Dimension dimension[1];
} Array;

typedef struct Block {
    word next;
    word bytes;
} Block;

extern byte *qb_array_more(word bytes);

static word freeBlocks;
word qb_array_bounds;

static word addressOf(const void *pointer) {
    return (word)pointer;
}

static Block *blockAt(word address) {
    return (Block *)address;
}

static byte *allocate(word bytes) {
    Block *previous = (Block *)0;
    Block *block = blockAt(freeBlocks);
    Block *following;
    byte *data;
    word remaining;

    while (block != (Block *)0) {
        if (block->bytes >= bytes) {
            data = (byte *)(block + 1);
            remaining = block->bytes - bytes;
            if (remaining >= sizeof(Block) + 2) {
                following = (Block *)(data + bytes);
                following->next = block->next;
                following->bytes = remaining - sizeof(Block);
                if (previous == (Block *)0) {
                    freeBlocks = addressOf(following);
                } else {
                    previous->next = addressOf(following);
                }
            } else if (previous == (Block *)0) {
                freeBlocks = block->next;
            } else {
                previous->next = block->next;
            }
            return data;
        }
        previous = block;
        block = blockAt(block->next);
    }

    block = (Block *)qb_array_more(bytes + sizeof(Block));
    if (block == (Block *)0) {
        return (byte *)0;
    }
    block->next = 0;
    block->bytes = bytes;
    return (byte *)(block + 1);
}

static void clear(Array *array) {
    array->data = 0;
    array->segment = 0;
    array->next = 0;
    array->bytes = 0;
    array->dimensions = 0;
    array->features = 0;
    array->adjusted = 0;
    array->element = 0;
}

word qb_array_dim(word arrayAddress, word typeAndDimensions, word elementBytes) {
    Array *array = (Array *)arrayAddress;
    const word *bounds = (const word *)qb_array_bounds;
    word dimensions = typeAndDimensions & 255;
    word index;
    word count;
    sword upper;
    sword lower;
    sword adjustment = 0;
    dword total = 1;
    dword byteCount;
    byte *data;

    clear(array);
    if (dimensions == 0 || elementBytes == 0) {
        return 0;
    }

    for (index = 0; index < dimensions; ++index) {
        upper = (sword)bounds[index * 2];
        lower = (sword)bounds[index * 2 + 1];
        if (upper < lower) {
            clear(array);
            return 0;
        }
        count = (word)(upper - lower + 1);
        total *= count;
        if (total > 65535UL) {
            clear(array);
            return 0;
        }
        adjustment = adjustment * count - lower;
        array->dimension[index].count = count;
        array->dimension[index].lower = lower;
    }

    byteCount = total * elementBytes;
    if (byteCount == 0 || byteCount > 65535UL) {
        clear(array);
        return 0;
    }

    data = allocate((word)byteCount);
    if (data == (byte *)0) {
        clear(array);
        return 0;
    }

    array->data = addressOf(data);
    array->bytes = (word)byteCount;
    array->dimensions = (byte)dimensions;
    array->features = (byte)(typeAndDimensions >> 8);
    array->adjusted = adjustment * elementBytes + array->data;
    array->element = elementBytes;
    return 1;
}

void qb_array_erase(word arrayAddress) {
    Array *array = (Array *)arrayAddress;
    Block *block;

    if (array->data == 0) {
        clear(array);
        return;
    }

    block = ((Block *)array->data) - 1;
    block->next = freeBlocks;
    freeBlocks = addressOf(block);
    clear(array);
}
