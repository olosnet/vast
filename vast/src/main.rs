use lvast::cameras::traits::VastCameraDriver as _;

fn main() {
    let mut camera_driver = lvast::cameras::svb::SVBVastCameraDriver::new();
    println!("SVB SDK Version: {}", camera_driver.get_version());

    let cameras = camera_driver.init().unwrap_or_else(|e| {
        eprintln!("Failed to initialize camera driver: {}", e);
        std::process::exit(1);
    });

    println!("Found {} cameras", cameras.len());
    for camera in cameras.iter() {
        println!("Camera: {} ({})", camera.name, camera.id);
    }
}
