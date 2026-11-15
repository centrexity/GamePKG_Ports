// Port-owned guest-address translation over GamePKG-owned RAM windows.
// Guest numbers are never converted into host pointers. No code bytes execute.
use crate::gamepkg;
struct Span {
    guest: u32,
    bytes: u64,
    window: u64,
    offset: u64,
    writable: bool,
}
pub struct Memory {
    spans: std::vec::Vec<Span>,
    calls: u64,
}
impl Memory {
    pub fn new() -> Result<Self, i32> {
        let initial = crate::call_state::CallState::call_default().call_encode();
        Ok(Self {
            spans: std::vec::Vec::new(),
            calls: gamepkg::local_record_create(&initial)?,
        })
    }
    // One title's views/tasks explicitly share an opaque GamePKG record ID.
    // The record lives until explicit release or local-runtime revocation.
    pub fn call_view(&self) -> Self {
        Self {
            spans: std::vec::Vec::new(),
            calls: self.calls,
        }
    }
    pub fn call_handle(&self) -> u64 {
        self.calls
    }
    pub fn with_call_handle(record: u64) -> Result<Self, i32> {
        let (_, bytes) = gamepkg::local_record_snapshot(record, 280)?;
        crate::call_state::CallState::call_decode(&bytes)?;
        Ok(Self {
            spans: std::vec::Vec::new(),
            calls: record,
        })
    }
    pub fn call_bounds(&self, low: u32, high: u32) -> Result<(), i32> {
        for _ in 0..64 {
            let (version, bytes) = gamepkg::local_record_snapshot(self.calls, 280)?;
            let mut state = crate::call_state::CallState::call_decode(&bytes)?;
            state.call_set_bounds(low, high)?;
            match gamepkg::local_record_compare_exchange(self.calls, version, &state.call_encode())
            {
                Ok(_) => return Ok(()),
                Err(-2) => {}
                Err(error) => return Err(error),
            }
        }
        Err(-2)
    }
    pub fn call_begin(&self, target: u32) -> Result<bool, i32> {
        for _ in 0..64 {
            let (version, bytes) = gamepkg::local_record_snapshot(self.calls, 280)?;
            let mut state = crate::call_state::CallState::call_decode(&bytes)?;
            let call_is_code = state.call_record(target);
            let values = if call_is_code {
                None
            } else {
                state.call_noncode_values(target)
            };
            match gamepkg::local_record_compare_exchange(self.calls, version, &state.call_encode())
            {
                Ok(_) => {
                    if let Some(words) = values {
                        gamepkg::diagnostic(2, 0x544b0001, &words)?;
                    }
                    return Ok(call_is_code);
                }
                Err(-2) => {}
                Err(error) => return Err(error),
            }
        }
        Err(-2)
    }
    pub fn call_absent(&self, target: u32) -> Result<(), i32> {
        let (_, bytes) = gamepkg::local_record_snapshot(self.calls, 280)?;
        let state = crate::call_state::CallState::call_decode(&bytes)?;
        gamepkg::diagnostic(2, 0x544b0002, &state.call_missing_values(target))?;
        Ok(())
    }
    pub fn call_snapshot(&self) -> Result<([u32; 16], u32, u64), i32> {
        let (_, bytes) = gamepkg::local_record_snapshot(self.calls, 280)?;
        let state = crate::call_state::CallState::call_decode(&bytes)?;
        let (index, total) = state.call_counters();
        Ok((state.call_trace(), index, total))
    }
    pub fn attach(
        &mut self,
        guest: u32,
        bytes: u64,
        window: u64,
        offset: u64,
        writable: bool,
    ) -> Result<(), i32> {
        if bytes == 0 || self.spans.len() >= 32 {
            return Err(-1);
        }
        let end = u64::from(guest).checked_add(bytes).ok_or(-1)?;
        if end > 4294967296 {
            return Err(-1);
        }
        for span in &self.spans {
            if u64::from(guest) < u64::from(span.guest) + span.bytes && end > u64::from(span.guest)
            {
                return Err(-1);
            }
        }
        let mut windows = [0u8; 512];
        let length = gamepkg::local_call(1, &[], &mut windows)?;
        if length % 16 != 0 {
            return Err(-3);
        }
        let mut found = false;
        for entry in windows[..length].chunks_exact(16) {
            let id = u64::from_le_bytes([
                entry[0], entry[1], entry[2], entry[3], entry[4], entry[5], entry[6], entry[7],
            ]);
            let size = u64::from_le_bytes([
                entry[8], entry[9], entry[10], entry[11], entry[12], entry[13], entry[14],
                entry[15],
            ]);
            if id == window {
                if offset.checked_add(bytes).ok_or(-1)? > size {
                    return Err(-1);
                }
                found = true;
            }
        }
        if !found {
            return Err(-4);
        }
        self.spans.push(Span {
            guest,
            bytes,
            window,
            offset,
            writable,
        });
        Ok(())
    }
    fn resolve(&self, address: u32, length: u64, write: bool) -> Result<(u64, u64), i32> {
        let end = u64::from(address).checked_add(length).ok_or(-1)?;
        if end > 4294967296 {
            return Err(-1);
        }
        for span in &self.spans {
            if u64::from(address) >= u64::from(span.guest)
                && end <= u64::from(span.guest) + span.bytes
            {
                if write && !span.writable {
                    return Err(-4);
                }
                return Ok((
                    span.window,
                    span.offset + u64::from(address) - u64::from(span.guest),
                ));
            }
        }
        Err(-4)
    }
    // Copy only disjoint backing ranges. The original caller retains its
    // explicit forward/backward overlap loop; this is its memcpy fast path.
    pub fn copy_nonoverlapping(
        &mut self,
        destination: u32,
        source: u32,
        length: u32,
    ) -> Result<(), i32> {
        if length == 0 {
            return Ok(());
        }
        if length > 64 * 1024 * 1024 {
            return Err(-1);
        }
        let (from_window, from) = self.resolve(source, u64::from(length), false)?;
        let (to_window, to) = self.resolve(destination, u64::from(length), true)?;
        let bytes = u64::from(length);
        if from_window == to_window && from < to + bytes && to < from + bytes {
            return Err(-1);
        }
        gamepkg::local_memory_copy(from_window, from, to_window, to, bytes)
    }
    pub fn fill(&mut self, address: u32, value: u8, length: u32) -> Result<(), i32> {
        if length == 0 {
            return Ok(());
        }
        if length > 64 * 1024 * 1024 {
            return Err(-1);
        }
        let (window, offset) = self.resolve(address, u64::from(length), true)?;
        gamepkg::local_memory_fill(window, offset, u64::from(length), value)
    }
    pub fn read_bytes(&self, address: u32, output: &mut [u8]) -> Result<(), i32> {
        if output.len() > 1048576 {
            return Err(-1);
        }
        if output.is_empty() {
            return Ok(());
        }
        let (window, offset) = self.resolve(address, output.len() as u64, false)?;
        let mut request = std::vec::Vec::new();
        request.extend(u64::to_le_bytes(window));
        request.extend(u64::to_le_bytes(offset));
        request.extend(u32::to_le_bytes(output.len() as u32));
        if gamepkg::local_call(5, &request, output)? != output.len() {
            return Err(-3);
        }
        Ok(())
    }
    pub fn write_bytes(&mut self, address: u32, input: &[u8]) -> Result<(), i32> {
        // The local ABI request also contains its 16-byte handle/offset header.
        if input.len() > 1048560 {
            return Err(-1);
        }
        if input.is_empty() {
            return Ok(());
        }
        let (window, offset) = self.resolve(address, input.len() as u64, true)?;
        let mut request = std::vec::Vec::new();
        request.extend(u64::to_le_bytes(window));
        request.extend(u64::to_le_bytes(offset));
        request.extend(input);
        if gamepkg::local_call(6, &request, &mut [])? != 0 {
            return Err(-3);
        }
        Ok(())
    }
    pub fn read_u8(&self, address: u32) -> Result<u8, i32> {
        let mut bytes = [0u8; 1];
        self.read_bytes(address, &mut bytes)?;
        Ok(u8::from_le_bytes(bytes))
    }
    pub fn write_u8(&mut self, address: u32, value: u8) -> Result<(), i32> {
        self.write_bytes(address, &u8::to_le_bytes(value))
    }
    pub fn read_i8(&self, address: u32) -> Result<i8, i32> {
        let mut bytes = [0u8; 1];
        self.read_bytes(address, &mut bytes)?;
        Ok(i8::from_le_bytes(bytes))
    }
    pub fn write_i8(&mut self, address: u32, value: i8) -> Result<(), i32> {
        self.write_bytes(address, &i8::to_le_bytes(value))
    }
    pub fn read_u16(&self, address: u32) -> Result<u16, i32> {
        let mut bytes = [0u8; 2];
        self.read_bytes(address, &mut bytes)?;
        Ok(u16::from_le_bytes(bytes))
    }
    pub fn write_u16(&mut self, address: u32, value: u16) -> Result<(), i32> {
        self.write_bytes(address, &u16::to_le_bytes(value))
    }
    pub fn read_i16(&self, address: u32) -> Result<i16, i32> {
        let mut bytes = [0u8; 2];
        self.read_bytes(address, &mut bytes)?;
        Ok(i16::from_le_bytes(bytes))
    }
    pub fn write_i16(&mut self, address: u32, value: i16) -> Result<(), i32> {
        self.write_bytes(address, &i16::to_le_bytes(value))
    }
    pub fn read_u32(&self, address: u32) -> Result<u32, i32> {
        let mut bytes = [0u8; 4];
        self.read_bytes(address, &mut bytes)?;
        Ok(u32::from_le_bytes(bytes))
    }
    pub fn write_u32(&mut self, address: u32, value: u32) -> Result<(), i32> {
        self.write_bytes(address, &u32::to_le_bytes(value))
    }
    pub fn read_i32(&self, address: u32) -> Result<i32, i32> {
        let mut bytes = [0u8; 4];
        self.read_bytes(address, &mut bytes)?;
        Ok(i32::from_le_bytes(bytes))
    }
    pub fn write_i32(&mut self, address: u32, value: i32) -> Result<(), i32> {
        self.write_bytes(address, &i32::to_le_bytes(value))
    }
    pub fn read_u64(&self, address: u32) -> Result<u64, i32> {
        let mut bytes = [0u8; 8];
        self.read_bytes(address, &mut bytes)?;
        Ok(u64::from_le_bytes(bytes))
    }
    pub fn write_u64(&mut self, address: u32, value: u64) -> Result<(), i32> {
        self.write_bytes(address, &u64::to_le_bytes(value))
    }
    pub fn read_i64(&self, address: u32) -> Result<i64, i32> {
        let mut bytes = [0u8; 8];
        self.read_bytes(address, &mut bytes)?;
        Ok(i64::from_le_bytes(bytes))
    }
    pub fn write_i64(&mut self, address: u32, value: i64) -> Result<(), i32> {
        self.write_bytes(address, &i64::to_le_bytes(value))
    }
    pub fn read_f32(&self, address: u32) -> Result<f32, i32> {
        let mut bytes = [0u8; 4];
        self.read_bytes(address, &mut bytes)?;
        Ok(f32::from_le_bytes(bytes))
    }
    pub fn write_f32(&mut self, address: u32, value: f32) -> Result<(), i32> {
        self.write_bytes(address, &f32::to_le_bytes(value))
    }
    pub fn read_f64(&self, address: u32) -> Result<f64, i32> {
        let mut bytes = [0u8; 8];
        self.read_bytes(address, &mut bytes)?;
        Ok(f64::from_le_bytes(bytes))
    }
    pub fn write_f64(&mut self, address: u32, value: f64) -> Result<(), i32> {
        self.write_bytes(address, &f64::to_le_bytes(value))
    }
}

// These values describe guest byte addresses, never host pointer provenance.
// None is the original uninitialized/null scratch value, distinct from guest 0.
pub fn guest_address(pointer: Option<u32>) -> Result<u32, i32> {
    pointer.ok_or(-1)
}
pub fn guest_position(pointer: Option<u32>) -> Result<u64, i32> {
    Ok(u64::from(guest_address(pointer)?))
}
pub fn guest_offset(pointer: Option<u32>, bytes: u32) -> Result<u64, i32> {
    // The original casts the proven unsigned count to native isize. Large
    // negative-on-i686 counts have no portable pointer-arithmetic meaning.
    if bytes > 2147483647 {
        return Err(-1);
    }
    let end = guest_position(pointer)? + u64::from(bytes);
    if end > 4294967296 {
        return Err(-1);
    }
    Ok(end)
}
