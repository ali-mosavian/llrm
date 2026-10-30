/* The OS layer's interface for this target (runtime/shared/interface.toml), rendered: far data pointers. */
#ifndef LLRM_OS_H
#define LLRM_OS_H
/* borland.h erases __cdecl; the row's default is the register convention, the OS layer's is the stack's. */
#define LLRM_OS_CDECL __attribute__((__cdecl__))

short LLRM_OS_CDECL llrm_os_open(const char __far *, unsigned char);
#pragma aux llrm_os_open "LL$OPEN"
short LLRM_OS_CDECL llrm_os_create(const char __far *);
#pragma aux llrm_os_create "LL$CREATE"
long LLRM_OS_CDECL llrm_os_read(short, unsigned char __far *, unsigned);
#pragma aux llrm_os_read "LL$READ"
long LLRM_OS_CDECL llrm_os_seek(short, long, unsigned char);
#pragma aux llrm_os_seek "LL$SEEK"
long LLRM_OS_CDECL llrm_os_write_file(short, const unsigned char __far *, unsigned);
#pragma aux llrm_os_write_file "LL$WRITE_FILE"
int LLRM_OS_CDECL llrm_os_close(short);
#pragma aux llrm_os_close "LL$CLOSE"
void LLRM_OS_CDECL llrm_os_exit(unsigned char);
#pragma aux llrm_os_exit "LL$EXIT"
unsigned char * LLRM_OS_CDECL llrm_os_more(unsigned);
#pragma aux llrm_os_more "LL$MORE"
unsigned char LLRM_OS_CDECL llrm_os_console_read_key(void);
#pragma aux llrm_os_console_read_key "LL$CONSOLE_READ_KEY"
unsigned char LLRM_OS_CDECL llrm_os_console_key_ready(void);
#pragma aux llrm_os_console_key_ready "LL$CONSOLE_KEY_READY"
unsigned LLRM_OS_CDECL llrm_os_block_resize(unsigned, unsigned);
#pragma aux llrm_os_block_resize "LL$BLOCK_RESIZE"
int LLRM_OS_CDECL llrm_os_command_line(unsigned char __far *, unsigned);
#pragma aux llrm_os_command_line "LL$COMMAND_LINE"
int LLRM_OS_CDECL llrm_os_environment(unsigned, unsigned char __far *, unsigned);
#pragma aux llrm_os_environment "LL$ENVIRONMENT"
unsigned LLRM_OS_CDECL llrm_os_environment_read(unsigned, unsigned char __far *, unsigned);
#pragma aux llrm_os_environment_read "LL$ENVIRONMENT_READ"
int LLRM_OS_CDECL llrm_os_environment_write(unsigned, const unsigned char __far *, unsigned);
#pragma aux llrm_os_environment_write "LL$ENVIRONMENT_WRITE"
int LLRM_OS_CDECL llrm_os_execute(const char __far *, const char __far *);
#pragma aux llrm_os_execute "LL$EXECUTE"
int LLRM_OS_CDECL llrm_os_delete(const char __far *);
#pragma aux llrm_os_delete "LL$DELETE"
int LLRM_OS_CDECL llrm_os_rename(const char __far *, const char __far *);
#pragma aux llrm_os_rename "LL$RENAME"
int LLRM_OS_CDECL llrm_os_mkdir(const char __far *);
#pragma aux llrm_os_mkdir "LL$MKDIR"
int LLRM_OS_CDECL llrm_os_chdir(const char __far *);
#pragma aux llrm_os_chdir "LL$CHDIR"
int LLRM_OS_CDECL llrm_os_rmdir(const char __far *);
#pragma aux llrm_os_rmdir "LL$RMDIR"
int LLRM_OS_CDECL llrm_os_find_first(const char __far *, unsigned, unsigned char __far *);
#pragma aux llrm_os_find_first "LL$FIND_FIRST"
int LLRM_OS_CDECL llrm_os_find_next(unsigned char __far *);
#pragma aux llrm_os_find_next "LL$FIND_NEXT"
long LLRM_OS_CDECL llrm_os_free_bytes(unsigned);
#pragma aux llrm_os_free_bytes "LL$FREE_BYTES"
int LLRM_OS_CDECL llrm_os_current_directory(unsigned, unsigned char __far *);
#pragma aux llrm_os_current_directory "LL$CURRENT_DIRECTORY"
int LLRM_OS_CDECL llrm_os_current_drive(void);
#pragma aux llrm_os_current_drive "LL$CURRENT_DRIVE"
#define LLRM_OS_DENIED 5
#define LLRM_OS_NOT_FOUND 2
#define LLRM_OS_PATH_NOT_FOUND 3
#define LLRM_OS_STDIN 0
#define LLRM_OS_STDOUT 1
#define LLRM_OS_STDERR 2

/* The colour text screen, 80x25 cells of a character and an attribute. */
#define LLRM_OS_TEXT_SCREEN ((unsigned char __far *)((((unsigned long)PHYSICAL_TEXT_SCREEN >> 4) << 16) | ((unsigned long)PHYSICAL_TEXT_SCREEN & 15)))

#endif
