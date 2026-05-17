use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use crate::base::errors::{VastError, VastErrorType};
use crate::cameras::types::{
    CameraBayerPattern, CameraFrameFormat, VastCamera, VastCameraAcquireImage,
    VastCameraCapBinning, VastCameraCapCooler, VastCameraCapExposure, VastCameraCapGain,
    VastCameraCapGuiding, VastCameraCapOffset, VastCameraCapRange, VastCameraCapRoi,
    VastCameraCapRoiCombination, VastCameraCapWhiteBalance, VastCameraCapabilities,
    VastCameraDriver, VastCameraFrame, VastCameraGuide, VastCameraGuideDirection, VastCameraID,
    VastCameraInfo, VastCameraSettings, VastCameraStreamingPreview,
};

pub struct SVBVastCameraDriver {
    sdk_lock: Mutex<()>,
}

impl From<crate::drivers::bindings::svb::SVB_BAYER_PATTERN> for CameraBayerPattern {
    fn from(pattern: crate::drivers::bindings::svb::SVB_BAYER_PATTERN) -> Self {
        match pattern {
            crate::drivers::bindings::svb::SVB_BAYER_PATTERN_SVB_BAYER_RG => {
                CameraBayerPattern::RGGB
            }
            crate::drivers::bindings::svb::SVB_BAYER_PATTERN_SVB_BAYER_BG => {
                CameraBayerPattern::BGGR
            }
            crate::drivers::bindings::svb::SVB_BAYER_PATTERN_SVB_BAYER_GR => {
                CameraBayerPattern::GRBG
            }
            crate::drivers::bindings::svb::SVB_BAYER_PATTERN_SVB_BAYER_GB => {
                CameraBayerPattern::GBRG
            }
            _ => CameraBayerPattern::RGGB,
        }
    }
}

impl From<crate::drivers::bindings::svb::SVB_IMG_TYPE> for CameraFrameFormat {
    fn from(value: crate::drivers::bindings::svb::SVB_IMG_TYPE) -> Self {
        match value {
            crate::drivers::bindings::svb::SVB_IMG_TYPE_SVB_IMG_RAW8 => CameraFrameFormat::RAW8,
            crate::drivers::bindings::svb::SVB_IMG_TYPE_SVB_IMG_RAW10 => CameraFrameFormat::RAW10,
            crate::drivers::bindings::svb::SVB_IMG_TYPE_SVB_IMG_RAW12 => CameraFrameFormat::RAW12,
            crate::drivers::bindings::svb::SVB_IMG_TYPE_SVB_IMG_RAW14 => CameraFrameFormat::RAW14,
            crate::drivers::bindings::svb::SVB_IMG_TYPE_SVB_IMG_RAW16 => CameraFrameFormat::RAW16,
            crate::drivers::bindings::svb::SVB_IMG_TYPE_SVB_IMG_RGB24 => CameraFrameFormat::RGB24,
            crate::drivers::bindings::svb::SVB_IMG_TYPE_SVB_IMG_RGB32 => CameraFrameFormat::RGB32,
            crate::drivers::bindings::svb::SVB_IMG_TYPE_SVB_IMG_Y8 => CameraFrameFormat::RAW8,
            crate::drivers::bindings::svb::SVB_IMG_TYPE_SVB_IMG_Y16 => CameraFrameFormat::RAW16,
            _ => CameraFrameFormat::RAW8,
        }
    }
}

fn svb_error_code_to_string(code: u32) -> &'static str {
    match code {
        crate::drivers::bindings::svb::SVB_ERROR_CODE_SVB_ERROR_INVALID_INDEX => "INVALID_INDEX",
        crate::drivers::bindings::svb::SVB_ERROR_CODE_SVB_ERROR_INVALID_ID => "INVALID_ID",
        crate::drivers::bindings::svb::SVB_ERROR_CODE_SVB_ERROR_INVALID_CONTROL_TYPE => {
            "INVALID_CONTROL_TYPE"
        }
        crate::drivers::bindings::svb::SVB_ERROR_CODE_SVB_ERROR_CAMERA_CLOSED => "CAMERA_CLOSED",
        crate::drivers::bindings::svb::SVB_ERROR_CODE_SVB_ERROR_CAMERA_REMOVED => "CAMERA_REMOVED",
        crate::drivers::bindings::svb::SVB_ERROR_CODE_SVB_ERROR_INVALID_PATH => "INVALID_PATH",
        crate::drivers::bindings::svb::SVB_ERROR_CODE_SVB_ERROR_INVALID_FILEFORMAT => {
            "INVALID_FILEFORMAT"
        }
        crate::drivers::bindings::svb::SVB_ERROR_CODE_SVB_ERROR_INVALID_SIZE => "INVALID_SIZE",
        crate::drivers::bindings::svb::SVB_ERROR_CODE_SVB_ERROR_INVALID_IMGTYPE => {
            "INVALID_IMGTYPE"
        }
        crate::drivers::bindings::svb::SVB_ERROR_CODE_SVB_ERROR_OUTOF_BOUNDARY => "OUTOF_BOUNDARY",
        crate::drivers::bindings::svb::SVB_ERROR_CODE_SVB_ERROR_TIMEOUT => "TIMEOUT",
        crate::drivers::bindings::svb::SVB_ERROR_CODE_SVB_ERROR_INVALID_SEQUENCE => {
            "INVALID_SEQUENCE"
        }
        crate::drivers::bindings::svb::SVB_ERROR_CODE_SVB_ERROR_BUFFER_TOO_SMALL => {
            "BUFFER_TOO_SMALL"
        }
        crate::drivers::bindings::svb::SVB_ERROR_CODE_SVB_ERROR_VIDEO_MODE_ACTIVE => {
            "VIDEO_MODE_ACTIVE"
        }
        crate::drivers::bindings::svb::SVB_ERROR_CODE_SVB_ERROR_EXPOSURE_IN_PROGRESS => {
            "EXPOSURE_IN_PROGRESS"
        }
        crate::drivers::bindings::svb::SVB_ERROR_CODE_SVB_ERROR_GENERAL_ERROR => "GENERAL_ERROR",
        crate::drivers::bindings::svb::SVB_ERROR_CODE_SVB_ERROR_INVALID_MODE => "INVALID_MODE",
        crate::drivers::bindings::svb::SVB_ERROR_CODE_SVB_ERROR_INVALID_DIRECTION => {
            "INVALID_DIRECTION"
        }
        crate::drivers::bindings::svb::SVB_ERROR_CODE_SVB_ERROR_UNKNOW_SENSOR_TYPE => "SENSOR_TYPE",
        crate::drivers::bindings::svb::SVB_ERROR_CODE_SVB_ERROR_END => "ERROR_END",
        _ => "UNKNOWN_ERROR",
    }
}

fn svb_control_type_to_string(control_type: u32) -> &'static str {
    match control_type {
        crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_GAIN => "GAIN",
        crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_EXPOSURE => "EXPOSURE",
        crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_BLACK_LEVEL => "BLACK_LEVEL",
        crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_COOLER_ENABLE => "COOLER_ENABLE",
        crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_TARGET_TEMPERATURE => {
            "TARGET_TEMPERATURE"
        }
        crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_CURRENT_TEMPERATURE => {
            "CURRENT_TEMPERATURE"
        }
        crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_BAD_PIXEL_CORRECTION_ENABLE => {
            "BAD_PIXEL_CORRECTION_ENABLE"
        }
        _ => "UNKNOWN_CONTROL",
    }
}

impl VastCameraDriver for SVBVastCameraDriver {
    fn new() -> Self {
        Self {
            sdk_lock: Mutex::new(()),
        }
    }

    fn init(&mut self) -> Result<Vec<VastCameraInfo>, VastError> {
        let _guard = self.sdk_lock.lock().map_err(|_| VastError {
            error_type: VastErrorType::CameraDriverError,
            message: "SVB SDK lock poisoned".to_string(),
        })?;

        unsafe {
            let connected_cameras = crate::drivers::bindings::svb::SVBGetNumOfConnectedCameras();

            let mut cameras = Vec::new();
            for i in 0..connected_cameras {
                let mut camera_info = crate::drivers::bindings::svb::SVB_CAMERA_INFO {
                    FriendlyName: [0; 32usize],
                    CameraSN: [0; 32usize],
                    PortType: [0; 32usize],
                    DeviceID: 0,
                    CameraID: 0,
                };
                let result = crate::drivers::bindings::svb::SVBGetCameraInfo(
                    &mut camera_info,
                    i as ::std::os::raw::c_int,
                );

                if result != 0 {
                    return Err(VastError {
                        error_type: VastErrorType::CameraDriverError,
                        message: format!(
                            "Error initializing camera {}",
                            svb_error_code_to_string(result as u32)
                        ),
                    });
                }

                let camera_info = VastCameraInfo {
                    id: VastCameraID::IntID(camera_info.CameraID),
                    name: std::ffi::CStr::from_ptr(camera_info.FriendlyName.as_ptr())
                        .to_str()
                        .unwrap_or("unknown")
                        .to_string(),
                    serial_number: std::ffi::CStr::from_ptr(camera_info.CameraSN.as_ptr())
                        .to_str()
                        .unwrap_or("unknown")
                        .to_string(),
                    raw_extra_info: "".to_string(),
                };

                cameras.push(camera_info);
            }

            Ok(cameras)
        }
    }

    fn id(&self) -> &str {
        "SVBONY_CAMERA_DRIVER"
    }

    fn get_manufacturer(&self) -> &str {
        "SVBONY"
    }

    fn get_version(&self) -> &str {
        let _guard = self.sdk_lock.lock().unwrap_or_else(|e| e.into_inner());

        unsafe {
            let version = crate::drivers::bindings::svb::SVBGetSDKVersion();

            std::ffi::CStr::from_ptr(version)
                .to_str()
                .unwrap_or("unknown")
        }
    }
}

pub struct SvbVastCamera {
    _driver: Arc<SVBVastCameraDriver>,
    camera_lock: Arc<Mutex<()>>,
    camera_controls: HashMap<u32, crate::drivers::bindings::svb::SVB_CONTROL_CAPS>,

    camera_id: Option<i32>,
    camera_name: String,
    camera_capabilities: VastCameraCapabilities,
    camera_settings: VastCameraSettings,
    camera_is_trigger_cam: bool,
}

fn control_range(control: &crate::drivers::bindings::svb::SVB_CONTROL_CAPS) -> Option<(u32, u32)> {
    Some((
        u32::try_from(control.MinValue).ok()?,
        u32::try_from(control.MaxValue).ok()?,
    ))
}

fn control_step(min: u32, max: u32) -> u32 {
    if max > min {
        1
    } else {
        0
    }
}

fn control_cap_range(
    control: &crate::drivers::bindings::svb::SVB_CONTROL_CAPS,
) -> Option<VastCameraCapRange> {
    let (min, max) = control_range(control)?;
    Some(VastCameraCapRange {
        min,
        max,
        step: control_step(min, max),
    })
}

fn control_is_writable(control: &crate::drivers::bindings::svb::SVB_CONTROL_CAPS) -> bool {
    control.IsWritable != 0
}

fn exposure_range_microseconds(
    control: &crate::drivers::bindings::svb::SVB_CONTROL_CAPS,
) -> Option<(u64, u64)> {
    Some((
        u64::try_from(control.MinValue).ok()?,
        u64::try_from(control.MaxValue).ok()?,
    ))
}

fn supported_bins(bins: &[std::os::raw::c_int]) -> Vec<u32> {
    bins.iter()
        .take_while(|bin| **bin != 0)
        .filter_map(|bin| u32::try_from(*bin).ok())
        .collect()
}

fn roi_combinations(
    max_width: u32,
    max_height: u32,
    bins: &[u32],
) -> Vec<VastCameraCapRoiCombination> {
    bins.iter()
        .copied()
        .filter(|bin| *bin > 0)
        .map(|bin| VastCameraCapRoiCombination {
            bin,
            max_width: max_width / bin,
            max_height: max_height / bin,
            width_step: 8,
            height_step: 2,
        })
        .collect()
}

fn frame_size_bytes(width: u32, height: u32, format: CameraFrameFormat) -> usize {
    let bytes_per_pixel = match format {
        CameraFrameFormat::RAW8 | CameraFrameFormat::RAW10 | CameraFrameFormat::RAW12 => 1,
        CameraFrameFormat::RAW14 | CameraFrameFormat::RAW16 => 2,
        CameraFrameFormat::RGB24 => 3,
        CameraFrameFormat::RGB32 => 4,
    };

    width as usize * height as usize * bytes_per_pixel
}

fn svb_guide_direction(direction: VastCameraGuideDirection) -> i32 {
    match direction {
        VastCameraGuideDirection::North => {
            crate::drivers::bindings::svb::SVB_GUIDE_DIRECTION_SVB_GUIDE_NORTH as i32
        }
        VastCameraGuideDirection::South => {
            crate::drivers::bindings::svb::SVB_GUIDE_DIRECTION_SVB_GUIDE_SOUTH as i32
        }
        VastCameraGuideDirection::East => {
            crate::drivers::bindings::svb::SVB_GUIDE_DIRECTION_SVB_GUIDE_EAST as i32
        }
        VastCameraGuideDirection::West => {
            crate::drivers::bindings::svb::SVB_GUIDE_DIRECTION_SVB_GUIDE_WEST as i32
        }
    }
}

fn svb_result(result: i32, message: &str) -> Result<(), VastError> {
    if result == 0 {
        Ok(())
    } else {
        Err(VastError {
            error_type: VastErrorType::CameraDriverError,
            message: format!("{message}: {}", svb_error_code_to_string(result as u32)),
        })
    }
}

impl VastCamera<i32, SVBVastCameraDriver> for SvbVastCamera {
    fn new(driver: Arc<SVBVastCameraDriver>) -> Self {
        Self {
            _driver: driver,
            camera_lock: Arc::new(Mutex::new(())),
            camera_controls: HashMap::new(),
            camera_id: None,
            camera_name: String::new(),
            camera_capabilities: VastCameraCapabilities::default(),
            camera_settings: VastCameraSettings::default(),
            camera_is_trigger_cam: false,
        }
    }

    fn connect(&mut self, camera_id: i32) -> Result<(), VastError> {
        log::info!("Opening SVB camera: {}", camera_id);

        unsafe {
            let result = {
                let _guard = self._driver.sdk_lock.lock().map_err(|_| VastError {
                    error_type: VastErrorType::CameraDriverError,
                    message: "SVB SDK lock poisoned".to_string(),
                })?;
                crate::drivers::bindings::svb::SVBOpenCamera(camera_id)
            };
            if result != 0 {
                return Err(VastError {
                    error_type: VastErrorType::CameraDriverError,
                    message: format!("SVBOpenCamera failed: {}", result),
                });
            }

            self.camera_id = Some(camera_id);
            let camera_lock = Arc::clone(&self.camera_lock);
            let _guard = camera_lock.lock().map_err(|_| VastError {
                error_type: VastErrorType::CameraDriverError,
                message: "SVB camera lock poisoned".to_string(),
            })?;
            let mut camera_info = crate::drivers::bindings::svb::SVB_CAMERA_INFO {
                FriendlyName: [0; 32usize],
                CameraSN: [0; 32usize],
                PortType: [0; 32usize],
                DeviceID: 0,
                CameraID: 0,
            };
            let mut result =
                crate::drivers::bindings::svb::SVBGetCameraInfo(&mut camera_info, camera_id);
            if result == 0 {
                self.camera_name = std::ffi::CStr::from_ptr(camera_info.FriendlyName.as_ptr())
                    .to_str()
                    .unwrap_or("unknown")
                    .to_string();
            }

            log::info!("Initialize default values...");

            result = crate::drivers::bindings::svb::SVBRestoreDefaultParam(camera_id);
            if result != 0 {
                return Err(VastError {
                    error_type: VastErrorType::CameraDriverError,
                    message: format!("SVBSetDefaultValues failed: {}", result),
                });
            }

            log::info!("Disable driver's autosave...");

            result = crate::drivers::bindings::svb::SVBSetAutoSaveParam(camera_id, 0);
            if result != 0 {
                return Err(VastError {
                    error_type: VastErrorType::CameraDriverError,
                    message: format!("SVBSetAutoSave failed: {}", result),
                });
            }

            log::info!("Get camera properties...");
            let mut p_camera_property =
                std::mem::zeroed::<crate::drivers::bindings::svb::SVB_CAMERA_PROPERTY>();

            result = crate::drivers::bindings::svb::SVBGetCameraProperty(
                camera_id,
                &mut p_camera_property,
            );
            if result != 0 {
                return Err(VastError {
                    error_type: VastErrorType::CameraDriverError,
                    message: format!("SVBGetCameraProperty failed: {}", result),
                });
            }

            if p_camera_property.IsColorCam == 1 {
                self.camera_capabilities.bayer_pattern =
                    Some(p_camera_property.BayerPattern.into());
            }

            self.camera_capabilities.max_height = p_camera_property.MaxHeight as u32;
            self.camera_capabilities.max_width = p_camera_property.MaxWidth as u32;
            self.camera_capabilities.adc_bits = p_camera_property.MaxBitDepth as u32;
            self.camera_is_trigger_cam = p_camera_property.IsTriggerCam == 1;

            let bins = supported_bins(&p_camera_property.SupportedBins);
            if !bins.is_empty() {
                self.camera_capabilities.roi = Some(VastCameraCapRoi {
                    combinations: roi_combinations(
                        self.camera_capabilities.max_width,
                        self.camera_capabilities.max_height,
                        &bins,
                    ),
                });
                self.camera_capabilities.binning = Some(VastCameraCapBinning { modes: bins });
            }

            let mut p_camera_property_ex =
                std::mem::zeroed::<crate::drivers::bindings::svb::SVB_CAMERA_PROPERTY_EX>();
            result = crate::drivers::bindings::svb::SVBGetCameraPropertyEx(
                camera_id,
                &mut p_camera_property_ex,
            );
            if result == 0 {
                self.camera_capabilities.guiding = Some(VastCameraCapGuiding {
                    pulse_guide: p_camera_property_ex.bSupportPulseGuide != 0,
                });
            }

            for format in p_camera_property.SupportedVideoFormat.iter() {
                if *format == crate::drivers::bindings::svb::SVB_IMG_TYPE_SVB_IMG_END {
                    break;
                }

                self.camera_capabilities
                    .frame_formats
                    .push((*format).into());
            }

            let mut pi_number_of_controls: std::os::raw::c_int = 0;
            result = crate::drivers::bindings::svb::SVBGetNumOfControls(
                camera_id,
                &mut pi_number_of_controls,
            );

            if result != 0 {
                return Err(VastError {
                    error_type: VastErrorType::CameraDriverError,
                    message: format!("SVBGetNumOfControls failed: {}", result),
                });
            }

            let mut white_balance_red = None;
            let mut white_balance_green = None;
            let mut white_balance_blue = None;

            for i in 0..pi_number_of_controls {
                let mut p_control_caps =
                    std::mem::zeroed::<crate::drivers::bindings::svb::SVB_CONTROL_CAPS>();
                result = crate::drivers::bindings::svb::SVBGetControlCaps(
                    camera_id,
                    i,
                    &mut p_control_caps,
                );

                if result != 0 {
                    return Err(VastError {
                        error_type: VastErrorType::CameraDriverError,
                        message: format!("SVBGetControl failed: {}", result),
                    });
                }

                log::debug!(
                    "Control\n\t:name: {:?}\n\tdescription: {:?}\n\tmax_value: {:?}\n\tmin_value: {:?}\n\tdefault_value: {:?}\n\tis_auto_supported: {:?}\n\tis_writable: {:?}\n\tcontrol_type: {:?}",
                    p_control_caps.Name,
                    p_control_caps.Description,
                    p_control_caps.MaxValue,
                    p_control_caps.MinValue,
                    p_control_caps.DefaultValue,
                    p_control_caps.IsAutoSupported,
                    p_control_caps.IsWritable,
                    p_control_caps.ControlType,
                );

                self.camera_controls
                    .insert(p_control_caps.ControlType, p_control_caps);

                match p_control_caps.ControlType {
                    crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_EXPOSURE => {
                        if let Some((min, max)) = exposure_range_microseconds(&p_control_caps) {
                            self.camera_capabilities.exposure = VastCameraCapExposure {
                                min_microseconds: min,
                                max_microseconds: max,
                                step: 1,
                            };
                        }
                    }
                    crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_GAIN => {
                        if control_is_writable(&p_control_caps) {
                            if let Some((min, max)) = control_range(&p_control_caps) {
                                self.camera_capabilities.gain = Some(VastCameraCapGain {
                                    min,
                                    max,
                                    step: control_step(min, max),
                                });
                            }
                        }
                    }
                    crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_BLACK_LEVEL => {
                        if control_is_writable(&p_control_caps) {
                            if let Some((min, max)) = control_range(&p_control_caps) {
                                self.camera_capabilities.offset = Some(VastCameraCapOffset {
                                    min,
                                    max,
                                    step: control_step(min, max),
                                });
                            }
                        }
                    }
                    crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_TARGET_TEMPERATURE => {
                        if control_is_writable(&p_control_caps) {
                            self.camera_capabilities.cooler = Some(VastCameraCapCooler {
                                min: p_control_caps.MinValue as f32,
                                max: p_control_caps.MaxValue as f32,
                                step: 1.0,
                            });
                        }
                    }
                    crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_WB_R => {
                        if control_is_writable(&p_control_caps) {
                            white_balance_red = control_cap_range(&p_control_caps);
                        }
                    }
                    crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_WB_G => {
                        if control_is_writable(&p_control_caps) {
                            white_balance_green = control_cap_range(&p_control_caps);
                        }
                    }
                    crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_WB_B => {
                        if control_is_writable(&p_control_caps) {
                            white_balance_blue = control_cap_range(&p_control_caps);
                        }
                    }
                    crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_CONTRAST => {
                        if control_is_writable(&p_control_caps) {
                            self.camera_capabilities.contrast = control_cap_range(&p_control_caps);
                        }
                    }
                    crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_SHARPNESS => {
                        if control_is_writable(&p_control_caps) {
                            self.camera_capabilities.sharpness = control_cap_range(&p_control_caps);
                        }
                    }
                    crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_SATURATION => {
                        if control_is_writable(&p_control_caps) {
                            self.camera_capabilities.saturation =
                                control_cap_range(&p_control_caps);
                        }
                    }
                    crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_FRAME_SPEED_MODE => {
                        if control_is_writable(&p_control_caps) {
                            self.camera_capabilities.usb_speed = control_cap_range(&p_control_caps);
                        }
                    }
                    _ => {}
                }
            }

            if let (Some(red), Some(green), Some(blue)) =
                (white_balance_red, white_balance_green, white_balance_blue)
            {
                self.camera_capabilities.white_balance =
                    Some(VastCameraCapWhiteBalance { red, green, blue });
            }

            // Set bad pixel correction to disabled
            if self.camera_controls.contains_key(
                &crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_BAD_PIXEL_CORRECTION_ENABLE,
            ) {
                result = crate::drivers::bindings::svb::SVBSetControlValue(
                    camera_id,
                    crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_BAD_PIXEL_CORRECTION_ENABLE
                        as i32,
                    0,
                    0,
                );
                if result != 0 {
                    return Err(VastError {
                        error_type: VastErrorType::CameraDriverError,
                        message: format!("SVBSetControlValue failed: {}", result),
                    });
                }
            }

            // INDI applies this SDK workaround before using writable controls.
            crate::drivers::bindings::svb::SVBSetControlValue(
                camera_id,
                crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_EXPOSURE as i32,
                1_000_000,
                0,
            );
            crate::drivers::bindings::svb::SVBSetCameraMode(
                camera_id,
                crate::drivers::bindings::svb::SVB_CAMERA_MODE_SVB_MODE_TRIG_SOFT,
            );
        }

        self.get_camera_settings()?;

        Ok(())
    }

    fn get_name(&self) -> &str {
        &self.camera_name
    }

    fn get_capabilities(&self) -> VastCameraCapabilities {
        self.camera_capabilities.clone()
    }

    fn get_current_offset(&self) -> u32 {
        self.get_control_value(crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_BLACK_LEVEL)
            .unwrap_or(0)
    }

    fn get_current_cooler(&self) -> (bool, u32) {
        let enabled = self
            .get_control_value(crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_COOLER_ENABLE)
            .unwrap_or(0)
            != 0;
        let target = self
            .get_control_value(
                crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_TARGET_TEMPERATURE,
            )
            .unwrap_or(0);

        (enabled, target)
    }

    fn get_current_temperature(&self) -> f32 {
        self.get_control_value(
            crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_CURRENT_TEMPERATURE,
        )
        .map(|temperature| temperature as f32 / 10.0)
        .unwrap_or(0.0)
    }

    fn set_camera_settings(&mut self, settings: VastCameraSettings) -> Result<(), VastError> {
        if self.camera_capabilities.exposure.max_microseconds > 0
            && settings.exposure_microseconds != self.camera_settings.exposure_microseconds
        {
            if let Some(exposure) = settings.exposure_microseconds {
                self.set_control_value_result(
                    crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_EXPOSURE,
                    exposure.try_into().unwrap_or(u32::MAX),
                )?;
            }
        }

        if self.camera_capabilities.gain.is_some() && settings.gain != self.camera_settings.gain {
            if let Some(gain) = settings.gain {
                self.set_control_value_result(
                    crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_GAIN,
                    gain,
                )?;
            }
        }

        if self.camera_capabilities.offset.is_some()
            && settings.offset != self.camera_settings.offset
        {
            if let Some(offset) = settings.offset {
                self.set_control_value_result(
                    crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_BLACK_LEVEL,
                    offset,
                )?;
            }
        }

        if self.camera_capabilities.cooler.is_some()
            && settings.cooler != self.camera_settings.cooler
        {
            if let Some((enabled, temperature)) = settings.cooler {
                self.set_control_value_result(
                    crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_COOLER_ENABLE,
                    u32::from(enabled),
                )?;
                self.set_control_value_result(
                    crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_TARGET_TEMPERATURE,
                    temperature,
                )?;
            }
        }

        if self.camera_capabilities.white_balance.is_some()
            && settings.white_balance != self.camera_settings.white_balance
        {
            if let Some((red, green, blue)) = settings.white_balance {
                self.set_control_value_result(
                    crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_WB_R,
                    red,
                )?;
                self.set_control_value_result(
                    crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_WB_G,
                    green,
                )?;
                self.set_control_value_result(
                    crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_WB_B,
                    blue,
                )?;
            }
        }

        if self.camera_capabilities.contrast.is_some()
            && settings.contrast != self.camera_settings.contrast
        {
            if let Some(contrast) = settings.contrast {
                self.set_control_value_result(
                    crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_CONTRAST,
                    contrast,
                )?;
            }
        }

        if self.camera_capabilities.sharpness.is_some()
            && settings.sharpness != self.camera_settings.sharpness
        {
            if let Some(sharpness) = settings.sharpness {
                self.set_control_value_result(
                    crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_SHARPNESS,
                    sharpness,
                )?;
            }
        }

        if self.camera_capabilities.saturation.is_some()
            && settings.saturation != self.camera_settings.saturation
        {
            if let Some(saturation) = settings.saturation {
                self.set_control_value_result(
                    crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_SATURATION,
                    saturation,
                )?;
            }
        }

        if self.camera_capabilities.usb_speed.is_some()
            && settings.usb_speed != self.camera_settings.usb_speed
        {
            if let Some(usb_speed) = settings.usb_speed {
                self.set_control_value_result(
                    crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_FRAME_SPEED_MODE,
                    usb_speed,
                )?;
            }
        }

        let roi_changed =
            self.camera_capabilities.roi.is_some() && settings.roi != self.camera_settings.roi;
        let binning_changed = self.camera_capabilities.binning.is_some()
            && settings.binning != self.camera_settings.binning;
        if roi_changed || binning_changed {
            let (current_x, current_y, current_width, current_height, current_bin) =
                self.get_current_roi_parts()?;
            let (x, y, width, height) =
                settings
                    .roi
                    .unwrap_or((current_x, current_y, current_width, current_height));
            let bin = settings
                .binning
                .map(|(horizontal, _vertical)| horizontal)
                .unwrap_or(current_bin);
            self.set_roi_format(x, y, width, height, bin)?;
        }

        self.get_camera_settings()?;
        Ok(())
    }

    fn get_camera_settings(&mut self) -> Result<VastCameraSettings, VastError> {
        let mut settings = VastCameraSettings::default();

        if self.camera_capabilities.exposure.max_microseconds > 0 {
            settings.exposure_microseconds = Some(self.current_exposure());
        }

        if self.camera_capabilities.gain.is_some() {
            settings.gain = Some(self.current_gain());
        }

        if self.camera_capabilities.iso.is_some() {
            settings.iso = Some(self.current_iso());
        }

        if self.camera_capabilities.offset.is_some() {
            settings.offset = Some(self.get_current_offset());
        }

        if self.camera_capabilities.cooler.is_some() {
            settings.cooler = Some(self.get_current_cooler());
        }

        if self.camera_capabilities.white_balance.is_some() {
            settings.white_balance = Some((
                self.get_control_value(crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_WB_R)
                    .unwrap_or(0),
                self.get_control_value(crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_WB_G)
                    .unwrap_or(0),
                self.get_control_value(crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_WB_B)
                    .unwrap_or(0),
            ));
        }

        if self.camera_capabilities.contrast.is_some() {
            settings.contrast = self
                .get_control_value(crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_CONTRAST)
                .ok();
        }

        if self.camera_capabilities.sharpness.is_some() {
            settings.sharpness = self
                .get_control_value(crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_SHARPNESS)
                .ok();
        }

        if self.camera_capabilities.saturation.is_some() {
            settings.saturation = self
                .get_control_value(crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_SATURATION)
                .ok();
        }

        if self.camera_capabilities.usb_speed.is_some() {
            settings.usb_speed = self
                .get_control_value(
                    crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_FRAME_SPEED_MODE,
                )
                .ok();
        }

        if self.camera_capabilities.roi.is_some() || self.camera_capabilities.binning.is_some() {
            let (x, y, width, height, bin) = self.get_current_roi_parts()?;
            if self.camera_capabilities.roi.is_some() {
                settings.roi = Some((x, y, width, height));
            }
            if self.camera_capabilities.binning.is_some() {
                settings.binning = Some((bin, bin));
            }
        }

        self.camera_settings = settings.clone();
        Ok(settings)
    }

    fn get_settings(&self) -> VastCameraSettings {
        self.camera_settings.clone()
    }

    fn disconnect(&mut self) -> Result<(), VastError> {
        if let Some(camera_id) = self.camera_id.take() {
            let _guard = self._driver.sdk_lock.lock().map_err(|_| VastError {
                error_type: VastErrorType::CameraDriverError,
                message: "SVB SDK lock poisoned".to_string(),
            })?;
            let result = unsafe { crate::drivers::bindings::svb::SVBCloseCamera(camera_id) };
            svb_result(result, "SVBCloseCamera failed")?;
        }

        Ok(())
    }
}

impl VastCameraAcquireImage for SvbVastCamera {
    fn start_image_acquisition(&mut self) -> Result<(), VastError> {
        let camera_id = self.open_camera_id()?;

        let _guard = self.sdk_guard()?;
        let result = unsafe {
            crate::drivers::bindings::svb::SVBSetCameraMode(
                camera_id,
                crate::drivers::bindings::svb::SVB_CAMERA_MODE_SVB_MODE_TRIG_SOFT,
            )
        };
        svb_result(result, "SVBSetCameraMode failed")?;

        let result = unsafe { crate::drivers::bindings::svb::SVBStartVideoCapture(camera_id) };
        svb_result(result, "SVBStartVideoCapture failed")?;

        let result = unsafe { crate::drivers::bindings::svb::SVBSendSoftTrigger(camera_id) };
        svb_result(result, "SVBSendSoftTrigger failed")
    }

    fn abort_image_acquisition(&mut self) -> Result<(), VastError> {
        let camera_id = self.open_camera_id()?;
        let _guard = self.sdk_guard()?;
        let result = unsafe { crate::drivers::bindings::svb::SVBStopVideoCapture(camera_id) };
        svb_result(result, "SVBStopVideoCapture failed")
    }

    fn get_acquired_image(&mut self, timeout_millis: u32) -> Result<VastCameraFrame, VastError> {
        let frame = self.read_video_frame(timeout_millis)?;
        self.abort_image_acquisition()?;
        Ok(frame)
    }
}

impl VastCameraGuide for SvbVastCamera {
    fn pulse_guide(
        &mut self,
        direction: VastCameraGuideDirection,
        duration_millis: u32,
    ) -> Result<(), VastError> {
        let camera_id = self.open_camera_id()?;
        let _guard = self.sdk_guard()?;
        let result = unsafe {
            crate::drivers::bindings::svb::SVBPulseGuide(
                camera_id,
                svb_guide_direction(direction),
                duration_millis as i32,
            )
        };
        svb_result(result, "SVBPulseGuide failed")
    }
}

impl VastCameraStreamingPreview for SvbVastCamera {
    fn start_streaming_preview(&mut self) -> Result<(), VastError> {
        let camera_id = self.open_camera_id()?;
        let _guard = self.sdk_guard()?;
        let result = unsafe {
            crate::drivers::bindings::svb::SVBSetCameraMode(
                camera_id,
                crate::drivers::bindings::svb::SVB_CAMERA_MODE_SVB_MODE_NORMAL,
            )
        };
        svb_result(result, "SVBSetCameraMode failed")?;

        let result = unsafe { crate::drivers::bindings::svb::SVBStartVideoCapture(camera_id) };
        svb_result(result, "SVBStartVideoCapture failed")
    }

    fn get_streaming_preview_frame(
        &mut self,
        timeout_millis: u32,
    ) -> Result<VastCameraFrame, VastError> {
        self.read_video_frame(timeout_millis)
    }

    fn stop_streaming_preview(&mut self) -> Result<(), VastError> {
        let camera_id = self.open_camera_id()?;
        let _guard = self.sdk_guard()?;
        let result = unsafe { crate::drivers::bindings::svb::SVBStopVideoCapture(camera_id) };
        svb_result(result, "SVBStopVideoCapture failed")
    }
}

impl SvbVastCamera {
    fn sdk_guard(&self) -> Result<MutexGuard<'_, ()>, VastError> {
        self.camera_lock.lock().map_err(|_| VastError {
            error_type: VastErrorType::CameraDriverError,
            message: "SVB camera lock poisoned".to_string(),
        })
    }

    fn open_camera_id(&self) -> Result<i32, VastError> {
        self.camera_id.ok_or_else(|| VastError {
            error_type: VastErrorType::CameraError,
            message: "Camera is not connected".to_string(),
        })
    }

    fn current_image_format(&self) -> Result<CameraFrameFormat, VastError> {
        let camera_id = self.open_camera_id()?;
        let mut image_type = 0;
        let _guard = self.sdk_guard()?;
        let result = unsafe {
            crate::drivers::bindings::svb::SVBGetOutputImageType(camera_id, &mut image_type)
        };
        svb_result(result, "SVBGetOutputImageType failed")?;
        Ok(image_type.into())
    }

    fn read_video_frame(&self, timeout_millis: u32) -> Result<VastCameraFrame, VastError> {
        let camera_id = self.open_camera_id()?;
        let (.., width, height, _) = self.get_current_roi_parts()?;
        let format = self.current_image_format()?;
        let mut data = vec![0; frame_size_bytes(width, height, format)];
        let _guard = self.sdk_guard()?;
        let result = unsafe {
            crate::drivers::bindings::svb::SVBGetVideoData(
                camera_id,
                data.as_mut_ptr(),
                data.len() as i64,
                timeout_millis as i32,
            )
        };
        svb_result(result, "SVBGetVideoData failed")?;

        Ok(VastCameraFrame {
            width,
            height,
            format,
            data,
        })
    }

    fn current_gain(&self) -> u32 {
        self.get_control_value(crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_GAIN)
            .unwrap_or(0)
    }

    fn current_iso(&self) -> u32 {
        0
    }

    fn current_exposure(&self) -> u64 {
        self.get_control_value(crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_EXPOSURE)
            .map(u64::from)
            .unwrap_or(0)
    }

    fn get_control_value(&self, control_type: u32) -> Result<u32, VastError> {
        let Some(camera_id) = self.camera_id else {
            return Ok(0);
        };

        let mut value = 0;
        let mut auto = 0;
        let _guard = self.sdk_guard()?;
        let result = unsafe {
            crate::drivers::bindings::svb::SVBGetControlValue(
                camera_id,
                control_type as i32,
                &mut value,
                &mut auto,
            )
        };
        svb_result(result, "SVBGetControlValue failed")?;

        Ok(value.try_into().unwrap_or(0))
    }

    fn set_control_value_result(&self, control_type: u32, value: u32) -> Result<(), VastError> {
        let Some(camera_id) = self.camera_id else {
            return Ok(());
        };

        let requested_value = value;
        let (value, control_context) = self
            .camera_controls
            .get(&control_type)
            .map(|control| {
                let min = u32::try_from(control.MinValue).unwrap_or(0);
                let max = u32::try_from(control.MaxValue).unwrap_or(u32::MAX);
                let value = value.clamp(min, max);
                (
                    value,
                    format!(
                        "requested={requested_value}, set={value}, range={min}..{max}, writable={}",
                        control.IsWritable
                    ),
                )
            })
            .unwrap_or((
                value,
                format!(
                    "requested={requested_value}, set={value}, range=unknown, writable=unknown"
                ),
            ));

        let _guard = self.sdk_guard()?;
        let result = unsafe {
            crate::drivers::bindings::svb::SVBSetControlValue(
                camera_id,
                control_type as i32,
                value.into(),
                0,
            )
        };
        svb_result(
            result,
            &format!(
                "SVBSetControlValue failed for {} ({control_type}, {control_context})",
                svb_control_type_to_string(control_type),
            ),
        )
    }

    fn set_roi_format(
        &self,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        bin: u32,
    ) -> Result<(), VastError> {
        let Some(camera_id) = self.camera_id else {
            return Ok(());
        };

        let _guard = self.sdk_guard()?;
        let result = unsafe {
            crate::drivers::bindings::svb::SVBSetROIFormat(
                camera_id,
                x as i32,
                y as i32,
                width as i32,
                height as i32,
                bin as i32,
            )
        };
        svb_result(result, "SVBSetROIFormat failed")
    }

    fn get_current_roi_parts(&self) -> Result<(u32, u32, u32, u32, u32), VastError> {
        let Some(camera_id) = self.camera_id else {
            return Ok((0, 0, 0, 0, 1));
        };

        let mut x = 0;
        let mut y = 0;
        let mut width = 0;
        let mut height = 0;
        let mut bin = 0;
        let _guard = self.sdk_guard()?;
        let result = unsafe {
            crate::drivers::bindings::svb::SVBGetROIFormat(
                camera_id,
                &mut x,
                &mut y,
                &mut width,
                &mut height,
                &mut bin,
            )
        };
        svb_result(result, "SVBGetROIFormat failed")?;

        Ok((
            x.try_into().unwrap_or(0),
            y.try_into().unwrap_or(0),
            width.try_into().unwrap_or(0),
            height.try_into().unwrap_or(0),
            bin.try_into().unwrap_or(1),
        ))
    }
}
