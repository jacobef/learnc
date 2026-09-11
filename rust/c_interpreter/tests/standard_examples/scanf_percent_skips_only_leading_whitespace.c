#include <stdio.h>
#include <wchar.h>

int main(void) {
    int i = 0;

    if (sscanf("foo  %bar 42", "foo%%bar%d", &i) != 1 || i != 42) return 1;
    if (sscanf("foo  %  bar 42", "foo%%bar%d", &i) != 0) return 2;
    if (sscanf("foo  %  bar 42", "foo%% bar%d", &i) != 1 || i != 42) return 3;
    if (swscanf(L"foo  %bar 42", L"foo%%bar%d", &i) != 1 || i != 42) return 4;
    if (sscanf(" x", "x") != 0) return 5;
    if (sscanf("%", "%%%d", &i) != 0) return 6;
    if (swscanf(L"%", L"%%%d", &i) != 0) return 7;
    if (sscanf("123", "%*d%d", &i) != 0) return 8;
    if (sscanf("", "%n%d", &i, &i) != 0 || i != 0) return 9;
    if (sscanf("", "%%") != EOF) return 10;
    return 0;
}
