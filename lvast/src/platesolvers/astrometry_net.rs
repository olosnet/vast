use std::{path::{Path, PathBuf}};

use crate::{
    base::errors::VastResult,
    types::{
        common::EquatorialDegrees,
        platsolver::{
            VastPlatesolver, VastPlatesolverBackendExt, VastPlatesolverDiagnostics,
            VastPlatesolverParity, VastPlatesolverRequest, VastPlatesolverRequestSourceExt,
            VastPlatesolverSolveResult, VastPlatesolverSolution, VastPlatesolverSource,
        },
    },
};

/// Astrometry.net `solve-field` backend.
#[derive(Clone, Debug)]
pub struct AstrometryNetPlatesolver {
    /// `solve-field` executable path.
    pub executable_path: PathBuf,
    /// Keeps temporary files when enabled for debugging.
    pub keep_temporary_files: bool,
}

impl Default for AstrometryNetPlatesolver {
    fn default() -> Self {
        Self {
            executable_path: PathBuf::from("solve-field"),
            keep_temporary_files: false,
        }
    }
}

impl AstrometryNetPlatesolver {
    /// Creates solver using default executable lookup.
    pub fn new() -> Self {
        Self::default()
    }

    /// Overrides `solve-field` executable path.
    pub fn with_executable_path(mut self, executable_path: impl Into<PathBuf>) -> Self {
        self.executable_path = executable_path.into();
        self
    }

    /// Keeps temporary solve files on disk for inspection.
    pub fn with_keep_temporary_files(mut self, keep_temporary_files: bool) -> Self {
        self.keep_temporary_files = keep_temporary_files;
        self
    }
}

impl VastPlatesolverBackendExt for AstrometryNetPlatesolver {}

impl VastPlatesolver for AstrometryNetPlatesolver {
    fn implementation_name(&self) -> &'static str {
        "astrometry.net"
    }

    fn solve_with_diagnostics(
        &self,
        request: &VastPlatesolverRequest,
    ) -> VastResult<VastPlatesolverSolveResult> {
        request.validate()?;

        let working_dir = Self::create_working_dir("astrometry-net")?;
        let result = self.solve_in_dir(request, &working_dir);
        let working_directory = self.keep_temporary_files.then_some(working_dir.clone());

        if !self.keep_temporary_files {
            let _ = std::fs::remove_dir_all(&working_dir);
        }

        result.map(|mut result| {
            result.diagnostics.working_directory = working_directory;
            result
        })
    }
}

impl AstrometryNetPlatesolver {
    fn solve_in_dir(
        &self,
        request: &VastPlatesolverRequest,
        working_dir: &Path,
    ) -> VastResult<VastPlatesolverSolveResult> {
        let input_path = request.prepare_input_source(working_dir)?;
        let output_base = "solution";
        let args = build_solve_field_args(request, &input_path, working_dir, output_base);
        let output = Self::run_command_with_timeout(
            &self.executable_path,
            &args,
            request.timeout_seconds,
            "Astrometry.net",
        )?;

        let solved_path = working_dir.join(format!("{output_base}.solved"));
        let wcs_path = working_dir.join(format!("{output_base}.wcs"));
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let diagnostics = VastPlatesolverDiagnostics {
            implementation_name: "astrometry.net",
            command: std::iter::once("solve-field".to_string())
                .chain(args.iter().cloned())
                .collect(),
            warnings: collect_astrometry_warnings(&stdout, &stderr),
            stdout: stdout.clone(),
            stderr: stderr.clone(),
            working_directory: None,
        };

        if !is_solved_marker_present(&solved_path)? || !wcs_path.is_file() {
            return Err(runtime_error(format!(
                "Astrometry.net did not produce a valid solution{}{}",
                Self::format_optional_output("stdout", &output.stdout),
                Self::format_optional_output("stderr", &output.stderr),
            )));
        }

        Ok(VastPlatesolverSolveResult {
            solution: parse_wcs_solution(&wcs_path, request)?,
            diagnostics,
        })
    }
}

pub(super) fn build_solve_field_args(
    request: &VastPlatesolverRequest,
    input_path: &Path,
    working_dir: &Path,
    output_base: &str,
) -> Vec<String> {
    let mut args = vec![
        "--overwrite".to_string(),
        "--no-plots".to_string(),
        "--dir".to_string(),
        working_dir.to_string_lossy().into_owned(),
        "--out".to_string(),
        output_base.to_string(),
        input_path.to_string_lossy().into_owned(),
    ];

    if let Some(position_hint) = request.position_hint {
        args.push("--ra".to_string());
        args.push(format!("{:.8}", position_hint.center.ra / 15.0));
        args.push("--dec".to_string());
        args.push(format!("{:.8}", position_hint.center.dec));
        args.push("--radius".to_string());
        args.push(format!("{:.8}", position_hint.radius_deg));
    }

    if let Some(scale_hint) = request.scale_hint {
        args.push("--scale-units".to_string());
        args.push("arcsecperpix".to_string());
        args.push("--scale-low".to_string());
        args.push(format!("{:.8}", scale_hint.min_arcsec_per_pixel));
        args.push("--scale-high".to_string());
        args.push(format!("{:.8}", scale_hint.max_arcsec_per_pixel));
    } else if !request.blind_solve {
        args.push("--guess-scale".to_string());
    }

    if let Some(parity_hint) = request.parity_hint {
        match parity_hint {
            VastPlatesolverParity::Positive => {
                args.push("--parity".to_string());
                args.push("pos".to_string());
            }
            VastPlatesolverParity::Negative => {
                args.push("--parity".to_string());
                args.push("neg".to_string());
            }
            VastPlatesolverParity::Any => {}
        }
    }

    if let Some(downsample_factor) = request.downsample_factor {
        args.push("--downsample".to_string());
        args.push(downsample_factor.to_string());
    }

    if let Some(timeout_seconds) = request.timeout_seconds {
        args.push("--cpulimit".to_string());
        args.push(timeout_seconds.to_string());
    }

    args
}

fn is_solved_marker_present(path: &Path) -> VastResult<bool> {
    if !path.is_file() {
        return Ok(false);
    }

    let bytes = std::fs::read(path)
        .map_err(|err| file_error(format!("failed to read Astrometry.net solved marker {}: {err}", path.display())))?;
    Ok(bytes.iter().any(|byte| *byte == 1))
}

fn parse_wcs_solution(
    path: &Path,
    request: &VastPlatesolverRequest,
) -> VastResult<VastPlatesolverSolution> {
    let headers = read_fits_headers(path)?;

    let ra_deg = parse_header_f64(&headers, "CRVAL1")?;
    let dec_deg = parse_header_f64(&headers, "CRVAL2")?;
    let cd1_1 = parse_header_f64(&headers, "CD1_1")?;
    let cd1_2 = parse_header_f64(&headers, "CD1_2")?;
    let cd2_1 = parse_header_f64(&headers, "CD2_1")?;
    let cd2_2 = parse_header_f64(&headers, "CD2_2")?;
    let scale_x_deg = (cd1_1.powi(2) + cd2_1.powi(2)).sqrt();
    let scale_y_deg = (cd1_2.powi(2) + cd2_2.powi(2)).sqrt();
    let (width_pixels, height_pixels) = match &request.source {
        VastPlatesolverSource::ImageFrame(source) => (source.width as f64, source.height as f64),
        VastPlatesolverSource::FilePath(_) => (
            parse_header_f64(&headers, "NAXIS1")?,
            parse_header_f64(&headers, "NAXIS2")?,
        ),
    };

    Ok(VastPlatesolverSolution {
        center: EquatorialDegrees {
            ra: ra_deg,
            dec: dec_deg,
        },
        pixel_scale_arcsec_per_pixel: ((scale_x_deg + scale_y_deg) * 0.5) * 3600.0,
        rotation_deg: cd2_1.atan2(cd1_1).to_degrees(),
        field_width_deg: width_pixels * scale_x_deg.abs(),
        field_height_deg: height_pixels * scale_y_deg.abs(),
        parity: if cd1_1 * cd2_2 - cd1_2 * cd2_1 >= 0.0 {
            VastPlatesolverParity::Positive
        } else {
            VastPlatesolverParity::Negative
        },
        reference_pixel_x: headers
            .iter()
            .find(|(key, _)| key == "CRPIX1")
            .and_then(|(_, value)| value.parse::<f64>().ok()),
        reference_pixel_y: headers
            .iter()
            .find(|(key, _)| key == "CRPIX2")
            .and_then(|(_, value)| value.parse::<f64>().ok()),
        cd_matrix: Some([[cd1_1, cd1_2], [cd2_1, cd2_2]]),
        wcs_headers: headers,
    })
}

fn read_fits_headers(path: &Path) -> VastResult<Vec<(String, String)>> {
    let bytes = std::fs::read(path)
        .map_err(|err| <AstrometryNetPlatesolver as VastPlatesolverBackendExt>::file_error(format!("failed to read Astrometry.net WCS file {}: {err}", path.display())))?;
    let mut headers = Vec::new();

    for chunk in bytes.chunks(80) {
        if chunk.len() < 80 {
            break;
        }
        let card = String::from_utf8_lossy(chunk).to_string();
        let key = card[..8].trim().to_string();
        if key == "END" {
            break;
        }
        if let Some((_, value)) = card.split_once('=') {
            let value = value.split('/').next().unwrap_or("").trim().trim_matches(' ').trim_matches('"').to_string();
            headers.push((key, value));
        }
    }

    Ok(headers)
}

fn parse_header_f64(headers: &[(String, String)], key: &str) -> VastResult<f64> {
    headers
        .iter()
        .find(|(candidate, _)| candidate == key)
        .ok_or_else(|| runtime_error(format!("Astrometry.net WCS missing {key}")))?
        .1
        .parse::<f64>()
        .map_err(|err| runtime_error(format!("Astrometry.net WCS field {key} is not a number: {err}")))
}

fn file_error(message: impl Into<String>) -> crate::base::errors::VastError {
    <AstrometryNetPlatesolver as VastPlatesolverBackendExt>::file_error(message)
}

fn runtime_error(message: impl Into<String>) -> crate::base::errors::VastError {
    <AstrometryNetPlatesolver as VastPlatesolverBackendExt>::runtime_error(message)
}

fn collect_astrometry_warnings(stdout: &str, stderr: &str) -> Vec<String> {
    stdout
        .lines()
        .chain(stderr.lines())
        .filter(|line| line.to_ascii_lowercase().contains("warning"))
        .map(|line| line.trim().to_string())
        .collect()
}
