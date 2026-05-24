use chrono::{DateTime, Utc};

use crate::types::{
    common::{EquatorialDegrees, Nutation},
    consts::{JD_J2000, JULIAN_DAY_UNIX_EPOCH, SECONDS_PER_DAY},
};

/// Converts a UTC timestamp to Julian Day.
#[must_use]
pub fn datetime_to_julian_day(datetime: DateTime<Utc>) -> f64 {
    let unix_seconds =
        datetime.timestamp() as f64 + f64::from(datetime.timestamp_subsec_nanos()) * 1e-9;
    unix_seconds / SECONDS_PER_DAY + JULIAN_DAY_UNIX_EPOCH
}

/// Converts J2000 catalog coordinates to JNow/observed-of-date coordinates.
///
/// This follows same transformation order used by INDI: precession, nutation, then aberration.
/// Right ascension is expressed in hours and declination in degrees.
#[must_use]
pub fn j2000_to_jnow(ra_hours: f64, dec_deg: f64, jd: f64) -> (f64, f64) {
    let nutation = Nutation::from_julian_day(jd);

    EquatorialDegrees::from_ra_hours_dec_degrees(ra_hours, dec_deg)
        .precessed_between_julian_days(JD_J2000, jd)
        .with_nutation(nutation)
        .with_aberration(jd)
        .to_ra_hours_dec_degrees()
}

/// Converts J2000 catalog coordinates to JNow/observed-of-date coordinates at a UTC instant.
#[must_use]
pub fn j2000_to_jnow_at(ra_hours: f64, dec_deg: f64, observed_at: DateTime<Utc>) -> (f64, f64) {
    j2000_to_jnow(ra_hours, dec_deg, datetime_to_julian_day(observed_at))
}

/// Converts JNow/observed-of-date coordinates back to J2000 catalog coordinates.
///
/// This mirrors INDI inverse path: remove aberration, remove nutation, then precess back to
/// J2000. Right ascension is expressed in hours and declination in degrees.
#[must_use]
pub fn jnow_to_j2000(ra_hours: f64, dec_deg: f64, jd: f64) -> (f64, f64) {
    let nutation = Nutation::from_julian_day(jd);

    EquatorialDegrees::from_ra_hours_dec_degrees(ra_hours, dec_deg)
        .without_aberration(jd)
        .without_nutation(nutation)
        .precessed_between_julian_days(jd, JD_J2000)
        .to_ra_hours_dec_degrees()
}

/// Converts JNow/observed-of-date coordinates back to J2000 at a UTC instant.
#[must_use]
pub fn jnow_to_j2000_at(ra_hours: f64, dec_deg: f64, observed_at: DateTime<Utc>) -> (f64, f64) {
    jnow_to_j2000(ra_hours, dec_deg, datetime_to_julian_day(observed_at))
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
