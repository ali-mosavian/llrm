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
    word data;
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

static Block *allocate(word bytes) {
    Block *block = blockAt(freeBlocks);

    while (block != (Block *)0) {
        if (block->bytes >= bytes) {
            freeBlocks = block->next;
            return block;
        }
        block = blockAt(block->next);
    }

    if (bytes > 65535U - sizeof(Block) - 15U) {
        return (Block *)0;
    }
    block = (Block *)qb_array_more(bytes + sizeof(Block) + 15U);
    if (block == (Block *)0) {
        return (Block *)0;
    }
    block->next = 0;
    block->bytes = bytes;
    block->data = (addressOf(block + 1) + 15U) & (word)~15U;
    return block;
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
    Block *block;
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

    block = allocate((word)byteCount);
    if (block == (Block *)0) {
        clear(array);
        return 0;
    }
    data = (byte *)block->data;

    array->data = addressOf(data);
    array->next = addressOf(block);
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

    block = blockAt(array->next);
    block->next = freeBlocks;
    freeBlocks = addressOf(block);
    clear(array);
}
