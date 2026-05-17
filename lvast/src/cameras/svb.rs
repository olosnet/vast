use std::collections::HashMap;
use std::sync::Arc;

use crate::base::errors::{VastError, VastErrorType};
use crate::cameras::types::{
    CameraBayerPattern, CameraFrameFormat, VastCamera, VastCameraCapabilities, VastCameraCooler,
    VastCameraDriver, VastCameraGain, VastCameraID, VastCameraInfo, VastCameraOffset,
    fancy_info_str,
};

pub struct SVBVastCameraDriver;

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

impl VastCameraDriver for SVBVastCameraDriver {
    fn new() -> Self {
        Self
    }

    fn init(&mut self) -> Result<Vec<VastCameraInfo>, VastError> {
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
    camera_controls: HashMap<u32, crate::drivers::bindings::svb::SVB_CONTROL_CAPS>,

    camera_id: Option<i32>,
    camera_name: String,
    camera_capabilities: VastCameraCapabilities,
    camera_is_trigger_cam: bool,
}

fn control_range(control: &crate::drivers::bindings::svb::SVB_CONTROL_CAPS) -> Option<(u32, u32)> {
    Some((
        u32::try_from(control.MinValue).ok()?,
        u32::try_from(control.MaxValue).ok()?,
    ))
}

fn control_step(min: u32, max: u32) -> u32 {
    if max > min { 1 } else { 0 }
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
            camera_controls: HashMap::new(),
            camera_id: None,
            camera_name: String::new(),
            camera_capabilities: VastCameraCapabilities::default(),
            camera_is_trigger_cam: false,
        }
    }

    fn connect(&mut self, camera_id: i32) -> Result<(), VastError> {
        log::info!("Opening SVB camera: {}", camera_id);

        unsafe {
            let mut result = crate::drivers::bindings::svb::SVBOpenCamera(camera_id);
            if result != 0 {
                return Err(VastError {
                    error_type: VastErrorType::CameraDriverError,
                    message: format!("SVBOpenCamera failed: {}", result),
                });
            }

            self.camera_id = Some(camera_id);
            let mut camera_info = crate::drivers::bindings::svb::SVB_CAMERA_INFO {
                FriendlyName: [0; 32usize],
                CameraSN: [0; 32usize],
                PortType: [0; 32usize],
                DeviceID: 0,
                CameraID: 0,
            };
            result = crate::drivers::bindings::svb::SVBGetCameraInfo(&mut camera_info, camera_id);
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
            self.camera_is_trigger_cam = p_camera_property.IsTriggerCam == 1;

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
                    crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_GAIN => {
                        if let Some((min, max)) = control_range(&p_control_caps) {
                            self.camera_capabilities.gain = Some(VastCameraGain {
                                min,
                                max,
                                step: control_step(min, max),
                            });
                        }
                    }
                    crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_BLACK_LEVEL => {
                        if let Some((min, max)) = control_range(&p_control_caps) {
                            self.camera_capabilities.offset = Some(VastCameraOffset {
                                min,
                                max,
                                step: control_step(min, max),
                            });
                        }
                    }
                    crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_TARGET_TEMPERATURE => {
                        self.camera_capabilities.cooler = Some(VastCameraCooler {
                            min: p_control_caps.MinValue as f32,
                            max: p_control_caps.MaxValue as f32,
                            step: 1.0,
                        });
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }

    fn camera_info_str(&self) -> String {
        fancy_info_str(&self.camera_capabilities)
    }

    fn get_name(&self) -> &str {
        &self.camera_name
    }

    fn get_capabilities(&self) -> VastCameraCapabilities {
        self.camera_capabilities.clone()
    }

    fn get_bayer_pattern(&self) -> &Option<CameraBayerPattern> {
        &self.camera_capabilities.bayer_pattern
    }

    fn get_max_height(&self) -> u32 {
        self.camera_capabilities.max_height
    }

    fn get_max_width(&self) -> u32 {
        self.camera_capabilities.max_width
    }

    fn get_current_binning(&self) -> Result<(u32, u32), VastError> {
        let (_, _, _, _, bin) = self.get_current_roi_parts()?;
        Ok((bin, bin))
    }

    fn get_current_roi(&self) -> Result<(u32, u32, u32, u32), VastError> {
        let (x, y, width, height, _) = self.get_current_roi_parts()?;
        Ok((x, y, width, height))
    }

    fn get_current_gain(&self) -> u32 {
        self.get_control_value(crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_GAIN)
            .unwrap_or(0)
    }

    fn get_current_iso(&self) -> u32 {
        0
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

    fn set_gain(&mut self, gain: u32) {
        self.set_control_value(
            crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_GAIN,
            gain,
        );
    }

    fn set_iso(&mut self, _iso: u32) {}

    fn set_offset(&mut self, offset: u32) {
        self.set_control_value(
            crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_BLACK_LEVEL,
            offset,
        );
    }

    fn set_cooler(&mut self, on: bool, temperature: u32) {
        self.set_control_value(
            crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_COOLER_ENABLE,
            u32::from(on),
        );
        self.set_control_value(
            crate::drivers::bindings::svb::SVB_CONTROL_TYPE_SVB_TARGET_TEMPERATURE,
            temperature,
        );
    }

    fn set_roi(&mut self, x: u32, y: u32, width: u32, height: u32) {
        if let Some(camera_id) = self.camera_id {
            let bin = self
                .get_current_roi_parts()
                .map(|(_, _, _, _, bin)| bin)
                .unwrap_or(1);
            unsafe {
                crate::drivers::bindings::svb::SVBSetROIFormat(
                    camera_id,
                    x as i32,
                    y as i32,
                    width as i32,
                    height as i32,
                    bin as i32,
                );
            }
        }
    }

    fn set_binning(&mut self, h: u32, _v: u32) {
        if let Some(camera_id) = self.camera_id {
            let (x, y, width, height, _) = self.get_current_roi_parts().unwrap_or((
                0,
                0,
                self.camera_capabilities.max_width,
                self.camera_capabilities.max_height,
                1,
            ));
            unsafe {
                crate::drivers::bindings::svb::SVBSetROIFormat(
                    camera_id,
                    x as i32,
                    y as i32,
                    width as i32,
                    height as i32,
                    h as i32,
                );
            }
        }
    }

    fn disconnect(&mut self) -> Result<(), VastError> {
        if let Some(camera_id) = self.camera_id.take() {
            let result = unsafe { crate::drivers::bindings::svb::SVBCloseCamera(camera_id) };
            svb_result(result, "SVBCloseCamera failed")?;
        }

        Ok(())
    }
}

impl SvbVastCamera {
    fn get_control_value(&self, control_type: u32) -> Result<u32, VastError> {
        let Some(camera_id) = self.camera_id else {
            return Ok(0);
        };

        let mut value = 0;
        let mut auto = 0;
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

    fn set_control_value(&self, control_type: u32, value: u32) {
        if let Some(camera_id) = self.camera_id {
            unsafe {
                crate::drivers::bindings::svb::SVBSetControlValue(
                    camera_id,
                    control_type as i32,
                    value.into(),
                    0,
                );
            }
        }
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
