#include <time.h>

int main(void) {
    struct tm t;
    struct tm *round_trip;
    time_t future;
    t.tm_year = 101;
    t.tm_mon = 6;
    t.tm_mday = 4;
    t.tm_hour = 0;
    t.tm_min = 0;
    t.tm_sec = 1;
    t.tm_isdst = -1;
    if (mktime(&t) == (time_t)-1) return 1;
    if (t.tm_wday != 3) return 2;
    if (t.tm_yday != 184) return 3;

    t.tm_year = 140;
    t.tm_mon = 0;
    t.tm_mday = 1;
    t.tm_isdst = -1;
    if (mktime(&t) <= 2147483647L) return 4;
    if (t.tm_year != 140) return 5;

    future = 2208988800L;
    round_trip = gmtime(&future);
    if (round_trip == NULL || round_trip->tm_year != 140) return 6;
    if (difftime(future, (time_t)0) != 2208988800.0) return 7;
    return 0;
}
