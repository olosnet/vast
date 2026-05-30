use std::f64::consts::PI;

use crate::types::consts::{
    ABERRATION_ARGUMENTS, Coefficients4, HOURS_TO_DEGREES, JD_J2000, NUTATION_ARGUMENTS,
    NUTATION_COEFFICIENTS, SPEED_OF_LIGHT_AU_PER_DAY, X_COEFFICIENTS, Y_COEFFICIENTS,
    Z_COEFFICIENTS,
};

#[derive(Clone, Copy, Debug)]
pub struct EquatorialDegrees {
    pub ra: f64,
    pub dec: f64,
}

impl EquatorialDegrees {
    pub fn from_ra_hours_dec_degrees(ra_hours: f64, dec_deg: f64) -> Self {
        Self {
            ra: Self::range_degrees(ra_hours * HOURS_TO_DEGREES),
            dec: dec_deg,
        }
    }

    pub fn to_ra_hours_dec_degrees(self) -> (f64, f64) {
        ((self.ra / HOURS_TO_DEGREES).rem_euclid(24.0), self.dec)
    }

    pub fn precessed_between_julian_days(self, from_jd: f64, to_jd: f64) -> Self {
        let mean_ra = self.ra.to_radians();
        let mean_dec = self.dec.to_radians();

        let t = (to_jd - from_jd) / 36525.0 / 3600.0;
        let t2 = t * t;
        let t3 = t2 * t;
        let epoch_offset = (from_jd - JD_J2000) / 36525.0 / 3600.0;
        let epoch_offset2 = epoch_offset * epoch_offset;

        let zeta = ((2306.2181 + 1.39656 * epoch_offset - 0.000139 * epoch_offset2) * t
            + (0.30188 - 0.000344 * epoch_offset) * t2
            + 0.017998 * t3)
            .to_radians();
        let eta = ((2306.2181 + 1.39656 * epoch_offset - 0.000139 * epoch_offset2) * t
            + (1.09468 + 0.000066 * epoch_offset) * t2
            + 0.018203 * t3)
            .to_radians();
        let theta = ((2004.3109 - 0.85330 * epoch_offset - 0.000217 * epoch_offset2) * t
            - (0.42665 + 0.000217 * epoch_offset) * t2
            - 0.041833 * t3)
            .to_radians();

        let a = mean_dec.cos() * (mean_ra + zeta).sin();
        let b =
            theta.cos() * mean_dec.cos() * (mean_ra + zeta).cos() - theta.sin() * mean_dec.sin();
        let c =
            theta.sin() * mean_dec.cos() * (mean_ra + zeta).cos() + theta.cos() * mean_dec.sin();

        let ra = a.atan2(b) + eta;
        let dec = if mean_dec.abs() > 0.4 * PI {
            let mut pole_dec = Self::clamp_unit((a * a + b * b).sqrt()).acos();
            if mean_dec < 0.0 {
                pole_dec = -pole_dec;
            }
            pole_dec
        } else {
            Self::clamp_unit(c).asin()
        };

        Self {
            ra: Self::range_degrees(ra.to_degrees()),
            dec: dec.to_degrees(),
        }
    }

    pub fn with_nutation(mut self, nutation: Nutation) -> Self {
        self.apply_nutation(nutation);
        self
    }

    pub fn without_nutation(mut self, nutation: Nutation) -> Self {
        self.remove_nutation(nutation);
        self
    }

    pub fn with_aberration(mut self, jd: f64) -> Self {
        self.apply_aberration(jd);
        self
    }

    pub fn without_aberration(mut self, jd: f64) -> Self {
        self.remove_aberration(jd);
        self
    }

    pub fn apply_nutation(&mut self, nutation: Nutation) {
        *self = self.transform_nutation(nutation, false);
    }

    pub fn remove_nutation(&mut self, nutation: Nutation) {
        *self = self.transform_nutation(nutation, true);
    }

    pub fn apply_aberration(&mut self, jd: f64) {
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

            x += Self::weighted_sine_cosine(X_COEFFICIENTS[index], t, argument);
            y += Self::weighted_sine_cosine(Y_COEFFICIENTS[index], t, argument);
            z += Self::weighted_sine_cosine(Z_COEFFICIENTS[index], t, argument);
        }

        let mean_ra = self.ra.to_radians();
        let mean_dec = self.dec.to_radians();

        *self = if mean_dec < PI * 0.4999 {
            let delta_ra = ((y * mean_ra.cos()) - (x * mean_ra.sin()))
                / mean_dec.cos()
                / SPEED_OF_LIGHT_AU_PER_DAY;
            let delta_dec = ((x * mean_ra.cos() + y * mean_ra.sin()) * mean_dec.sin()
                - z * mean_dec.cos())
                / -SPEED_OF_LIGHT_AU_PER_DAY;

            Self {
                ra: Self::range_degrees((mean_ra + delta_ra).to_degrees()),
                dec: (mean_dec + delta_dec).to_degrees(),
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
            let dec = Self::clamp_unit((px * px + py * py).sqrt()).acos() + mean_dec.cos() * z;

            Self {
                ra: Self::range_degrees(ra.to_degrees()),
                dec: dec.to_degrees(),
            }
        };
    }

    pub fn remove_aberration(&mut self, jd: f64) {
        let aberrated = self.with_aberration(jd);
        let delta_ra = Self::signed_angle_delta_degrees(aberrated.ra, self.ra);

        self.ra = Self::range_degrees(self.ra - delta_ra);
        self.dec -= aberrated.dec - self.dec;
    }

    fn transform_nutation(self, nutation: Nutation, reverse: bool) -> Self {
        let mean_ra = self.ra.to_radians();
        let mean_dec = self.dec.to_radians();

        let true_obliquity = (nutation.ecliptic + nutation.obliquity).to_radians();
        let sin_obliquity = true_obliquity.sin();
        let sin_ra = mean_ra.sin();
        let cos_ra = mean_ra.cos();
        let tan_dec = mean_dec.tan();

        let mut delta_ra = (true_obliquity.cos() + sin_obliquity * sin_ra * tan_dec)
            * nutation.longitude
            - cos_ra * tan_dec * nutation.obliquity;
        let mut delta_dec =
            (sin_obliquity * cos_ra) * nutation.longitude + sin_ra * nutation.obliquity;

        if reverse {
            delta_ra = -delta_ra;
            delta_dec = -delta_dec;
        }

        Self {
            ra: Self::range_degrees(self.ra + delta_ra),
            dec: self.dec + delta_dec,
        }
    }

    fn range_degrees(degrees: f64) -> f64 {
        degrees.rem_euclid(360.0)
    }

    fn clamp_unit(value: f64) -> f64 {
        value.clamp(-1.0, 1.0)
    }

    fn signed_angle_delta_degrees(lhs: f64, rhs: f64) -> f64 {
        ((lhs - rhs + 540.0).rem_euclid(360.0)) - 180.0
    }

    fn weighted_sine_cosine((sin1, sin2, cos1, cos2): Coefficients4, t: f64, argument: f64) -> f64 {
        (sin1 + sin2 * t) * argument.sin() + (cos1 + cos2 * t) * argument.cos()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Nutation {
    pub longitude: f64,
    pub obliquity: f64,
    pub ecliptic: f64,
}

impl Nutation {
    pub fn from_julian_day(jd: f64) -> Self {
        let t = (jd - JD_J2000) / 36525.0;
        let t2 = t * t;
        let t3 = t2 * t;

        let d = (297.85036 + 445267.111480 * t - 0.0019142 * t2 + t3 / 189474.0).to_radians();
        let m = (357.52772 + 35999.050340 * t - 0.0001603 * t2 - t3 / 300000.0).to_radians();
        let mm = (134.96298 + 477198.867398 * t + 0.0086972 * t2 + t3 / 56250.0).to_radians();
        let f = (93.2719100 + 483202.017538 * t - 0.0036825 * t2 + t3 / 327270.0).to_radians();
        let o = (125.04452 - 1934.136261 * t + 0.0020708 * t2 + t3 / 450000.0).to_radians();

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

        Self {
            longitude,
            obliquity,
            ecliptic,
        }
    }
}

pub enum TemperatureUnit {
    Celsius,
    Fahrenheit,
    Kelvin,
}
