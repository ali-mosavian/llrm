/* Shared by the runtime's C files. */

typedef signed char i8;
typedef unsigned char u8;
typedef int i16;
typedef unsigned int u16;
typedef unsigned long u32;
typedef long i32;

/* A string's or vec's data pointer follows its descriptor:
 * [flags][pad][length][capacity][data...][NUL]. */
enum {
    HEAP = 0x01,
    READONLY = 0x08
};

extern void rt_write(const char *text, u16 length);
extern void rt_exit(u8 code);
extern void rt_panic(const char *message);

extern u8 *flags_of(char *data);
extern u16 *length_of(char *data);
extern u16 *capacity_of(char *data);
extern char *rt_alloc(u16 capacity, u16 size);
extern char *rt_reserve(char *data, u16 capacity, u16 size);
extern void rt_drop(char *data);
extern void copy_bytes(char *to, const char *from, u16 count);
