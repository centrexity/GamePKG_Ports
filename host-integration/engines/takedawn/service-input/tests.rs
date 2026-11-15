#[path = "pad.rs"]
mod production;
use production::*;
impl Default for Duke {
    fn default() -> Self {
        Self::new()
    }
}
impl Default for Pads {
    fn default() -> Self {
        Self::new()
    }
}
impl PartialEq for Duke {
    fn eq(&self, other: &Self) -> bool {
        self.buttons == other.buttons && self.analog == other.analog && self.axes == other.axes
    }
}
impl std::fmt::Debug for Duke {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Duke")
            .field("buttons", &self.buttons)
            .field("analog", &self.analog)
            .field("axes", &self.axes)
            .finish()
    }
}
impl PartialEq for Report {
    fn eq(&self, other: &Self) -> bool {
        self.packet == other.packet && self.pad == other.pad
    }
}
impl std::fmt::Debug for Report {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Report")
            .field("packet", &self.packet)
            .field("pad", &self.pad)
            .finish()
    }
}

fn raw() -> gamepkg::Gamepad {
    gamepkg::Gamepad {
        device: 10,
        timestamp_ns: 1,
        buttons: 0,
        left_x: 0,
        left_y: 0,
        right_x: 0,
        right_y: 0,
        left_trigger: 0,
        right_trigger: 0,
    }
}

#[test]
fn every_button_combination_preserves_duke_layout() {
    for buttons in 0..=u16::MAX {
        let mut r = raw();
        r.buttons = u32::from(buttons) | 0x03ff_0000;
        let pad = duke(r, false);
        assert_eq!(pad.buttons, buttons & 0xff);
        for (dst, bit) in [(0, 12), (1, 13), (2, 14), (3, 15), (4, 9), (5, 8)] {
            assert_eq!(
                pad.analog[dst],
                if buttons & (1 << bit) != 0 { 255 } else { 0 }
            );
        }
        let mirrored = duke(r, true);
        assert_eq!(
            mirrored.analog[..4],
            [pad.analog[1], pad.analog[0], pad.analog[3], pad.analog[2]]
        );
        assert_eq!(mirrored.analog[4..], pad.analog[4..]);
    }
}

#[test]
fn full_analog_ranges_have_no_extra_deadzone_or_axis_inversion() {
    for value in 0..=u16::MAX {
        let mut r = raw();
        r.left_trigger = value;
        r.right_trigger = u16::MAX - value;
        r.left_x = value as i16;
        r.left_y = value as i16;
        r.right_x = value as i16;
        r.right_y = value as i16;
        let pad = duke(r, false);
        assert_eq!(pad.analog[6], (value / 257) as u8);
        assert_eq!(pad.analog[7], ((u16::MAX - value) / 257) as u8);
        assert_eq!(pad.axes, [value as i16; 4]);
    }
}

#[test]
fn packets_track_controls_focus_device_replacement_and_recovery() {
    let mut pads = Pads::default();
    let mut r = raw();
    let first = pads.accept(0, Ok(r), false).unwrap().unwrap();
    r.timestamp_ns += 1;
    assert_eq!(pads.accept(0, Ok(r), false).unwrap().unwrap(), first);
    r.buttons = 1 << 12;
    let held = pads.accept(0, Ok(r), false).unwrap().unwrap();
    assert_eq!(held.packet, first.packet + 1);
    // Host focus suppression supplies a neutral snapshot with the same ID.
    r.buttons = 0;
    let neutral = pads.accept(0, Ok(r), false).unwrap().unwrap();
    assert_eq!(neutral.pad, Duke::default());
    assert_eq!(neutral.packet, held.packet + 1);
    r.device += 1;
    let replaced = pads.accept(0, Ok(r), false).unwrap().unwrap();
    assert_eq!(replaced.packet, neutral.packet + 1);
    assert_eq!(pads.accept(0, Err(-3), false), Err(-3));
    let recovered = pads.accept(0, Ok(r), false).unwrap().unwrap();
    assert_eq!(recovered.packet, replaced.packet + 1);
    r.device = 0;
    assert_eq!(pads.accept(0, Ok(r), false), Ok(None));
    r.device = 30;
    assert_eq!(
        pads.accept(0, Ok(r), false).unwrap().unwrap().packet,
        recovered.packet + 1
    );
    assert_eq!(pads.accept(1, Ok(r), false).unwrap().unwrap().packet, 1);
}

#[test]
fn invalid_players_and_unbound_transport_fail_closed() {
    let mut pads = Pads::default();
    assert_eq!(pads.read(4, false), Err(-1));
    assert_eq!(Pads::rumble(4, 0, 0), Err(-1));
    // The real generated client is linked, with no pipe bootstrap.
    assert_eq!(pads.read(0, false), Err(-4));
    assert_eq!(Pads::rumble(0, 65535, 65535), Err(-4));
}
