use std::{sync::Arc, thread};

use lvast::imageformats::{
    fits::FitsImageSaver,
    types::{ImageHeaders, ImageSaver},
};
use lvast::{
    cameras::svb::SvbVastCamera,
    types::camera::{VastCamera, VastCameraAcquireImage, VastCameraDriver as _},
};

fn safe_filename_part(value: &str) -> String {
    value
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

fn save_test_fits(camera: &mut SvbVastCamera) -> Result<(), lvast::base::errors::VastError> {
    let settings = camera.get_camera_settings()?;
    let capabilities = camera.get_capabilities();

    camera.start_image_acquisition()?;
    let timeout_millis = settings
        .exposure_microseconds
        .map(|exposure| {
            (exposure / 1_000)
                .saturating_add(5_000)
                .min(u64::from(u32::MAX)) as u32
        })
        .unwrap_or(30_000);
    println!("Waiting for test frame ({timeout_millis} ms timeout)...");
    let frame = camera.get_acquired_image(timeout_millis)?;
    println!(
        "Acquired test frame: {}x{} {}, {} bytes",
        frame.width,
        frame.height,
        frame.format,
        frame.data.len()
    );

    let (frame_x, frame_y) = settings
        .roi
        .map(|(x, y, _, _)| (Some(x), Some(y)))
        .unwrap_or((None, None));
    let (bin_x, bin_y) = settings
        .binning
        .map(|(x, y)| (Some(x), Some(y)))
        .unwrap_or((None, None));

    let headers = ImageHeaders {
        software: Some("vast camera FITS smoke test".to_string()),
        image_type: Some("Light".to_string()),
        instrument: Some(camera.get_name().to_string()),
        exposure_seconds: settings
            .exposure_microseconds
            .map(|exposure| exposure as f64 / 1_000_000.0),
        gain: settings.gain,
        offset: settings.offset,
        ccd_temperature: Some(f64::from(camera.get_current_temperature())),
        bin_x,
        bin_y,
        frame_x,
        frame_y,
        frame_width: Some(frame.width),
        frame_height: Some(frame.height),
        bayer_pattern: capabilities
            .bayer_pattern
            .map(|pattern| pattern.to_string()),
        ..Default::default()
    };

    let filename = format!("svb-test-{}.fits", safe_filename_part(camera.get_name()));
    let saver = FitsImageSaver::new(frame.width, frame.height, frame.format);
    saver.save(
        frame.data,
        Some(headers.to_fits_headers()),
        filename.clone(),
    )?;
    println!("Saved FITS test frame: {filename}");

    Ok(())
}

fn main() {
    let mut camera_driver = lvast::cameras::svb::SVBVastCameraDriver::new();

    println!("SVB SDK Version: {}", camera_driver.get_version());

    let cameras = camera_driver.init().unwrap_or_else(|e| {
        eprintln!("Failed to initialize camera driver: {}", e);
        std::process::exit(1);
    });

    println!("Found {} cameras", cameras.len());
    let camera_driver = Arc::new(camera_driver);

    let mut handles = Vec::new();
    for camera in cameras {
        let camera_driver = Arc::clone(&camera_driver);
        let thread_name = format!("svb-camera-{}", camera.id);
        let handle = thread::Builder::new()
            .name(thread_name.clone())
            //.stack_size(16 * 1024 * 1024)
            .spawn(move || {
                println!("Camera: {} ({})", camera.name, camera.id);

                println!("Connecting...");

                let mut connected_camera = SvbVastCamera::new(Arc::clone(&camera_driver));
                if let Err(e) = connected_camera.connect(camera.id.clone().into()) {
                    eprintln!("Failed to connect to camera: {}", e);
                    return;
                }

                println!("Connected");
                println!("Reading capabilities...");
                let capabilities = connected_camera.get_capabilities();
                println!("{}", capabilities.fancy_info_str());

                println!("Reading settings...");
                match connected_camera.get_camera_settings() {
                    Ok(mut settings) => {
                        println!("{}", settings.fancy_info_str());

                        let mut planned_changes = Vec::new();
                        if let Some(gain) = &capabilities.gain {
                            let value = 100;
                            settings.gain = Some(value);
                            planned_changes.push(format!(
                                "gain={} -> {} (range {}..{})",
                                connected_camera.get_settings().gain.unwrap_or(0),
                                value,
                                gain.min,
                                gain.max
                            ));
                        }
                        if let Some(offset) = &capabilities.offset {
                            let value = 10;
                            settings.offset = Some(value);
                            planned_changes.push(format!(
                                "offset={} -> {} (range {}..{})",
                                connected_camera.get_settings().offset.unwrap_or(0),
                                value,
                                offset.min,
                                offset.max
                            ));
                        }

                        if settings.gain.is_some() || settings.offset.is_some() {
                            println!("Setting test gain/offset: {}", planned_changes.join(", "));
                            if let Err(e) = connected_camera.set_camera_settings(settings) {
                                eprintln!("Failed to set camera settings: {}", e);
                            } else if let Ok(settings) = connected_camera.get_camera_settings() {
                                println!("Settings after test set:\n{}", settings.fancy_info_str());
                            }
                        }
                    }
                    Err(e) => eprintln!("Failed to retrieve camera settings: {}", e),
                }

                println!("Capturing FITS test frame...");
                if let Err(e) = save_test_fits(&mut connected_camera) {
                    eprintln!("Failed to save FITS test frame: {}", e);
                }

                connected_camera.disconnect().unwrap_or_else(|e| {
                    eprintln!("Failed to disconnect from camera: {}", e);
                });
            });

        match handle {
            Ok(handle) => handles.push(handle),
            Err(e) => eprintln!("Failed to start camera worker {thread_name}: {e}"),
        }
    }

    for handle in handles {
        if handle.join().is_err() {
            eprintln!("Camera worker thread panicked");
        }
    }
}
