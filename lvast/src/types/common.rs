use crate::types::consts::HOURS_TO_DEGREES;

#[derive(Clone, Copy, Debug)]
pub struct EquatorialDegrees {
    pub ra: f64,
    pub dec: f64,
}

impl EquatorialDegrees {
    pub fn from_hours_degrees(ra_hours: f64, dec_deg: f64) -> Self {
        Self {
            ra: (ra_hours * HOURS_TO_DEGREES).rem_euclid(360.0),
            dec: dec_deg,
        }
    }

    pub fn to_hours_degrees(self) -> (f64, f64) {
        ((self.ra / HOURS_TO_DEGREES).rem_euclid(24.0), self.dec)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Nutation {
    pub longitude: f64,
    pub obliquity: f64,
    pub ecliptic: f64,
}
