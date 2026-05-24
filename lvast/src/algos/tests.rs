use crate::types::consts;

use super::*;
use chrono::TimeZone;
use chrono::Utc;

fn ra_error_arcsec(lhs_hours: f64, rhs_hours: f64, dec_deg: f64) -> f64 {
    let delta_degrees =
        ((lhs_hours - rhs_hours) * consts::HOURS_TO_DEGREES + 540.0).rem_euclid(360.0) - 180.0;

    delta_degrees.abs() * 3600.0 * dec_deg.to_radians().cos()
}

#[test]
fn datetime_to_julian_day_matches_j2000_epoch() {
    let datetime = Utc.with_ymd_and_hms(2000, 1, 1, 12, 0, 0).single().unwrap();
    assert!((datetime_to_julian_day(datetime) - consts::JD_J2000).abs() < 1e-9);
}

#[test]
fn j2000_to_jnow_matches_indi_reference_case() {
    let (ra, dec) = j2000_to_jnow(20.69053168, 45.28033881, 2459019.833333);
    let ra_err = ra_error_arcsec(ra, 20.70237028, 45.35036333);
    let dec_err = (dec - 45.35036333).abs() * 3600.0;
    let total_err = ra_err.hypot(dec_err);

    assert!(
        total_err < 1.0,
        "expected <1 arcsec total error, got RA={ra_err:.3}\" Dec={dec_err:.3}\" total={total_err:.3}\""
    );
}

#[test]
fn jnow_round_trip_recovers_j2000_position() {
    for jd in [2459019.833333, 2461112.5] {
        let (ra_jnow, dec_jnow) = j2000_to_jnow(20.69053168, 45.28033881, jd);
        let (ra_j2000, dec_j2000) = jnow_to_j2000(ra_jnow, dec_jnow, jd);
        let ra_err = ra_error_arcsec(ra_j2000, 20.69053168, 45.28033881);
        let dec_err = (dec_j2000 - 45.28033881).abs() * 3600.0;

        assert!(
            ra_err < 1.0,
            "expected <1 arcsec RA round-trip error at JD {jd}, got {ra_err:.3}\""
        );
        assert!(
            dec_err < 1.0,
            "expected <1 arcsec Dec round-trip error at JD {jd}, got {dec_err:.3}\""
        );
    }
}
