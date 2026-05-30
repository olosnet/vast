use crate::types::{consts, imageformats::{ImageFrame, StandardImageFrameFormat}};

use super::{convert::*, images::*, stars::*};
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

#[test]
fn applies_binary_threshold_to_raw8() {
    let frame = ImageFrame {
        width: 4,
        height: 1,
        format: StandardImageFrameFormat::RAW8,
        data: vec![0, 64, 128, 255],
    };
    let threshold = compute_threshold(&frame, ThresholdMethod::Fixed(100.0)).unwrap();

    let masked = apply_threshold(&frame, &threshold, ThresholdMode::Binary).unwrap();

    assert_eq!(masked.data, vec![0, 0, 255, 255]);
}

#[test]
fn computes_mean_percentage_threshold() {
    let frame = ImageFrame {
        width: 4,
        height: 1,
        format: StandardImageFrameFormat::RAW8,
        data: vec![10, 10, 10, 30],
    };

    let threshold = compute_threshold(&frame, ThresholdMethod::MeanPercentage { percentage: 120.0 }).unwrap();

    assert_eq!(threshold.channels.len(), 1);
    assert_eq!(threshold.channels[0], 18.0);
}

#[test]
fn detects_star_blob_and_centroid() {
    let frame = ImageFrame {
        width: 7,
        height: 7,
        format: StandardImageFrameFormat::RAW8,
        data: vec![
            5, 5, 5, 5, 5, 5, 5,
            5, 5, 5, 5, 5, 5, 5,
            5, 5, 20, 40, 20, 5, 5,
            5, 5, 40, 100, 40, 5, 5,
            5, 5, 20, 40, 20, 5, 5,
            5, 5, 5, 5, 5, 5, 5,
            5, 5, 5, 5, 5, 5, 5,
        ],
    };

    let blobs = detect_star_blobs(
        &frame,
        StarDetectionOptions {
            threshold_method: ThresholdMethod::Fixed(15.0),
            min_pixels: 4,
            border_margin: 0,
            ..StarDetectionOptions::default()
        },
    )
    .unwrap();

    assert_eq!(blobs.len(), 1);
    assert_eq!(blobs[0].peak_x, 3);
    assert_eq!(blobs[0].peak_y, 3);

    let centroid = compute_blob_centroid(&frame, &blobs[0]).unwrap();
    assert!((centroid.x - 3.5).abs() < 0.1);
    assert!((centroid.y - 3.5).abs() < 0.1);

    let background = estimate_blob_background(&frame, &blobs[0], 2.0, 3.5).unwrap();
    assert!((background.mean - 5.0).abs() < 0.1);

    let shape = compute_blob_shape_metrics(&frame, &blobs[0], centroid, background).unwrap();
    assert!(shape.hfr > 0.5);
    assert!((shape.hfd - shape.hfr * 2.0).abs() < 1e-9);
}

#[test]
fn rejects_hot_pixel_blob_with_min_pixels() {
    let frame = ImageFrame {
        width: 5,
        height: 5,
        format: StandardImageFrameFormat::RAW8,
        data: vec![
            0, 0, 0, 0, 0,
            0, 0, 0, 0, 0,
            0, 0, 255, 0, 0,
            0, 0, 0, 0, 0,
            0, 0, 0, 0, 0,
        ],
    };

    let blobs = detect_star_blobs(
        &frame,
        StarDetectionOptions {
            threshold_method: ThresholdMethod::Fixed(100.0),
            min_pixels: 2,
            border_margin: 0,
            ..StarDetectionOptions::default()
        },
    )
    .unwrap();

    assert!(blobs.is_empty());
}

#[test]
fn scores_centered_star_above_edge_star() {
    let frame = ImageFrame {
        width: 16,
        height: 16,
        format: StandardImageFrameFormat::RAW8,
        data: vec![0; 16 * 16],
    };
    let blobs = vec![
        StarBlob {
            channel_index: 0,
            threshold: 10.0,
            pixels: vec![(7, 7), (7, 8), (8, 7), (8, 8)],
            min_x: 7,
            min_y: 7,
            max_x: 8,
            max_y: 8,
            peak_x: 7,
            peak_y: 7,
            peak_value: 200.0,
        },
        StarBlob {
            channel_index: 0,
            threshold: 10.0,
            pixels: vec![(0, 0), (0, 1), (1, 0), (1, 1)],
            min_x: 0,
            min_y: 0,
            max_x: 1,
            max_y: 1,
            peak_x: 0,
            peak_y: 0,
            peak_value: 220.0,
        },
    ];
    let mut data = vec![0_u8; 16 * 16];
    data[7 * 16 + 7] = 140;
    data[7 * 16 + 8] = 110;
    data[8 * 16 + 7] = 110;
    data[8 * 16 + 8] = 90;
    data[0] = 150;
    data[1] = 95;
    data[16] = 95;
    data[17] = 80;
    let frame = ImageFrame { data, ..frame };

    let scores = score_star_blobs(
        &frame,
        &blobs,
        StarScoringOptions {
            border_guard: 2,
            ..StarScoringOptions::default()
        },
    )
    .unwrap();

    assert_eq!(scores[0].index, 0);
    assert!(scores[0].score > scores[1].score);
}

#[test]
fn median_filter_removes_hot_pixel() {
    let frame = ImageFrame {
        width: 3,
        height: 3,
        format: StandardImageFrameFormat::RAW8,
        data: vec![
            0, 0, 0,
            0, 255, 0,
            0, 0, 0,
        ],
    };

    let filtered = median_filter_channel_3x3(&frame, 0).unwrap();

    assert_eq!(filtered[4], 0.0);
}

#[test]
fn finds_star_candidates_and_ignores_hot_pixel() {
    let frame = ImageFrame {
        width: 17,
        height: 17,
        format: StandardImageFrameFormat::RAW8,
        data: vec![
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 255, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 10, 20, 30, 20, 10, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 10, 30, 60, 80, 60, 30, 10, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 20, 60, 110, 150, 110, 60, 20, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 30, 80, 150, 220, 150, 80, 30, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 20, 60, 110, 150, 110, 60, 20, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 10, 30, 60, 80, 60, 30, 10, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 10, 20, 30, 20, 10, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ],
    };

    let candidates = find_star_candidates(
        &frame,
        StarCandidateOptions {
            border_margin: 0,
            max_candidates: 3,
            ..StarCandidateOptions::default()
        },
    )
    .unwrap();

    assert!(!candidates.is_empty());
    assert!((candidates[0].x - 8.5).abs() <= 1.0);
    assert!((candidates[0].y - 8.5).abs() <= 1.0);
    assert!(candidates.iter().all(|candidate| candidate.x > 2.0 || candidate.y > 2.0));
}
