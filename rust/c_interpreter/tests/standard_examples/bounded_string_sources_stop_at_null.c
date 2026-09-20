#include <string.h>
#include <wchar.h>

int main(void) {
    char padded[6] = "xxxxx";
    if (strncpy(padded, "a", 4) != padded) return 1;
    if (padded[0] != 'a' || padded[1] || padded[2] || padded[3] || padded[4] != 'x') return 2;
    char source[4];
    source[0] = 'b';
    source[1] = 0;
    char joined[8] = "a";
    if (strncat(joined, source, 1000000) != joined || strcmp(joined, "ab")) return 3;
    char exact[2] = {'x', 'y'};
    strncpy(padded, exact, 2);
    if (padded[0] != 'x' || padded[1] != 'y') return 4;
    char shared[8] = "a";
    strncpy(shared + 2, shared, 4);
    if (shared[2] != 'a' || shared[3] || shared[4] || shared[5]) return 5;
    wchar_t wide[6] = L"xxxxx";
    if (wcsncpy(wide, L"a", 4) != wide) return 6;
    if (wide[0] != L'a' || wide[1] || wide[2] || wide[3] || wide[4] != L'x') return 7;
    wchar_t wide_joined[8] = L"a";
    if (wcsncat(wide_joined, L"b", 1000000) != wide_joined || wcscmp(wide_joined, L"ab")) return 8;
    return 0;
}
