use std::path::{Path, PathBuf};

use super::{astap::*, astrometry_net::*};
use crate::types::{
    common::EquatorialDegrees,
    imageformats::{ImageFrame, StandardImageFrameFormat},
    platsolver::{
        VastPlatesolver, VastPlatesolverParity, VastPlatesolverPositionHint,
        VastPlatesolverRequest, VastPlatesolverScaleHint, VastPlatesolverSolution,
        VastPlatesolverSource,
    },
};

#[test]
fn builds_astap_args_from_generic_request() {
    let request = VastPlatesolverRequest {
        source: VastPlatesolverSource::ImageFrame(ImageFrame {
            width: 1024,
            height: 768,
            format: StandardImageFrameFormat::RAW8,
            data: vec![0; 1024 * 768],
        }),
        position_hint: Some(VastPlatesolverPositionHint {
            center: EquatorialDegrees { ra: 180.0, dec: 45.0 },
            radius_deg: 10.0,
        }),
        scale_hint: Some(VastPlatesolverScaleHint {
            min_arcsec_per_pixel: 1.0,
            max_arcsec_per_pixel: 2.0,
        }),
        parity_hint: Some(VastPlatesolverParity::Negative),
        downsample_factor: Some(2),
        timeout_seconds: Some(30),
        blind_solve: false,
    };

    let args = build_astap_args(
        &request,
        Path::new("/tmp/input.fits"),
        Path::new("/tmp/output"),
        Some(Path::new("/opt/astap")),
    );

    assert!(args.windows(2).any(|pair| pair == ["-f", "/tmp/input.fits"]));
    assert!(args.windows(2).any(|pair| pair == ["-ra", "12.00000000"]));
    assert!(args.windows(2).any(|pair| pair == ["-spd", "135.00000000"]));
    assert!(args.windows(2).any(|pair| pair == ["-r", "10.00000000"]));
    assert!(args.windows(2).any(|pair| pair == ["-z", "2"]));
    assert!(args.windows(2).any(|pair| pair == ["-d", "/opt/astap"]));
    assert!(!args.iter().any(|arg| arg == "-parity"));
}

#[test]
fn parses_astap_solution_ini() {
    let values = std::collections::HashMap::from([
        ("PLTSOLVD".to_string(), "T".to_string()),
        ("CRVAL1".to_string(), "210.0".to_string()),
        ("CRVAL2".to_string(), "54.0".to_string()),
        ("CDELT1".to_string(), "-0.0005".to_string()),
        ("CDELT2".to_string(), "0.0005".to_string()),
        ("CROTA2".to_string(), "90.0".to_string()),
        ("CD1_1".to_string(), "-0.0005".to_string()),
        ("CD1_2".to_string(), "0.0".to_string()),
        ("CD2_1".to_string(), "0.0".to_string()),
        ("CD2_2".to_string(), "0.0005".to_string()),
        ("NAXIS1".to_string(), "1000".to_string()),
        ("NAXIS2".to_string(), "500".to_string()),
    ]);

    let request = VastPlatesolverRequest::from_file_path(PathBuf::from("test.fits"));
    let result = VastPlatesolverSolution {
        center: EquatorialDegrees { ra: 210.0, dec: 54.0 },
        pixel_scale_arcsec_per_pixel: 1.8,
        rotation_deg: 90.0,
        field_width_deg: 0.5,
        field_height_deg: 0.25,
        parity: VastPlatesolverParity::Negative,
        reference_pixel_x: None,
        reference_pixel_y: None,
        cd_matrix: Some([[-0.0005, 0.0], [0.0, 0.0005]]),
        wcs_headers: Vec::new(),
    };
    let solution = solution_from_ini(&values, &request).unwrap();

    assert_eq!(solution.center.ra, result.center.ra);
    assert_eq!(solution.center.dec, result.center.dec);
    assert!((solution.pixel_scale_arcsec_per_pixel - result.pixel_scale_arcsec_per_pixel).abs() < 1e-9);
    assert_eq!(solution.rotation_deg, result.rotation_deg);
    assert_eq!(solution.field_width_deg, result.field_width_deg);
    assert_eq!(solution.field_height_deg, result.field_height_deg);
    assert_eq!(solution.parity, result.parity);
    assert_eq!(solution.cd_matrix, result.cd_matrix);
}

#[test]
fn astap_mock_solver_returns_solution_with_diagnostics() {
    let temp_dir = std::env::temp_dir().join(format!("lvast-astap-mock-{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let script_path = temp_dir.join("astap-mock.sh");
    let script = r#"#!/bin/sh
out=""
while [ $# -gt 0 ]; do
  if [ "$1" = "-o" ]; then
    out="$2"
    shift 2
  else
    shift
  fi
done
cat > "${out}.ini" <<'EOF'
PLTSOLVD=T
CRVAL1=100.0
CRVAL2=20.0
CRPIX1=250.0
CRPIX2=125.0
CDELT1=-0.001
CDELT2=0.001
CD1_1=-0.001
CD1_2=0.0
CD2_1=0.0
CD2_2=0.001
CROTA2=0.0
WARNING=mock warning
NAXIS1=500
NAXIS2=250
EOF
echo mock-stdout
exit 0
"#;
    std::fs::write(&script_path, script).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&script_path).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&script_path, permissions).unwrap();
    }

    let solver = AstapPlatesolver::new().with_executable_path(&script_path);
    let result = solver
        .solve_with_diagnostics(&VastPlatesolverRequest::from_image_frame(ImageFrame {
            width: 500,
            height: 250,
            format: StandardImageFrameFormat::RAW8,
            data: vec![0; 500 * 250],
        }))
        .unwrap();

    assert_eq!(result.solution.center.ra, 100.0);
    assert_eq!(result.solution.reference_pixel_x, Some(250.0));
    assert_eq!(result.diagnostics.warnings, vec!["mock warning"]);
    assert_eq!(result.diagnostics.stdout, "mock-stdout");
}

#[test]
fn builds_solve_field_args_from_generic_request() {
    let request = VastPlatesolverRequest {
        source: VastPlatesolverSource::ImageFrame(ImageFrame {
            width: 800,
            height: 600,
            format: StandardImageFrameFormat::RAW8,
            data: vec![0; 800 * 600],
        }),
        position_hint: Some(VastPlatesolverPositionHint {
            center: EquatorialDegrees { ra: 180.0, dec: 10.0 },
            radius_deg: 5.0,
        }),
        scale_hint: Some(VastPlatesolverScaleHint {
            min_arcsec_per_pixel: 1.2,
            max_arcsec_per_pixel: 1.8,
        }),
        parity_hint: Some(VastPlatesolverParity::Negative),
        downsample_factor: Some(2),
        timeout_seconds: Some(60),
        blind_solve: false,
    };

    let args = build_solve_field_args(&request, Path::new("/tmp/input.fits"), Path::new("/tmp/work"), "solution");

    assert!(args.windows(2).any(|pair| pair == ["--ra", "12.00000000"]));
    assert!(args.windows(2).any(|pair| pair == ["--dec", "10.00000000"]));
    assert!(args.windows(2).any(|pair| pair == ["--radius", "5.00000000"]));
    assert!(args.windows(2).any(|pair| pair == ["--scale-units", "arcsecperpix"]));
    assert!(args.windows(2).any(|pair| pair == ["--parity", "neg"]));
    assert!(args.windows(2).any(|pair| pair == ["--cpulimit", "60"]));
}

#[test]
fn astrometry_net_mock_solver_returns_solution_with_diagnostics() {
    let temp_dir = std::env::temp_dir().join(format!("lvast-astrometry-mock-{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let script_path = temp_dir.join("solve-field-mock.sh");
    let script = r#"#!/bin/sh
dir=""
out=""
while [ $# -gt 0 ]; do
  case "$1" in
    --dir) dir="$2"; shift 2 ;;
    --out) out="$2"; shift 2 ;;
    *) shift ;;
  esac
done
printf '\001' > "$dir/$out.solved"
python3 - <<'PY' "$dir/$out.wcs"
import sys
path = sys.argv[1]
cards = [
    "SIMPLE  =                    T",
    "BITPIX  =                    8",
    "NAXIS   =                    2",
    "NAXIS1  =                  800",
    "NAXIS2  =                  600",
    "CRPIX1  =                400.0",
    "CRPIX2  =                300.0",
    "CRVAL1  =                123.0",
    "CRVAL2  =                 45.0",
    "CD1_1   =              -0.0010",
    "CD1_2   =               0.0000",
    "CD2_1   =               0.0000",
    "CD2_2   =               0.0010",
    "END",
]
buf = b''.join(card.ljust(80).encode('ascii') for card in cards)
buf += b' ' * ((2880 - len(buf) % 2880) % 2880)
buf += b'\x00' * 2880
open(path, 'wb').write(buf)
PY
echo "warning: mock astrometry warning"
exit 0
"#;
    std::fs::write(&script_path, script).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&script_path).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&script_path, permissions).unwrap();
    }

    let solver = AstrometryNetPlatesolver::new().with_executable_path(&script_path);
    let result = solver
        .solve_with_diagnostics(&VastPlatesolverRequest::from_image_frame(ImageFrame {
            width: 800,
            height: 600,
            format: StandardImageFrameFormat::RAW8,
            data: vec![0; 800 * 600],
        }))
        .unwrap();

    assert_eq!(result.solution.center.ra, 123.0);
    assert_eq!(result.solution.reference_pixel_x, Some(400.0));
    assert_eq!(result.solution.cd_matrix, Some([[-0.001, 0.0], [0.0, 0.001]]));
    assert_eq!(result.diagnostics.warnings, vec!["warning: mock astrometry warning"]);
}
