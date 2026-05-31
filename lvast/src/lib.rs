//! Core library for camera discovery, camera control, image acquisition, and image export.

/// Astronomical coordinate conversion helpers.
pub mod algos;
/// Common error types shared by library modules.
pub mod base;
/// Camera driver traits, shared camera data types, and concrete camera implementations.
pub mod cameras;
/// Low-level native SDK bindings or native drivers used by devices.
pub mod drivers;
/// Focuser driver traits and concrete focuser implementations.
pub mod focusers;
/// Image format metadata and image saver implementations.
pub mod imageformats;
/// Mount driver traits, shared mount data types and concrete mounts implementations.
pub mod mounts;
/// Plate-solving backends and adapters.
pub mod platesolvers;
pub mod types;
