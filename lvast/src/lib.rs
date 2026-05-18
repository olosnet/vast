//! Core library for camera discovery, camera control, image acquisition, and image export.

/// Common error types shared by library modules.
pub mod base;
/// Camera driver traits, shared camera data types, and concrete camera implementations.
pub mod cameras;
/// Low-level native SDK bindings or native drivers used by devices.
pub mod drivers;
/// Image format metadata and image saver implementations.
pub mod imageformats;
