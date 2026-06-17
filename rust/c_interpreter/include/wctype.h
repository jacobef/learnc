#ifndef __CODEX_WCTYPE_H
#define __CODEX_WCTYPE_H

typedef int wint_t;
typedef unsigned int wctype_t;
typedef int wctrans_t;

#define WEOF ((wint_t)-1)

int iswalnum(wint_t wc);
int iswalpha(wint_t wc);
int iswblank(wint_t wc);
int iswcntrl(wint_t wc);
int iswdigit(wint_t wc);
int iswgraph(wint_t wc);
int iswlower(wint_t wc);
int iswprint(wint_t wc);
int iswpunct(wint_t wc);
int iswspace(wint_t wc);
int iswupper(wint_t wc);
int iswxdigit(wint_t wc);

int iswctype(wint_t wc, wctype_t desc);
wint_t towlower(wint_t wc);
wint_t towupper(wint_t wc);
wint_t towctrans(wint_t wc, wctrans_t desc);
wctrans_t wctrans(const char *property);
wctype_t wctype(const char *property);

#endif
