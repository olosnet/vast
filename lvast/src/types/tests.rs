use super::{guiding::*, imageformats::*, platsolver::*};
use crate::types::{
    camera::{CameraFrameFormat, VastCameraFrame},
    imageformats::StandardImageFrameFormat,
};

#[test]
fn camera_and_image_formats_round_trip() {
    for format in [
        CameraFrameFormat::RAW8,
        CameraFrameFormat::RAW16,
        CameraFrameFormat::RAW14,
        CameraFrameFormat::RAW12,
        CameraFrameFormat::RAW10,
        CameraFrameFormat::RGB24,
        CameraFrameFormat::RGB32,
    ] {
        let image_format: StandardImageFrameFormat = format.into();
        let camera_format: CameraFrameFormat = image_format.into();
        assert_eq!(camera_format, format);
    }
}

#[test]
fn camera_and_image_frames_round_trip() {
    let camera_frame = VastCameraFrame {
        width: 2,
        height: 1,
        format: CameraFrameFormat::RGB24,
        data: vec![1, 2, 3, 4, 5, 6],
    };

    let image_frame: ImageFrame = camera_frame.clone().into();
    let converted_camera_frame: VastCameraFrame = image_frame.into();

    assert_eq!(converted_camera_frame, camera_frame);
}

#[test]
fn rejects_invalid_scale_hint() {
    let request = VastPlatesolverRequest {
        source: VastPlatesolverSource::ImageFrame(ImageFrame {
            width: 2,
            height: 2,
            format: StandardImageFrameFormat::RAW8,
            data: vec![0; 4],
        }),
        position_hint: None,
        scale_hint: Some(VastPlatesolverScaleHint {
            min_arcsec_per_pixel: 2.0,
            max_arcsec_per_pixel: 1.0,
        }),
        parity_hint: None,
        downsample_factor: None,
        timeout_seconds: None,
        blind_solve: true,
    };

    assert!(request.validate().is_err());
}

#[test]
fn accepts_valid_image_frame_request() {
    let request = VastPlatesolverRequest::from_image_frame(ImageFrame {
        width: 2,
        height: 2,
        format: StandardImageFrameFormat::RAW8,
        data: vec![0; 4],
    });

    assert!(request.validate().is_ok());
}

#[test]
fn creates_requested_backend() {
    assert_eq!(create_platesolver(VastPlatesolverBackend::Astap).implementation_name(), "astap");
    assert_eq!(
        create_platesolver(VastPlatesolverBackend::AstrometryNet).implementation_name(),
        "astrometry.net"
    );
}

#[test]
fn accepts_default_guide_session_config() {
    assert!(VastGuideSessionConfig::default().validate().is_ok());
}

#[test]
fn rejects_invalid_guide_session_config() {
    let mut config = VastGuideSessionConfig::default();
    config.exposure_millis = 0;

    assert!(config.validate().is_err());
}

#[test]
fn rejects_invalid_guide_calibration() {
    let calibration = VastGuideCalibration {
        camera_angle_deg: 0.0,
        ra_millis_per_pixel: 0.0,
        dec_millis_per_pixel: 10.0,
        ra_inverted: false,
        dec_inverted: false,
        image_scale_arcsec_per_pixel: Some(1.5),
        calibration_position: None,
    };

    assert!(calibration.validate().is_err());
}

#[test]
fn guide_lock_reports_state_from_lost_reason() {
    let lock = VastGuideStarLock {
        x: 10.5,
        y: 12.5,
        snr: Some(25.0),
        hfr: Some(2.1),
        lost_frames: 0,
        lost_reason: None,
    };
    assert!(lock.is_locked());

    let lost = VastGuideStarLock {
        lost_reason: Some(VastGuideLostStarReason::LowSignal),
        ..lock
    };
    assert!(!lost.is_locked());
}
