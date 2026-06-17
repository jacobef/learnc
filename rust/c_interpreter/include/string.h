#ifndef __CODEX_STRING_H
#define __CODEX_STRING_H

#include <stddef.h>

void *memcpy(void *restrict dest, const void *restrict src, size_t n);
void *memmove(void *dest, const void *src, size_t n);
void *memset(void *dest, int ch, size_t n);
void *memchr(const void *s, int c, size_t n);
int memcmp(const void *lhs, const void *rhs, size_t n);

char *strcpy(char *restrict dest, const char *restrict src);
char *strncpy(char *restrict dest, const char *restrict src, size_t n);
char *strcat(char *restrict dest, const char *restrict src);
char *strncat(char *restrict dest, const char *restrict src, size_t n);

int strcmp(const char *lhs, const char *rhs);
int strncmp(const char *lhs, const char *rhs, size_t n);
int strcoll(const char *lhs, const char *rhs);

size_t strxfrm(char *restrict dest, const char *restrict src, size_t n);
char *strerror(int errnum);
char *strdup(const char *s);

size_t strlen(const char *s);
char *strchr(const char *s, int c);
char *strrchr(const char *s, int c);
size_t strspn(const char *s, const char *accept);
size_t strcspn(const char *s, const char *reject);
char *strpbrk(const char *s, const char *accept);
char *strstr(const char *haystack, const char *needle);
char *strtok(char *restrict s, const char *restrict delim);

#endif
