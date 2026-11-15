// Port-owned raw NV2A -> canonical NVI1 data. No OS, shader compiler or crates.
// The SDK independently validates submitted uniforms. This is not a launcher.

pub const BYTES: usize = 13_072;
pub const ABSENT: u32 = 0x0000_ff0f;
const WORDS: usize = BYTES / 4;
const VS: usize = 804;
const RC: usize = 2980;

pub struct Error {
    pub record: usize,
    pub field: &'static str,
    pub unsupported: bool,
}
fn invalid(record: usize, field: &'static str) -> Error {
    Error {
        record,
        field,
        unsupported: false,
    }
}
fn unsupported(record: usize, field: &'static str) -> Error {
    Error {
        record,
        field,
        unsupported: true,
    }
}

pub enum VertexMode {
    CLIP,
    SCREEN,
}

pub struct Program {
    slots: [[u32; 16]; 136],
    count: usize,
    inputs: u32,
    mode: VertexMode,
}

fn used(mac: u32, ilu: u32) -> [bool; 3] {
    [
        mac != 0,
        (mac == 2 || (mac >= 4 && mac <= 12)),
        (mac == 3 || mac == 4) || ilu != 0,
    ]
}

fn vertex_source(w: [u32; 4], which: usize, record: usize) -> Result<[u32; 4], Error> {
    let (neg, swizzle, reg, mux) = match which {
        0 => ((w[1] >> 8) & 1, w[1] & 255, w[2] >> 28, (w[2] >> 26) & 3),
        1 => (
            (w[2] >> 25) & 1,
            (w[2] >> 17) & 255,
            (w[2] >> 13) & 15,
            (w[2] >> 11) & 3,
        ),
        _ => (
            (w[2] >> 10) & 1,
            (w[2] >> 2) & 255,
            ((w[2] & 3) << 2) | (w[3] >> 30),
            (w[3] >> 28) & 3,
        ),
    };
    if mux == 0 {
        return Ok([0; 4]);
    }
    let index = match mux {
        1 => reg,
        2 => (w[1] >> 9) & 15,
        _ => (w[1] >> 13) & 255,
    };
    if (mux == 1 && index > 12) || (mux == 3 && index > 191) {
        return Err(invalid(record, "source register"));
    }
    let relative = if mux == 3 { (w[3] >> 1) & 1 } else { 0 };
    Ok([mux, index, swizzle, neg | (relative << 1)])
}

fn vertex_destination(
    temp: u32,
    mask: u32,
    output: u32,
    out_mask: u32,
    record: usize,
) -> Result<u32, Error> {
    if mask != 0 && temp > 12 {
        return Err(invalid(record, "destination temporary"));
    }
    if out_mask != 0 && !(output == 0 || (output >= 3 && output <= 12)) {
        return Err(invalid(record, "destination output"));
    }
    Ok((if mask == 0 { 15 } else { temp })
        | (mask << 4)
        | ((if out_mask == 0 { 255 } else { output }) << 8)
        | (out_mask << 16))
}

impl Program {
    // `raw` begins at the selected program start. Stop at FINAL, never at a
    // legacy parser's silent truncation. Trailing inactive slots are ignored.
    pub fn decode(raw: &[[u32; 4]], mode: VertexMode) -> Result<Self, Error> {
        let mut result = Self {
            slots: [[0; 16]; 136],
            count: 0,
            inputs: 0,
            mode,
        };
        if raw.is_empty() {
            return Err(invalid(0, "empty vertex program"));
        }
        for (i, w) in raw.iter().copied().take(136).enumerate() {
            let mac = (w[1] >> 21) & 15;
            let ilu = (w[1] >> 25) & 7;
            if mac > 13 {
                return Err(invalid(i, "reserved MAC opcode"));
            }
            // Word zero is not an instruction field. Do not reinterpret it as
            // the obsolete prose layout in d3d8_vsh_parse.h.
            if w[1] >> 28 != 0 {
                return Err(invalid(i, "reserved vertex word bits"));
            }
            let omask = (w[3] >> 12) & 15;
            if omask != 0 && w[3] & (1 << 11) == 0 {
                return Err(unsupported(i, "constant-write vertex instruction"));
            }
            let temp = (w[3] >> 20) & 15;
            let output = (w[3] >> 3) & 255;
            let output_unit = (w[3] >> 2) & 1;
            let mut slot = [0; 16];
            slot[..4].copy_from_slice(&[mac, ilu, ABSENT, ABSENT]);
            if mac != 0 && mac != 13 {
                slot[2] = vertex_destination(
                    temp,
                    (w[3] >> 24) & 15,
                    output,
                    if output_unit == 0 { omask } else { 0 },
                    i,
                )?;
            }
            if ilu != 0 {
                slot[3] = vertex_destination(
                    if mac != 0 { 1 } else { temp },
                    (w[3] >> 16) & 15,
                    output,
                    if output_unit == 1 { omask } else { 0 },
                    i,
                )?;
            }
            // Reject output writes attached to an inactive/address-only unit;
            // the old translator silently ignores them.
            if omask != 0
                && ((output_unit == 0 && (mac == 0 || mac == 13)) || (output_unit == 1 && ilu == 0))
            {
                return Err(invalid(i, "output of inactive vertex unit"));
            }
            for (operand, consumes) in used(mac, ilu).into_iter().enumerate() {
                if !consumes {
                    continue;
                }
                let s = vertex_source(w, operand, i)?;
                if s[0] == 2 {
                    result.inputs |= 1 << s[1];
                }
                slot[4 + operand * 4..8 + operand * 4].copy_from_slice(&s);
            }
            result.slots[i] = slot;
            result.count = i + 1;
            if w[3] & 1 != 0 {
                return Ok(result);
            }
        }
        Err(invalid(result.count, "missing FINAL within 136 slots"))
    }

    pub fn instruction(&self, index: usize) -> Option<[u32; 16]> {
        if index < self.count {
            Some(self.slots[index])
        } else {
            None
        }
    }
    pub fn instruction_count(&self) -> usize {
        self.count
    }
    pub fn input_mask(&self) -> u32 {
        self.inputs
    }
}

pub struct CombinerRegisters {
    pub rgb_inputs: [u32; 8],
    pub alpha_inputs: [u32; 8],
    pub rgb_outputs: [u32; 8],
    pub alpha_outputs: [u32; 8],
    pub c0: [u32; 8],
    pub c1: [u32; 8],
    pub control: u32,
    pub final_inputs: [u32; 2],
    pub final_c0: u32,
    pub final_c1: u32,
}

impl Default for CombinerRegisters {
    fn default() -> Self {
        Self {
            rgb_inputs: [0; 8],
            alpha_inputs: [0; 8],
            rgb_outputs: [0; 8],
            alpha_outputs: [0; 8],
            c0: [0; 8],
            c1: [0; 8],
            control: 0,
            final_inputs: [0; 2],
            final_c0: 0,
            final_c1: 0,
        }
    }
}
fn general(reg: u32) -> bool {
    reg <= 5 || (8..=13).contains(&reg)
}
fn combiner_input(
    packed: u32,
    final_input: bool,
    special: bool,
    record: usize,
) -> Result<u32, Error> {
    let reg = packed & 15;
    if packed > 255 || !(general(reg) || (special && reg >= 14)) || (final_input && packed >> 5 > 1)
    {
        return Err(invalid(record, "combiner input selector/mapping"));
    }
    Ok(packed)
}
fn combiner_output(raw: u32, alpha: bool, record: usize) -> Result<[u32; 4], Error> {
    let allowed = if alpha {
        0xfff | (1 << 14) | (7 << 15)
    } else {
        0xfffff
    };
    let map = (raw >> 15) & 7;
    if raw & !allowed != 0 || !(map <= 4 || map == 6) {
        return Err(invalid(record, "reserved combiner output bits/mapping"));
    }
    let ab = (raw >> 4) & 15;
    let cd = raw & 15;
    let sum = (raw >> 8) & 15;
    if !general(ab) || !general(cd) || !general(sum) {
        return Err(invalid(record, "combiner output register"));
    }
    let flags = ((raw >> 13) & 1)
        | (((raw >> 12) & 1) << 1)
        | (((raw >> 14) & 1) << 2)
        | (((raw >> 19) & 1) << 3)
        | (((raw >> 18) & 1) << 4)
        | (map << 8);
    Ok([ab, cd, sum, flags])
}
fn bits(value: [f32; 4]) -> [u32; 4] {
    [
        f32::to_bits(value[0]),
        f32::to_bits(value[1]),
        f32::to_bits(value[2]),
        f32::to_bits(value[3]),
    ]
}
fn has_nan(value: [f32; 4]) -> bool {
    for x in value {
        if x.is_nan() {
            return true;
        }
    }
    false
}
fn finite(value: [f32; 4]) -> bool {
    for x in value {
        if !x.is_finite() {
            return false;
        }
    }
    true
}
fn viewport_nan(value: [[u32; 4]; 5]) -> bool {
    for word in value {
        for x in word {
            if f32::from_bits(x).is_nan() {
                return true;
            }
        }
    }
    false
}
fn argb_bits(raw: u32) -> [u32; 4] {
    bits([
        ((raw >> 16) & 255) as f32 / 255.0,
        ((raw >> 8) & 255) as f32 / 255.0,
        (raw & 255) as f32 / 255.0,
        ((raw >> 24) & 255) as f32 / 255.0,
    ])
}

pub struct Block {
    words: [u32; WORDS],
}

impl Default for Block {
    fn default() -> Self {
        Self::fixed()
    }
}

impl Block {
    // Fixed pretransformed vertices and the existing basic modulation path.
    pub fn fixed() -> Self {
        let mut words = [0; WORDS];
        words[..16].copy_from_slice(&[
            0x3149564e,
            1,
            BYTES as u32,
            0,
            0,
            0,
            0x1e19,
            6,
            0,
            0,
            0,
            0,
            0,
            8,
            0,
            0,
        ]);
        for slot in 0..4 {
            words[3212 + slot * 4..3216 + slot * 4].copy_from_slice(&[
                1f32.to_bits(),
                1f32.to_bits(),
                0,
                0,
            ]);
        }
        words[3264..].copy_from_slice(&[1f32.to_bits(), 1f32.to_bits(), 1f32.to_bits(), 0]);
        Self { words }
    }

    pub fn programmed(program: &Program) -> Self {
        let mut result = Self::fixed();
        result.words[4] = program.count as u32;
        result.words[6] = program.inputs;
        result.words[7] = 4 | match program.mode {
            VertexMode::SCREEN => 1,
            VertexMode::CLIP => 0,
        };
        for i in 0..program.count {
            result.words[VS + i * 16..VS + (i + 1) * 16].copy_from_slice(&program.slots[i]);
        }
        result
    }

    // Validate into a separate value: on any error the previous block survives.
    pub fn with_combiners(&self, raw: &CombinerRegisters) -> Result<Self, Error> {
        let count = (raw.control & 255) as usize;
        if raw.control & !0x111ff != 0 || count > 8 {
            return Err(invalid(count, "combiner count/control"));
        }
        if raw.final_inputs[1] & 0x1f != 0 {
            return Err(invalid(0, "reserved final-combiner flags"));
        }
        let mut result = Self { words: self.words };
        result.words[3] = raw.final_inputs[1] & 0xe0;
        result.words[5] = count as u32;
        result.words[7] &= !4;
        result.words[8..11].copy_from_slice(&[
            (raw.control >> 12) & 1,
            (raw.control >> 16) & 1,
            (raw.control >> 8) & 1,
        ]);
        result.words[RC..RC + 128].fill(0);
        for i in 0..count {
            let at = RC + i * 16;
            for k in 0..4 {
                result.words[at + k] =
                    combiner_input((raw.rgb_inputs[i] >> (24 - 8 * k)) & 255, false, false, i)?;
                result.words[at + 4 + k] =
                    combiner_input((raw.alpha_inputs[i] >> (24 - 8 * k)) & 255, false, false, i)?;
            }
            result.words[at + 8..at + 12].copy_from_slice(&combiner_output(
                raw.rgb_outputs[i],
                false,
                i,
            )?);
            result.words[at + 12..at + 16].copy_from_slice(&combiner_output(
                raw.alpha_outputs[i],
                true,
                i,
            )?);
        }
        for k in 0..7 {
            let word = raw.final_inputs[if k < 4 { 0 } else { 1 }];
            let value = (word >> (24 - 8 * (k % 4))) & 255;
            result.words[3108 + k] = combiner_input(value, true, k != 4 && k != 5, k)?;
        }
        for i in 0..8 {
            result.words[3116 + i * 4..3120 + i * 4].copy_from_slice(&argb_bits(raw.c0[i]));
            result.words[3148 + i * 4..3152 + i * 4].copy_from_slice(&argb_bits(raw.c1[i]));
        }
        result.words[3180..3184].copy_from_slice(&argb_bits(raw.final_c0));
        result.words[3184..3188].copy_from_slice(&argb_bits(raw.final_c1));
        Ok(result)
    }

    pub fn set_vertex_constant(&mut self, index: usize, value: [f32; 4]) -> Result<(), Error> {
        if index >= 192 || has_nan(value) {
            return Err(invalid(index, "vertex constant"));
        }
        self.words[16 + index * 4..20 + index * 4].copy_from_slice(&bits(value));
        Ok(())
    }

    // Five words in the pinned viewport/fog/specular layout. Controls keep
    // their u32 bits; floating words preserve IEEE bits without native casts.
    pub fn set_viewport(&mut self, value: [[u32; 4]; 5]) -> Result<(), Error> {
        // Current host checks all five words as float bits for NaNs, including
        // the integer controls. Keep the same structural admission behavior.
        if viewport_nan(value) {
            return Err(invalid(0, "viewport NaN"));
        }
        for (i, word) in value.iter().enumerate() {
            self.words[784 + i * 4..788 + i * 4].copy_from_slice(word);
        }
        Ok(())
    }

    pub fn set_alpha(&mut self, reference: f32, function: u32, enabled: bool) -> Result<(), Error> {
        if !reference.is_finite()
            || !(0.0..=1.0).contains(&reference)
            || !(1..=8).contains(&function)
        {
            return Err(invalid(0, "alpha test"));
        }
        self.words[12..15].copy_from_slice(&[reference.to_bits(), function, u32::from(enabled)]);
        Ok(())
    }

    pub fn set_fog(
        &mut self,
        color: [f32; 4],
        vertex_factor: bool,
        enabled: bool,
    ) -> Result<(), Error> {
        if has_nan(color) {
            return Err(invalid(0, "fog color"));
        }
        self.words[3188..3192].copy_from_slice(&bits(color));
        self.words[11] = u32::from(vertex_factor);
        self.words[15] = u32::from(enabled);
        Ok(())
    }

    pub fn set_texture(&mut self, slot: usize, value: &Texture) -> Result<(), Error> {
        if slot >= 4 {
            return Err(invalid(slot, "texture slot"));
        }
        value.validate(slot)?;
        for (at, v) in [
            (3192, value.mode),
            (3196, value.signed_channels),
            (3200, value.previous),
            (3204, value.clip),
            (3208, u32::from(value.alpha_only)),
        ] {
            self.words[at + slot] = v;
        }
        for (at, v) in [
            (3212 + slot * 4, value.scale),
            (3232 + slot * 4, value.bump),
            (3248 + slot * 4, value.luminance),
        ] {
            self.words[at..at + 4].copy_from_slice(&bits(v));
        }
        self.words[3228 + slot] = value.lod_bias.to_bits();
        Ok(())
    }

    pub fn encode(&self, cube_mask: u32) -> Result<[u8; BYTES], Error> {
        if cube_mask > 15 {
            return Err(invalid(0, "cube-slot mask"));
        }
        if self.words[7] & 4 != 0
            && !(self.words[3192] == 0 || self.words[3192] == 1 || self.words[3192] == 4)
        {
            return Err(unsupported(0, "basic fragment texture mode"));
        }
        for slot in 0..4 {
            let mode = self.words[3192 + slot];
            let cube = cube_mask & (1 << slot) != 0;
            if (mode == 3 && !cube) || ((mode == 1 || mode == 6 || mode == 7) && cube) {
                return Err(invalid(slot, "texture/template dimensionality"));
            }
        }
        let mut bytes = [0; BYTES];
        for (i, word) in self.words.iter().enumerate() {
            bytes[i * 4..i * 4 + 4].copy_from_slice(&u32::to_le_bytes(*word));
        }
        Ok(bytes)
    }
}

pub struct Texture {
    pub mode: u32,
    pub signed_channels: u32,
    pub previous: u32,
    pub clip: u32,
    pub alpha_only: bool,
    pub scale: [f32; 4],
    pub lod_bias: f32,
    pub bump: [f32; 4],
    pub luminance: [f32; 4],
}
impl Default for Texture {
    fn default() -> Self {
        Self {
            mode: 0,
            signed_channels: 0,
            previous: 0,
            clip: 0,
            alpha_only: false,
            scale: [1.0, 1.0, 0.0, 0.0],
            lod_bias: 0.0,
            bump: [0.0; 4],
            luminance: [0.0; 4],
        }
    }
}
impl Texture {
    fn validate(&self, slot: usize) -> Result<(), Error> {
        if self.mode > 7
            || self.signed_channels > 15
            || self.previous > 3
            || self.clip > 15
            || (self.mode != 5 && self.clip != 0)
        {
            return Err(invalid(slot, "texture mode/flags"));
        }
        if self.mode == 2 || ((self.mode == 6 || self.mode == 7) && slot == 0) {
            return Err(unsupported(slot, "volume/stage-zero bump"));
        }
        if (self.mode == 6 || self.mode == 7) && self.previous >= slot as u32 {
            return Err(invalid(slot, "dependent texture predecessor"));
        }
        if (!finite(self.scale) || !finite(self.bump) || !finite(self.luminance))
            || !self.lod_bias.is_finite()
        {
            return Err(invalid(slot, "nonfinite texture parameters"));
        }
        Ok(())
    }
}
