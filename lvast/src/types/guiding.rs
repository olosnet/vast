use crate::{
    base::errors::{VastError, VastErrorType, VastResult},
    types::{camera::VastCameraGuideDirection, common::EquatorialDegrees},
};

/// High-level internal guider runtime state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VastGuiderState {
    /// Guider idle and not processing frames.
    Idle,
    /// Searching for usable guide star candidates.
    Searching,
    /// Running calibration moves.
    Calibrating,
    /// Actively guiding on locked star.
    Guiding,
    /// Temporarily paused by user or workflow.
    Paused,
    /// Locked star lost and reacquisition in progress.
    LostStar,
    /// Stopping pending frame loop or pulse drain.
    Stopping,
    /// Fatal guider error state.
    Error,
}

/// Preferred correction output target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VastGuideOutputKind {
    /// ST4 pulse guiding via guide camera output port.
    CameraSt4,
    /// Pulse guiding via mount backend.
    MountPulse,
}

/// Calibration reuse policy for future sessions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VastGuideCalibrationReuse {
    /// Always require fresh calibration.
    Never,
    /// Reuse calibration during current process lifetime.
    Session,
    /// Reuse persisted calibration when context still matches.
    Persistent,
}

/// Reason guide lock failed or was dropped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VastGuideLostStarReason {
    /// No candidate satisfied acquisition filters.
    NoCandidates,
    /// Previous star signal dropped below validity threshold.
    LowSignal,
    /// Star saturated or shape became invalid.
    InvalidShape,
    /// Search window reached image edge.
    OutOfBounds,
    /// Frame rejected due to clouds/noise/bad statistics.
    BadFrame,
    /// User or workflow explicitly cancelled guiding.
    Cancelled,
}

/// One guide pulse command in backend-neutral form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VastGuidePulse {
    /// Pulse direction.
    pub direction: VastCameraGuideDirection,
    /// Pulse duration in milliseconds.
    pub duration_millis: u32,
}

impl VastGuidePulse {
    /// Validates pulse duration.
    pub fn validate(&self) -> VastResult<()> {
        if self.duration_millis == 0 {
            return Err(invalid_input("guide pulse duration must be greater than zero"));
        }

        Ok(())
    }
}

/// Backend-neutral guide output used by internal guider runtime.
pub trait VastGuideOutput: Send + Sync {
    /// Returns concrete output backend kind.
    fn kind(&self) -> VastGuideOutputKind;

    /// Dispatches one guide pulse.
    fn pulse(&mut self, pulse: VastGuidePulse) -> VastResult<()>;

    /// Stops all active or queued guide pulses.
    fn stop_all(&mut self) -> VastResult<()>;
}

/// Static guider configuration shared across one guide session.
#[derive(Clone, Debug, PartialEq)]
pub struct VastGuideSessionConfig {
    /// Guide exposure time in milliseconds.
    pub exposure_millis: u32,
    /// Optional guide camera gain.
    pub gain: Option<u32>,
    /// Optional square binning factor.
    pub binning: Option<u32>,
    /// Optional guide ROI as `(x, y, width, height)`.
    pub roi: Option<(u32, u32, u32, u32)>,
    /// Minimum measured drift before correction is emitted.
    pub min_move_pixels: f64,
    /// RA aggressiveness factor in range `(0, 1]` normally.
    pub ra_aggressiveness: f64,
    /// DEC aggressiveness factor in range `(0, 1]` normally.
    pub dec_aggressiveness: f64,
    /// Maximum single correction pulse in milliseconds.
    pub max_pulse_millis: u32,
    /// Preferred guider output backend.
    pub output: VastGuideOutputKind,
    /// Calibration reuse policy.
    pub calibration_reuse: VastGuideCalibrationReuse,
    /// Allowed consecutive rejected/lost frames before lock drop.
    pub max_lost_frames: u32,
}

impl Default for VastGuideSessionConfig {
    fn default() -> Self {
        Self {
            exposure_millis: 1000,
            gain: None,
            binning: None,
            roi: None,
            min_move_pixels: 0.15,
            ra_aggressiveness: 0.7,
            dec_aggressiveness: 0.7,
            max_pulse_millis: 2000,
            output: VastGuideOutputKind::CameraSt4,
            calibration_reuse: VastGuideCalibrationReuse::Session,
            max_lost_frames: 5,
        }
    }
}

impl VastGuideSessionConfig {
    /// Validates generic guide session configuration before runtime start.
    pub fn validate(&self) -> VastResult<()> {
        if self.exposure_millis == 0 {
            return Err(invalid_input("guide exposure must be greater than zero"));
        }
        if let Some(bin) = self.binning {
            if bin == 0 {
                return Err(invalid_input("guide binning must be greater than zero"));
            }
        }
        if let Some((_, _, width, height)) = self.roi {
            if width == 0 || height == 0 {
                return Err(invalid_input("guide ROI dimensions must be greater than zero"));
            }
        }
        if !self.min_move_pixels.is_finite() || self.min_move_pixels < 0.0 {
            return Err(invalid_input("guide min_move_pixels must be finite and non-negative"));
        }
        if !self.ra_aggressiveness.is_finite() || self.ra_aggressiveness <= 0.0 {
            return Err(invalid_input("guide RA aggressiveness must be finite and greater than zero"));
        }
        if !self.dec_aggressiveness.is_finite() || self.dec_aggressiveness <= 0.0 {
            return Err(invalid_input("guide DEC aggressiveness must be finite and greater than zero"));
        }
        if self.max_pulse_millis == 0 {
            return Err(invalid_input("guide max pulse must be greater than zero"));
        }
        if self.max_lost_frames == 0 {
            return Err(invalid_input("guide max_lost_frames must be greater than zero"));
        }

        Ok(())
    }
}

/// Derived calibration mapping from guide-camera pixels to mount correction axes.
#[derive(Clone, Debug, PartialEq)]
pub struct VastGuideCalibration {
    /// Angle of camera +X axis relative to RA+ axis in degrees.
    pub camera_angle_deg: f64,
    /// Milliseconds required to move one pixel along RA axis.
    pub ra_millis_per_pixel: f64,
    /// Milliseconds required to move one pixel along DEC axis.
    pub dec_millis_per_pixel: f64,
    /// Whether RA calibration direction is reversed.
    pub ra_inverted: bool,
    /// Whether DEC calibration direction is reversed.
    pub dec_inverted: bool,
    /// Optional guide-camera image scale in arcseconds per pixel.
    pub image_scale_arcsec_per_pixel: Option<f64>,
    /// Optional mount coordinates used when calibration was created.
    pub calibration_position: Option<EquatorialDegrees>,
}

impl VastGuideCalibration {
    /// Validates calibration numbers are usable by future controller logic.
    pub fn validate(&self) -> VastResult<()> {
        if !self.camera_angle_deg.is_finite() {
            return Err(invalid_input("guide calibration camera angle must be finite"));
        }
        if !self.ra_millis_per_pixel.is_finite() || self.ra_millis_per_pixel <= 0.0 {
            return Err(invalid_input("guide calibration RA ms/pixel must be finite and greater than zero"));
        }
        if !self.dec_millis_per_pixel.is_finite() || self.dec_millis_per_pixel <= 0.0 {
            return Err(invalid_input("guide calibration DEC ms/pixel must be finite and greater than zero"));
        }
        if let Some(image_scale) = self.image_scale_arcsec_per_pixel {
            if !image_scale.is_finite() || image_scale <= 0.0 {
                return Err(invalid_input("guide calibration image scale must be finite and greater than zero"));
            }
        }

        Ok(())
    }
}

/// One measured guide-star drift sample for control and graphing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VastGuideDriftSample {
    /// Monotonic frame sequence number within guide session.
    pub frame_index: u64,
    /// Horizontal drift in guide-camera pixels.
    pub dx_pixels: f64,
    /// Vertical drift in guide-camera pixels.
    pub dy_pixels: f64,
    /// Drift projected onto RA axis in guide-camera pixels.
    pub ra_error_pixels: f64,
    /// Drift projected onto DEC axis in guide-camera pixels.
    pub dec_error_pixels: f64,
    /// Optional RA error in arcseconds.
    pub ra_error_arcsec: Option<f64>,
    /// Optional DEC error in arcseconds.
    pub dec_error_arcsec: Option<f64>,
    /// Capture timestamp in Unix milliseconds.
    pub timestamp_unix_millis: u64,
}

/// Controller output derived from one drift sample.
#[derive(Clone, Debug, PartialEq)]
pub struct VastGuideCorrection {
    /// Original drift sample this correction responds to.
    pub sample: VastGuideDriftSample,
    /// Optional RA correction pulse.
    pub ra_pulse: Option<VastGuidePulse>,
    /// Optional DEC correction pulse.
    pub dec_pulse: Option<VastGuidePulse>,
    /// Free-form explanation for skipped or clipped corrections.
    pub note: Option<String>,
}

/// Star lock state shared between frame loop, UI, and recovery logic.
#[derive(Clone, Debug, PartialEq)]
pub struct VastGuideStarLock {
    /// Locked star centroid X coordinate in image pixel-center space.
    pub x: f64,
    /// Locked star centroid Y coordinate in image pixel-center space.
    pub y: f64,
    /// Locked star signal-to-noise ratio when available.
    pub snr: Option<f64>,
    /// Locked star HFR when available.
    pub hfr: Option<f64>,
    /// Consecutive frames star was not valid.
    pub lost_frames: u32,
    /// Last known lost-star reason.
    pub lost_reason: Option<VastGuideLostStarReason>,
}

impl VastGuideStarLock {
    /// Returns `true` when star is considered currently locked.
    pub fn is_locked(&self) -> bool {
        self.lost_reason.is_none()
    }
}

/// Rolling guider telemetry suitable for graphing and session summaries.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VastGuideMetrics {
    /// RMS RA error in pixels over current window.
    pub ra_rms_pixels: f64,
    /// RMS DEC error in pixels over current window.
    pub dec_rms_pixels: f64,
    /// RMS total error in pixels over current window.
    pub total_rms_pixels: f64,
    /// Successfully guided frame count.
    pub accepted_frames: u64,
    /// Rejected or dropped frame count.
    pub rejected_frames: u64,
}

impl VastGuideMetrics {
    /// Returns combined RMS magnitude from per-axis RMS components.
    pub fn recompute_total_rms_pixels(&self) -> f64 {
        self.ra_rms_pixels.hypot(self.dec_rms_pixels)
    }
}

/// Result of processing one guide frame.
#[derive(Clone, Debug, PartialEq)]
pub struct VastGuideFrameResult {
    /// Guider state after frame processing.
    pub state: VastGuiderState,
    /// Current or last-known star lock state.
    pub star_lock: Option<VastGuideStarLock>,
    /// Measured frame drift when valid.
    pub drift: Option<VastGuideDriftSample>,
    /// Controller output selected for this frame.
    pub correction: Option<VastGuideCorrection>,
    /// Rolling guide metrics after frame processing.
    pub metrics: Option<VastGuideMetrics>,
}

fn invalid_input(message: impl Into<String>) -> VastError {
    VastError::new(VastErrorType::InvalidInput, message.into())
}
