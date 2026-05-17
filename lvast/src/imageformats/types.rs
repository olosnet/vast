use crate::base::errors::VastError;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ImageHeaders {
    pub software: Option<String>,
    pub image_type: Option<String>,
    pub object: Option<String>,
    pub instrument: Option<String>,
    pub telescope: Option<String>,
    pub observer: Option<String>,
    pub date_obs: Option<String>,
    pub exposure_seconds: Option<f64>,
    pub filter: Option<String>,
    pub gain: Option<u32>,
    pub offset: Option<u32>,
    pub ccd_temperature: Option<f64>,
    pub target_temperature: Option<f64>,
    pub bin_x: Option<u32>,
    pub bin_y: Option<u32>,
    pub frame_x: Option<u32>,
    pub frame_y: Option<u32>,
    pub frame_width: Option<u32>,
    pub frame_height: Option<u32>,
    pub bayer_pattern: Option<String>,
    pub ra_degrees: Option<f64>,
    pub dec_degrees: Option<f64>,
    pub pixel_scale_arcsec: Option<f64>,
    pub focal_length_mm: Option<f64>,
    pub aperture_mm: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FitsHeaderValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
}

impl From<&str> for FitsHeaderValue {
    fn from(value: &str) -> Self {
        Self::String(value.to_string())
    }
}

impl From<String> for FitsHeaderValue {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<u32> for FitsHeaderValue {
    fn from(value: u32) -> Self {
        Self::Integer(i64::from(value))
    }
}

impl From<i64> for FitsHeaderValue {
    fn from(value: i64) -> Self {
        Self::Integer(value)
    }
}

impl From<f64> for FitsHeaderValue {
    fn from(value: f64) -> Self {
        Self::Float(value)
    }
}

impl From<bool> for FitsHeaderValue {
    fn from(value: bool) -> Self {
        Self::Boolean(value)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct HeaderCard {
    pub key: &'static str,
    pub value: FitsHeaderValue,
    pub comment: &'static str,
}

impl ImageHeaders {
    pub fn to_fits_headers(&self) -> Vec<HeaderCard> {
        let mut headers = Vec::new();

        headers.push(HeaderCard {
            key: "SIMPLE",
            value: true.into(),
            comment: "file conforms to FITS standard",
        });
        headers.push(HeaderCard {
            key: "EXTEND",
            value: true.into(),
            comment: "FITS dataset may contain extensions",
        });

        push_optional(
            &mut headers,
            "CREATOR",
            self.software.clone(),
            "Capture software",
        );
        push_optional(
            &mut headers,
            "IMAGETYP",
            self.image_type.clone(),
            "Image type",
        );
        push_optional(&mut headers, "OBJECT", self.object.clone(), "Target name");
        push_optional(
            &mut headers,
            "INSTRUME",
            self.instrument.clone(),
            "Imaging instrument",
        );
        push_optional(
            &mut headers,
            "TELESCOP",
            self.telescope.clone(),
            "Telescope",
        );
        push_optional(&mut headers, "OBSERVER", self.observer.clone(), "Observer");
        push_optional(
            &mut headers,
            "DATE-OBS",
            self.date_obs.clone(),
            "UTC observation start",
        );
        push_optional(&mut headers, "FILTER", self.filter.clone(), "Filter name");
        push_optional(
            &mut headers,
            "EXPTIME",
            self.exposure_seconds,
            "Exposure time (s)",
        );
        push_optional(
            &mut headers,
            "EXPOSURE",
            self.exposure_seconds,
            "Exposure time (s)",
        );
        push_optional(&mut headers, "GAIN", self.gain, "Camera gain");
        push_optional(&mut headers, "OFFSET", self.offset, "Camera offset");
        push_optional(
            &mut headers,
            "CCD-TEMP",
            self.ccd_temperature,
            "CCD temperature (C)",
        );
        push_optional(
            &mut headers,
            "SET-TEMP",
            self.target_temperature,
            "Target temperature (C)",
        );
        push_optional(&mut headers, "XBINNING", self.bin_x, "X binning");
        push_optional(&mut headers, "YBINNING", self.bin_y, "Y binning");
        push_optional(&mut headers, "XORGSUBF", self.frame_x, "Subframe X origin");
        push_optional(&mut headers, "YORGSUBF", self.frame_y, "Subframe Y origin");
        push_optional(
            &mut headers,
            "XPIXSZ",
            self.frame_width,
            "Subframe width (px)",
        );
        push_optional(
            &mut headers,
            "YPIXSZ",
            self.frame_height,
            "Subframe height (px)",
        );
        push_optional(
            &mut headers,
            "BAYERPAT",
            self.bayer_pattern.clone(),
            "Bayer pattern",
        );
        push_optional(
            &mut headers,
            "RA",
            self.ra_degrees,
            "Right ascension (degrees)",
        );
        push_optional(
            &mut headers,
            "DEC",
            self.dec_degrees,
            "Declination (degrees)",
        );
        push_optional(
            &mut headers,
            "SCALE",
            self.pixel_scale_arcsec,
            "Image scale (arcsec/pixel)",
        );
        push_optional(
            &mut headers,
            "FOCALLEN",
            self.focal_length_mm,
            "Focal length (mm)",
        );
        push_optional(
            &mut headers,
            "APTDIA",
            self.aperture_mm,
            "Aperture diameter (mm)",
        );

        headers
    }
}

fn push_optional<T: Into<FitsHeaderValue>>(
    headers: &mut Vec<HeaderCard>,
    key: &'static str,
    value: Option<T>,
    comment: &'static str,
) {
    if let Some(value) = value {
        headers.push(HeaderCard {
            key,
            value: value.into(),
            comment,
        });
    }
}

pub trait ImageSaver {
    fn save(
        &self,
        data: Vec<u8>,
        image_headers: Option<Vec<HeaderCard>>,
        path: String,
    ) -> Result<(), VastError>;
}
