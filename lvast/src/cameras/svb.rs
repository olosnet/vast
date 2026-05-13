use crate::base::errors::{VastError, VastErrorType};
use crate::cameras::traits::{VastCameraDriver, VastCameraID, VastCameraInfo};

pub struct SVBVastCameraDriver;

impl SVBVastCameraDriver {
    pub fn new() -> Self {
        Self
    }
}

fn svb_error_code_to_string(code: u32) -> &'static str {
    match code {
        crate::bindings::svb::SVB_ERROR_CODE_SVB_ERROR_INVALID_INDEX => "INVALID_INDEX",
        crate::bindings::svb::SVB_ERROR_CODE_SVB_ERROR_INVALID_ID => "INVALID_ID",
        crate::bindings::svb::SVB_ERROR_CODE_SVB_ERROR_INVALID_CONTROL_TYPE => {
            "INVALID_CONTROL_TYPE"
        }
        crate::bindings::svb::SVB_ERROR_CODE_SVB_ERROR_CAMERA_CLOSED => "CAMERA_CLOSED",
        crate::bindings::svb::SVB_ERROR_CODE_SVB_ERROR_CAMERA_REMOVED => "CAMERA_REMOVED",
        crate::bindings::svb::SVB_ERROR_CODE_SVB_ERROR_INVALID_PATH => "INVALID_PATH",
        crate::bindings::svb::SVB_ERROR_CODE_SVB_ERROR_INVALID_FILEFORMAT => "INVALID_FILEFORMAT",
        crate::bindings::svb::SVB_ERROR_CODE_SVB_ERROR_INVALID_SIZE => "INVALID_SIZE",
        crate::bindings::svb::SVB_ERROR_CODE_SVB_ERROR_INVALID_IMGTYPE => "INVALID_IMGTYPE",
        crate::bindings::svb::SVB_ERROR_CODE_SVB_ERROR_OUTOF_BOUNDARY => "OUTOF_BOUNDARY",
        crate::bindings::svb::SVB_ERROR_CODE_SVB_ERROR_TIMEOUT => "TIMEOUT",
        crate::bindings::svb::SVB_ERROR_CODE_SVB_ERROR_INVALID_SEQUENCE => "INVALID_SEQUENCE",
        crate::bindings::svb::SVB_ERROR_CODE_SVB_ERROR_BUFFER_TOO_SMALL => "BUFFER_TOO_SMALL",
        crate::bindings::svb::SVB_ERROR_CODE_SVB_ERROR_VIDEO_MODE_ACTIVE => "VIDEO_MODE_ACTIVE",
        crate::bindings::svb::SVB_ERROR_CODE_SVB_ERROR_EXPOSURE_IN_PROGRESS => {
            "EXPOSURE_IN_PROGRESS"
        }
        crate::bindings::svb::SVB_ERROR_CODE_SVB_ERROR_GENERAL_ERROR => "GENERAL_ERROR",
        crate::bindings::svb::SVB_ERROR_CODE_SVB_ERROR_INVALID_MODE => "INVALID_MODE",
        crate::bindings::svb::SVB_ERROR_CODE_SVB_ERROR_INVALID_DIRECTION => "INVALID_DIRECTION",
        crate::bindings::svb::SVB_ERROR_CODE_SVB_ERROR_UNKNOW_SENSOR_TYPE => "SENSOR_TYPE",
        crate::bindings::svb::SVB_ERROR_CODE_SVB_ERROR_END => "ERROR_END",
        _ => "UNKNOWN_ERROR",
    }
}

impl VastCameraDriver for SVBVastCameraDriver {
    fn init(&mut self) -> Result<Vec<VastCameraInfo>, VastError> {
        unsafe {
            let connected_cameras = crate::bindings::svb::SVBGetNumOfConnectedCameras();

            let mut cameras = Vec::new();
            for i in 0..connected_cameras {
                let mut camera_info = crate::bindings::svb::SVB_CAMERA_INFO {
                    FriendlyName: [0; 32usize],
                    CameraSN: [0; 32usize],
                    PortType: [0; 32usize],
                    DeviceID: 0,
                    CameraID: 0,
                };
                let result = crate::bindings::svb::SVBGetCameraInfo(
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
            let version = crate::bindings::svb::SVBGetSDKVersion();

            std::ffi::CStr::from_ptr(version)
                .to_str()
                .unwrap_or("unknown")
        }
    }
}

pub struct SvbVastCamera;
