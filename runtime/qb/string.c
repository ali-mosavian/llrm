/* QB dynamic strings: stable four-byte descriptors, movable near payloads. */

typedef unsigned char byte;
typedef unsigned short word;

typedef struct {
    word length;
    word data;
} String;

typedef struct {
    word owner;
    word length;
} Allocation;

#define HEAP_BYTES 8192
#define TEMPORARIES 32

static byte heap[HEAP_BYTES];
static word heapUsed;
static String nullString;
static String temporary[TEMPORARIES];
static byte temporaryUsed[TEMPORARIES];

static word addressOf(const void *pointer) {
    return (word)pointer;
}

static String *descriptor(word address) {
    return (String *)address;
}

static word descriptorAddress(const String *value) {
    return addressOf(value);
}

static word temporaryIndex(const String *value) {
    word address = descriptorAddress(value);
    word first = descriptorAddress(&temporary[0]);
    word bytes = TEMPORARIES * sizeof(String);

    if (address < first || address >= first + bytes || ((address - first) % sizeof(String)) != 0) {
        return TEMPORARIES;
    }
    return (address - first) / sizeof(String);
}

static String *temporaryDescriptor(void) {
    word index;

    for (index = 0; index < TEMPORARIES; ++index) {
        if (temporaryUsed[index] == 0) {
            temporaryUsed[index] = 1;
            temporary[index].length = 0;
            temporary[index].data = 0;
            return &temporary[index];
        }
    }
    return (String *)0;
}

static void releaseTemporary(String *value) {
    word index = temporaryIndex(value);

    if (index != TEMPORARIES) {
        temporaryUsed[index] = 0;
        value->length = 0;
        value->data = 0;
    }
}

static void copyBytes(byte *destination, const byte *source, word length) {
    while (length != 0) {
        *destination++ = *source++;
        --length;
    }
}

static void compact(void) {
    byte *read = heap;
    byte *write = heap;
    byte *end = heap + heapUsed;

    while (read != end) {
        Allocation *old = (Allocation *)read;
        word bytes = old->length;
        byte *payload = read + sizeof(Allocation);
        String *owner = descriptor(old->owner);

        if (owner->data == addressOf(payload)) {
            Allocation *next = (Allocation *)write;
            byte *nextPayload = write + sizeof(Allocation);
            if (write != read) {
                copyBytes(nextPayload, payload, bytes);
            }
            next->owner = old->owner;
            next->length = bytes;
            owner->data = addressOf(nextPayload);
            write = nextPayload + bytes;
        }
        read = payload + bytes;
    }
    heapUsed = (word)(write - heap);
}

static byte *allocate(word owner, word length) {
    word header = sizeof(Allocation);
    Allocation *allocation;
    byte *payload;

    compact();
    if (length > HEAP_BYTES - header || heapUsed > HEAP_BYTES - header - length) {
        return (byte *)0;
    }

    allocation = (Allocation *)(heap + heapUsed);
    payload = (byte *)(allocation + 1);
    allocation->owner = owner;
    allocation->length = length;
    heapUsed += header + length;
    return payload;
}

void qb_string_delete(word destinationAddress) {
    String *destination = descriptor(destinationAddress);

    destination->length = 0;
    destination->data = 0;
    releaseTemporary(destination);
}

void qb_string_assign(word sourceAddress, word destinationAddress) {
    String *source = descriptor(sourceAddress);
    String *destination = descriptor(destinationAddress);
    word length;
    byte *payload;
    Allocation *allocation;

    if (source == destination) {
        return;
    }

    length = source->length;
    qb_string_delete(destinationAddress);
    if (temporaryIndex(source) != TEMPORARIES) {
        destination->length = length;
        destination->data = source->data;
        if (length != 0) {
            allocation = ((Allocation *)source->data) - 1;
            allocation->owner = destinationAddress;
        }
        releaseTemporary(source);
        return;
    }
    if (length == 0) {
        return;
    }

    payload = allocate(destinationAddress, length);
    if (payload == (byte *)0) {
        return;
    }

    copyBytes(payload, (const byte *)source->data, length);
    destination->length = length;
    destination->data = addressOf(payload);
}

word qb_string_space(word length) {
    String *result;
    byte *payload;
    word index;

    if (length == 0) {
        return descriptorAddress(&nullString);
    }
    result = temporaryDescriptor();
    if (result == (String *)0) {
        return descriptorAddress(&nullString);
    }
    payload = allocate(descriptorAddress(result), length);
    if (payload == (byte *)0) {
        releaseTemporary(result);
        return descriptorAddress(&nullString);
    }
    for (index = 0; index < length; ++index) {
        payload[index] = ' ';
    }
    result->length = length;
    result->data = addressOf(payload);
    return descriptorAddress(result);
}
