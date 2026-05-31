use std::{sync::Arc, time::Instant};

use crate::{
    drivers::native::fake_camera::driver::{
        FakeCameraDefectProfile, FakeCameraDriver, FakeCameraFocalPreset, FakeCameraSensorPreset,
        FakeCameraSkyFieldPreset, FakeVastCamera,
    },
    types::{
        camera::{
            CameraBayerPattern, VastCamera, VastCameraAcquireImage, VastCameraDriver,
            VastCameraGuide, VastCameraID,
        },
        common::EquatorialDegrees,
    },
};

fn raw16_samples(frame: &crate::types::camera::VastCameraFrame) -> Vec<u16> {
    frame
        .data
        .chunks_exact(2)
        .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]))
        .collect()
}

#[test]
fn fake_camera_renders_non_empty_synthetic_frame() {
    let driver = Arc::new(FakeCameraDriver::new());
    let mut camera = FakeVastCamera::new(driver);
    camera.connect(VastCameraID::StrID("fake-camera-0".to_string())).unwrap();
    camera.set_sensor_preset(FakeCameraSensorPreset::Asi294Mc);
    camera.set_focal_preset(FakeCameraFocalPreset::Mm600);
    camera.set_sky_field_preset(FakeCameraSkyFieldPreset::M45Pleiades);
    camera.set_seeing_arcsec(2.0).unwrap();
    camera.set_sensor_noise(3.0).unwrap();

    camera.start_image_acquisition().unwrap();
    let frame = camera.get_acquired_image(2_000).unwrap();

    assert_eq!(frame.width, 4144);
    assert_eq!(frame.height, 2822);
    assert_eq!(frame.data.len(), 4144 * 2822 * 2);

    let samples = raw16_samples(&frame)
        .into_iter()
        .map(u32::from)
        .collect::<Vec<_>>();
    let min = *samples.iter().min().unwrap();
    let max = *samples.iter().max().unwrap();
    assert!(max > min + 1000, "expected stars above noisy background, min={min} max={max}");
}

#[test]
fn fake_camera_guide_pulse_changes_pointing() {
    let driver = Arc::new(FakeCameraDriver::new());
    let mut camera = FakeVastCamera::new(driver);
    camera.connect(VastCameraID::IntID(0)).unwrap();

    let before = camera.simulation_config().center;
    camera
        .pulse_guide(crate::types::camera::VastCameraGuideDirection::East, 1000)
        .unwrap();
    let after = camera.simulation_config().center;

    assert_ne!(before.ra, after.ra);
    assert_eq!(before.dec, after.dec);
}

#[test]
fn fake_camera_rejects_invalid_configuration() {
    let driver = Arc::new(FakeCameraDriver::new());
    let mut camera = FakeVastCamera::new(driver);

    assert!(camera.set_seeing_arcsec(0.0).is_err());
    assert!(camera.set_sensor_noise(-1.0).is_err());
    assert!(camera
        .set_output_ra_dec(EquatorialDegrees { ra: 10.0, dec: 95.0 })
        .is_err());
}

#[test]
fn fake_camera_uses_selected_field_center() {
    let driver = Arc::new(FakeCameraDriver::new());
    let mut camera = FakeVastCamera::new(driver);

    camera.set_sky_field_preset(FakeCameraSkyFieldPreset::VegaLyra);

    assert_eq!(camera.simulation_config().center, EquatorialDegrees { ra: 279.2347, dec: 38.7837 });
}

#[test]
fn fake_camera_focal_preset_reports_fov() {
    let fov = FakeCameraFocalPreset::Mm400.approximate_fov_degrees(FakeCameraSensorPreset::Asi294Mc);

    assert!(fov.0 > 2.5 && fov.0 < 2.8, "unexpected width fov: {}", fov.0);
    assert!(fov.1 > 1.8 && fov.1 < 2.0, "unexpected height fov: {}", fov.1);
}

#[test]
fn fake_camera_renders_dense_sadr_field_quickly() {
    let driver = Arc::new(FakeCameraDriver::new());
    let mut camera = FakeVastCamera::new(driver);
    camera.connect(VastCameraID::StrID("fake-camera-0".to_string())).unwrap();
    camera.set_sensor_preset(FakeCameraSensorPreset::Asi294Mc);
    camera.set_focal_preset(FakeCameraFocalPreset::Mm400);
    camera.set_sky_field_preset(FakeCameraSkyFieldPreset::SadrCygnus);
    camera.set_seeing_arcsec(2.2).unwrap();
    camera.set_sensor_noise(3.0).unwrap();
    camera
        .set_camera_settings(crate::types::camera::VastCameraSettings {
            exposure_microseconds: Some(250_000),
            binning: Some((4, 4)),
            ..Default::default()
        })
        .unwrap();

    camera.start_image_acquisition().unwrap();
    let started = Instant::now();
    let frame = camera.get_acquired_image(2_000).unwrap();
    let elapsed = started.elapsed();

    assert_eq!(frame.width, 1036);
    assert_eq!(frame.height, 705);
    assert!(elapsed.as_secs_f64() < 1.5, "dense sadr render too slow: {elapsed:?}");
}

#[test]
fn fake_camera_no_stars_mode_produces_dark_like_frame() {
    let driver = Arc::new(FakeCameraDriver::new());
    let mut camera = FakeVastCamera::new(driver);
    camera.connect(VastCameraID::StrID("fake-camera-0".to_string())).unwrap();
    camera.set_sky_field_preset(FakeCameraSkyFieldPreset::NoStars);
    camera.set_sensor_noise(2.0).unwrap();
    camera
        .set_camera_settings(crate::types::camera::VastCameraSettings {
            exposure_microseconds: Some(1_000_000),
            offset: Some(512),
            binning: Some((4, 4)),
            ..Default::default()
        })
        .unwrap();

    camera.start_image_acquisition().unwrap();
    let frame = camera.get_acquired_image(2_000).unwrap();
    let samples = raw16_samples(&frame)
        .into_iter()
        .map(u32::from)
        .collect::<Vec<_>>();
    let max = *samples.iter().max().unwrap();
    let bright_outliers = samples.iter().filter(|&&value| value > 1_000).count();

    assert!(max < 1_000, "dark frame unexpectedly contains very bright pixels, max={max}");
    assert_eq!(bright_outliers, 0, "dark frame unexpectedly contains bright outliers: {bright_outliers}");
}

#[test]
fn fake_camera_defect_profile_adds_hot_pixels() {
    let driver = Arc::new(FakeCameraDriver::new());
    let mut camera = FakeVastCamera::new(driver);
    camera.connect(VastCameraID::StrID("fake-camera-0".to_string())).unwrap();
    camera.set_sky_field_preset(FakeCameraSkyFieldPreset::NoStars);
    camera.set_defect_profile(FakeCameraDefectProfile::Heavy);
    camera.set_sensor_noise(1.0).unwrap();
    camera
        .set_camera_settings(crate::types::camera::VastCameraSettings {
            exposure_microseconds: Some(1_000_000),
            offset: Some(512),
            binning: Some((4, 4)),
            ..Default::default()
        })
        .unwrap();

    camera.start_image_acquisition().unwrap();
    let frame = camera.get_acquired_image(2_000).unwrap();
    let samples = raw16_samples(&frame)
        .into_iter()
        .map(u32::from)
        .collect::<Vec<_>>();
    let hot_pixels = samples.iter().filter(|&&value| value > 6_000).count();
    let cold_pixels = samples.iter().filter(|&&value| value < 520).count();

    assert!(hot_pixels > 8, "expected hot pixels in heavy defect profile, got {hot_pixels}");
    assert!(cold_pixels > 8, "expected cold pixels in heavy defect profile, got {cold_pixels}");
}

#[test]
fn fake_camera_flat_field_mode_produces_bright_smooth_frame() {
    let driver = Arc::new(FakeCameraDriver::new());
    let mut camera = FakeVastCamera::new(driver);
    camera.connect(VastCameraID::StrID("fake-camera-0".to_string())).unwrap();
    camera.set_sky_field_preset(FakeCameraSkyFieldPreset::FlatField);
    camera.set_sensor_noise(2.0).unwrap();
    camera
        .set_camera_settings(crate::types::camera::VastCameraSettings {
            exposure_microseconds: Some(500_000),
            offset: Some(512),
            binning: Some((4, 4)),
            ..Default::default()
        })
        .unwrap();

    camera.start_image_acquisition().unwrap();
    let frame = camera.get_acquired_image(2_000).unwrap();
    let samples = raw16_samples(&frame)
        .into_iter()
        .map(u32::from)
        .collect::<Vec<_>>();
    let min = *samples.iter().min().unwrap();
    let max = *samples.iter().max().unwrap();
    let mean = samples.iter().map(|&value| value as f64).sum::<f64>() / samples.len() as f64;

    assert!(mean > 10_000.0, "flat frame too dim, mean={mean}");
    assert!(max < 20_000, "flat frame has unexpected stellar-like spikes, max={max}");
    assert!(max > min + 1_000, "flat frame too uniform, min={min} max={max}");
}

#[test]
fn fake_camera_color_bayer_pattern_biases_green_sites_higher() {
    let driver = Arc::new(FakeCameraDriver::new());
    let mut camera = FakeVastCamera::new(driver);
    camera.connect(VastCameraID::StrID("fake-camera-0".to_string())).unwrap();
    camera.set_sky_field_preset(FakeCameraSkyFieldPreset::FlatField);
    camera.set_bayer_pattern(Some(CameraBayerPattern::RGGB));
    camera.set_sensor_noise(0.0).unwrap();
    camera
        .set_camera_settings(crate::types::camera::VastCameraSettings {
            exposure_microseconds: Some(500_000),
            offset: Some(512),
            roi: Some((0, 0, 32, 32)),
            ..Default::default()
        })
        .unwrap();

    camera.start_image_acquisition().unwrap();
    let frame = camera.get_acquired_image(2_000).unwrap();
    let samples = raw16_samples(&frame);

    let mut red_total = 0_u64;
    let mut green_total = 0_u64;
    let mut blue_total = 0_u64;
    let mut red_count = 0_u64;
    let mut green_count = 0_u64;
    let mut blue_count = 0_u64;

    for y in 0..frame.height as usize {
        for x in 0..frame.width as usize {
            let sample = u64::from(samples[y * frame.width as usize + x]);
            match (x & 1, y & 1) {
                (0, 0) => {
                    red_total += sample;
                    red_count += 1;
                }
                (1, 1) => {
                    blue_total += sample;
                    blue_count += 1;
                }
                _ => {
                    green_total += sample;
                    green_count += 1;
                }
            }
        }
    }

    let red_mean = red_total as f64 / red_count as f64;
    let green_mean = green_total as f64 / green_count as f64;
    let blue_mean = blue_total as f64 / blue_count as f64;

    assert!(green_mean > red_mean, "expected green mean > red mean, red={red_mean} green={green_mean}");
    assert!(green_mean > blue_mean, "expected green mean > blue mean, blue={blue_mean} green={green_mean}");
}

#[test]
fn fake_camera_can_switch_from_color_to_mono_capabilities() {
    let driver = Arc::new(FakeCameraDriver::new());
    let mut camera = FakeVastCamera::new(driver);

    assert_eq!(
        camera.get_capabilities().bayer_pattern.as_ref().map(ToString::to_string),
        Some("RGGB".to_string())
    );

    camera.set_bayer_pattern(None);

    assert_eq!(camera.get_capabilities().bayer_pattern, None);
    assert_eq!(camera.simulation_config().bayer_pattern, None);
}
