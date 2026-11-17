// Synchronous title save file calls. The checked worker owns the native file
// and publication. This adapter owns only guest tokens and cursor state.
use crate::gamepkg;
use crate::kernel_file;
use crate::kernel_pool;
use crate::memory;
use crate::registers;
use crate::save_paths;
use crate::save_worker;

const REGULAR_KIND: u32 = 6;
const REGULAR_LIMIT: usize = 64;
const WRITE_BLOCK: u32 = 65508;

pub struct SaveSession {
    connection: save_worker::Connection,
    grant: u64,
    ready: bool,
    failed: bool,
}

impl SaveSession {
    pub fn regular_session_launch(entry: u32, grant: u64, stamp: &str) -> Result<Self, i32> {
        Ok(Self {
            connection: save_worker::Connection::launch_save_actor(
                entry, grant, "4541005b", stamp,
            )?,
            grant,
            ready: false,
            failed: false,
        })
    }

    fn regular_session_reply(&mut self) -> Result<save_worker::Reply, i32> {
        let timeout = gamepkg::monotonic()?.0.saturating_add(6_000_000_000);
        loop {
            let polled = self.connection.poll(20);
            match polled {
                Ok(Some(packet)) => return Ok(packet),
                Ok(None) if gamepkg::monotonic()?.0 < timeout => {}
                _ => {
                    self.failed = true;
                    return Err(-3);
                }
            }
        }
    }

    pub fn regular_session_start(&mut self) -> Result<(), i32> {
        let boot = self.regular_session_reply()?;
        if boot.status != 0 {
            return Err(boot.status);
        }
        if boot.sequence != 1 || boot.bytes.len() != 68 {
            return Err(-3);
        }
        self.ready = true;
        Ok(())
    }

    pub fn regular_session_ids(&self) -> Result<[u64; 2], i32> {
        if !self.ready || self.failed {
            return Err(-4);
        }
        Ok([self.grant, self.connection.task_id()])
    }

    fn regular_session_request(
        &mut self,
        operation: u32,
        packet: &[u8],
    ) -> Result<std::vec::Vec<u8>, i32> {
        self.regular_session_ids()?;
        self.connection.submit(operation, packet)?;
        let answer = self.regular_session_reply()?;
        if answer.status != 0 {
            return Err(answer.status);
        }
        Ok(answer.bytes)
    }

    pub fn regular_session_stop(&mut self) -> Result<(), i32> {
        let final_status = self.regular_session_request(save_worker::SHUTDOWN, &[]);
        self.ready = false;
        if final_status?.len() != 68 {
            return Err(-3);
        }
        Ok(())
    }

    pub fn regular_session_cancel(&mut self) -> Result<(), i32> {
        self.ready = false;
        self.connection.cancel_save_actor()
    }

    pub fn regular_session_accept_join(
        &mut self,
        proof: &gamepkg::LocalTaskJoin,
    ) -> Result<(), i32> {
        self.connection.accept_join(proof)
    }

    pub fn regular_session_finish(&mut self) -> Result<Option<u64>, i32> {
        self.connection.finish()
    }
}

struct RegularEntry {
    token: u32,
    record: u64,
    file: u64,
    access: u32,
    share: u32,
    offset: u64,
}

impl Copy for RegularEntry {}
impl Clone for RegularEntry {
    fn clone(&self) -> Self {
        *self
    }
}

pub struct RegularFiles<'a> {
    session: &'a mut SaveSession,
    entries: std::vec::Vec<RegularEntry>,
    guard: Option<kernel_pool::HandleGuard>,
    orphan: u32,
    retired: u64,
}

impl<'a> RegularFiles<'a> {
    pub fn regular_files_new(session: &'a mut SaveSession) -> Result<Self, i32> {
        session.regular_session_ids()?;
        let mut regular_entries = std::vec::Vec::new();
        regular_entries.resize(
            REGULAR_LIMIT,
            RegularEntry {
                token: 0,
                record: 0,
                file: 0,
                access: 0,
                share: 0,
                offset: 0,
            },
        );
        Ok(Self {
            session,
            entries: regular_entries,
            guard: None,
            orphan: 0,
            retired: 0,
        })
    }

    fn regular_files_index(&self, token: u32) -> Result<usize, i32> {
        for (regular_slot, item) in self.entries.iter().enumerate() {
            if item.token == token {
                return Ok(regular_slot);
            }
        }
        Err(-4)
    }

    pub fn regular_files_owns(&self, token: u32) -> bool {
        token != 0 && self.regular_files_index(token).is_ok()
    }

    fn regular_files_unlock(&mut self, ram: &memory::Memory) -> Result<(), i32> {
        if let Some(retained_guard) = self.guard.as_mut() {
            retained_guard.handles_guard_close(ram)?;
            self.guard = None;
        }
        Ok(())
    }

    fn regular_files_settle(
        &mut self,
        ram: &memory::Memory,
        table: &kernel_pool::Handles,
    ) -> Result<(), i32> {
        self.regular_files_unlock(ram)?;
        if self.retired != 0 {
            match gamepkg::local_record_release(self.retired) {
                Ok(()) | Err(-4) => self.retired = 0,
                Err(code) => return Err(code),
            }
        }
        if self.orphan != 0 {
            self.regular_files_close(ram, table, self.orphan)?;
        }
        Ok(())
    }

    fn regular_files_checked(
        &mut self,
        ram: &memory::Memory,
        table: &kernel_pool::Handles,
        token: u32,
    ) -> Result<usize, i32> {
        if token == 0 {
            return Err(-4);
        }
        let regular_index = self.regular_files_index(token)?;
        self.guard = Some(table.handles_lock(ram)?);
        let matching = self
            .guard
            .as_ref()
            .ok_or(-3)?
            .handle_guard_lookup(ram, token);
        self.regular_files_unlock(ram)?;
        if matching? != Some([self.entries[regular_index].record, u64::from(REGULAR_KIND)]) {
            return Err(-4);
        }
        Ok(regular_index)
    }

    pub fn regular_files_invoke(
        &mut self,
        ram: &mut memory::Memory,
        table: &kernel_pool::Handles,
        cpu: &mut registers::Registers,
        ordinal: u32,
    ) -> Result<(), i32> {
        if !cpu.valid() || cpu.esp == 0 || gamepkg::local_task_status()?.1 {
            return Err(-1);
        }
        self.regular_files_settle(ram, table)?;
        let frame_bytes = match ordinal {
            187 => 8,
            190 => 40,
            202 => 28,
            198 => 12,
            211 | 226 => 24,
            219 | 236 => 36,
            _ => return Err(-5),
        };
        let mut frame = [0u8; 40];
        let next_stack = cpu.esp.checked_add(frame_bytes).ok_or(-1)?;
        ram.read_bytes(cpu.esp, &mut frame[..frame_bytes as usize])?;
        let status = match ordinal {
            187 => {
                self.regular_files_close(ram, table, kernel_file::rom_kernel_word(&frame, 4)?)?;
                0
            }
            190 | 202 => self.regular_files_open(ram, table, &frame, ordinal == 190)?,
            198 | 226 => self.regular_files_control(ram, table, &frame, ordinal == 198)?,
            211 => self.regular_files_query(ram, table, &frame)?,
            219 | 236 => self.regular_files_transfer(ram, table, &frame, ordinal == 236)?,
            _ => return Err(-5),
        };
        ram.kernel_set_caller(kernel_file::rom_kernel_word(&frame, 0)?);
        cpu.esp = next_stack;
        cpu.eax = status;
        Ok(())
    }

    fn regular_files_reserve(
        &mut self,
        ram: &memory::Memory,
        table: &kernel_pool::Handles,
        resolved_regular_path: std::string::String,
        access: u32,
        share: u32,
    ) -> Result<usize, i32> {
        for registered in &self.entries {
            if registered.token != 0
                && ((access & !registered.share) != 0 || (registered.access & !share) != 0)
            {
                let (_, recorded_name) = gamepkg::local_record_snapshot(registered.record, 528)?;
                if recorded_name.eq_ignore_ascii_case(resolved_regular_path.as_bytes()) {
                    return Err(-2);
                }
            }
        }
        let regular_index = match self.regular_files_index(0) {
            Ok(free_slot) => free_slot,
            Err(_) => return Err(-2),
        };
        self.retired = gamepkg::local_record_create(resolved_regular_path.as_bytes())?;
        self.guard = Some(table.handles_lock(ram)?);
        let token = self.guard.as_ref().ok_or(-3)?.handle_guard_insert(
            ram,
            self.retired,
            REGULAR_KIND,
            true,
        )?;
        self.entries[regular_index] = RegularEntry {
            token,
            record: self.retired,
            file: 0,
            access,
            share,
            offset: 0,
        };
        self.orphan = token;
        self.retired = 0;
        self.regular_files_unlock(ram)?;
        Ok(regular_index)
    }

    fn regular_files_open(
        &mut self,
        ram: &mut memory::Memory,
        table: &kernel_pool::Handles,
        frame: &[u8],
        create: bool,
    ) -> Result<u32, i32> {
        let handle_address = kernel_file::rom_kernel_word(frame, 4)?;
        let requested = kernel_file::rom_kernel_word(frame, 8)?;
        let object_address = kernel_file::rom_kernel_word(frame, 12)?;
        let iosb = kernel_file::rom_kernel_word(frame, 16)?;
        if handle_address == 0 || object_address == 0 || iosb == 0 || requested & !0xc0100003 != 0 {
            return Err(-5);
        }
        let access =
            u32::from(requested & 0x80000001 != 0) | (u32::from(requested & 0x40000002 != 0) << 1);
        let [share, disposition, options, flags] = if create {
            if kernel_file::rom_kernel_word(frame, 20)? != 0 {
                return Err(-5);
            }
            [
                kernel_file::rom_kernel_word(frame, 28)?,
                kernel_file::rom_kernel_word(frame, 32)?,
                kernel_file::rom_kernel_word(frame, 36)?,
                kernel_file::rom_kernel_word(frame, 24)?,
            ]
        } else {
            [
                kernel_file::rom_kernel_word(frame, 20)?,
                1,
                kernel_file::rom_kernel_word(frame, 24)?,
                0,
            ]
        };
        let writable = access & 2 != 0;
        if access == 0
            || share & !3 != 0
            || !(1..=3).contains(&disposition)
            || options & !0x62 != 0
            || options & 0x20 == 0
            || (flags & !0x26 != 0 && flags != 0x80)
            || (!writable && (options & 2 != 0 || disposition != 1))
            || ram.read_u32(object_address)? != 0
            || ram.read_u32(object_address.checked_add(8).ok_or(-1)?)? != 0x40
        {
            return Err(-5);
        }
        if u64::from(handle_address) < u64::from(iosb) + 8
            && u64::from(iosb) < u64::from(handle_address) + 4
        {
            return Err(-1);
        }
        let opaque_name = kernel_file::rom_kernel_name(ram, object_address)?;
        ram.memory_check_write(handle_address, 4)?;
        ram.memory_check_write(iosb, 8)?;
        let granted_name =
            regular_name(self.session.grant, gamepkg::text_from_utf8(&opaque_name)?)?;
        let write_mode = u32::from(writable)
            | (options & 2)
            | (if disposition == 1 {
                0
            } else {
                (flags & 0x26) << 8
            });
        let request = [
            u32::to_le_bytes(write_mode).as_slice(),
            &u32::to_le_bytes(disposition),
            granted_name.as_bytes(),
        ]
        .concat();
        let regular_index =
            match self.regular_files_reserve(ram, table, granted_name, access, share) {
                Ok(slot_index) => slot_index,
                Err(code) => {
                    self.regular_files_settle(ram, table)?;
                    return Err(code);
                }
            };
        let reply = self
            .session
            .regular_session_request(save_worker::OPEN_FILE, &request);
        let completion = match reply {
            Ok(raw) => self.regular_open_publish(ram, regular_index, handle_address, iosb, &raw),
            Err(code) => Err(code),
        };
        self.regular_files_settle(ram, table)?;
        completion
    }

    fn regular_open_publish(
        &mut self,
        ram: &mut memory::Memory,
        regular_index: usize,
        handle_address: u32,
        iosb: u32,
        raw: &[u8],
    ) -> Result<u32, i32> {
        if raw.len() != 12 {
            self.session.failed = true;
            return Err(-3);
        }
        let native_file = regular_u64(raw)?;
        let information = kernel_file::rom_kernel_word(raw, 8)?;
        if native_file == 0 {
            let status = match information {
                4 => 0xc0000035,
                5 => 0xc0000034,
                _ => return Err(-3),
            };
            ram.write_u64(iosb, regular_status(status, information))?;
            return Ok(status);
        }
        self.entries[regular_index].file = native_file;
        if information != 1 && information != 2 {
            return Err(-3);
        }
        ram.memory_write_pair(
            handle_address,
            &u32::to_le_bytes(self.orphan),
            iosb,
            &u64::to_le_bytes(regular_status(0, information)),
        )?;
        self.orphan = 0;
        Ok(0)
    }

    fn regular_files_query(
        &mut self,
        ram: &mut memory::Memory,
        table: &kernel_pool::Handles,
        frame: &[u8],
    ) -> Result<u32, i32> {
        let token = kernel_file::rom_kernel_word(frame, 4)?;
        let iosb = kernel_file::rom_kernel_word(frame, 8)?;
        let output_address = kernel_file::rom_kernel_word(frame, 12)?;
        let required = match kernel_file::rom_kernel_word(frame, 20)? {
            14 => 8,
            34 => 56,
            _ => return Err(-5),
        };
        if kernel_file::rom_kernel_word(frame, 16)? < required || iosb == 0 || output_address == 0 {
            return Err(-5);
        }
        ram.memory_check_write(iosb, 8)?;
        ram.memory_check_write(output_address, required)?;
        let regular_index = self.regular_files_checked(ram, table, token)?;
        if required == 8 {
            ram.memory_write_pair(
                output_address,
                &u64::to_le_bytes(self.entries[regular_index].offset),
                iosb,
                &u64::to_le_bytes(regular_status(0, 8)),
            )?;
            return Ok(0);
        }
        let request = u64::to_le_bytes(self.entries[regular_index].file);
        let reply = self
            .session
            .regular_session_request(save_worker::QUERY_FILE, &request)?;
        if reply.len() != 52 {
            return Err(-3);
        }
        ram.memory_write_pair(
            output_address,
            &reply,
            iosb,
            &u64::to_le_bytes(regular_status(0, 56)),
        )?;
        Ok(0)
    }

    fn regular_files_control(
        &mut self,
        ram: &mut memory::Memory,
        table: &kernel_pool::Handles,
        frame: &[u8],
        flush_requested: bool,
    ) -> Result<u32, i32> {
        let token = kernel_file::rom_kernel_word(frame, 4)?;
        let iosb = kernel_file::rom_kernel_word(frame, 8)?;
        if iosb == 0 {
            return Err(-5);
        }
        ram.memory_check_write(iosb, 8)?;
        let regular_index = self.regular_files_checked(ram, table, token)?;
        let file = &mut self.entries[regular_index];
        let control_class = if flush_requested {
            0
        } else {
            kernel_file::rom_kernel_word(frame, 20)?
        };
        let mut value = 0;
        if !flush_requested {
            if (control_class != 14 && control_class != 20)
                || kernel_file::rom_kernel_word(frame, 16)? != 8
            {
                return Err(-5);
            }
            let input_address = kernel_file::rom_kernel_word(frame, 12)?;
            if input_address == 0 {
                return Err(-1);
            }
            let signed_value = ram.read_i64(input_address)?;
            if signed_value < 0 {
                return Err(-1);
            }
            value = signed_value as u64;
        }
        if control_class == 14 {
            ram.write_u64(iosb, 0)?;
            file.offset = value;
            return Ok(0);
        }
        if file.access & 2 == 0 {
            return Err(-4);
        }
        let reply = if flush_requested {
            self.session
                .regular_session_request(save_worker::FLUSH_FILE, &u64::to_le_bytes(file.file))?
        } else {
            let resize_request = [u64::to_le_bytes(file.file), u64::to_le_bytes(value)].concat();
            self.session
                .regular_session_request(save_worker::RESIZE, &resize_request)?
        };
        if !reply.is_empty() {
            self.session.failed = true;
            return Err(-3);
        }
        if let Err(code) = ram.write_u64(iosb, 0) {
            self.session.failed = true;
            return Err(code);
        }
        Ok(0)
    }
    fn regular_files_transfer(
        &mut self,
        ram: &mut memory::Memory,
        table: &kernel_pool::Handles,
        frame: &[u8],
        write: bool,
    ) -> Result<u32, i32> {
        let token = kernel_file::rom_kernel_word(frame, 4)?;
        let iosb = kernel_file::rom_kernel_word(frame, 20)?;
        let buffer = kernel_file::rom_kernel_word(frame, 24)?;
        let length = kernel_file::rom_kernel_word(frame, 28)?;
        let offset_address = kernel_file::rom_kernel_word(frame, 32)?;
        if frame[8..20] != [0; 12]
            || iosb == 0
            || (length != 0 && buffer == 0)
            || length > WRITE_BLOCK
        {
            return Err(-5);
        }
        let regular_index = self.regular_files_checked(ram, table, token)?;
        let selected = &mut self.entries[regular_index];
        if selected.access & if write { 2 } else { 1 } == 0 {
            return Err(-4);
        }
        let source_offset = if offset_address == 0 {
            -2
        } else {
            ram.read_i64(offset_address)?
        };
        let offset = match source_offset {
            -2 => selected.offset,
            -1 if write => gamepkg::file_size(selected.file)?,
            n if n >= 0 => n as u64,
            _ => return Err(-5),
        };
        offset.checked_add(u64::from(length)).ok_or(-1)?;
        ram.memory_check_write(iosb, 8)?;
        let mut bytes = std::vec::Vec::new();
        bytes.resize(length as usize, 0u8);
        let count;
        let mut status = 0;
        if write {
            if length != 0 {
                ram.read_bytes(buffer, &mut bytes)?;
            }
            let request = [
                u64::to_le_bytes(selected.file).as_slice(),
                &u64::to_le_bytes(offset),
                &bytes,
            ]
            .concat();
            let reply = self
                .session
                .regular_session_request(save_worker::WRITE, &request)?;
            if !reply.is_empty() {
                self.session.failed = true;
                return Err(-3);
            }
            count = length;
            if let Err(code) = ram.write_u64(iosb, regular_status(status, count)) {
                self.session.failed = true;
                return Err(code);
            }
        } else {
            if length != 0 {
                ram.memory_check_write(buffer, length)?;
            }
            count = gamepkg::file_read(selected.file, offset, &mut bytes)? as u32;
            if count == 0 && length != 0 {
                status = 0xc0000011;
            }
            if count == 0 {
                ram.write_u64(iosb, regular_status(status, count))?;
            } else {
                ram.memory_write_pair(
                    buffer,
                    &bytes[..count as usize],
                    iosb,
                    &u64::to_le_bytes(regular_status(status, count)),
                )?;
            }
        }
        selected.offset = offset + u64::from(count);
        Ok(status)
    }

    fn regular_files_close(
        &mut self,
        ram: &memory::Memory,
        table: &kernel_pool::Handles,
        token: u32,
    ) -> Result<(), i32> {
        let regular_index = self.regular_files_checked(ram, table, token)?;
        let selected = &mut self.entries[regular_index];
        if selected.file != 0 {
            let reply = self
                .session
                .regular_session_request(save_worker::CLOSE, &u64::to_le_bytes(selected.file))?;
            selected.file = 0;
            if !reply.is_empty() {
                self.session.failed = true;
                return Err(-3);
            }
        }
        self.guard = Some(table.handles_cleanup_lock(ram)?);
        let removed = self
            .guard
            .as_ref()
            .ok_or(-3)?
            .handle_guard_remove(ram, token)?;
        if removed != Some([selected.record, u64::from(REGULAR_KIND)]) {
            return Err(-3);
        }
        self.retired = selected.record;
        selected.token = 0;
        selected.record = 0;
        if self.orphan == token {
            self.orphan = 0;
        }
        self.regular_files_unlock(ram)?;
        gamepkg::local_record_release(self.retired)?;
        self.retired = 0;
        Ok(())
    }

    pub fn regular_files_drain(
        &mut self,
        ram: &memory::Memory,
        table: &kernel_pool::Handles,
    ) -> Result<(), i32> {
        self.regular_files_settle(ram, table)?;
        for regular_index in 0..self.entries.len() {
            let token = self.entries[regular_index].token;
            if token != 0 {
                self.regular_files_close(ram, table, token)?;
            }
        }
        Ok(())
    }
}

fn regular_u64(raw: &[u8]) -> Result<u64, i32> {
    let part = raw.get(..8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        part[0], part[1], part[2], part[3], part[4], part[5], part[6], part[7],
    ]))
}

fn regular_status(status: u32, information: u32) -> u64 {
    u64::from(status) | (u64::from(information) << 32)
}

fn regular_name(grant: u64, requested_name: &str) -> Result<std::string::String, i32> {
    let prefix = "\\Device\\Harddisk0\\partition1\\";
    if !requested_name
        .get(..prefix.len())
        .ok_or(-5)?
        .eq_ignore_ascii_case(prefix)
    {
        return Err(-5);
    }
    let rest = requested_name.get(prefix.len()..).ok_or(-5)?;
    let mut normalized_path = std::vec::Vec::new();
    normalized_path.resize(rest.len(), 0u8);
    for (encoded_byte, byte) in normalized_path.iter_mut().zip(rest.bytes()) {
        *encoded_byte = if byte == 92 { 47 } else { byte };
        if byte == 47 {
            return Err(-5);
        }
    }
    let components = save_paths::save_path_segments(gamepkg::text_from_utf8(&normalized_path)?)?;
    if components.len() < 3 {
        return Err(-5);
    }
    let volume = components[0];
    let title = components[1];
    if !title.eq_ignore_ascii_case("4541005b") {
        return Err(-5);
    }
    let mut canonical = std::string::String::new();
    if volume.eq_ignore_ascii_case("UDATA") {
        canonical.push_str("UDATA/4541005b");
    } else if volume.eq_ignore_ascii_case("TDATA") {
        canonical.push_str("TDATA/4541005b");
    } else {
        return Err(-5);
    }
    for requested_part in &components[2..] {
        let segment = gamepkg::text_component(requested_part.as_bytes())?;
        let names = gamepkg::directory_list(grant, &canonical)?;
        let mut spelling = None;
        for candidate in &names {
            if candidate.name.eq_ignore_ascii_case(segment) {
                if spelling.is_some() {
                    return Err(-4);
                }
                spelling = Some(candidate.name.as_str());
            }
        }
        canonical.push_str("/");
        canonical.push_str(spelling.unwrap_or(segment));
    }
    Ok(canonical)
}
