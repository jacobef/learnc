use super::*;

pub(super) fn make_local_time(tm: &mut HostTm) -> i64 {
    #[cfg(not(target_os = "wasi"))]
    {
        unsafe { mktime(tm) as i64 }
    }
    #[cfg(target_os = "wasi")]
    {
        make_utc_time(tm).unwrap_or(-1)
    }
}

// WASI has no local timezone service and its mktime stub traps. The browser
// execution environment uses UTC. Normalize through days in the Gregorian
// calendar so out-of-range input fields and dates before 1970 remain valid.
#[cfg(any(test, target_os = "wasi"))]
fn civil_days(year: i64, month: i64, day: i64) -> i64 {
    let year = year - i64::from(month <= 2);
    let era = year.div_euclid(400);
    let y = year - era * 400;
    let m = month + if month > 2 { -3 } else { 9 };
    era * 146097 + y * 365 + y / 4 - y / 100 + (153 * m + 2) / 5 + day - 1 - 719468
}

#[cfg(any(test, target_os = "wasi"))]
fn make_utc_time(tm: &mut HostTm) -> Option<i64> {
    let year = i64::from(tm.tm_year) + 1900 + i64::from(tm.tm_mon).div_euclid(12);
    let month = i64::from(tm.tm_mon).rem_euclid(12) + 1;
    let seconds = civil_days(year, month, 1) * 86400
        + (i64::from(tm.tm_mday) - 1) * 86400
        + i64::from(tm.tm_hour) * 3600
        + i64::from(tm.tm_min) * 60
        + i64::from(tm.tm_sec);
    let days = seconds.div_euclid(86400);
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = y + i64::from(month <= 2);
    let normalized_year = c_int::try_from(year - 1900).ok()?;
    tm.tm_year = normalized_year;
    tm.tm_mon = (month - 1) as c_int;
    tm.tm_mday = day as c_int;
    tm.tm_hour = (seconds.rem_euclid(86400) / 3600) as c_int;
    tm.tm_min = (seconds.rem_euclid(3600) / 60) as c_int;
    tm.tm_sec = seconds.rem_euclid(60) as c_int;
    tm.tm_wday = (days + 4).rem_euclid(7) as c_int;
    tm.tm_yday = (days - civil_days(year, 1, 1)) as c_int;
    tm.tm_isdst = 0;
    tm.tm_gmtoff = 0;
    tm.tm_zone = c"UTC".as_ptr();
    Some(seconds)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn utc_normalization_handles_negative_dates_leap_years_and_2038() {
        for (year, month, day, expected) in [
            (2001, 7, 4, 994204800),
            (1970, 1, 1, 0),
            (1969, 12, 31, -86400),
            (2000, 3, 0, 951782400),
            (2040, 1, 1, 2208988800),
        ] {
            let mut tm: HostTm = unsafe { std::mem::zeroed() };
            tm.tm_year = year - 1900;
            tm.tm_mon = month - 1;
            tm.tm_mday = day;
            assert_eq!(make_utc_time(&mut tm), Some(expected));
            assert_eq!(make_utc_time(&mut tm), Some(expected));
        }
    }
}
