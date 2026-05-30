use std::path::PathBuf;

use crate::{
    base::errors::VastResult,
    types::{common::EquatorialDegrees, imageformats::ImageFrame},
};

/// Generic image source accepted by a plate solver.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VastPlatesolverSource {
    /// Solve directly from in-memory image frame.
    ImageFrame(ImageFrame),
    /// Solve from image file already stored on disk.
    FilePath(PathBuf),
}

/// Approximate sky position hint used to reduce search space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VastPlatesolverPositionHint {
    /// Approximate image center in equatorial degrees.
    pub center: EquatorialDegrees,
    /// Search radius around hinted center in degrees.
    pub radius_deg: f64,
}

/// Approximate image scale hint used to constrain candidate solutions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VastPlatesolverScaleHint {
    /// Minimum expected image scale in arcseconds per pixel.
    pub min_arcsec_per_pixel: f64,
    /// Maximum expected image scale in arcseconds per pixel.
    pub max_arcsec_per_pixel: f64,
}

/// Orientation parity hint shared by common plate-solver backends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VastPlatesolverParity {
    /// Do not constrain parity.
    Any,
    /// Use native image parity.
    Positive,
    /// Use mirrored image parity.
    Negative,
}

/// High-level solve request passed by library users.
///
/// This keeps software-facing parameters backend-agnostic. Concrete solvers can
/// translate these hints into ASTAP, Astrometry.net, or other implementation-
/// specific flags internally.
#[derive(Clone, Debug, PartialEq)]
pub struct VastPlatesolverRequest {
    /// Image source to solve.
    pub source: VastPlatesolverSource,
    /// Optional approximate pointing information.
    pub position_hint: Option<VastPlatesolverPositionHint>,
    /// Optional approximate image scale range.
    pub scale_hint: Option<VastPlatesolverScaleHint>,
    /// Optional parity constraint.
    pub parity_hint: Option<VastPlatesolverParity>,
    /// Optional downsample factor for faster solving.
    pub downsample_factor: Option<u8>,
    /// Optional total solver timeout.
    pub timeout_seconds: Option<u64>,
    /// When `true`, solver should not require hints to attempt solve.
    pub blind_solve: bool,
}

impl VastPlatesolverRequest {
    /// Creates blind-solve request from in-memory image frame.
    pub fn from_image_frame(frame: ImageFrame) -> Self {
        Self {
            source: VastPlatesolverSource::ImageFrame(frame),
            position_hint: None,
            scale_hint: None,
            parity_hint: None,
            downsample_factor: None,
            timeout_seconds: None,
            blind_solve: true,
        }
    }

    /// Creates blind-solve request from image file path.
    pub fn from_file_path(path: PathBuf) -> Self {
        Self {
            source: VastPlatesolverSource::FilePath(path),
            position_hint: None,
            scale_hint: None,
            parity_hint: None,
            downsample_factor: None,
            timeout_seconds: None,
            blind_solve: true,
        }
    }
}

/// Backend-neutral plate-solve result.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VastPlatesolverSolution {
    /// Solved image center in equatorial degrees.
    pub center: EquatorialDegrees,
    /// Solved image scale in arcseconds per pixel.
    pub pixel_scale_arcsec_per_pixel: f64,
    /// Solved image rotation angle in degrees.
    pub rotation_deg: f64,
    /// Solved horizontal field of view in degrees.
    pub field_width_deg: f64,
    /// Solved vertical field of view in degrees.
    pub field_height_deg: f64,
    /// Solved parity.
    pub parity: VastPlatesolverParity,
}

/// Generic high-level plate-solver interface.
///
/// Implementations map [`VastPlatesolverRequest`] to their own backend-specific
/// flags and return normalized [`VastPlatesolverSolution`] values.
pub trait VastPlatesolver: Send + Sync {
    /// Solves astrometric solution for requested image source.
    fn solve(&self, request: &VastPlatesolverRequest) -> VastResult<VastPlatesolverSolution>;
}
