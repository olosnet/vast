use std::{collections::HashMap, path::{Path, PathBuf}};

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

/// ASTAP-backed plate solver.
#[derive(Clone, Debug)]
pub struct AstapPlatesolver {
    /// ASTAP executable path.
    pub executable_path: PathBuf,
    /// Optional ASTAP star database directory.
    pub database_path: Option<PathBuf>,
    /// Keeps temporary files when enabled for debugging.
    pub keep_temporary_files: bool,
}

impl Default for AstapPlatesolver {
    fn default() -> Self {
        Self {
            executable_path: PathBuf::from("astap"),
            database_path: None,
            keep_temporary_files: false,
        }
    }
}

impl AstapPlatesolver {
    /// Creates ASTAP solver using default executable lookup.
    pub fn new() -> Self {
        Self::default()
    }

    /// Overrides ASTAP executable path.
    pub fn with_executable_path(mut self, executable_path: impl Into<PathBuf>) -> Self {
        self.executable_path = executable_path.into();
        self
    }

    /// Sets ASTAP star database directory.
    pub fn with_database_path(mut self, database_path: impl Into<PathBuf>) -> Self {
        self.database_path = Some(database_path.into());
        self
    }

    /// Keeps temporary solve files on disk for inspection.
    pub fn with_keep_temporary_files(mut self, keep_temporary_files: bool) -> Self {
        self.keep_temporary_files = keep_temporary_files;
        self
    }
}

impl VastPlatesolverBackendExt for AstapPlatesolver {}

impl VastPlatesolver for AstapPlatesolver {
    fn implementation_name(&self) -> &'static str {
        "astap"
    }

    fn solve_with_diagnostics(
        &self,
        request: &VastPlatesolverRequest,
    ) -> VastResult<VastPlatesolverSolveResult> {
        request.validate()?;

        let working_dir = Self::create_working_dir("astap")?;
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

impl AstapPlatesolver {
    fn solve_in_dir(
        &self,
        request: &VastPlatesolverRequest,
        working_dir: &Path,
    ) -> VastResult<VastPlatesolverSolveResult> {
        let input_path = request.prepare_input_source(working_dir)?;
        let output_base = working_dir.join("solution");
        let args = build_astap_args(request, &input_path, &output_base, self.database_path.as_deref());
        let output = Self::run_command_with_timeout(
            &self.executable_path,
            &args,
            request.timeout_seconds,
            "ASTAP",
        )?;

        let ini_path = output_base.with_extension("ini");
        let values = if ini_path.exists() {
            parse_astap_ini(&ini_path)?
        } else {
            HashMap::new()
        };
        let diagnostics = build_diagnostics(&args, &output, &values);

        match output.status.code() {
            Some(0) => Ok(VastPlatesolverSolveResult {
                solution: solution_from_ini(&values, request)?,
                diagnostics,
            }),
            Some(1) => Err(runtime_error(format!(
                "ASTAP returned no solution: {}{}{}",
                read_ini_message(&values, "ERROR").unwrap_or_else(|| "no additional error details".to_string()),
                Self::format_optional_output("stdout", &output.stdout),
                Self::format_optional_output("stderr", &output.stderr),
            ))),
            Some(2) => Err(runtime_error(format!(
                "ASTAP detected not enough stars: {}{}{}",
                read_ini_message(&values, "ERROR").unwrap_or_else(|| "no additional error details".to_string()),
                Self::format_optional_output("stdout", &output.stdout),
                Self::format_optional_output("stderr", &output.stderr),
            ))),
            Some(16) => Err(file_error(format!(
                "ASTAP could not read input image {}{}{}",
                input_path.display(),
                Self::format_optional_output("stdout", &output.stdout),
                Self::format_optional_output("stderr", &output.stderr),
            ))),
            Some(32) | Some(33) => Err(runtime_error(format!(
                "ASTAP could not access star database{}{}",
                Self::format_optional_output("stdout", &output.stdout),
                Self::format_optional_output("stderr", &output.stderr),
            ))),
            Some(34) => Err(runtime_error(format!(
                "ASTAP failed while writing solution output{}{}",
                Self::format_optional_output("stdout", &output.stdout),
                Self::format_optional_output("stderr", &output.stderr),
            ))),
            Some(code) => Err(runtime_error(format!(
                "ASTAP failed with exit code {code}{}{}",
                Self::format_optional_output("stdout", &output.stdout),
                Self::format_optional_output("stderr", &output.stderr),
            ))),
            None => Err(runtime_error(format!(
                "ASTAP terminated by signal{}{}",
                Self::format_optional_output("stdout", &output.stdout),
                Self::format_optional_output("stderr", &output.stderr),
            ))),
        }
    }
}

pub(super) fn build_astap_args(
    request: &VastPlatesolverRequest,
    input_path: &Path,
    output_base: &Path,
    database_path: Option<&Path>,
) -> Vec<String> {
    let mut args = vec![
        "-f".to_string(),
        input_path.to_string_lossy().into_owned(),
        "-o".to_string(),
        output_base.to_string_lossy().into_owned(),
    ];

    if let Some(position_hint) = request.position_hint {
        args.push("-ra".to_string());
        args.push(format!("{:.8}", position_hint.center.ra / 15.0));
        args.push("-spd".to_string());
        args.push(format!("{:.8}", position_hint.center.dec + 90.0));
        args.push("-r".to_string());
        args.push(format!("{:.8}", position_hint.radius_deg));
    } else if request.blind_solve {
        args.push("-r".to_string());
        args.push("180".to_string());
    }

    if let Some(field_height_deg) = estimate_field_height_deg(request) {
        args.push("-fov".to_string());
        args.push(format!("{field_height_deg:.8}"));
    } else if request.blind_solve {
        args.push("-fov".to_string());
        args.push("0".to_string());
    }

    if let Some(downsample_factor) = request.downsample_factor {
        args.push("-z".to_string());
        args.push(downsample_factor.to_string());
    }

    if let Some(database_path) = database_path {
        args.push("-d".to_string());
        args.push(database_path.to_string_lossy().into_owned());
    }

    args
}

fn estimate_field_height_deg(request: &VastPlatesolverRequest) -> Option<f64> {
    let scale_hint = request.scale_hint?;
    let height = match &request.source {
        VastPlatesolverSource::ImageFrame(frame) => frame.height as f64,
        VastPlatesolverSource::FilePath(_) => return None,
    };

    let average_scale = (scale_hint.min_arcsec_per_pixel + scale_hint.max_arcsec_per_pixel) * 0.5;
    Some(height * average_scale / 3600.0)
}

fn parse_astap_ini(path: &Path) -> VastResult<HashMap<String, String>> {
    let content = std::fs::read_to_string(path)
        .map_err(|err| <AstapPlatesolver as VastPlatesolverBackendExt>::file_error(format!("failed to read ASTAP ini {}: {err}", path.display())))?;
    let mut values = HashMap::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            values.insert(key.trim().to_string(), value.trim().to_string());
        }
    }

    Ok(values)
}

pub(super) fn solution_from_ini(
    values: &HashMap<String, String>,
    request: &VastPlatesolverRequest,
) -> VastResult<VastPlatesolverSolution> {
    if values
        .get("PLTSOLVD")
        .map(|value| value.eq_ignore_ascii_case("T"))
        != Some(true)
    {
        return Err(runtime_error(
            read_ini_message(values, "ERROR")
                .or_else(|| read_ini_message(values, "WARNING"))
                .unwrap_or_else(|| "ASTAP did not report a valid solution".to_string()),
        ));
    }

    let ra_deg = parse_ini_f64(values, "CRVAL1")?;
    let dec_deg = parse_ini_f64(values, "CRVAL2")?;
    let scale_x_deg_per_pixel = parse_scale_x_deg_per_pixel(values)?;
    let scale_y_deg_per_pixel = parse_scale_y_deg_per_pixel(values)?;
    let pixel_scale_arcsec_per_pixel = ((scale_x_deg_per_pixel.abs() + scale_y_deg_per_pixel.abs()) * 0.5) * 3600.0;
    let rotation_deg = parse_rotation_deg(values)?;
    let parity = parse_parity(values);
    let (width_pixels, height_pixels) = parse_image_dimensions(values, request);

    Ok(VastPlatesolverSolution {
        center: EquatorialDegrees {
            ra: ra_deg,
            dec: dec_deg,
        },
        pixel_scale_arcsec_per_pixel,
        rotation_deg,
        field_width_deg: width_pixels * scale_x_deg_per_pixel.abs(),
        field_height_deg: height_pixels * scale_y_deg_per_pixel.abs(),
        parity,
        reference_pixel_x: values.get("CRPIX1").and_then(|value| value.parse::<f64>().ok()),
        reference_pixel_y: values.get("CRPIX2").and_then(|value| value.parse::<f64>().ok()),
        cd_matrix: parse_cd_matrix(values),
        wcs_headers: values
            .iter()
            .filter(|(key, _)| is_wcs_key(key))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect(),
    })
}

fn build_diagnostics(
    args: &[String],
    output: &std::process::Output,
    values: &HashMap<String, String>,
) -> VastPlatesolverDiagnostics {
    let mut command = Vec::with_capacity(args.len() + 1);
    command.push("astap".to_string());
    command.extend(args.iter().cloned());

    let mut warnings = Vec::new();
    if let Some(warning) = read_ini_message(values, "WARNING") {
        warnings.push(warning);
    }

    VastPlatesolverDiagnostics {
        implementation_name: "astap",
        command,
        stdout: String::from_utf8_lossy(&output.stdout).trim().to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        warnings,
        working_directory: None,
    }
}

fn parse_cd_matrix(values: &HashMap<String, String>) -> Option<[[f64; 2]; 2]> {
    Some([
        [values.get("CD1_1")?.parse().ok()?, values.get("CD1_2")?.parse().ok()?],
        [values.get("CD2_1")?.parse().ok()?, values.get("CD2_2")?.parse().ok()?],
    ])
}

fn is_wcs_key(key: &str) -> bool {
    matches!(
        key,
        "CRPIX1" | "CRPIX2" | "CRVAL1" | "CRVAL2" | "CDELT1" | "CDELT2" | "CROTA1" | "CROTA2"
    ) || key.starts_with("CD")
}

fn parse_scale_x_deg_per_pixel(values: &HashMap<String, String>) -> VastResult<f64> {
    if let Some(value) = values.get("CDELT1") {
        return parse_f64_value("CDELT1", value);
    }

    let cd1_1 = parse_ini_f64(values, "CD1_1")?;
    let cd2_1 = parse_ini_f64(values, "CD2_1")?;
    Ok((cd1_1.powi(2) + cd2_1.powi(2)).sqrt())
}

fn parse_scale_y_deg_per_pixel(values: &HashMap<String, String>) -> VastResult<f64> {
    if let Some(value) = values.get("CDELT2") {
        return parse_f64_value("CDELT2", value);
    }

    let cd1_2 = parse_ini_f64(values, "CD1_2")?;
    let cd2_2 = parse_ini_f64(values, "CD2_2")?;
    Ok((cd1_2.powi(2) + cd2_2.powi(2)).sqrt())
}

fn parse_rotation_deg(values: &HashMap<String, String>) -> VastResult<f64> {
    if let Some(value) = values.get("CROTA2").or_else(|| values.get("CROTA1")) {
        return parse_f64_value("CROTA", value);
    }

    let cd1_1 = parse_ini_f64(values, "CD1_1")?;
    let cd2_1 = parse_ini_f64(values, "CD2_1")?;
    Ok(cd2_1.atan2(cd1_1).to_degrees())
}

fn parse_parity(values: &HashMap<String, String>) -> VastPlatesolverParity {
    let Some(cd1_1) = values.get("CD1_1").and_then(|value| value.parse::<f64>().ok()) else {
        return VastPlatesolverParity::Any;
    };
    let Some(cd1_2) = values.get("CD1_2").and_then(|value| value.parse::<f64>().ok()) else {
        return VastPlatesolverParity::Any;
    };
    let Some(cd2_1) = values.get("CD2_1").and_then(|value| value.parse::<f64>().ok()) else {
        return VastPlatesolverParity::Any;
    };
    let Some(cd2_2) = values.get("CD2_2").and_then(|value| value.parse::<f64>().ok()) else {
        return VastPlatesolverParity::Any;
    };

    if cd1_1 * cd2_2 - cd1_2 * cd2_1 >= 0.0 {
        VastPlatesolverParity::Positive
    } else {
        VastPlatesolverParity::Negative
    }
}

fn parse_image_dimensions(values: &HashMap<String, String>, request: &VastPlatesolverRequest) -> (f64, f64) {
    let width = values
        .get("NAXIS1")
        .and_then(|value| value.parse::<f64>().ok())
        .or_else(|| match &request.source {
            VastPlatesolverSource::ImageFrame(frame) => Some(frame.width as f64),
            VastPlatesolverSource::FilePath(_) => None,
        })
        .unwrap_or(0.0);
    let height = values
        .get("NAXIS2")
        .and_then(|value| value.parse::<f64>().ok())
        .or_else(|| match &request.source {
            VastPlatesolverSource::ImageFrame(frame) => Some(frame.height as f64),
            VastPlatesolverSource::FilePath(_) => None,
        })
        .unwrap_or(0.0);

    (width, height)
}

fn parse_ini_f64(values: &HashMap<String, String>, key: &str) -> VastResult<f64> {
    let value = values
        .get(key)
        .ok_or_else(|| runtime_error(format!("ASTAP solution missing {key}")))?;
    parse_f64_value(key, value)
}

fn parse_f64_value(key: &str, value: &str) -> VastResult<f64> {
    value
        .parse::<f64>()
        .map_err(|err| runtime_error(format!("ASTAP field {key} is not a number: {err}")))
}

fn file_error(message: impl Into<String>) -> crate::base::errors::VastError {
    <AstapPlatesolver as VastPlatesolverBackendExt>::file_error(message)
}

fn runtime_error(message: impl Into<String>) -> crate::base::errors::VastError {
    <AstapPlatesolver as VastPlatesolverBackendExt>::runtime_error(message)
}

fn read_ini_message(values: &HashMap<String, String>, key: &str) -> Option<String> {
    values.get(key).cloned().filter(|value| !value.is_empty())
}
