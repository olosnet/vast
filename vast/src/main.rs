use std::{
    io::{self, Write},
    sync::Arc,
};

use lvast::{
    base::errors::VastError,
    cameras::fake::{
        FakeCameraDefectProfile, FakeCameraDriver, FakeCameraFocalPreset,
        FakeCameraSensorPreset, FakeCameraSkyFieldPreset, FakeVastCamera,
    },
    imageformats::fits::FitsImageSaver,
    types::{
        camera::{VastCamera, VastCameraAcquireImage, VastCameraDriver as _, VastCameraID, VastCameraSettings},
        imageformats::{ImageHeaders, ImageSaver},
    },
};

fn prompt(message: &str) -> io::Result<String> {
    print!("{message}");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

fn map_vast<T>(result: Result<T, VastError>) -> Result<T, Box<dyn std::error::Error>> {
    result.map_err(|err| io::Error::other(err.to_string()).into())
}

fn prompt_with_default(message: &str, default: &str) -> io::Result<String> {
    let input = prompt(&format!("{message} [{default}]: "))?;
    if input.is_empty() {
        Ok(default.to_string())
    } else {
        Ok(input)
    }
}

fn prompt_f64(message: &str, default: f64) -> io::Result<f64> {
    loop {
        let input = prompt_with_default(message, &format!("{default}"))?;
        match input.parse::<f64>() {
            Ok(value) => return Ok(value),
            Err(err) => eprintln!("Invalid number '{input}': {err}"),
        }
    }
}

fn prompt_u64(message: &str, default: u64) -> io::Result<u64> {
    loop {
        let input = prompt_with_default(message, &format!("{default}"))?;
        match input.parse::<u64>() {
            Ok(value) => return Ok(value),
            Err(err) => eprintln!("Invalid integer '{input}': {err}"),
        }
    }
}

fn prompt_u32(message: &str, default: u32) -> io::Result<u32> {
    loop {
        let input = prompt_with_default(message, &format!("{default}"))?;
        match input.parse::<u32>() {
            Ok(value) => return Ok(value),
            Err(err) => eprintln!("Invalid integer '{input}': {err}"),
        }
    }
}

fn choose_sensor_preset() -> io::Result<FakeCameraSensorPreset> {
    let presets = [FakeCameraSensorPreset::Asi294Mc];

    println!("Sensor presets:");
    for (index, preset) in presets.iter().enumerate() {
        println!("  {}. {}", index + 1, preset.label());
    }

    loop {
        let choice = prompt_with_default("Choose sensor preset", "1")?;
        match choice.parse::<usize>() {
            Ok(index) if (1..=presets.len()).contains(&index) => return Ok(presets[index - 1]),
            _ => eprintln!("Choose value in 1..={}", presets.len()),
        }
    }
}

fn choose_focal_preset(sensor: FakeCameraSensorPreset) -> io::Result<FakeCameraFocalPreset> {
    let presets = [
        FakeCameraFocalPreset::Mm135,
        FakeCameraFocalPreset::Mm250,
        FakeCameraFocalPreset::Mm400,
        FakeCameraFocalPreset::Mm600,
        FakeCameraFocalPreset::Mm800,
        FakeCameraFocalPreset::Mm1000,
    ];

    println!("Focal presets:");
    for (index, preset) in presets.iter().enumerate() {
        println!("  {}. {}", index + 1, preset.label(sensor));
    }

    loop {
        let choice = prompt_with_default("Choose focal preset", "3")?;
        match choice.parse::<usize>() {
            Ok(index) if (1..=presets.len()).contains(&index) => return Ok(presets[index - 1]),
            _ => eprintln!("Choose value in 1..={}", presets.len()),
        }
    }
}

fn choose_field_preset() -> io::Result<FakeCameraSkyFieldPreset> {
    let presets = [
        ("M42 Orion", FakeCameraSkyFieldPreset::M42Orion),
        ("M31 Andromeda", FakeCameraSkyFieldPreset::M31Andromeda),
        ("M45 Pleiades", FakeCameraSkyFieldPreset::M45Pleiades),
        ("Vega Lyra", FakeCameraSkyFieldPreset::VegaLyra),
        ("Polaris", FakeCameraSkyFieldPreset::Polaris),
        ("Sadr Cygnus", FakeCameraSkyFieldPreset::SadrCygnus),
        ("No stars (dark frame)", FakeCameraSkyFieldPreset::NoStars),
        ("Flat field", FakeCameraSkyFieldPreset::FlatField),
    ];

    println!("Sky field presets:");
    for (index, (name, preset)) in presets.iter().enumerate() {
        let center = preset.center();
        println!(
            "  {}. {} (RA {:.4} deg, Dec {:.4} deg)",
            index + 1,
            name,
            center.ra,
            center.dec
        );
    }

    loop {
        let choice = prompt_with_default("Choose sky field", "3")?;
        match choice.parse::<usize>() {
            Ok(index) if (1..=presets.len()).contains(&index) => return Ok(presets[index - 1].1),
            _ => eprintln!("Choose value in 1..={}", presets.len()),
        }
    }
}

fn default_output_path(field: FakeCameraSkyFieldPreset, focal: FakeCameraFocalPreset) -> String {
    let field_name = match field {
        FakeCameraSkyFieldPreset::M42Orion => "m42-orion",
        FakeCameraSkyFieldPreset::M31Andromeda => "m31-andromeda",
        FakeCameraSkyFieldPreset::M45Pleiades => "m45-pleiades",
        FakeCameraSkyFieldPreset::VegaLyra => "vega-lyra",
        FakeCameraSkyFieldPreset::Polaris => "polaris",
        FakeCameraSkyFieldPreset::SadrCygnus => "sadr-cygnus",
        FakeCameraSkyFieldPreset::NoStars => "dark",
        FakeCameraSkyFieldPreset::FlatField => "flat",
    };
    format!("fake-camera-{field_name}-{}mm.fits", focal.focal_length_mm() as u32)
}

fn choose_defect_profile() -> io::Result<FakeCameraDefectProfile> {
    let presets = [
        FakeCameraDefectProfile::Clean,
        FakeCameraDefectProfile::Light,
        FakeCameraDefectProfile::Moderate,
        FakeCameraDefectProfile::Heavy,
    ];

    println!("Sensor defect presets:");
    for (index, preset) in presets.iter().enumerate() {
        println!("  {}. {}", index + 1, preset.label());
    }

    loop {
        let choice = prompt_with_default("Choose defect profile", "1")?;
        match choice.parse::<usize>() {
            Ok(index) if (1..=presets.len()).contains(&index) => return Ok(presets[index - 1]),
            _ => eprintln!("Choose value in 1..={}", presets.len()),
        }
    }
}

fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    println!("Fake camera FITS capture tool");
    println!();

    let sensor = choose_sensor_preset()?;
    let focal = choose_focal_preset(sensor)?;
    let field = choose_field_preset()?;
    let defect_profile = choose_defect_profile()?;
    let seeing_arcsec = prompt_f64("Seeing arcsec", 2.5)?;
    let sensor_noise = prompt_f64("Sensor noise sigma", 8.0)?;
    let exposure_millis = prompt_u64("Exposure milliseconds", 1000)?;
    let gain = prompt_u32("Gain", 100)?;
    let offset = prompt_u32("Offset", 512)?;
    let output_path = prompt_with_default("Output FITS path", &default_output_path(field, focal))?;

    let center = field.center();
    let fov = focal.approximate_fov_degrees(sensor);
    println!();
    println!("Capture plan:");
    println!("  Sensor: {}", sensor.label());
    println!("  Focal: {}", focal.label(sensor));
    println!("  Field center: RA {:.4} deg, Dec {:.4} deg", center.ra, center.dec);
    println!("  Approx FOV: {:.2} x {:.2} deg", fov.0, fov.1);
    println!("  Defects: {}", defect_profile.label());
    println!("  Seeing: {:.2} arcsec", seeing_arcsec);
    println!("  Noise sigma: {:.2}", sensor_noise);
    println!("  Exposure: {} ms", exposure_millis);
    println!("  Output: {}", output_path);
    println!();

    let mut driver = FakeCameraDriver::new();
    let cameras = map_vast(driver.init())?;
    let camera_id = cameras
        .first()
        .map(|camera| camera.id.clone())
        .unwrap_or(VastCameraID::IntID(0));
    let driver = Arc::new(driver);

    let mut camera = FakeVastCamera::new(Arc::clone(&driver));
    map_vast(camera.connect(camera_id))?;
    camera.set_sensor_preset(sensor);
    camera.set_focal_preset(focal);
    camera.set_sky_field_preset(field);
    camera.set_defect_profile(defect_profile);
    map_vast(camera.set_seeing_arcsec(seeing_arcsec))?;
    map_vast(camera.set_sensor_noise(sensor_noise))?;
    map_vast(camera.set_camera_settings(VastCameraSettings {
        exposure_microseconds: Some(exposure_millis.saturating_mul(1_000)),
        gain: Some(gain),
        offset: Some(offset),
        ..VastCameraSettings::default()
    }))?;

    map_vast(camera.start_image_acquisition())?;
    let timeout_millis = exposure_millis.saturating_add(5_000).min(u64::from(u32::MAX)) as u32;
    let frame = map_vast(camera.get_acquired_image(timeout_millis))?;

    let config = camera.simulation_config();
    let camera_capabilities = camera.get_capabilities();
    let pixel_scale_arcsec = config.pixel_scale_arcsec_per_pixel(1);
    let headers = ImageHeaders {
        software: Some("vast fake camera interactive test".to_string()),
        image_type: Some(
            if field.is_dark_mode() {
                "Dark"
            } else if field.is_flat_mode() {
                "Flat"
            } else {
                "Light"
            }
            .to_string(),
        ),
        object: Some(format!("{:?}", field)),
        instrument: Some(camera.get_name().to_string()),
        telescope: Some("Fake camera sky preset".to_string()),
        exposure_seconds: Some(exposure_millis as f64 / 1_000.0),
        gain: Some(gain),
        offset: Some(offset),
        ccd_temperature: Some(f64::from(camera.get_current_temperature())),
        frame_width: Some(frame.width),
        frame_height: Some(frame.height),
        pixel_size_x_um: Some(config.pixel_size_um()),
        pixel_size_y_um: Some(config.pixel_size_um()),
        bayer_pattern: camera_capabilities.bayer_pattern.map(|pattern| pattern.to_string()),
        ra_degrees: Some(config.center.ra),
        dec_degrees: Some(config.center.dec),
        pixel_scale_arcsec: Some(pixel_scale_arcsec),
        focal_length_mm: Some(config.focal_length_mm()),
        ..Default::default()
    };

    let saver = FitsImageSaver::new(frame.width, frame.height, frame.format.into());
    map_vast(saver.save(
        frame.data,
        Some(headers.to_fits_headers()),
        output_path.clone(),
    ))?;

    println!("Saved FITS frame: {output_path}");
    Ok(())
}
