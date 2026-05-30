use crate::types::{consts, imageformats::{ImageFrame, StandardImageFrameFormat}};

use super::{convert::*, images::*};
use chrono::{TimeZone, Utc};

fn ra_error_arcsec(lhs_hours: f64, rhs_hours: f64, dec_deg: f64) -> f64 {
    let delta_degrees =
        ((lhs_hours - rhs_hours) * consts::HOURS_TO_DEGREES + 540.0).rem_euclid(360.0) - 180.0;

    delta_degrees.abs() * 3600.0 * dec_deg.to_radians().cos()
}

#[test]
fn datetime_to_julian_day_matches_j2000_epoch() {
    let datetime = Utc.with_ymd_and_hms(2000, 1, 1, 12, 0, 0).single().unwrap();
    assert!((datetime_to_julian_day(datetime) - consts::JD_J2000).abs() < 1e-9);
}

#[test]
fn j2000_to_jnow_matches_indi_reference_case() {
    let (ra, dec) = j2000_to_jnow(20.69053168, 45.28033881, 2459019.833333);
    let ra_err = ra_error_arcsec(ra, 20.70237028, 45.35036333);
    let dec_err = (dec - 45.35036333).abs() * 3600.0;
    let total_err = ra_err.hypot(dec_err);

    assert!(
        total_err < 1.0,
        "expected <1 arcsec total error, got RA={ra_err:.3}\" Dec={dec_err:.3}\" total={total_err:.3}\""
    );
}

#[test]
fn jnow_round_trip_recovers_j2000_position() {
    for jd in [2459019.833333, 2461112.5] {
        let (ra_jnow, dec_jnow) = j2000_to_jnow(20.69053168, 45.28033881, jd);
        let (ra_j2000, dec_j2000) = jnow_to_j2000(ra_jnow, dec_jnow, jd);
        let ra_err = ra_error_arcsec(ra_j2000, 20.69053168, 45.28033881);
        let dec_err = (dec_j2000 - 45.28033881).abs() * 3600.0;

        assert!(
            ra_err < 1.0,
            "expected <1 arcsec RA round-trip error at JD {jd}, got {ra_err:.3}\""
        );
        assert!(
            dec_err < 1.0,
            "expected <1 arcsec Dec round-trip error at JD {jd}, got {dec_err:.3}\""
        );
    }
}

#[test]
fn computes_raw16_stats() {
    let frame = ImageFrame {
        width: 4,
        height: 1,
        format: StandardImageFrameFormat::RAW16,
        data: vec![0, 0, 0x10, 0x00, 0x20, 0x00, 0x30, 0x00],
    };

    let stats = compute_raw_image_stats(&frame).unwrap();

    assert_eq!(stats.channels, 1);
    assert_eq!(stats.channel_stats[0].min, 0.0);
    assert_eq!(stats.channel_stats[0].max, 48.0);
    assert_eq!(stats.channel_stats[0].median, 24.0);
}

#[test]
fn builds_histogram_for_raw8() {
    let frame = ImageFrame {
        width: 4,
        height: 1,
        format: StandardImageFrameFormat::RAW8,
        data: vec![0, 64, 128, 255],
    };

    let histogram = build_raw_histogram(&frame, Some(4)).unwrap();

    assert_eq!(histogram.channels.len(), 1);
    assert_eq!(histogram.bin_count, 4);
    assert_eq!(histogram.channels[0].cumulative_frequencies[4], 4);
}

#[test]
fn computes_auto_stretch_window() {
    let frame = ImageFrame {
        width: 4,
        height: 1,
        format: StandardImageFrameFormat::RAW8,
        data: vec![10, 20, 30, 40],
    };

    let stretch = compute_auto_stretch(&frame).unwrap();

    assert_eq!(stretch.channels.len(), 1);
    assert!(stretch.channels[0].highlights > stretch.channels[0].shadows);
}

#[test]
fn applies_linear_stretch_to_raw8() {
    let frame = ImageFrame {
        width: 4,
        height: 1,
        format: StandardImageFrameFormat::RAW8,
        data: vec![10, 20, 30, 40],
    };
    let stretch = ImageStretch {
        format: StandardImageFrameFormat::RAW8,
        channels: vec![StretchWindow {
            shadows: 10.0,
            highlights: 40.0,
        }],
    };

    let stretched = apply_stretch(&frame, &stretch).unwrap();

    assert_eq!(stretched.data[0], 0);
    assert_eq!(stretched.data[3], 255);
}

#[test]
fn applies_linear_stretch_to_planar_rgb24() {
    let frame = ImageFrame {
        width: 2,
        height: 1,
        format: StandardImageFrameFormat::RGB24,
        data: vec![10, 20, 30, 40, 50, 60],
    };
    let stretch = ImageStretch {
        format: StandardImageFrameFormat::RGB24,
        channels: vec![
            StretchWindow {
                shadows: 10.0,
                highlights: 20.0,
            },
            StretchWindow {
                shadows: 30.0,
                highlights: 40.0,
            },
            StretchWindow {
                shadows: 50.0,
                highlights: 60.0,
            },
        ],
    };

    let stretched = apply_stretch(&frame, &stretch).unwrap();

    assert_eq!(stretched.data, vec![0, 255, 0, 255, 0, 255]);
}

#[test]
fn computes_percentile_auto_stretch() {
    let frame = ImageFrame {
        width: 5,
        height: 1,
        format: StandardImageFrameFormat::RAW8,
        data: vec![0, 10, 20, 30, 255],
    };

    let stretch = compute_percentile_auto_stretch(&frame, 0.2, 0.8).unwrap();

    assert_eq!(stretch.channels.len(), 1);
    assert!(stretch.channels[0].shadows >= 0.0);
    assert!(stretch.channels[0].highlights <= 255.0);
    assert!(stretch.channels[0].highlights > stretch.channels[0].shadows);
}

#[test]
fn applies_midtones_transfer_to_raw8() {
    let frame = ImageFrame {
        width: 3,
        height: 1,
        format: StandardImageFrameFormat::RAW8,
        data: vec![0, 128, 255],
    };

    let stretched = apply_midtones_transfer(
        &frame,
        &[MidtonesTransfer {
            shadows: 0.0,
            midtones: 0.25,
            highlights: 255.0,
        }],
    )
    .unwrap();

    assert_eq!(stretched.data[0], 0);
    assert_eq!(stretched.data[2], 255);
    assert!(stretched.data[1] > 128);
}

#[test]
fn renders_histogram_visualization() {
    let frame = ImageFrame {
        width: 4,
        height: 1,
        format: StandardImageFrameFormat::RAW8,
        data: vec![0, 64, 128, 255],
    };
    let histogram = build_raw_histogram(&frame, Some(4)).unwrap();

    let rendered = render_histogram_visualization(
        &histogram,
        HistogramRenderOptions {
            width: 8,
            height: 8,
            logarithmic: false,
        },
    )
    .unwrap();

    assert_eq!(rendered.width, 8);
    assert_eq!(rendered.height, 8);
    assert_eq!(rendered.format, StandardImageFrameFormat::RGB24);
    assert_eq!(rendered.data.len(), 8 * 8 * 3);
    assert!(rendered.data.iter().any(|value| *value > 0));
}
