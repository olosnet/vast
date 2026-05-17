use std::sync::Arc;

use lvast::cameras::{
    svb::SvbVastCamera,
    types::{VastCamera, VastCameraDriver as _},
};

fn nearby_test_value(current: Option<u32>, min: u32, max: u32) -> u32 {
    let current = current.unwrap_or(min).clamp(min, max);
    if current < max {
        current + 1
    } else {
        current.saturating_sub(1).max(min)
    }
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

    for camera in cameras.iter() {
        println!("Camera: {} ({})", camera.name, camera.id);

        println!("Connecting...");

        let mut connected_camera = SvbVastCamera::new(Arc::clone(&camera_driver));
        if let Err(e) = connected_camera.connect(camera.id.clone().into()) {
            eprintln!("Failed to connect to camera: {}", e);
            continue;
        }

        println!("Connected");
        let capabilities = connected_camera.get_capabilities();
        println!("{}", capabilities.fancy_info_str());

        match connected_camera.get_camera_settings() {
            Ok(mut settings) => {
                println!("{}", settings.fancy_info_str());

                let mut planned_changes = Vec::new();
                if let Some(gain) = &capabilities.gain {
                    let value = nearby_test_value(settings.gain, gain.min, gain.max);
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
                    let value = nearby_test_value(settings.offset, offset.min, offset.max);
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

        connected_camera.disconnect().unwrap_or_else(|e| {
            eprintln!("Failed to disconnect from camera: {}", e);
        });
    }
}
