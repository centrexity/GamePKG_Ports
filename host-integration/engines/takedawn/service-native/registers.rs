// Explicit values replacing the original generated functions' TLS globals.
// This is function-call state; it does not fetch or decode instructions.
pub struct Registers {
    pub eax: u32,
    pub ebx: u32,
    pub ecx: u32,
    pub edx: u32,
    pub esi: u32,
    pub edi: u32,
    pub ebp: u32,
    pub esp: u32,
    pub seh_ebp: u32,
    pub df: i32,
    pub fp_top: i32,
    pub fp_cc: u16,
    pub fp_control_word: u16,
    pub fp_stack: [f64; 8],
    pub xmm0: [u32; 4],
    pub xmm1: [u32; 4],
    pub xmm2: [u32; 4],
    pub xmm3: [u32; 4],
    pub xmm4: [u32; 4],
    pub xmm5: [u32; 4],
    pub xmm6: [u32; 4],
    pub xmm7: [u32; 4],
    // Guest TIB/PCR address only. Never a native thread FS register or pointer.
    pub fs_base: u32,
    pub eflags: u32,
    pub fp_cmp: i32,
    pub mm0: [u32; 2],
    pub mm1: [u32; 2],
    pub mm2: [u32; 2],
    pub mm3: [u32; 2],
    pub mm4: [u32; 2],
    pub mm5: [u32; 2],
    pub mm6: [u32; 2],
    pub mm7: [u32; 2],
}
fn register_word(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}
impl Registers {
    pub fn new() -> Self {
        Self {
            eax: 0,
            ebx: 0,
            ecx: 0,
            edx: 0,
            esi: 0,
            edi: 0,
            ebp: 0,
            esp: 0,
            seh_ebp: 0,
            df: 0,
            fp_top: 0,
            fp_cc: 0,
            fp_control_word: 0x037f,
            fp_stack: [0.0; 8],
            xmm0: [0; 4],
            xmm1: [0; 4],
            xmm2: [0; 4],
            xmm3: [0; 4],
            xmm4: [0; 4],
            xmm5: [0; 4],
            xmm6: [0; 4],
            xmm7: [0; 4],
            fs_base: 0,
            eflags: 0x202,
            fp_cmp: 0,
            mm0: [0; 2],
            mm1: [0; 2],
            mm2: [0; 2],
            mm3: [0; 2],
            mm4: [0; 2],
            mm5: [0; 2],
            mm6: [0; 2],
            mm7: [0; 2],
        }
    }
    pub fn valid(&self) -> bool {
        (0..8).contains(&self.fp_top)
    }
    pub fn encode(&self) -> std::vec::Vec<u8> {
        let mut bytes = std::vec::Vec::new();
        for value in [
            self.eax,
            self.ebx,
            self.ecx,
            self.edx,
            self.esi,
            self.edi,
            self.ebp,
            self.esp,
            self.seh_ebp,
        ] {
            bytes.extend(u32::to_le_bytes(value));
        }
        bytes.extend(i32::to_le_bytes(self.df));
        bytes.extend(i32::to_le_bytes(self.fp_top));
        bytes.extend(u16::to_le_bytes(self.fp_cc));
        bytes.extend(u16::to_le_bytes(self.fp_control_word));
        for value in self.fp_stack {
            bytes.extend(f64::to_le_bytes(value));
        }
        for words in [
            self.xmm0, self.xmm1, self.xmm2, self.xmm3, self.xmm4, self.xmm5, self.xmm6, self.xmm7,
        ] {
            for value in words {
                bytes.extend(u32::to_le_bytes(value));
            }
        }
        bytes.extend(u32::to_le_bytes(self.fs_base));
        bytes.extend(u32::to_le_bytes(self.eflags));
        bytes.extend(i32::to_le_bytes(self.fp_cmp));
        for words in [
            self.mm0, self.mm1, self.mm2, self.mm3, self.mm4, self.mm5, self.mm6, self.mm7,
        ] {
            for value in words {
                bytes.extend(u32::to_le_bytes(value));
            }
        }
        bytes
    }
    pub fn decode(&mut self, bytes: &[u8]) -> Result<(), i32> {
        if bytes.len() != 316 {
            return Err(-1);
        }
        let top = register_word(bytes, 40) as i32;
        if !(0..8).contains(&top) {
            return Err(-1);
        }
        self.eax = register_word(bytes, 0);
        self.ebx = register_word(bytes, 4);
        self.ecx = register_word(bytes, 8);
        self.edx = register_word(bytes, 12);
        self.esi = register_word(bytes, 16);
        self.edi = register_word(bytes, 20);
        self.ebp = register_word(bytes, 24);
        self.esp = register_word(bytes, 28);
        self.seh_ebp = register_word(bytes, 32);
        self.df = register_word(bytes, 36) as i32;
        self.fp_top = top;
        self.fp_cc = u16::from_le_bytes([bytes[44], bytes[45]]);
        self.fp_control_word = u16::from_le_bytes([bytes[46], bytes[47]]);
        for index in 0..8 {
            let offset = 48 + index * 8;
            self.fp_stack[index] = f64::from_le_bytes([
                bytes[offset],
                bytes[offset + 1],
                bytes[offset + 2],
                bytes[offset + 3],
                bytes[offset + 4],
                bytes[offset + 5],
                bytes[offset + 6],
                bytes[offset + 7],
            ]);
        }
        for lane in 0..4 {
            self.xmm0[lane] = register_word(bytes, 112 + lane * 4);
        }
        for lane in 0..4 {
            self.xmm1[lane] = register_word(bytes, 128 + lane * 4);
        }
        for lane in 0..4 {
            self.xmm2[lane] = register_word(bytes, 144 + lane * 4);
        }
        for lane in 0..4 {
            self.xmm3[lane] = register_word(bytes, 160 + lane * 4);
        }
        for lane in 0..4 {
            self.xmm4[lane] = register_word(bytes, 176 + lane * 4);
        }
        for lane in 0..4 {
            self.xmm5[lane] = register_word(bytes, 192 + lane * 4);
        }
        for lane in 0..4 {
            self.xmm6[lane] = register_word(bytes, 208 + lane * 4);
        }
        for lane in 0..4 {
            self.xmm7[lane] = register_word(bytes, 224 + lane * 4);
        }
        self.fs_base = register_word(bytes, 240);
        self.eflags = register_word(bytes, 244);
        self.fp_cmp = register_word(bytes, 248) as i32;
        self.mm0 = [register_word(bytes, 252), register_word(bytes, 256)];
        self.mm1 = [register_word(bytes, 260), register_word(bytes, 264)];
        self.mm2 = [register_word(bytes, 268), register_word(bytes, 272)];
        self.mm3 = [register_word(bytes, 276), register_word(bytes, 280)];
        self.mm4 = [register_word(bytes, 284), register_word(bytes, 288)];
        self.mm5 = [register_word(bytes, 292), register_word(bytes, 296)];
        self.mm6 = [register_word(bytes, 300), register_word(bytes, 304)];
        self.mm7 = [register_word(bytes, 308), register_word(bytes, 312)];

        Ok(())
    }
    // The original IRQ epilogue restores all arithmetic state, but deliberately
    // leaves the holder's current TIB/PCR base in place.
    pub fn irq_restore(&mut self, bytes: &[u8]) -> Result<(), i32> {
        let holder_fs = self.fs_base;
        self.decode(bytes)?;
        self.fs_base = holder_fs;
        Ok(())
    }
    pub fn irq_prepare(&mut self, stack_top: u32) {
        self.esp = stack_top;
        self.eax = 0;
        self.ecx = 0;
        self.edx = 0;
        self.ebx = 0;
        self.esi = 0;
        self.edi = 0;
    }
}
