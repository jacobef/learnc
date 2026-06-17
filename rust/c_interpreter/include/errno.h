#ifndef __CODEX_ERRNO_H
#define __CODEX_ERRNO_H

#define EDOM 33
#define ERANGE 34
#define EILSEQ 92

int *__codex_errno_location(void);
#define errno (*__codex_errno_location())

#endif
