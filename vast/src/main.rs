use std::sync::Arc;

use lvast::cameras::{
    svb::SvbVastCamera,
    types::{VastCamera, VastCameraDriver as _},
};

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
        println!("{}", connected_camera.camera_info_str());

        connected_camera.disconnect().unwrap_or_else(|e| {
            eprintln!("Failed to disconnect from camera: {}", e);
        });
    }
}
