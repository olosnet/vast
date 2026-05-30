use std::{
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use crate::{
    base::errors::{VastError, VastErrorType, VastResult},
    imageformats::fits::FitsImageSaver,
    types::{
        common::EquatorialDegrees,
        imageformats::{ImageFrame, ImageFrameFormat, ImageSaver},
    },
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
#[derive(Clone, Copy, Debug)]
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

/// Backend selection for runtime plate-solver dispatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VastPlatesolverBackend {
    /// ASTAP command-line backend.
    Astap,
    /// Astrometry.net `solve-field` backend.
    AstrometryNet,
}

/// High-level solve request passed by library users.
///
/// This keeps software-facing parameters backend-agnostic. Concrete solvers can
/// translate these hints into ASTAP, Astrometry.net, or other implementation-
/// specific flags internally.
#[derive(Clone, Debug)]
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

    /// Validates generic solve request before dispatching to a backend.
    pub fn validate(&self) -> VastResult<()> {
        match &self.source {
            VastPlatesolverSource::ImageFrame(frame) => {
                if frame.width == 0 || frame.height == 0 {
                    return Err(invalid_input("plate solver image frame must have non-zero dimensions"));
                }

                let expected_len = (frame.width as usize)
                    .checked_mul(frame.height as usize)
                    .and_then(|pixels| pixels.checked_mul(frame.format.bytes_per_pixel()))
                    .ok_or_else(|| invalid_input("plate solver image dimensions overflow"))?;
                if frame.data.len() != expected_len {
                    return Err(invalid_input(format!(
                        "invalid plate solver image data length: got {}, expected {}",
                        frame.data.len(),
                        expected_len
                    )));
                }
            }
            VastPlatesolverSource::FilePath(path) => {
                if path.as_os_str().is_empty() {
                    return Err(invalid_input("plate solver file path must not be empty"));
                }
            }
        }

        if let Some(position_hint) = self.position_hint {
            if !position_hint.center.ra.is_finite() {
                return Err(invalid_input("plate solver position hint RA must be finite"));
            }
            if !position_hint.center.dec.is_finite() || !(-90.0..=90.0).contains(&position_hint.center.dec) {
                return Err(invalid_input("plate solver position hint Dec must be within -90..=90 degrees"));
            }
            if !position_hint.radius_deg.is_finite() || position_hint.radius_deg <= 0.0 || position_hint.radius_deg > 180.0 {
                return Err(invalid_input("plate solver search radius must be within 0..=180 degrees"));
            }
        }

        if let Some(scale_hint) = self.scale_hint {
            if !scale_hint.min_arcsec_per_pixel.is_finite()
                || !scale_hint.max_arcsec_per_pixel.is_finite()
                || scale_hint.min_arcsec_per_pixel <= 0.0
                || scale_hint.max_arcsec_per_pixel <= 0.0
                || scale_hint.min_arcsec_per_pixel > scale_hint.max_arcsec_per_pixel
            {
                return Err(invalid_input(
                    "plate solver scale hint must be finite, positive, and ordered low..=high",
                ));
            }
        }

        if let Some(downsample_factor) = self.downsample_factor {
            if downsample_factor == 0 {
                return Err(invalid_input("plate solver downsample factor must be greater than zero"));
            }
        }

        if let Some(timeout_seconds) = self.timeout_seconds {
            if timeout_seconds == 0 {
                return Err(invalid_input("plate solver timeout must be greater than zero"));
            }
        }

        Ok(())
    }
}

/// Shared source-preparation helpers for external plate-solver backends.
pub trait VastPlatesolverRequestSourceExt {
    /// Materializes request source into path consumable by solver backend.
    fn prepare_input_source(&self, working_dir: &Path) -> VastResult<PathBuf>;
}

impl VastPlatesolverRequestSourceExt for VastPlatesolverRequest {
    fn prepare_input_source(&self, working_dir: &Path) -> VastResult<PathBuf> {
        self.validate()?;

        match &self.source {
            VastPlatesolverSource::ImageFrame(frame) => {
                let path = working_dir.join("input.fits");
                FitsImageSaver::new(frame.width, frame.height, frame.format).save(
                    frame.data.clone(),
                    None,
                    path.to_string_lossy().into_owned(),
                )?;
                Ok(path)
            }
            VastPlatesolverSource::FilePath(path) => {
                if !path.is_file() {
                    return Err(VastError::new(
                        VastErrorType::FileError,
                        format!("plate solver input file does not exist: {}", path.display()),
                    ));
                }
                Ok(path.clone())
            }
        }
    }
}

/// Backend-neutral plate-solve result.
#[derive(Clone, Debug)]
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
    /// Reference pixel X coordinate.
    pub reference_pixel_x: Option<f64>,
    /// Reference pixel Y coordinate.
    pub reference_pixel_y: Option<f64>,
    /// Full 2x2 CD matrix when backend exposes it.
    pub cd_matrix: Option<[[f64; 2]; 2]>,
    /// Backend-specific WCS keyword snapshot.
    pub wcs_headers: Vec<(String, String)>,
}

/// Backend-neutral solve diagnostics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VastPlatesolverDiagnostics {
    /// Solver backend name.
    pub implementation_name: &'static str,
    /// Executed command line split into program and arguments.
    pub command: Vec<String>,
    /// Captured stdout from solver process.
    pub stdout: String,
    /// Captured stderr from solver process.
    pub stderr: String,
    /// Non-fatal backend warnings.
    pub warnings: Vec<String>,
    /// Backend working directory when temporary files were kept.
    pub working_directory: Option<PathBuf>,
}

/// Full solve result including normalized solution and backend diagnostics.
#[derive(Clone, Debug)]
pub struct VastPlatesolverSolveResult {
    /// Normalized astrometric solution.
    pub solution: VastPlatesolverSolution,
    /// Backend execution diagnostics.
    pub diagnostics: VastPlatesolverDiagnostics,
}

/// Generic high-level plate-solver interface.
///
/// Implementations map [`VastPlatesolverRequest`] to their own backend-specific
/// flags and return normalized [`VastPlatesolverSolution`] values.
pub trait VastPlatesolver: Send + Sync {
    /// Returns backend implementation name.
    fn implementation_name(&self) -> &'static str;

    /// Solves astrometric solution and returns backend diagnostics.
    fn solve_with_diagnostics(
        &self,
        request: &VastPlatesolverRequest,
    ) -> VastResult<VastPlatesolverSolveResult>;

    /// Solves astrometric solution for requested image source.
    fn solve(&self, request: &VastPlatesolverRequest) -> VastResult<VastPlatesolverSolution> {
        Ok(self.solve_with_diagnostics(request)?.solution)
    }
}

/// Shared process and filesystem helpers for command-line solver backends.
pub trait VastPlatesolverBackendExt {
    /// Creates unique temporary working directory for solver backend.
    fn create_working_dir(prefix: &str) -> VastResult<PathBuf> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| Self::file_error(format!("system clock error while preparing temp dir: {err}")))?
            .as_nanos();
        let path = std::env::temp_dir().join(format!("lvast-{prefix}-{}-{nonce}", std::process::id()));
        std::fs::create_dir_all(&path)
            .map_err(|err| Self::file_error(format!("failed to create temp dir {}: {err}", path.display())))?;
        Ok(path)
    }

    /// Runs solver process with optional wall-clock timeout.
    fn run_command_with_timeout(
        executable_path: &Path,
        args: &[String],
        timeout_seconds: Option<u64>,
        process_name: &str,
    ) -> VastResult<Output> {
        if let Some(timeout_seconds) = timeout_seconds {
            let mut child = Command::new(executable_path)
                .args(args)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|err| {
                    Self::file_error(format!(
                        "failed to launch {process_name} {}: {err}",
                        executable_path.display()
                    ))
                })?;

            let deadline = Instant::now() + Duration::from_secs(timeout_seconds);
            loop {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(Self::runtime_error(format!(
                        "{process_name} solve timed out after {timeout_seconds} seconds"
                    )));
                }

                if child
                    .try_wait()
                    .map_err(|err| Self::file_error(format!("failed while waiting for {process_name}: {err}")))?
                    .is_some()
                {
                    return child.wait_with_output().map_err(|err| {
                        Self::file_error(format!("failed to collect {process_name} output: {err}"))
                    });
                }

                thread::sleep(Duration::from_millis(100));
            }
        }

        Command::new(executable_path)
            .args(args)
            .output()
            .map_err(|err| Self::file_error(format!("failed to run {process_name} {}: {err}", executable_path.display())))
    }

    /// Formats optional process output suffix for user-facing errors.
    fn format_optional_output(label: &str, bytes: &[u8]) -> String {
        let text = String::from_utf8_lossy(bytes).trim().to_string();
        if text.is_empty() {
            String::new()
        } else {
            format!("; {label}: {text}")
        }
    }

    /// Creates file-domain error for backend helpers.
    fn file_error(message: impl Into<String>) -> VastError {
        VastError::new(VastErrorType::FileError, message.into())
    }

    /// Creates generic runtime/configuration error for backend helpers.
    fn runtime_error(message: impl Into<String>) -> VastError {
        VastError::new(VastErrorType::InvalidInput, message.into())
    }
}

/// Creates concrete backend from runtime selection.
pub fn create_platesolver(backend: VastPlatesolverBackend) -> Box<dyn VastPlatesolver> {
    match backend {
        VastPlatesolverBackend::Astap => Box::new(crate::platesolvers::astap::AstapPlatesolver::new()),
        VastPlatesolverBackend::AstrometryNet => {
            Box::new(crate::platesolvers::astrometry_net::AstrometryNetPlatesolver::new())
        }
    }
}

fn invalid_input(message: impl Into<String>) -> VastError {
    VastError::new(VastErrorType::InvalidInput, message.into())
}
