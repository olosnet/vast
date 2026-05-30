use std::{
    cell::RefCell,
    collections::HashMap,
    sync::OnceLock,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use crate::{
    base::errors::{VastError, VastErrorType, VastResult},
    types::{
        camera::{
            CameraFrameFormat, VastCamera, VastCameraAcquireImage, VastCameraCapBinning,
            VastCameraCapExposure, VastCameraCapGain, VastCameraCapGuiding, VastCameraCapOffset,
            VastCameraCapRoi, VastCameraCapRoiCombination, VastCameraCapabilities,
            VastCameraDriver, VastCameraFrame, VastCameraGuide, VastCameraGuideDirection,
            VastCameraID, VastCameraInfo, VastCameraSettings,
        },
        common::EquatorialDegrees,
    },
};

const DEFAULT_CAMERA_ID: &str = "fake-camera-0";
const DEFAULT_CAMERA_NAME: &str = "Fake Native Camera";
const DEFAULT_SENSOR_OFFSET: u32 = 512;
const DEFAULT_TEMPERATURE_C: f32 = 20.0;
const DEFAULT_EXPOSURE_US: u64 = 1_000_000;
const GUIDE_RATE_ARCSEC_PER_SECOND: f64 = 7.5;
const EFFECTIVE_RENDER_MAGNITUDE: f64 = 18.0;
const SKY_BACKGROUND_ADU: f64 = 180.0;
const DARK_CURRENT_ADU: f64 = 14.0;
const FLAT_FIELD_LEVEL_ADU: f64 = 24_000.0;
const OPTICAL_BLUR_SIGMA_PIXELS: f64 = 0.95;
const SATURATION_LEVEL_ADU: f64 = 55_000.0;
const MIN_VISIBLE_COMPONENT_ADU: f64 = 0.35;

thread_local! {
    static GAUSSIAN_AXIS_CACHE: RefCell<HashMap<(u16, u8, i32, i32), Arc<[f64]>>> = RefCell::new(HashMap::new());
}

fn invalid_input(message: impl Into<String>) -> VastError {
    VastError::new(VastErrorType::InvalidInput, message.into())
}

fn camera_error(message: impl Into<String>) -> VastError {
    VastError::new(VastErrorType::CameraError, message.into())
}

fn normalize_ra_deg(ra: f64) -> f64 {
    ra.rem_euclid(360.0)
}

fn signed_ra_delta_deg(lhs: f64, rhs: f64) -> f64 {
    ((lhs - rhs + 540.0).rem_euclid(360.0)) - 180.0
}

fn frame_bytes_len(width: u32, height: u32, format: CameraFrameFormat) -> usize {
    let bytes_per_pixel = match format {
        CameraFrameFormat::RAW8 => 1,
        CameraFrameFormat::RAW10
        | CameraFrameFormat::RAW12
        | CameraFrameFormat::RAW14
        | CameraFrameFormat::RAW16 => 2,
        CameraFrameFormat::RGB24 => 3,
        CameraFrameFormat::RGB32 => 4,
    };
    width as usize * height as usize * bytes_per_pixel
}

fn uniform01(seed: u64) -> f64 {
    let mut state = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
    state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
    ((state >> 11) as f64) / ((1_u64 << 53) as f64)
}

fn gaussian_noise(index: u64) -> f64 {
    let u1 = uniform01(index).clamp(f64::MIN_POSITIVE, 1.0 - f64::EPSILON);
    let u2 = uniform01(index.rotate_left(32) ^ 0x9E37_79B9_7F4A_7C15);
    (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
}

fn poisson_sample(lambda: f64, seed: u64) -> f64 {
    if lambda <= 0.0 {
        return 0.0;
    }

    if lambda < 12.0 {
        let limit = (-lambda).exp();
        let mut product = 1.0;
        let mut count = 0.0;
        let mut state = seed;
        loop {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let sample = ((state >> 11) as f64) / ((1_u64 << 53) as f64);
            product *= sample.max(f64::MIN_POSITIVE);
            if product <= limit {
                return count;
            }
            count += 1.0;
        }
    }

    (lambda + gaussian_noise(seed) * lambda.sqrt()).max(0.0)
}

fn component_render_radius(sigma_pixels: f64, amplitude: f64, max_radius: i32) -> Option<i32> {
    if !sigma_pixels.is_finite() || sigma_pixels <= 0.0 || amplitude <= MIN_VISIBLE_COMPONENT_ADU {
        return None;
    }

    let radius = (sigma_pixels
        * (-2.0 * (MIN_VISIBLE_COMPONENT_ADU / amplitude).ln()).sqrt())
    .ceil()
    .max(1.0) as i32;

    Some(radius.min(max_radius))
}

fn gaussian_axis_weights(sigma_pixels: f64, center: f64, start: i32, end: i32) -> Arc<[f64]> {
    let sigma_key = (sigma_pixels * 128.0).round().clamp(1.0, u16::MAX as f64) as u16;
    let frac_key = (center.fract().rem_euclid(1.0) * 256.0).round().clamp(0.0, 255.0) as u8;
    let start_offset = start - center.floor() as i32;
    let span = (end - start + 1).clamp(1, i32::from(u16::MAX));

    GAUSSIAN_AXIS_CACHE.with(|cache| {
        let key = (sigma_key, frac_key, start_offset, span);
        if let Some(weights) = cache.borrow().get(&key) {
            return Arc::clone(weights);
        }

        let two_sigma2 = 2.0 * sigma_pixels * sigma_pixels;
        let weights = (start..=end)
            .map(|p| {
                let delta = p as f64 + 0.5 - center;
                (-(delta * delta) / two_sigma2).exp()
            })
            .collect::<Arc<[f64]>>();
        cache.borrow_mut().insert(key, Arc::clone(&weights));
        weights
    })
}

fn normalized_pixel_variation(index: u64, salt: u64) -> f64 {
    uniform01(index ^ salt) * 2.0 - 1.0
}

struct DeferredBloom {
    x: f64,
    y: f64,
    sigma_pixels: f64,
    amplitude: f64,
    radius: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FakeCameraSensorPreset {
    Asi294Mc,
}

impl FakeCameraSensorPreset {
    pub fn resolution(self) -> (u32, u32) {
        match self {
            Self::Asi294Mc => (4144, 2822),
        }
    }

    pub fn pixel_size_um(self) -> f64 {
        match self {
            Self::Asi294Mc => 4.63,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Asi294Mc => "ASI294MC 4144x2822 4.63um",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FakeCameraFocalPreset {
    Mm135,
    Mm250,
    Mm400,
    Mm600,
    Mm800,
    Mm1000,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FakeCameraDefectProfile {
    Clean,
    Light,
    Moderate,
    Heavy,
}

impl FakeCameraDefectProfile {
    pub fn label(self) -> &'static str {
        match self {
            Self::Clean => "Clean sensor",
            Self::Light => "Light hot/cold pixels",
            Self::Moderate => "Moderate hot/cold pixels",
            Self::Heavy => "Heavy hot/cold pixels",
        }
    }

    fn params(self) -> Option<SensorDefectParams> {
        match self {
            Self::Clean => None,
            Self::Light => Some(SensorDefectParams {
                hot_rate: 0.000_015,
                cold_rate: 0.000_010,
                hot_min_adu: 1_500.0,
                hot_max_adu: 18_000.0,
                cold_min_scale: 0.05,
                cold_max_scale: 0.55,
            }),
            Self::Moderate => Some(SensorDefectParams {
                hot_rate: 0.000_035,
                cold_rate: 0.000_020,
                hot_min_adu: 3_500.0,
                hot_max_adu: 28_000.0,
                cold_min_scale: 0.02,
                cold_max_scale: 0.40,
            }),
            Self::Heavy => Some(SensorDefectParams {
                hot_rate: 0.000_080,
                cold_rate: 0.000_040,
                hot_min_adu: 6_000.0,
                hot_max_adu: 42_000.0,
                cold_min_scale: 0.0,
                cold_max_scale: 0.25,
            }),
        }
    }
}

#[derive(Clone, Copy)]
struct SensorDefectParams {
    hot_rate: f64,
    cold_rate: f64,
    hot_min_adu: f64,
    hot_max_adu: f64,
    cold_min_scale: f64,
    cold_max_scale: f64,
}

impl FakeCameraFocalPreset {
    pub fn focal_length_mm(self) -> f64 {
        match self {
            Self::Mm135 => 135.0,
            Self::Mm250 => 250.0,
            Self::Mm400 => 400.0,
            Self::Mm600 => 600.0,
            Self::Mm800 => 800.0,
            Self::Mm1000 => 1000.0,
        }
    }

    pub fn approximate_fov_degrees(self, sensor: FakeCameraSensorPreset) -> (f64, f64) {
        let (width, height) = sensor.resolution();
        let pixel_size_mm = sensor.pixel_size_um() / 1_000.0;
        let sensor_width_mm = width as f64 * pixel_size_mm;
        let sensor_height_mm = height as f64 * pixel_size_mm;
        let focal_length_mm = self.focal_length_mm();

        let width_deg = 2.0 * (sensor_width_mm / (2.0 * focal_length_mm)).atan().to_degrees();
        let height_deg = 2.0 * (sensor_height_mm / (2.0 * focal_length_mm)).atan().to_degrees();
        (width_deg, height_deg)
    }

    pub fn label(self, sensor: FakeCameraSensorPreset) -> String {
        let (width_deg, height_deg) = self.approximate_fov_degrees(sensor);
        format!(
            "{} mm ({:.2} deg x {:.2} deg on {})",
            self.focal_length_mm(),
            width_deg,
            height_deg,
            sensor.label()
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FakeCameraSkyFieldPreset {
    M42Orion,
    M31Andromeda,
    M45Pleiades,
    VegaLyra,
    Polaris,
    SadrCygnus,
    NoStars,
    FlatField,
}

impl FakeCameraSkyFieldPreset {
    pub fn center(self) -> EquatorialDegrees {
        match self {
            Self::M42Orion => EquatorialDegrees { ra: 83.8221, dec: -5.3911 },
            Self::M31Andromeda => EquatorialDegrees { ra: 10.6847, dec: 41.2687 },
            Self::M45Pleiades => EquatorialDegrees { ra: 56.75, dec: 24.1167 },
            Self::VegaLyra => EquatorialDegrees { ra: 279.2347, dec: 38.7837 },
            Self::Polaris => EquatorialDegrees { ra: 37.9546, dec: 89.2641 },
            Self::SadrCygnus => EquatorialDegrees { ra: 305.5571, dec: 40.2567 },
            Self::NoStars => EquatorialDegrees { ra: 0.0, dec: 0.0 },
            Self::FlatField => EquatorialDegrees { ra: 0.0, dec: 0.0 },
        }
    }

    pub fn is_dark_mode(self) -> bool {
        matches!(self, Self::NoStars)
    }

    pub fn is_flat_mode(self) -> bool {
        matches!(self, Self::FlatField)
    }

    pub fn stars(self) -> &'static [RealSkyStar] {
        match self {
            Self::M42Orion => embedded_catalog("m42", include_str!("data/m42_orion_gaia.csv")),
            Self::M31Andromeda => embedded_catalog("m31", include_str!("data/m31_andromeda_gaia.csv")),
            Self::M45Pleiades => embedded_catalog("m45", include_str!("data/m45_pleiades_gaia.csv")),
            Self::VegaLyra => embedded_catalog("vega", include_str!("data/vega_lyra_gaia.csv")),
            Self::Polaris => embedded_catalog("polaris", include_str!("data/polaris_gaia.csv")),
            Self::SadrCygnus => embedded_catalog("sadr", include_str!("data/sadr_cygnus_gaia.csv")),
            Self::NoStars => &[],
            Self::FlatField => &[],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RealSkyStar {
    pub ra_deg: f64,
    pub dec_deg: f64,
    pub magnitude: f64,
}

fn parse_embedded_catalog(csv: &'static str) -> Vec<RealSkyStar> {
    csv.lines()
        .skip(1)
        .filter_map(|line| {
            let mut fields = line.split(',');
            Some(RealSkyStar {
                ra_deg: fields.next()?.trim().parse().ok()?,
                dec_deg: fields.next()?.trim().parse().ok()?,
                magnitude: fields.next()?.trim().parse().ok()?,
            })
        })
        .collect()
}

fn embedded_catalog(key: &'static str, csv: &'static str) -> &'static [RealSkyStar] {
    static M42: OnceLock<Vec<RealSkyStar>> = OnceLock::new();
    static M31: OnceLock<Vec<RealSkyStar>> = OnceLock::new();
    static M45: OnceLock<Vec<RealSkyStar>> = OnceLock::new();
    static VEGA: OnceLock<Vec<RealSkyStar>> = OnceLock::new();
    static POLARIS: OnceLock<Vec<RealSkyStar>> = OnceLock::new();
    static SADR: OnceLock<Vec<RealSkyStar>> = OnceLock::new();

    match key {
        "m42" => M42.get_or_init(|| parse_embedded_catalog(csv)).as_slice(),
        "m31" => M31.get_or_init(|| parse_embedded_catalog(csv)).as_slice(),
        "m45" => M45.get_or_init(|| parse_embedded_catalog(csv)).as_slice(),
        "vega" => VEGA.get_or_init(|| parse_embedded_catalog(csv)).as_slice(),
        "polaris" => POLARIS.get_or_init(|| parse_embedded_catalog(csv)).as_slice(),
        "sadr" => SADR.get_or_init(|| parse_embedded_catalog(csv)).as_slice(),
        _ => &[],
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FakeCameraSimulationConfig {
    pub sensor_preset: FakeCameraSensorPreset,
    pub focal_preset: FakeCameraFocalPreset,
    pub defect_profile: FakeCameraDefectProfile,
    pub sky_field: FakeCameraSkyFieldPreset,
    pub center: EquatorialDegrees,
    pub seeing_arcsec: f64,
    pub sensor_noise: f64,
}

impl Default for FakeCameraSimulationConfig {
    fn default() -> Self {
        let sky_field = FakeCameraSkyFieldPreset::M45Pleiades;
        Self {
            sensor_preset: FakeCameraSensorPreset::Asi294Mc,
            focal_preset: FakeCameraFocalPreset::Mm400,
            defect_profile: FakeCameraDefectProfile::Clean,
            sky_field,
            center: sky_field.center(),
            seeing_arcsec: 2.5,
            sensor_noise: 8.0,
        }
    }
}

impl FakeCameraSimulationConfig {
    pub fn focal_length_mm(&self) -> f64 {
        self.focal_preset.focal_length_mm()
    }

    pub fn resolution(&self) -> (u32, u32) {
        self.sensor_preset.resolution()
    }

    pub fn pixel_size_um(&self) -> f64 {
        self.sensor_preset.pixel_size_um()
    }

    pub fn approximate_fov_degrees(&self) -> (f64, f64) {
        self.focal_preset.approximate_fov_degrees(self.sensor_preset)
    }

    pub fn pixel_scale_arcsec_per_pixel(&self, bin: u32) -> f64 {
        206.265 * (self.pixel_size_um() * bin as f64) / self.focal_length_mm()
    }

    pub fn validate(&self) -> VastResult<()> {
        if !self.center.ra.is_finite() {
            return Err(invalid_input("fake camera output RA must be finite"));
        }
        if !self.center.dec.is_finite() || !(-90.0..=90.0).contains(&self.center.dec) {
            return Err(invalid_input("fake camera output Dec must be within -90..=90 degrees"));
        }
        if !self.seeing_arcsec.is_finite() || self.seeing_arcsec <= 0.0 {
            return Err(invalid_input("fake camera seeing must be finite and greater than zero"));
        }
        if !self.sensor_noise.is_finite() || self.sensor_noise < 0.0 {
            return Err(invalid_input("fake camera sensor_noise must be finite and non-negative"));
        }
        Ok(())
    }
}

pub struct FakeCameraDriver;

impl VastCameraDriver for FakeCameraDriver {
    fn new() -> Self {
        Self
    }

    fn init(&mut self) -> Result<Vec<VastCameraInfo>, VastError> {
        Ok(vec![VastCameraInfo {
            id: VastCameraID::StrID(DEFAULT_CAMERA_ID.to_string()),
            name: DEFAULT_CAMERA_NAME.to_string(),
            serial_number: "FAKE0001".to_string(),
            raw_extra_info: "real-field preset synthetic camera".to_string(),
        }])
    }

    fn id(&self) -> &str {
        "FAKE_NATIVE_CAMERA_DRIVER"
    }

    fn get_manufacturer(&self) -> &str {
        "OpenCode"
    }

    fn get_version(&self) -> &str {
        "0.2"
    }
}

pub struct FakeVastCamera {
    _driver: Arc<FakeCameraDriver>,
    connected: bool,
    camera_name: String,
    camera_capabilities: VastCameraCapabilities,
    camera_settings: VastCameraSettings,
    simulation: FakeCameraSimulationConfig,
    acquisition_started: Option<Instant>,
    frame_counter: u64,
}

impl FakeVastCamera {
    pub fn new(driver: Arc<FakeCameraDriver>) -> Self {
        let simulation = FakeCameraSimulationConfig::default();
        let (width, height) = simulation.resolution();
        Self {
            _driver: driver,
            connected: false,
            camera_name: DEFAULT_CAMERA_NAME.to_string(),
            camera_capabilities: build_capabilities(width, height),
            camera_settings: VastCameraSettings {
                exposure_microseconds: Some(DEFAULT_EXPOSURE_US),
                offset: Some(DEFAULT_SENSOR_OFFSET),
                ..VastCameraSettings::default()
            },
            simulation,
            acquisition_started: None,
            frame_counter: 0,
        }
    }

    pub fn simulation_config(&self) -> &FakeCameraSimulationConfig {
        &self.simulation
    }

    pub fn set_sensor_preset(&mut self, preset: FakeCameraSensorPreset) {
        self.simulation.sensor_preset = preset;
        let (width, height) = preset.resolution();
        self.camera_capabilities = build_capabilities(width, height);
        if let Some((x, y, roi_width, roi_height)) = self.camera_settings.roi {
            if x.saturating_add(roi_width) > width || y.saturating_add(roi_height) > height {
                self.camera_settings.roi = None;
            }
        }
    }

    pub fn set_focal_preset(&mut self, preset: FakeCameraFocalPreset) {
        self.simulation.focal_preset = preset;
    }

    pub fn set_sky_field_preset(&mut self, preset: FakeCameraSkyFieldPreset) {
        self.simulation.sky_field = preset;
        self.simulation.center = preset.center();
    }

    pub fn set_defect_profile(&mut self, profile: FakeCameraDefectProfile) {
        self.simulation.defect_profile = profile;
    }

    pub fn set_output_ra_dec(&mut self, center: EquatorialDegrees) -> VastResult<()> {
        if !center.ra.is_finite() {
            return Err(invalid_input("fake camera output RA must be finite"));
        }
        if !center.dec.is_finite() || !(-90.0..=90.0).contains(&center.dec) {
            return Err(invalid_input("fake camera output Dec must be within -90..=90 degrees"));
        }
        self.simulation.center = EquatorialDegrees {
            ra: normalize_ra_deg(center.ra),
            dec: center.dec,
        };
        Ok(())
    }

    pub fn set_seeing_arcsec(&mut self, seeing_arcsec: f64) -> VastResult<()> {
        if !seeing_arcsec.is_finite() || seeing_arcsec <= 0.0 {
            return Err(invalid_input("fake camera seeing must be finite and greater than zero"));
        }
        self.simulation.seeing_arcsec = seeing_arcsec;
        Ok(())
    }

    pub fn set_sensor_noise(&mut self, sensor_noise: f64) -> VastResult<()> {
        if !sensor_noise.is_finite() || sensor_noise < 0.0 {
            return Err(invalid_input("fake camera sensor_noise must be finite and non-negative"));
        }
        self.simulation.sensor_noise = sensor_noise;
        Ok(())
    }

    fn ensure_connected(&self) -> VastResult<()> {
        if self.connected {
            Ok(())
        } else {
            Err(camera_error("Fake camera is not connected"))
        }
    }

    fn effective_dimensions(&self) -> (u32, u32, u32) {
        let bin = self
            .camera_settings
            .binning
            .map(|(horizontal, _)| horizontal.max(1))
            .unwrap_or(1);
        if let Some((_, _, width, height)) = self.camera_settings.roi {
            (width, height, bin)
        } else {
            let (sensor_width, sensor_height) = self.simulation.resolution();
            (sensor_width / bin, sensor_height / bin, bin)
        }
    }

    fn effective_pixel_scale_arcsec(&self, bin: u32) -> f64 {
        self.simulation.pixel_scale_arcsec_per_pixel(bin)
    }

    fn render_frame(&mut self) -> VastResult<VastCameraFrame> {
        self.simulation.validate()?;
        let (width, height, bin) = self.effective_dimensions();
        if width == 0 || height == 0 {
            return Err(camera_error("Fake camera effective frame dimensions are zero"));
        }

        let pixel_scale_arcsec = self.effective_pixel_scale_arcsec(bin);
        let pixel_scale_deg = pixel_scale_arcsec / 3600.0;
        let width_f = width as f64;
        let height_f = height as f64;
        let center = self.simulation.center;
        let cos_dec = center.dec.to_radians().cos().abs().max(0.01);
        let sigma_pixels = ((self.simulation.seeing_arcsec / pixel_scale_arcsec / 2.355).powi(2)
            + OPTICAL_BLUR_SIGMA_PIXELS.powi(2))
        .sqrt()
        .max(1.1);
        let exposure_scale = self
            .camera_settings
            .exposure_microseconds
            .unwrap_or(DEFAULT_EXPOSURE_US) as f64
            / DEFAULT_EXPOSURE_US as f64;
        let sky_background = if self.simulation.sky_field.is_dark_mode() || self.simulation.sky_field.is_flat_mode() {
            0.0
        } else {
            SKY_BACKGROUND_ADU * exposure_scale
        };
        let background_level = f64::from(self.get_current_offset()) + sky_background + DARK_CURRENT_ADU * exposure_scale;
        let mut pixels = vec![background_level; width as usize * height as usize];

        if self.simulation.sky_field.is_flat_mode() {
            self.render_flat_field(&mut pixels, width, height, exposure_scale);
        }

        let mut deferred_blooms = Vec::new();

        for star in self.simulation.sky_field.stars() {
            if star.magnitude > EFFECTIVE_RENDER_MAGNITUDE {
                continue;
            }

            let delta_ra_deg = signed_ra_delta_deg(star.ra_deg, center.ra) * cos_dec;
            let delta_dec_deg = star.dec_deg - center.dec;
            let x = width_f * 0.5 + delta_ra_deg / pixel_scale_deg;
            let y = height_f * 0.5 - delta_dec_deg / pixel_scale_deg;

            let normalized = ((EFFECTIVE_RENDER_MAGNITUDE - star.magnitude) / EFFECTIVE_RENDER_MAGNITUDE)
                .clamp(0.0, 1.0);
            let exposure_bloat = 1.0 + 0.28 * exposure_scale.ln_1p();
            let star_sigma = sigma_pixels * exposure_bloat * (1.0 + 0.25 * normalized);
            let render_radius = (star_sigma * 4.5).ceil().max(4.0) as i32;

            if x < -f64::from(render_radius)
                || x >= width_f + f64::from(render_radius)
                || y < -f64::from(render_radius)
                || y >= height_f + f64::from(render_radius)
            {
                continue;
            }

            let brightness = 10_f64.powf(-0.4 * (star.magnitude - 1.5));
            let core_amplitude = (900.0 + brightness * 92_000.0) * exposure_scale;
            let halo_amplitude = core_amplitude * (0.16 + 0.26 * exposure_scale.min(10.0) / 10.0);
            let bloom_amplitude = if core_amplitude > SATURATION_LEVEL_ADU * 0.55 {
                (core_amplitude - SATURATION_LEVEL_ADU * 0.55) * 0.18
            } else {
                0.0
            };

            if let Some(core_radius) = component_render_radius(star_sigma, core_amplitude, render_radius) {
                self.render_star(&mut pixels, width, height, x, y, star_sigma, core_amplitude, core_radius);
            }

            let halo_sigma = star_sigma * 2.2;
            if normalized >= 0.55
                && let Some(halo_radius) = component_render_radius(halo_sigma, halo_amplitude, (render_radius * 2).max(6))
            {
                self.render_star(&mut pixels, width, height, x, y, halo_sigma, halo_amplitude, halo_radius);
            }

            if bloom_amplitude > 0.0 {
                let bloom_sigma = star_sigma * 4.5;
                if let Some(bloom_radius) = component_render_radius(bloom_sigma, bloom_amplitude, (render_radius * 4).max(12)) {
                    deferred_blooms.push(DeferredBloom {
                        x,
                        y,
                        sigma_pixels: bloom_sigma,
                        amplitude: bloom_amplitude,
                        radius: bloom_radius,
                    });
                }
            }
        }

        for bloom in deferred_blooms {
            self.render_star(
                &mut pixels,
                width,
                height,
                bloom.x,
                bloom.y,
                bloom.sigma_pixels,
                bloom.amplitude,
                bloom.radius,
            );
        }

        self.frame_counter = self.frame_counter.wrapping_add(1);
        let noise_seed = self.frame_counter.wrapping_mul(0xA076_1D64_78BD_642F);
        let defect_params = self.simulation.defect_profile.params();
        for (index, pixel) in pixels.iter_mut().enumerate() {
            let sample_seed = noise_seed.wrapping_add(index as u64);
            let mut lambda = pixel.max(0.0);
            if let Some(params) = defect_params {
                let selector = uniform01(index as u64 ^ 0xD6E8_FD9A_4D94_A4F1);
                if selector < params.hot_rate {
                    let hot_strength = params.hot_min_adu
                        + uniform01(index as u64 ^ 0xA076_1D64_78BD_642F)
                            * (params.hot_max_adu - params.hot_min_adu);
                    lambda += hot_strength * exposure_scale.max(0.25);
                } else if selector < params.hot_rate + params.cold_rate {
                    let cold_scale = params.cold_min_scale
                        + uniform01(index as u64 ^ 0xE703_7ED1_A0B4_28DB)
                            * (params.cold_max_scale - params.cold_min_scale);
                    lambda *= cold_scale;
                }
            }
            let shot = if lambda > 64.0 {
                lambda + gaussian_noise(sample_seed) * lambda.sqrt()
            } else {
                poisson_sample(lambda, sample_seed)
            };
            let read = gaussian_noise(sample_seed.rotate_left(17)) * self.simulation.sensor_noise;
            *pixel = (shot + read).clamp(0.0, u16::MAX as f64);
        }

        let mut data = Vec::with_capacity(frame_bytes_len(width, height, CameraFrameFormat::RAW16));
        for pixel in pixels {
            data.extend_from_slice(&(pixel.round() as u16).to_le_bytes());
        }

        Ok(VastCameraFrame {
            width,
            height,
            format: CameraFrameFormat::RAW16,
            data,
        })
    }

    fn render_flat_field(&self, pixels: &mut [f64], width: u32, height: u32, exposure_scale: f64) {
        let width_f = width as f64;
        let height_f = height as f64;
        let cx = (width_f - 1.0) * 0.5;
        let cy = (height_f - 1.0) * 0.5;
        let inv_rx = 1.0 / (width_f * 0.52).max(1.0);
        let inv_ry = 1.0 / (height_f * 0.48).max(1.0);
        let flat_level = FLAT_FIELD_LEVEL_ADU * exposure_scale;
        let dust_motes = [
            (0.22, 0.28, 0.055, 0.10),
            (0.61, 0.44, 0.045, 0.07),
            (0.78, 0.67, 0.060, 0.12),
            (0.36, 0.73, 0.038, 0.08),
        ];

        for y in 0..height as usize {
            let yf = y as f64;
            let dy = (yf - cy) * inv_ry;
            for x in 0..width as usize {
                let xf = x as f64;
                let dx = (xf - cx) * inv_rx;
                let mut illumination = 1.0 + 0.025 * dx - 0.018 * dy + 0.012 * dx * dy;
                let idx = y * width as usize + x;
                illumination += 0.008 * normalized_pixel_variation(idx as u64, 0x94D0_49BB_1331_11EB);

                let nx = xf / width_f.max(1.0);
                let ny = yf / height_f.max(1.0);
                for (mx, my, sigma, depth) in dust_motes {
                    let ddx = nx - mx;
                    let ddy = ny - my;
                    let rr = (ddx * ddx + ddy * ddy) / (2.0 * sigma * sigma);
                    illumination -= depth * (-rr).exp();
                }

                pixels[idx] += (flat_level * illumination.max(0.65)).max(0.0);
            }
        }
    }

    fn render_star(
        &self,
        pixels: &mut [f64],
        width: u32,
        height: u32,
        x: f64,
        y: f64,
        sigma_pixels: f64,
        amplitude: f64,
        radius: i32,
    ) {
        if amplitude <= 0.0 || radius <= 0 {
            return;
        }

        let width_i32 = width as i32;
        let height_i32 = height as i32;
        let x0 = x.floor() as i32;
        let y0 = y.floor() as i32;
        let x_min = (x0 - radius).max(0);
        let x_max = (x0 + radius).min(width_i32 - 1);
        let y_min = (y0 - radius).max(0);
        let y_max = (y0 + radius).min(height_i32 - 1);
        let two_sigma2 = 2.0 * sigma_pixels * sigma_pixels;
        let x_weights = gaussian_axis_weights(sigma_pixels, x, x_min, x_max);

        for py in y_min..=y_max {
            let dy = py as f64 + 0.5 - y;
            let row_scale = amplitude * (-(dy * dy) / two_sigma2).exp();
            let row_offset = py as usize * width as usize + x_min as usize;
            let row = &mut pixels[row_offset..row_offset + x_weights.len()];
            for (pixel, weight) in row.iter_mut().zip(x_weights.iter()) {
                *pixel += row_scale * weight;
            }
        }
    }
}

impl Default for FakeVastCamera {
    fn default() -> Self {
        Self::new(Arc::new(FakeCameraDriver::new()))
    }
}

impl VastCamera<VastCameraID, FakeCameraDriver> for FakeVastCamera {
    fn new(driver: Arc<FakeCameraDriver>) -> Self {
        Self::new(driver)
    }

    fn connect(&mut self, camera_id: VastCameraID) -> Result<(), VastError> {
        match camera_id {
            VastCameraID::StrID(id) if id == DEFAULT_CAMERA_ID => {
                self.connected = true;
                Ok(())
            }
            VastCameraID::IntID(0) => {
                self.connected = true;
                Ok(())
            }
            _ => Err(camera_error("Unknown fake camera id")),
        }
    }

    fn get_name(&self) -> &str {
        &self.camera_name
    }

    fn get_capabilities(&self) -> VastCameraCapabilities {
        self.camera_capabilities.clone()
    }

    fn get_current_offset(&self) -> u32 {
        self.camera_settings.offset.unwrap_or(DEFAULT_SENSOR_OFFSET)
    }

    fn get_current_cooler(&self) -> (bool, u32) {
        (false, 0)
    }

    fn get_current_temperature(&self) -> f32 {
        DEFAULT_TEMPERATURE_C
    }

    fn set_camera_settings(&mut self, settings: VastCameraSettings) -> Result<(), VastError> {
        self.ensure_connected()?;

        if let Some(exposure_microseconds) = settings.exposure_microseconds {
            if exposure_microseconds < self.camera_capabilities.exposure.min_microseconds
                || exposure_microseconds > self.camera_capabilities.exposure.max_microseconds
            {
                return Err(invalid_input(format!(
                    "fake camera exposure must be within {}..={} us",
                    self.camera_capabilities.exposure.min_microseconds,
                    self.camera_capabilities.exposure.max_microseconds
                )));
            }
            self.camera_settings.exposure_microseconds = Some(exposure_microseconds);
        }

        if let Some(gain) = settings.gain {
            let capability = self.camera_capabilities.gain.as_ref().expect("fake camera gain capability missing");
            if gain < capability.min || gain > capability.max {
                return Err(invalid_input(format!(
                    "fake camera gain must be within {}..={}",
                    capability.min, capability.max
                )));
            }
            self.camera_settings.gain = Some(gain);
        }

        if let Some(offset) = settings.offset {
            let capability = self.camera_capabilities.offset.as_ref().expect("fake camera offset capability missing");
            if offset < capability.min || offset > capability.max {
                return Err(invalid_input(format!(
                    "fake camera offset must be within {}..={}",
                    capability.min, capability.max
                )));
            }
            self.camera_settings.offset = Some(offset);
        }

        if let Some((horizontal, vertical)) = settings.binning {
            if horizontal == 0 || horizontal != vertical {
                return Err(invalid_input("fake camera only supports square binning greater than zero"));
            }
            let supported = self
                .camera_capabilities
                .binning
                .as_ref()
                .map(|modes| modes.modes.contains(&horizontal))
                .unwrap_or(false);
            if !supported {
                return Err(invalid_input("fake camera binning mode is not supported"));
            }
            self.camera_settings.binning = Some((horizontal, vertical));
        }

        if let Some((x, y, width, height)) = settings.roi {
            let (sensor_width, sensor_height) = self.simulation.resolution();
            if width == 0 || height == 0 {
                return Err(invalid_input("fake camera ROI dimensions must be non-zero"));
            }
            if x.saturating_add(width) > sensor_width || y.saturating_add(height) > sensor_height {
                return Err(invalid_input("fake camera ROI exceeds sensor dimensions"));
            }
            self.camera_settings.roi = Some((x, y, width, height));
        }

        Ok(())
    }

    fn get_camera_settings(&mut self) -> Result<VastCameraSettings, VastError> {
        self.ensure_connected()?;
        Ok(self.camera_settings.clone())
    }

    fn get_settings(&self) -> VastCameraSettings {
        self.camera_settings.clone()
    }

    fn disconnect(&mut self) -> Result<(), VastError> {
        self.connected = false;
        self.acquisition_started = None;
        Ok(())
    }
}

impl VastCameraAcquireImage for FakeVastCamera {
    fn start_image_acquisition(&mut self) -> Result<(), VastError> {
        self.ensure_connected()?;
        self.acquisition_started = Some(Instant::now());
        Ok(())
    }

    fn abort_image_acquisition(&mut self) -> Result<(), VastError> {
        self.ensure_connected()?;
        self.acquisition_started = None;
        Ok(())
    }

    fn get_acquired_image(&mut self, timeout_millis: u32) -> Result<VastCameraFrame, VastError> {
        self.ensure_connected()?;
        let acquisition_started = self
            .acquisition_started
            .ok_or_else(|| camera_error("Fake camera acquisition was not started"))?;
        let exposure = Duration::from_micros(
            self.camera_settings
                .exposure_microseconds
                .unwrap_or(DEFAULT_EXPOSURE_US),
        );
        let remaining = exposure.saturating_sub(acquisition_started.elapsed());
        if remaining > Duration::from_millis(u64::from(timeout_millis)) {
            return Err(camera_error("Fake camera acquisition timed out"));
        }
        if !remaining.is_zero() {
            thread::sleep(remaining);
        }

        self.acquisition_started = None;
        self.render_frame()
    }
}

impl VastCameraGuide for FakeVastCamera {
    fn pulse_guide(
        &mut self,
        direction: VastCameraGuideDirection,
        duration_millis: u32,
    ) -> Result<(), VastError> {
        self.ensure_connected()?;
        if duration_millis == 0 {
            return Err(invalid_input("fake camera guide pulse duration must be greater than zero"));
        }

        let delta_arcsec = GUIDE_RATE_ARCSEC_PER_SECOND * duration_millis as f64 / 1_000.0;
        let delta_deg = delta_arcsec / 3600.0;
        match direction {
            VastCameraGuideDirection::North => {
                self.simulation.center.dec = (self.simulation.center.dec + delta_deg).clamp(-90.0, 90.0);
            }
            VastCameraGuideDirection::South => {
                self.simulation.center.dec = (self.simulation.center.dec - delta_deg).clamp(-90.0, 90.0);
            }
            VastCameraGuideDirection::East => {
                let cos_dec = self.simulation.center.dec.to_radians().cos().abs().max(0.01);
                self.simulation.center.ra = normalize_ra_deg(self.simulation.center.ra + delta_deg / cos_dec);
            }
            VastCameraGuideDirection::West => {
                let cos_dec = self.simulation.center.dec.to_radians().cos().abs().max(0.01);
                self.simulation.center.ra = normalize_ra_deg(self.simulation.center.ra - delta_deg / cos_dec);
            }
        }

        Ok(())
    }
}

fn build_capabilities(width: u32, height: u32) -> VastCameraCapabilities {
    VastCameraCapabilities {
        gain: Some(VastCameraCapGain {
            min: 0,
            max: 500,
            step: 1,
        }),
        offset: Some(VastCameraCapOffset {
            min: 0,
            max: u16::MAX as u32,
            step: 1,
        }),
        roi: Some(VastCameraCapRoi {
            combinations: vec![
                VastCameraCapRoiCombination {
                    bin: 1,
                    max_width: width,
                    max_height: height,
                    width_step: 1,
                    height_step: 1,
                },
                VastCameraCapRoiCombination {
                    bin: 2,
                    max_width: width / 2,
                    max_height: height / 2,
                    width_step: 1,
                    height_step: 1,
                },
                VastCameraCapRoiCombination {
                    bin: 4,
                    max_width: width / 4,
                    max_height: height / 4,
                    width_step: 1,
                    height_step: 1,
                },
            ],
        }),
        binning: Some(VastCameraCapBinning { modes: vec![1, 2, 4] }),
        guiding: Some(VastCameraCapGuiding { pulse_guide: true }),
        exposure: VastCameraCapExposure {
            min_microseconds: 1_000,
            max_microseconds: 60_000_000,
            step: 1,
        },
        frame_formats: vec![CameraFrameFormat::RAW16],
        max_height: height,
        max_width: width,
        adc_bits: 16,
        ..VastCameraCapabilities::default()
    }
}
