/* The OS layer's interface for this target (runtime/shared/interface.toml), rendered: near data pointers. */
#ifndef LLRM_OS_H
#define LLRM_OS_H

short __cdecl llrm_os_open(const char *, unsigned char);
#pragma aux llrm_os_open "LL$OPEN"
short __cdecl llrm_os_create(const char *);
#pragma aux llrm_os_create "LL$CREATE"
long __cdecl llrm_os_read(short, unsigned char *, unsigned);
#pragma aux llrm_os_read "LL$READ"
long __cdecl llrm_os_seek(short, long, unsigned char);
#pragma aux llrm_os_seek "LL$SEEK"
long __cdecl llrm_os_write_file(short, const unsigned char *, unsigned);
#pragma aux llrm_os_write_file "LL$WRITE_FILE"
int __cdecl llrm_os_close(short);
#pragma aux llrm_os_close "LL$CLOSE"
void __cdecl llrm_os_exit(unsigned char);
#pragma aux llrm_os_exit "LL$EXIT"
unsigned char * __cdecl llrm_os_more(unsigned);
#pragma aux llrm_os_more "LL$MORE"
unsigned char __cdecl llrm_os_console_read_key(void);
#pragma aux llrm_os_console_read_key "LL$CONSOLE_READ_KEY"
unsigned char __cdecl llrm_os_console_key_ready(void);
#pragma aux llrm_os_console_key_ready "LL$CONSOLE_KEY_READY"
int __cdecl llrm_os_command_line(unsigned char *, unsigned);
#pragma aux llrm_os_command_line "LL$COMMAND_LINE"
int __cdecl llrm_os_environment(unsigned, unsigned char *, unsigned);
#pragma aux llrm_os_environment "LL$ENVIRONMENT"
unsigned __cdecl llrm_os_environment_read(unsigned, unsigned char *, unsigned);
#pragma aux llrm_os_environment_read "LL$ENVIRONMENT_READ"
int __cdecl llrm_os_environment_write(unsigned, const unsigned char *, unsigned);
#pragma aux llrm_os_environment_write "LL$ENVIRONMENT_WRITE"
int __cdecl llrm_os_execute(const char *, const char *);
#pragma aux llrm_os_execute "LL$EXECUTE"
int __cdecl llrm_os_delete(const char *);
#pragma aux llrm_os_delete "LL$DELETE"
int __cdecl llrm_os_rename(const char *, const char *);
#pragma aux llrm_os_rename "LL$RENAME"
int __cdecl llrm_os_mkdir(const char *);
#pragma aux llrm_os_mkdir "LL$MKDIR"
int __cdecl llrm_os_chdir(const char *);
#pragma aux llrm_os_chdir "LL$CHDIR"
int __cdecl llrm_os_rmdir(const char *);
#pragma aux llrm_os_rmdir "LL$RMDIR"
int __cdecl llrm_os_find_first(const char *, unsigned, unsigned char *);
#pragma aux llrm_os_find_first "LL$FIND_FIRST"
int __cdecl llrm_os_find_next(unsigned char *);
#pragma aux llrm_os_find_next "LL$FIND_NEXT"
long __cdecl llrm_os_free_bytes(unsigned);
#pragma aux llrm_os_free_bytes "LL$FREE_BYTES"
int __cdecl llrm_os_current_directory(unsigned, unsigned char *);
#pragma aux llrm_os_current_directory "LL$CURRENT_DIRECTORY"
int __cdecl llrm_os_current_drive(void);
#pragma aux llrm_os_current_drive "LL$CURRENT_DRIVE"
#define LLRM_OS_DENIED 5
#define LLRM_OS_NOT_FOUND 2
#define LLRM_OS_PATH_NOT_FOUND 3
#define LLRM_OS_STDIN 0
#define LLRM_OS_STDOUT 1
#define LLRM_OS_STDERR 2

/* The colour text screen, 80x25 cells of a character and an attribute. */
#define LLRM_OS_TEXT_SCREEN ((unsigned char *)PHYSICAL_TEXT_SCREEN)

#endif
