//! Fake camera wrapper exposing preset-based synthetic sky frames through camera module.

pub use crate::drivers::native::fake_camera::driver::{
    FakeCameraDefectProfile, FakeCameraDriver, FakeCameraFocalPreset, FakeCameraSensorPreset,
    FakeCameraSimulationConfig, FakeCameraSkyFieldPreset, FakeVastCamera,
};
