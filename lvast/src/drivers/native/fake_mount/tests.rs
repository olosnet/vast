use crate::{
    base::{
        connections::{Connection, ConnectionParams},
        errors::VastResult,
    },
    mounts::fake::FakeMount,
    types::{
        common::EquatorialDegrees,
        mount::{
            VastMount, VastMountStatus, VastMountType, VastTrackingMode,
        },
    },
};

struct MockConnection {
    connected: bool,
}

impl Connection for MockConnection {
    fn new(_params: &ConnectionParams) -> VastResult<Self>
    where
        Self: Sized,
    {
        Ok(Self { connected: true })
    }

    fn send(&mut self, _command: &str) -> VastResult<()> {
        Ok(())
    }

    fn receive(&mut self) -> VastResult<String> {
        Ok(String::new())
    }

    fn disconnect(&mut self) {
        self.connected = false;
    }

    fn is_connected(&mut self) -> bool {
        self.connected
    }
}

#[test]
fn fake_mount_tracks_goto_and_manual_motion() {
    let mut mount = FakeMount::new();
    mount.connect(Box::new(MockConnection { connected: true })).unwrap();
    mount.set_mount_type(VastMountType::AltAZ);

    mount
        .goto(EquatorialDegrees { ra: 210.0, dec: 32.5 })
        .unwrap();
    let status = mount.get_current_status().unwrap();

    assert_eq!(mount.mount_type(), VastMountType::AltAZ);
    assert_eq!(status.coords_j2000(), EquatorialDegrees { ra: 210.0, dec: 32.5 });
    assert_eq!(status.status(), VastMountStatus::Tracking);

    mount.move_north().unwrap();
    let status = mount.get_current_status().unwrap();
    assert_eq!(status.status(), VastMountStatus::Slewing);
    assert!(status.altitude() > 57.0);
}

#[test]
fn fake_mount_settings_change_status() {
    let mut mount = FakeMount::new();
    mount.connect(Box::new(MockConnection { connected: true })).unwrap();

    let settings = crate::types::mount::VastMountSettings::new(
        false,
        VastTrackingMode::Off,
        0,
        chrono::Utc::now(),
        60,
        12.5,
        43.0,
    );
    mount.set_settings(settings).unwrap();

    assert_eq!(mount.get_current_status().unwrap().status(), VastMountStatus::Stopped);
    assert_eq!(mount.get_current_settings().unwrap().utc_offset_minutes(), 60);
}
