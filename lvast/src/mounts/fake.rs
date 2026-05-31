//! Fake mount wrapper exposing in-memory mount behavior through mount module.

use crate::{
    base::{connections::Connection, errors::VastResult},
    drivers::native::fake_mount::driver::FakeMount as NativeFakeMount,
    types::{common::EquatorialDegrees, mount::{VastMount, VastMountCurrStatus, VastMountSettings}},
};

pub use crate::drivers::native::fake_mount::driver::FakeMount;

impl VastMount for NativeFakeMount {
    fn new() -> Self {
        Self::new()
    }

    fn connect(&mut self, connection: Box<dyn Connection>) -> VastResult<()> {
        self.connect_inner(connection)
    }

    fn get_name(&mut self) -> String {
        self.get_name_inner()
    }

    fn get_version(&mut self) -> String {
        self.get_version_inner()
    }

    fn get_current_settings(&mut self) -> VastResult<VastMountSettings> {
        self.get_current_settings_inner()
    }

    fn get_current_status(&mut self) -> VastResult<VastMountCurrStatus> {
        self.get_current_status_inner()
    }

    fn goto(&mut self, coords_j2000: EquatorialDegrees) -> VastResult<()> {
        self.goto_inner(coords_j2000)
    }

    fn goto_home(&mut self) -> VastResult<()> {
        self.goto_home_inner()
    }

    fn set_settings(&mut self, settings: VastMountSettings) -> VastResult<()> {
        self.set_settings_inner(settings)
    }

    fn stop(&mut self) -> VastResult<()> {
        self.stop_inner()
    }

    fn move_east(&mut self) -> VastResult<()> {
        self.move_east_inner()
    }

    fn move_west(&mut self) -> VastResult<()> {
        self.move_west_inner()
    }

    fn move_north(&mut self) -> VastResult<()> {
        self.move_north_inner()
    }

    fn move_south(&mut self) -> VastResult<()> {
        self.move_south_inner()
    }

    fn disconnect(&mut self) -> VastResult<()> {
        self.disconnect_inner()
    }
}
