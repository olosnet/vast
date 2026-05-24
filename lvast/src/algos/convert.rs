use chrono::{DateTime, Utc};
use std::f64::consts::PI;

use crate::types::{
    common::{EquatorialDegrees, Nutation},
    consts::{
        ABERRATION_ARGUMENTS, Coefficients4, JD_J2000, JULIAN_DAY_UNIX_EPOCH, NUTATION_ARGUMENTS,
        NUTATION_COEFFICIENTS, SECONDS_PER_DAY, SPEED_OF_LIGHT_AU_PER_DAY, X_COEFFICIENTS,
        Y_COEFFICIENTS, Z_COEFFICIENTS,
    },
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
/// This follows the same transformation order used by INDI: precession, nutation, then
/// aberration. Right ascension is expressed in hours and declination in degrees.
#[must_use]
pub fn j2000_to_jnow(ra_hours: f64, dec_deg: f64, jd: f64) -> (f64, f64) {
    let j2000 = EquatorialDegrees::from_hours_degrees(ra_hours, dec_deg);
    let mean_of_date = precess_equatorial(j2000, JD_J2000, jd);
    let nutated = apply_nutation(mean_of_date, jd, false);
    let observed = apply_aberration(nutated, jd);
    observed.to_hours_degrees()
}

/// Converts J2000 catalog coordinates to JNow/observed-of-date coordinates at a UTC instant.
#[must_use]
pub fn j2000_to_jnow_at(ra_hours: f64, dec_deg: f64, observed_at: DateTime<Utc>) -> (f64, f64) {
    j2000_to_jnow(ra_hours, dec_deg, datetime_to_julian_day(observed_at))
}

/// Converts JNow/observed-of-date coordinates back to J2000 catalog coordinates.
///
/// This mirrors INDI's inverse path: remove aberration, remove nutation, then precess back to
/// J2000. Right ascension is expressed in hours and declination in degrees.
#[must_use]
pub fn jnow_to_j2000(ra_hours: f64, dec_deg: f64, jd: f64) -> (f64, f64) {
    let observed = EquatorialDegrees::from_hours_degrees(ra_hours, dec_deg);
    let deaberrated = remove_aberration(observed, jd);
    let denutated = apply_nutation(deaberrated, jd, true);
    let j2000 = precess_equatorial(denutated, jd, JD_J2000);
    j2000.to_hours_degrees()
}

/// Converts JNow/observed-of-date coordinates back to J2000 at a UTC instant.
#[must_use]
pub fn jnow_to_j2000_at(ra_hours: f64, dec_deg: f64, observed_at: DateTime<Utc>) -> (f64, f64) {
    jnow_to_j2000(ra_hours, dec_deg, datetime_to_julian_day(observed_at))
}

fn precess_equatorial(position: EquatorialDegrees, from_jd: f64, to_jd: f64) -> EquatorialDegrees {
    let mean_ra = degrees_to_radians(position.ra);
    let mean_dec = degrees_to_radians(position.dec);

    let t = (to_jd - from_jd) / 36525.0 / 3600.0;
    let t2 = t * t;
    let t3 = t2 * t;
    let epoch_offset = (from_jd - JD_J2000) / 36525.0 / 3600.0;
    let epoch_offset2 = epoch_offset * epoch_offset;

    let zeta = degrees_to_radians(
        (2306.2181 + 1.39656 * epoch_offset - 0.000139 * epoch_offset2) * t
            + (0.30188 - 0.000344 * epoch_offset) * t2
            + 0.017998 * t3,
    );
    let eta = degrees_to_radians(
        (2306.2181 + 1.39656 * epoch_offset - 0.000139 * epoch_offset2) * t
            + (1.09468 + 0.000066 * epoch_offset) * t2
            + 0.018203 * t3,
    );
    let theta = degrees_to_radians(
        (2004.3109 - 0.85330 * epoch_offset - 0.000217 * epoch_offset2) * t
            - (0.42665 + 0.000217 * epoch_offset) * t2
            - 0.041833 * t3,
    );

    let a = mean_dec.cos() * (mean_ra + zeta).sin();
    let b = theta.cos() * mean_dec.cos() * (mean_ra + zeta).cos() - theta.sin() * mean_dec.sin();
    let c = theta.sin() * mean_dec.cos() * (mean_ra + zeta).cos() + theta.cos() * mean_dec.sin();

    let ra = a.atan2(b) + eta;
    let dec = if mean_dec.abs() > 0.4 * PI {
        let mut pole_dec = clamp_unit((a * a + b * b).sqrt()).acos();
        if mean_dec < 0.0 {
            pole_dec = -pole_dec;
        }
        pole_dec
    } else {
        clamp_unit(c).asin()
    };

    EquatorialDegrees {
        ra: range_degrees(radians_to_degrees(ra)),
        dec: radians_to_degrees(dec),
    }
}

fn apply_nutation(position: EquatorialDegrees, jd: f64, reverse: bool) -> EquatorialDegrees {
    let nutation = get_nutation(jd);
    let mean_ra = degrees_to_radians(position.ra);
    let mean_dec = degrees_to_radians(position.dec);

    let true_obliquity = degrees_to_radians(nutation.ecliptic + nutation.obliquity);
    let sin_obliquity = true_obliquity.sin();
    let sin_ra = mean_ra.sin();
    let cos_ra = mean_ra.cos();
    let tan_dec = mean_dec.tan();

    let mut delta_ra = (true_obliquity.cos() + sin_obliquity * sin_ra * tan_dec)
        * nutation.longitude
        - cos_ra * tan_dec * nutation.obliquity;
    let mut delta_dec = (sin_obliquity * cos_ra) * nutation.longitude + sin_ra * nutation.obliquity;

    if reverse {
        delta_ra = -delta_ra;
        delta_dec = -delta_dec;
    }

    EquatorialDegrees {
        ra: range_degrees(position.ra + delta_ra),
        dec: position.dec + delta_dec,
    }
}

fn get_nutation(jd: f64) -> Nutation {
    let t = (jd - JD_J2000) / 36525.0;
    let t2 = t * t;
    let t3 = t2 * t;

    let d = degrees_to_radians(297.85036 + 445267.111480 * t - 0.0019142 * t2 + t3 / 189474.0);
    let m = degrees_to_radians(357.52772 + 35999.050340 * t - 0.0001603 * t2 - t3 / 300000.0);
    let mm = degrees_to_radians(134.96298 + 477198.867398 * t + 0.0086972 * t2 + t3 / 56250.0);
    let f = degrees_to_radians(93.2719100 + 483202.017538 * t - 0.0036825 * t2 + t3 / 327270.0);
    let o = degrees_to_radians(125.04452 - 1934.136261 * t + 0.0020708 * t2 + t3 / 450000.0);

    let mut longitude = 0.0;
    let mut obliquity = 0.0;

    for index in 0..NUTATION_ARGUMENTS.len() {
        let (arg_d, arg_m, arg_mm, arg_f, arg_o) = NUTATION_ARGUMENTS[index];
        let (longitude1, longitude2, obliquity1, obliquity2) = NUTATION_COEFFICIENTS[index];

        let argument = arg_d * d + arg_m * m + arg_mm * mm + arg_f * f + arg_o * o;
        longitude += (longitude1 + longitude2 * t) * argument.sin();
        obliquity += (obliquity1 + obliquity2 * t) * argument.cos();
    }

    longitude /= 10000.0 * 3600.0;
    obliquity /= 10000.0 * 3600.0;

    let ecliptic =
        23.0 + 26.0 / 60.0 + 21.448 / 3600.0 - 46.8150 / 3600.0 * t - 0.00059 / 3600.0 * t2
            + 0.001813 / 3600.0 * t3;

    Nutation {
        longitude,
        obliquity,
        ecliptic,
    }
}

fn apply_aberration(position: EquatorialDegrees, jd: f64) -> EquatorialDegrees {
    let t = (jd - JD_J2000) / 36525.0;

    let l2 = 3.1761467 + 1021.3285546 * t;
    let l3 = 1.7534703 + 628.3075849 * t;
    let l4 = 6.2034809 + 334.0612431 * t;
    let l5 = 0.5995464 + 52.9690965 * t;
    let l6 = 0.8740168 + 21.329909095 * t;
    let l7 = 5.4812939 + 7.4781599 * t;
    let l8 = 5.3118863 + 3.8133036 * t;
    let ll = 3.8103444 + 8399.6847337 * t;
    let d = 5.1984667 + 7771.3771486 * t;
    let mm = 2.3555559 + 8328.6914289 * t;
    let f = 1.6279052 + 8433.4661601 * t;

    let mut x = 0.0;
    let mut y = 0.0;
    let mut z = 0.0;

    for index in 0..ABERRATION_ARGUMENTS.len() {
        let (a_l2, a_l3, a_l4, a_l5, a_l6, a_l7, a_l8, a_ll, a_d, a_mm, a_f) =
            ABERRATION_ARGUMENTS[index];
        let argument = a_l2 * l2
            + a_l3 * l3
            + a_l4 * l4
            + a_l5 * l5
            + a_l6 * l6
            + a_l7 * l7
            + a_l8 * l8
            + a_ll * ll
            + a_d * d
            + a_mm * mm
            + a_f * f;

        x += weighted_sine_cosine(X_COEFFICIENTS[index], t, argument);
        y += weighted_sine_cosine(Y_COEFFICIENTS[index], t, argument);
        z += weighted_sine_cosine(Z_COEFFICIENTS[index], t, argument);
    }

    let mean_ra = degrees_to_radians(position.ra);
    let mean_dec = degrees_to_radians(position.dec);

    if mean_dec < PI * 0.4999 {
        let delta_ra = ((y * mean_ra.cos()) - (x * mean_ra.sin()))
            / mean_dec.cos()
            / SPEED_OF_LIGHT_AU_PER_DAY;
        let delta_dec = ((x * mean_ra.cos() + y * mean_ra.sin()) * mean_dec.sin()
            - z * mean_dec.cos())
            / -SPEED_OF_LIGHT_AU_PER_DAY;

        EquatorialDegrees {
            ra: range_degrees(radians_to_degrees(mean_ra + delta_ra)),
            dec: radians_to_degrees(mean_dec + delta_dec),
        }
    } else {
        let mut px = mean_dec.cos() * mean_ra.cos();
        let mut py = mean_dec.cos() * mean_ra.sin();
        let x = x / SPEED_OF_LIGHT_AU_PER_DAY;
        let y = y / SPEED_OF_LIGHT_AU_PER_DAY;
        let z = z / SPEED_OF_LIGHT_AU_PER_DAY;

        px += x;
        py += y;

        let ra = py.atan2(px);
        let dec = clamp_unit((px * px + py * py).sqrt()).acos() + mean_dec.cos() * z;

        EquatorialDegrees {
            ra: range_degrees(radians_to_degrees(ra)),
            dec: radians_to_degrees(dec),
        }
    }
}

fn remove_aberration(position: EquatorialDegrees, jd: f64) -> EquatorialDegrees {
    let aberrated = apply_aberration(position, jd);
    let delta_ra = signed_angle_delta_degrees(aberrated.ra, position.ra);

    EquatorialDegrees {
        ra: range_degrees(position.ra - delta_ra),
        dec: position.dec - (aberrated.dec - position.dec),
    }
}

fn weighted_sine_cosine((sin1, sin2, cos1, cos2): Coefficients4, t: f64, argument: f64) -> f64 {
    (sin1 + sin2 * t) * argument.sin() + (cos1 + cos2 * t) * argument.cos()
}

fn degrees_to_radians(degrees: f64) -> f64 {
    degrees.to_radians()
}

fn radians_to_degrees(radians: f64) -> f64 {
    radians.to_degrees()
}

fn range_degrees(degrees: f64) -> f64 {
    degrees.rem_euclid(360.0)
}

fn signed_angle_delta_degrees(lhs: f64, rhs: f64) -> f64 {
    ((lhs - rhs + 540.0).rem_euclid(360.0)) - 180.0
}

fn clamp_unit(value: f64) -> f64 {
    value.clamp(-1.0, 1.0)
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
