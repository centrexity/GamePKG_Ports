// Burnout's Duke controller translation for the host-supplied port client.
// This is game logic, with no device provider, OS imports or transport bootstrap.
// Not yet selected by the legacy native launcher: see ../SERVICE_MIGRATION.md.

pub struct Duke {
    pub buttons: u16,
    // Original Xbox order: A, B, X, Y, Black, White, left/right trigger.
    pub analog: [u8; 8],
    pub axes: [i16; 4],
}

pub struct Report {
    pub packet: u32,
    pub pad: Duke,
}

struct Slot {
    device: u64,
    packet: u32,
    pad: Duke,
    valid: bool,
}

// Owner-thread state. No globals or native locks; the caller serializes access.
pub struct Pads {
    slots: [Slot; 4],
}

fn held(buttons: u32, bit: u32) -> u8 {
    if buttons & (1u32 << bit) != 0u32 {
        255
    } else {
        0
    }
}
pub fn duke(raw: gamepkg::Gamepad, mirror_faces: bool) -> Duke {
    let mut pad = Duke {
        // Guide, Misc and paddles are not Duke buttons.
        buttons: (raw.buttons & 0xff) as u16,
        analog: [
            held(raw.buttons, 12),
            held(raw.buttons, 13),
            held(raw.buttons, 14),
            held(raw.buttons, 15),
            held(raw.buttons, 9),
            held(raw.buttons, 8),
            // Preserve the existing native XInput adapter's quantization.
            (raw.left_trigger / 257) as u8,
            (raw.right_trigger / 257) as u8,
        ],
        // Service coordinates already have Y up. No second inversion/deadzone.
        axes: [raw.left_x, raw.left_y, raw.right_x, raw.right_y],
    };
    if mirror_faces {
        pad.analog.swap(0, 1);
        pad.analog.swap(2, 3);
    }
    pad
}

impl Duke {
    pub fn new() -> Self {
        Self {
            buttons: 0,
            analog: [0; 8],
            axes: [0; 4],
        }
    }
    fn copied(&self) -> Self {
        Self {
            buttons: self.buttons,
            analog: self.analog,
            axes: self.axes,
        }
    }
    fn same(&self, other: &Self) -> bool {
        self.buttons == other.buttons && self.analog == other.analog && self.axes == other.axes
    }
}
impl Slot {
    fn new() -> Self {
        Self {
            device: 0,
            packet: 0,
            pad: Duke::new(),
            valid: false,
        }
    }
}
impl Pads {
    pub fn new() -> Self {
        Self {
            slots: [Slot::new(), Slot::new(), Slot::new(), Slot::new()],
        }
    }

    // Read a logical player, never an XInput/SDL hardware slot. Errors propagate
    // without a direct-device fallback. Caller must release cached title state
    // on Err or None, rather than reusing its previous report.
    pub fn read(&mut self, player: u32, mirror_faces: bool) -> Result<Option<Report>, i32> {
        if player >= 4 {
            return Err(-1);
        }
        let raw = gamepkg::gamepad(player);
        self.accept(player, raw, mirror_faces)
    }

    pub fn accept(
        &mut self,
        player: u32,
        raw: Result<gamepkg::Gamepad, i32>,
        mirror_faces: bool,
    ) -> Result<Option<Report>, i32> {
        let slot = self.slots.get_mut(player as usize).ok_or(-1)?;
        let raw = match raw {
            Ok(raw) => raw,
            Err(error) => {
                slot.valid = false;
                return Err(error);
            }
        };
        if raw.device == 0 {
            slot.valid = false;
            return Ok(None);
        }
        let pad = duke(raw, mirror_faces);
        // Timestamp-only changes are not button changes. Device replacement,
        // focus-neutral reports and reconnects must change the title packet.
        if !slot.valid || slot.device != raw.device || !slot.pad.same(&pad) {
            slot.packet = slot.packet.wrapping_add(1);
        }
        slot.device = raw.device;
        slot.pad = pad.copied();
        slot.valid = true;
        Ok(Some(Report {
            packet: slot.packet,
            pad,
        }))
    }

    // input_core retains its existing gain/gate curve and 100 ms refresh.
    // Unsupported output remains Unsupported; no physical slot is guessed.
    pub fn rumble(player: u32, low: u16, high: u16) -> Result<(), i32> {
        if player >= 4 {
            return Err(-1);
        }
        gamepkg::rumble(player, low, high, 250)
    }
}
