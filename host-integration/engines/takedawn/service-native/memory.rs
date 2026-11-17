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
const VM_LOW: u32 = 0x03800000;
const VM_HIGH: u32 = 0x04000000;
const VM_CONFLICT: u32 = 0xc0000018;
const VM_INVALID: u32 = 0xc000000d;
const VM_EXHAUSTED: u32 = 0xc0000017;
const VM_NOT_ALLOCATED: u32 = 0xc00000a0;

struct GuestArena {
    task: u64,
    backing: u64,
    origin: u64,
    reservations: std::vec::Vec<[u32; 2]>,
    poisoned: bool,
}

fn vm_page_service(
    operation: u32,
    window: u64,
    offset: u64,
    size: u32,
    mode: u32,
) -> Result<(), i32> {
    let mut request = std::vec::Vec::new();
    request.extend(u64::to_le_bytes(window));
    request.extend(u64::to_le_bytes(offset));
    request.extend(u64::to_le_bytes(u64::from(size)));
    if operation != 4 {
        request.extend(u32::to_le_bytes(mode));
    }
    if gamepkg::local_call(operation, &request, &mut [])? != 0 {
        return Err(-3);
    }
    Ok(())
}

impl GuestArena {
    fn vm_offset(&self, base: u32) -> u64 {
        self.origin + u64::from(base - VM_LOW)
    }

    fn vm_page_runs(&self, base: u32, size: u32) -> Result<std::vec::Vec<[u32; 3]>, i32> {
        let mut runs: std::vec::Vec<[u32; 3]> = std::vec::Vec::new();
        let mut cursor = base;
        while cursor < base + size {
            let count = ((base + size - cursor) / 4096).min(256);
            for state in
                gamepkg::local_memory_page_states(self.backing, self.vm_offset(cursor), count)?
            {
                if let Some(last_run) = runs.last_mut() {
                    if last_run[2] == u32::from(state) {
                        last_run[1] += 4096;
                        cursor += 4096;
                        continue;
                    }
                }
                runs.push([cursor, 4096, u32::from(state)]);
                cursor += 4096;
            }
        }
        Ok(runs)
    }

    fn vm_commit(&mut self, runs: &[[u32; 3]], mode: u32) -> Result<(), i32> {
        for (index, span) in runs.iter().enumerate() {
            if span[2] == mode + 1 {
                continue;
            }
            let operation = if span[2] == 0 { 2 } else { 3 };
            if let Err(code) = vm_page_service(
                operation,
                self.backing,
                self.vm_offset(span[0]),
                span[1],
                mode,
            ) {
                for prior in &runs[..index] {
                    if prior[2] == mode + 1 {
                        continue;
                    }
                    let undo = if prior[2] == 0 { 4 } else { 3 };
                    if vm_page_service(
                        undo,
                        self.backing,
                        self.vm_offset(prior[0]),
                        prior[1],
                        prior[2].saturating_sub(1),
                    )
                    .is_err()
                    {
                        self.poisoned = true;
                        return Err(-3);
                    }
                }
                return Err(code);
            }
        }
        Ok(())
    }

    fn vm_empty_slot(&self) -> Option<usize> {
        for (index, reservation) in self.reservations.iter().enumerate() {
            if reservation[1] == 0 {
                return Some(index);
            }
        }
        None
    }

    fn vm_find(&self, base: u32, size: u32) -> Option<usize> {
        for (index, pair) in self.reservations.iter().enumerate() {
            if pair[1] != 0
                && base >= pair[0]
                && u64::from(base) + u64::from(size) <= u64::from(pair[1])
            {
                return Some(index);
            }
        }
        None
    }

    fn vm_free_range(&self, base: u32, size: u32, high: u32) -> bool {
        if base < VM_LOW || u64::from(base) + u64::from(size) > u64::from(high) {
            return false;
        }
        for pair in &self.reservations {
            if pair[1] != 0 && base < pair[1] && pair[0] < base + size {
                return false;
            }
        }
        true
    }

    fn vm_select(&self, hint: u32, size: u32, high: u32, top_down: bool) -> Option<u32> {
        if hint != 0 {
            return if self.vm_free_range(hint, size, high) {
                Some(hint)
            } else {
                None
            };
        }
        for index in 0..((VM_HIGH - VM_LOW) / 65536) {
            let lane = if top_down {
                (VM_HIGH - VM_LOW) / 65536 - index - 1
            } else {
                index
            };
            let candidate = VM_LOW + lane * 65536;
            if self.vm_free_range(candidate, size, high) {
                return Some(candidate);
            }
        }
        None
    }

    fn vm_parameters(&self, ram: &Memory, pointers: [u32; 2], region: [u32; 2]) -> Result<(), i32> {
        // Check translated backing overlap, including aliases and cross-span
        // words. The service independently checks both output writes together.
        let start = self.vm_offset(region[0]);
        let end = start + u64::from(region[1]);
        for pointer in pointers {
            for segment in ram.resolve_segments(pointer, 4, true)? {
                if segment[0] == self.backing && segment[1] < end && start < segment[1] + segment[2]
                {
                    return Err(-5);
                }
            }
        }
        Ok(())
    }

    fn vm_apply(
        &mut self,
        ram: &mut Memory,
        pointers: [u32; 2],
        old: [u32; 2],
        range: [u32; 2],
        operation: u32,
        mode: u32,
    ) -> Result<(), i32> {
        self.vm_parameters(ram, pointers, range)?;
        let runs = if operation == 2 {
            self.vm_page_runs(range[0], range[1])?
        } else {
            std::vec::Vec::new()
        };
        vm_write_parameters(ram, pointers, range)?;
        let changed = match operation {
            2 => self.vm_commit(&runs, mode),
            4 => vm_page_service(4, self.backing, self.vm_offset(range[0]), range[1], 0),
            _ => Ok(()),
        };
        if let Err(error) = changed {
            if vm_write_parameters(ram, pointers, old).is_err() {
                self.poisoned = true;
            }
            return Err(if self.poisoned { -3 } else { error });
        }
        Ok(())
    }

    fn vm_allocate(&mut self, ram: &mut Memory, request: [u32; 5]) -> Result<u32, i32> {
        let [base_at, zero_bits, size_at, flags, protection] = request;
        let pointers = [base_at, size_at];
        let old = [ram.read_u32(base_at)?, ram.read_u32(size_at)?];
        if old[1] == 0 || zero_bits > 21 || flags & !0x103000 != 0 || flags & 0x3000 == 0 {
            return Ok(VM_INVALID);
        }
        let mode = match protection {
            1 => 0,
            2 => 1,
            4 => 2,
            _ => return Err(-5),
        };
        let reserve = flags & 0x2000 != 0 || old[0] == 0;
        let base = old[0] & !(if reserve { 65535 } else { 4095 });
        if old[0] != 0 && base < VM_LOW {
            return Ok(VM_CONFLICT);
        }
        let span = u64::from(old[0] - base) + u64::from(old[1]);
        let size = (span + 4095) & !4095u64;
        if size > u64::from(VM_HIGH - VM_LOW) {
            return Ok(VM_EXHAUSTED);
        }
        let size = size as u32;
        if !reserve {
            if self.vm_find(base, size).is_none() {
                return Ok(VM_CONFLICT);
            }
            self.vm_apply(ram, pointers, old, [base, size], 2, mode)?;
            return Ok(0);
        }
        let available = match self.vm_empty_slot() {
            Some(slot) => slot,
            None => return Ok(VM_EXHAUSTED),
        };
        let high = if base == 0 && zero_bits != 0 {
            VM_HIGH.min((u32::MAX >> zero_bits) + 1)
        } else {
            VM_HIGH
        };
        let selected = match self.vm_select(base, size, high, flags & 0x100000 != 0) {
            Some(selected) => selected,
            None if old[0] == 0 => return Ok(VM_EXHAUSTED),
            None => return Ok(VM_CONFLICT),
        };
        self.vm_apply(
            ram,
            pointers,
            old,
            [selected, size],
            if flags & 0x1000 == 0 { 0 } else { 2 },
            mode,
        )?;
        self.reservations[available] = [selected, selected + size];
        Ok(0)
    }

    fn vm_free(&mut self, ram: &mut Memory, request: [u32; 3]) -> Result<u32, i32> {
        let [base_at, size_at, flags] = request;
        let pointers = [base_at, size_at];
        let old = [ram.read_u32(base_at)?, ram.read_u32(size_at)?];
        if flags != 0x4000 && flags != 0x8000 {
            return Ok(VM_INVALID);
        }
        let base = old[0] & !4095;
        let size = (u64::from(old[0] - base) + u64::from(old[1]) + 4095) & !4095u64;
        if size > u64::from(VM_HIGH - VM_LOW) {
            return Ok(VM_INVALID);
        }
        let entry = match self.vm_find(base, if old[1] == 0 { 1 } else { size as u32 }) {
            Some(index) => index,
            None => return Ok(VM_NOT_ALLOCATED),
        };
        let reservation = self.reservations[entry];
        if old[1] == 0 && base != reservation[0] {
            return Ok(0xc000009f);
        }
        let size = if old[1] == 0 {
            reservation[1] - base
        } else {
            size as u32
        };
        let end = base + size;
        let split = if flags == 0x8000 && base > reservation[0] && end < reservation[1] {
            match self.vm_empty_slot() {
                Some(slot) => Some(slot),
                None => return Ok(VM_EXHAUSTED),
            }
        } else {
            None
        };
        self.vm_apply(ram, pointers, old, [base, size], 4, 0)?;
        if flags == 0x8000 {
            self.reservations[entry] = if base > reservation[0] {
                [reservation[0], base]
            } else if end < reservation[1] {
                [end, reservation[1]]
            } else {
                [0; 2]
            };
            if let Some(slot) = split {
                self.reservations[slot] = [end, reservation[1]];
            }
        }
        Ok(0)
    }
}

fn vm_write_parameters(ram: &mut Memory, pointers: [u32; 2], values: [u32; 2]) -> Result<(), i32> {
    ram.memory_write_pair(
        pointers[0],
        &u32::to_le_bytes(values[0]),
        pointers[1],
        &u32::to_le_bytes(values[1]),
    )
}

pub struct Memory {
    spans: std::vec::Vec<Span>,
    calls: u64,
    kernel: u64,
    kernel_caller: u32,
    heap_endpoints: [u64; 2],
    heap_client: Option<crate::heap_service::HeapClient>,
    vm_arena: Option<GuestArena>,
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
            vm_arena: None,
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
            vm_arena: None,
        }
    }
    // Reserve the top of the existing main bank before starting the heap
    // worker. The returned address is the exclusive heap limit.
    pub fn memory_guest_arena(&mut self, heap_start: u32) -> Result<u32, i32> {
        self.call_snapshot()?;
        let (task, cancelled) = gamepkg::local_task_status()?;
        if cancelled
            || self.kernel != 0
            || self.heap_endpoints != [0; 2]
            || self.vm_arena.is_some()
            || heap_start < 65536
            || heap_start >= VM_LOW
        {
            return Err(-4);
        }
        let (backing, origin) = self.resolve(VM_LOW, u64::from(VM_HIGH - VM_LOW), true)?;
        if origin != u64::from(VM_LOW) {
            return Err(-4);
        }
        let mut reservations = std::vec::Vec::new();
        reservations.resize(128, [0; 2]);
        vm_page_service(4, backing, origin, VM_HIGH - VM_LOW, 0)?;
        self.vm_arena = Some(GuestArena {
            task,
            backing,
            origin,
            reservations,
            poisoned: false,
        });
        Ok(VM_LOW)
    }

    pub fn memory_guest_arena_ready(&self) -> Result<(), i32> {
        let arena = self.vm_arena.as_ref().ok_or(-5)?;
        let (task, cancelled) = gamepkg::local_task_status()?;
        if cancelled || arena.task != task || arena.poisoned {
            return Err(-4);
        }
        Ok(())
    }

    pub fn memory_guest_vm_call(&mut self, ordinal: u32, args: [u32; 5]) -> Result<u32, i32> {
        self.memory_guest_arena_ready()?;
        let mut arena = self.vm_arena.take().ok_or(-5)?;
        let result = match ordinal {
            184 => arena.vm_allocate(self, args),
            199 => arena.vm_free(self, [args[0], args[1], args[2]]),
            _ => Err(-5),
        };
        self.vm_arena = Some(arena);
        result
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
            vm_arena: None,
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
    // Xbox page flags for a translated guest address. Reserved backing
    // reports zero, while a hole has no accessible mapping.
    pub fn memory_guest_protection(&self, address: u32) -> Result<u32, i32> {
        self.call_snapshot()?;
        let (window, offset) = match self.resolve(address & !4095, 4096, false) {
            Ok(backing) => backing,
            Err(-4) => return Ok(1),
            Err(code) => return Err(code),
        };
        let states = gamepkg::local_memory_page_states(window, offset, 1)?;
        match states.as_slice() {
            [0] => Ok(0),
            [1] => Ok(1),
            [2] => Ok(2),
            [3] => Ok(4),
            _ => Err(-3),
        }
    }

    // Round outward in the guest address space, then request a single
    // checked protection change on the complete translated backing extent.
    // Never split a mutation between independent or wrapping alias spans.
    pub fn memory_set_guest_protection(
        &self,
        address: u32,
        bytes: u32,
        flags: u32,
    ) -> Result<(), i32> {
        let protection = match flags {
            1 => 0,
            2 => 1,
            4 => 2,
            _ => return Err(-5),
        };
        self.call_snapshot()?;
        let start = address & !4095;
        let end = u64::from(address) + u64::from(bytes);
        if bytes == 0 || end > 4294967296 {
            return Err(-1);
        }
        let rounded = (end + 4095) & !4095u64;
        let size = rounded - u64::from(start);
        let (window, offset) = self.resolve(start, size, true)?;
        if offset & 4095 != 0 {
            return Err(-1);
        }
        let request = [
            u64::to_le_bytes(window).as_slice(),
            &u64::to_le_bytes(offset),
            &u64::to_le_bytes(size),
            &u32::to_le_bytes(protection),
        ]
        .concat();
        if gamepkg::local_call(3, &request, &mut [])? != 0 {
            return Err(-3);
        }
        Ok(())
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
    // Preflight writable guest ranges before starting external I/O. Backing
    // permissions are checked again when writing, since no lease is acquired.
    pub fn memory_check_write(&self, address: u32, length: u32) -> Result<(), i32> {
        if length == 0 {
            return Ok(());
        }
        for segment in self.resolve_segments(address, u64::from(length), true)? {
            let mut page = segment[1] & !4095u64;
            let end = segment[1].checked_add(segment[2]).ok_or(-1)?;
            while page < end {
                let pages = ((end - page).div_ceil(4096)).min(256);
                for access in gamepkg::local_memory_page_states(segment[0], page, pages as u32)? {
                    if access != 3 {
                        return Err(-4);
                    }
                }
                page += pages * 4096;
            }
        }
        Ok(())
    }

    // Publish two guest ranges in order under the same local memory lock.
    // The service checks all backing permissions before touching either one.
    pub fn memory_write_pair(
        &mut self,
        first: u32,
        first_bytes: &[u8],
        last: u32,
        last_bytes: &[u8],
    ) -> Result<(), i32> {
        if first_bytes.len().saturating_add(last_bytes.len()) > 1048560 {
            return Err(-1);
        }
        let mut segments = std::vec::Vec::new();
        if !first_bytes.is_empty() {
            segments.extend(self.resolve_segments(first, first_bytes.len() as u64, true)?);
        }
        if !last_bytes.is_empty() {
            segments.extend(self.resolve_segments(last, last_bytes.len() as u64, true)?);
        }
        if segments.is_empty() {
            return Ok(());
        }
        let mut payload = first_bytes.to_vec();
        payload.extend(last_bytes);
        gamepkg::local_memory_write_segments(&segments, &payload)
    }

    // Publish related outputs together, refusing overlaps in backing storage
    // even when the guest supplied distinct virtual addresses.
    pub fn memory_write_disjoint(&mut self, outputs: [(u32, &[u8]); 3]) -> Result<(), i32> {
        let mut segments: std::vec::Vec<[u64; 3]> = std::vec::Vec::new();
        let mut payload = std::vec::Vec::new();
        for (address, bytes) in outputs {
            if bytes.is_empty() {
                continue;
            }
            if bytes.len().saturating_add(payload.len()) > 1048560 {
                return Err(-1);
            }
            for segment in self.resolve_segments(address, bytes.len() as u64, true)? {
                for prior in &segments {
                    if segment[0] == prior[0]
                        && segment[1] < prior[1] + prior[2]
                        && prior[1] < segment[1] + segment[2]
                    {
                        return Err(-1);
                    }
                }
                segments.push(segment);
            }
            payload.extend(bytes);
        }
        if segments.is_empty() {
            return Ok(());
        }
        gamepkg::local_memory_write_segments(&segments, &payload)
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
