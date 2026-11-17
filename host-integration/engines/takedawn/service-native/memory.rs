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
    kernel: u64,
    kernel_caller: u32,
    heap_endpoints: [u64; 2],
    heap_client: Option<crate::heap_service::HeapClient>,
}
impl Memory {
    pub fn new() -> Result<Self, i32> {
        let initial = crate::call_state::CallState::call_default().call_encode();
        Ok(Self {
            spans: std::vec::Vec::new(),
            calls: gamepkg::local_record_create(&initial)?,
            kernel: 0,
            kernel_caller: 0,
            heap_endpoints: [0; 2],
            heap_client: None,
        })
    }
    // One title's views/tasks explicitly share an opaque GamePKG record ID.
    // The record lives until explicit release or local-runtime revocation.
    pub fn call_view(&self) -> Self {
        Self {
            spans: std::vec::Vec::new(),
            calls: self.calls,
            kernel: self.kernel,
            kernel_caller: 0,
            heap_endpoints: self.heap_endpoints,
            heap_client: None,
        }
    }
    // Install the initialized primary client without copying its response
    // channel/sequence. Failure leaves caller ownership intact for cleanup.
    pub fn heap_install(
        &mut self,
        owner: &mut Option<crate::heap_service::HeapClient>,
    ) -> Result<(), i32> {
        self.call_snapshot()?;
        if self.heap_endpoints != [0; 2] || self.heap_client.is_some() {
            return Err(-1);
        }
        let client = owner.as_ref().ok_or(-1)?;
        let (record, endpoints) = client.heap_binding().ok_or(-4)?;
        if record != self.calls || endpoints[0] == 0 || endpoints[1] == 0 {
            return Err(-4);
        }
        self.heap_endpoints = endpoints;
        self.heap_client = owner.take();
        Ok(())
    }
    // A call_view shares endpoint IDs, never the primary client. Each caller
    // lazily registers its own exclusive reply/sequence state at first use.
    pub fn heap_take(&mut self) -> Result<crate::heap_service::HeapClient, i32> {
        self.call_snapshot()?;
        if let Some(client) = self.heap_client.take() {
            return Ok(client);
        }
        if self.heap_endpoints[0] == 0 || self.heap_endpoints[1] == 0 {
            return Err(-5);
        }
        let mut client = crate::heap_service::HeapClient::heap_client(
            self.heap_endpoints[0],
            self.heap_endpoints[1],
        )?;
        if let Err(code) = client.heap_register() {
            let _ = client.heap_abandon();
            return Err(code);
        }
        Ok(client)
    }
    pub fn heap_put(&mut self, client: crate::heap_service::HeapClient) {
        self.heap_client = Some(client);
    }
    pub fn heap_close(&mut self) -> Result<(), i32> {
        self.heap_endpoints = [0; 2];
        if let Some(mut client) = self.heap_client.take() {
            let outcome = if client.heap_root_record() != 0 {
                client.heap_stop()
            } else {
                client.heap_unregister()
            };
            let cleanup = client.heap_abandon();
            outcome?;
            cleanup?;
        }
        Ok(())
    }
    pub fn kernel_caller(&self) -> u32 {
        self.kernel_caller
    }
    pub fn kernel_set_caller(&mut self, caller: u32) {
        self.kernel_caller = caller;
    }
    pub fn kernel_clear(&mut self) -> Result<(), i32> {
        if self.kernel == 0 {
            return Ok(());
        }
        let id = self.kernel;
        self.kernel = 0;
        match gamepkg::local_record_release(id) {
            Ok(()) | Err(-4) => Ok(()),
            Err(code) => Err(code),
        }
    }
    pub fn kernel_handle(&self) -> u64 {
        self.kernel
    }
    pub fn kernel_install(&mut self, record: u64) -> Result<(), i32> {
        self.call_snapshot()?;
        if record == 0 || self.kernel != 0 {
            return Err(-1);
        }
        self.kernel = record;
        Ok(())
    }
    // Only shared opaque context IDs cross a reviewed task's setup message.
    // Reply queues and in-flight client/sequence state never cross with them.
    pub fn thread_context(&self) -> [u64; 4] {
        [
            self.calls,
            self.kernel,
            self.heap_endpoints[0],
            self.heap_endpoints[1],
        ]
    }
    pub fn with_context(context: [u64; 4]) -> Result<Self, i32> {
        let mut ram = Self::with_call_handle(context[0])?;
        if (context[2] == 0) != (context[3] == 0) {
            return Err(-1);
        }
        if context[1] != 0 {
            let _ = gamepkg::local_record_snapshot(context[1], 1548)?;
        }
        if context[3] != 0 && gamepkg::local_task_finished(context[3])? {
            return Err(-4);
        }
        ram.kernel = context[1];
        ram.heap_endpoints = [context[2], context[3]];
        Ok(ram)
    }
    pub fn call_handle(&self) -> u64 {
        self.calls
    }
    pub fn with_call_handle(record: u64) -> Result<Self, i32> {
        let (_, bytes) = gamepkg::local_record_snapshot(record, 288)?;
        crate::call_state::CallState::call_decode(&bytes)?;
        Ok(Self {
            spans: std::vec::Vec::new(),
            calls: record,
            kernel: 0,
            kernel_caller: 0,
            heap_endpoints: [0; 2],
            heap_client: None,
        })
    }
    // A stale snapshot has not committed a call or touched game state. Yield
    // through an unsignaled host-owned event before retrying, so one hot
    // caller cannot exhaust another caller's bounded CAS attempts.
    pub fn call_retry_pause() -> Result<(), i32> {
        let mut reply = [0u8; 8];
        if gamepkg::local_call(20, &[0u8; 8], &mut reply)? != 8 {
            return Err(-3);
        }
        let event = u64::from_le_bytes(reply);
        let mut request = std::vec::Vec::new();
        request.extend(u64::to_le_bytes(event));
        request.extend(u32::to_le_bytes(1));
        let waited = gamepkg::local_call(23, &request, &mut []);
        let released = gamepkg::local_call(25, &u64::to_le_bytes(event), &mut []);
        released?;
        match waited {
            Err(-2) => Ok(()),
            Err(code) => Err(code),
            _ => Err(-3),
        }
    }
    // The root's trailing service ID is shared through the existing four-ID
    // context. It is metadata, never a pointer or copied per-view clock state.
    pub fn services_handle(&self) -> Result<u64, i32> {
        let (_, bytes) = gamepkg::local_record_snapshot(self.calls, 288)?;
        Ok(crate::call_state::CallState::call_decode(&bytes)?.call_services())
    }
    pub fn services_link(&self, expected: u64, next: u64) -> Result<(), i32> {
        if expected == next {
            return Err(-1);
        }
        for _ in 0..64usize {
            let (version, bytes) = gamepkg::local_record_snapshot(self.calls, 288)?;
            let mut state = crate::call_state::CallState::call_decode(&bytes)?;
            state.call_set_services(expected, next)?;
            match gamepkg::local_record_compare_exchange(self.calls, version, &state.call_encode())
            {
                Ok(_) => return Ok(()),
                Err(-2) => Self::call_retry_pause()?,
                Err(code) => return Err(code),
            }
        }
        Err(-2)
    }
    pub fn call_bounds(&self, low: u32, high: u32) -> Result<(), i32> {
        for _ in 0..64 {
            let (version, bytes) = gamepkg::local_record_snapshot(self.calls, 288)?;
            let mut state = crate::call_state::CallState::call_decode(&bytes)?;
            state.call_set_bounds(low, high)?;
            match gamepkg::local_record_compare_exchange(self.calls, version, &state.call_encode())
            {
                Ok(_) => return Ok(()),
                Err(-2) => Self::call_retry_pause()?,
                Err(error) => return Err(error),
            }
        }
        Err(-2)
    }
    pub fn call_begin(&self, target: u32) -> Result<bool, i32> {
        for _ in 0..64 {
            let (version, bytes) = gamepkg::local_record_snapshot(self.calls, 288)?;
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
                Err(-2) => Self::call_retry_pause()?,
                Err(error) => return Err(error),
            }
        }
        Err(-2)
    }
    pub fn call_absent(&self, target: u32) -> Result<(), i32> {
        let (_, bytes) = gamepkg::local_record_snapshot(self.calls, 288)?;
        let state = crate::call_state::CallState::call_decode(&bytes)?;
        gamepkg::diagnostic(2, 0x544b0002, &state.call_missing_values(target))?;
        Ok(())
    }
    pub fn call_snapshot(&self) -> Result<([u32; 16], u32, u64), i32> {
        let (_, bytes) = gamepkg::local_record_snapshot(self.calls, 288)?;
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
    // One complete guest page backed by one opaque window. This grants no
    // access or native pointer; callers still use checked local operations.
    pub fn memory_page_backing(&self, address: u32) -> Result<[u64; 2], i32> {
        self.call_snapshot()?;
        if address & 4095 != 0 {
            return Err(-1);
        }
        let (window, offset) = self.resolve(address, 4096, false)?;
        if offset & 4095 != 0 {
            return Err(-1);
        }
        Ok([window, offset])
    }
    // Capture only one writable guest word, never a host address. The view's
    // write permission is checked here; the local runtime checks page access
    // again on every write. Copying this tuple is not a lease on the backing.
    pub fn memory_writable_word_backing(&self, address: u32) -> Result<[u64; 2], i32> {
        self.call_snapshot()?;
        if address & 3 != 0 {
            return Err(-1);
        }
        let (window, offset) = self.resolve(address, 4, true)?;
        if offset & 3 != 0 {
            return Err(-1);
        }
        if gamepkg::local_memory_page_states(window, offset & !4095u64, 1)? != [3u8] {
            return Err(-4);
        }
        Ok([window, offset])
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
    // Resolve only explicit adjacent guest spans, without masking unknown
    // addresses. Collect every range before asking the host to touch any byte.
    fn resolve_segments(
        &self,
        address: u32,
        length: u64,
        write: bool,
    ) -> Result<std::vec::Vec<[u64; 3]>, i32> {
        let end = u64::from(address).checked_add(length).ok_or(-1)?;
        if length == 0 || length > 1048576 || end > 4294967296 {
            return Err(-1);
        }
        let mut cursor = u64::from(address);
        let mut segments = std::vec::Vec::new();
        while cursor < end {
            let mut found = false;
            for span in &self.spans {
                let low = u64::from(span.guest);
                let high = low + span.bytes;
                if cursor >= low && cursor < high {
                    if write && !span.writable {
                        return Err(-4);
                    }
                    let bytes = (end - cursor).min(high - cursor);
                    segments.push([span.window, span.offset + cursor - low, bytes]);
                    cursor += bytes;
                    found = true;
                    break;
                }
            }
            if !found {
                return Err(-4);
            }
        }
        Ok(segments)
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
    // Read whole scalar units within one known span/host page. A boundary
    // shorter than one required unit uses the ordinary segmented exact read.
    // This is not a lease: actual host read/protection/revocation errors always
    // propagate and must never be treated as a reason to retry a smaller read.
    pub fn memory_read_chunk(
        &self,
        address: u32,
        unit: u32,
        output: &mut [u8],
    ) -> Result<usize, i32> {
        self.call_snapshot()?;
        if (unit != 1 && unit != 2 && unit != 4)
            || output.is_empty()
            || output.len() > 4096
            || output.len() % unit as usize != 0
        {
            return Err(-1);
        }
        let position = u64::from(address);
        for span in &self.spans {
            let low = u64::from(span.guest);
            let high = low + span.bytes;
            if position >= low && position < high {
                let offset = span.offset + position - low;
                let available = (high - position).min(4096 - (offset & 4095));
                let count = ((output.len() as u64).min(available) & !(u64::from(unit) - 1))
                    .max(u64::from(unit));
                self.read_bytes(address, &mut output[..count as usize])?;
                return Ok(count as usize);
            }
        }
        Err(-4)
    }
    pub fn read_bytes(&self, address: u32, output: &mut [u8]) -> Result<(), i32> {
        if output.len() > 1048576 {
            return Err(-1);
        }
        if output.is_empty() {
            return Ok(());
        }
        let (window, offset) = match self.resolve(address, output.len() as u64, false) {
            Ok(range) => range,
            Err(-4) => {
                let segments = self.resolve_segments(address, output.len() as u64, false)?;
                return gamepkg::local_memory_read_segments(&segments, output);
            }
            Err(error) => return Err(error),
        };
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
        let (window, offset) = match self.resolve(address, input.len() as u64, true) {
            Ok(range) => range,
            Err(-4) => {
                let segments = self.resolve_segments(address, input.len() as u64, true)?;
                return gamepkg::local_memory_write_segments(&segments, input);
            }
            Err(error) => return Err(error),
        };
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
