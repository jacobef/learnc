#ifndef __CBOXES_ERRNO_H
#define __CBOXES_ERRNO_H

#define EDOM 33
#define ERANGE 34
#define EILSEQ 92

int *__cboxes_errno_location(void);
#define errno (*__cboxes_errno_location())

#endif
