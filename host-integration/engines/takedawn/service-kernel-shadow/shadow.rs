// Port-owned 4096-entry guest-to-opaque-token map. Tokens confer no native authority.
use crate::gamepkg;
use crate::kernel_pool;
use crate::memory;
pub struct Shadow {
    root: u64,
    services: u64,
    record: u64,
    lock: u64,
    creator: u64,
    owned: bool,
    cleaning: bool,
    pages: std::vec::Vec<u64>,
}
pub struct Guard {
    root: u64,
    services: u64,
    record: u64,
    lock: u64,
    task: u64,
    held: bool,
    cleanup: bool,
}
fn shadow_word(bytes: &[u8], at: usize) -> Result<u64, i32> {
    let b = bytes.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
    ]))
}
fn shadow_u32(bytes: &[u8], at: usize) -> Result<u32, i32> {
    let b = bytes.get(at..at + 4).ok_or(-1)?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}
fn shadow_put(bytes: &mut [u8], at: usize, value: u64) -> Result<(), i32> {
    if at.checked_add(8).ok_or(-1)? > bytes.len() {
        return Err(-1);
    }
    let encoded = u64::to_le_bytes(value);
    for lane in 0..8usize {
        bytes[at + lane] = encoded[lane];
    }
    Ok(())
}
fn shadow_put32(bytes: &mut [u8], at: usize, value: u32) -> Result<(), i32> {
    if at.checked_add(4).ok_or(-1)? > bytes.len() {
        return Err(-1);
    }
    let encoded = u32::to_le_bytes(value);
    for lane in 0..4usize {
        bytes[at + lane] = encoded[lane];
    }
    Ok(())
}
fn shadow_owner(ram: &memory::Memory, services: u64) -> Result<(), i32> {
    if services == 0 || ram.services_handle()? != services {
        return Err(-4);
    }
    let values = kernel_pool::kernel_service_values(ram);
    if ram.services_handle()? != services {
        return Err(-4);
    }
    values?;
    if gamepkg::local_task_status()?.1 {
        return Err(-4);
    }
    Ok(())
}
fn shadow_root(
    record: u64,
    root: u64,
    services: u64,
    lock: u64,
) -> Result<(u64, std::vec::Vec<u8>), i32> {
    let (version, bytes) = gamepkg::local_record_snapshot(record, 152)?;
    if bytes.len() != 152 || shadow_word(&bytes, 0)? != 0x3153484b || shadow_word(&bytes, 40)? > 1 {
        return Err(-1);
    }
    if shadow_word(&bytes, 8)? != root
        || shadow_word(&bytes, 16)? != services
        || shadow_word(&bytes, 24)? != lock
        || root == 0
        || services == 0
        || lock == 0
    {
        return Err(-4);
    }
    let mut absent = false;
    for index in 0..13usize {
        let id = shadow_word(&bytes, 48 + index * 8)?;
        if id == 0 {
            absent = true;
        } else {
            if absent {
                return Err(-1);
            }
            for previous in 0..index {
                if shadow_word(&bytes, 48 + previous * 8)? == id {
                    return Err(-1);
                }
            }
        }
    }
    Ok((version, bytes))
}
fn shadow_publish(record: u64, version: u64, bytes: &[u8]) -> Result<(), i32> {
    match gamepkg::local_record_compare_exchange(record, version, bytes) {
        Ok(_) => Ok(()),
        Err(-2) => Err(-3),
        Err(code) => Err(code),
    }
}
fn shadow_page(record: u64, page: u64, index: usize) -> Result<(u64, std::vec::Vec<u8>), i32> {
    if index >= 13 || page == 0 {
        return Err(-1);
    }
    let count = if index == 12 { 16usize } else { 340 };
    let (version, bytes) = gamepkg::local_record_snapshot(page, 4096)?;
    if bytes.len() != 16 + count * 12
        || shadow_u32(&bytes, 0)? != 0x3150534b
        || shadow_u32(&bytes, 4)? != index as u32
    {
        return Err(-1);
    }
    if shadow_word(&bytes, 8)? != record {
        return Err(-4);
    }
    for slot in 0..count {
        if (shadow_u32(&bytes, 16 + slot * 12)? == 0) != (shadow_word(&bytes, 20 + slot * 12)? == 0)
        {
            return Err(-1);
        }
    }
    Ok((version, bytes))
}
fn shadow_unlock(lock: u64) -> Result<(), i32> {
    if gamepkg::local_call(24, &u64::to_le_bytes(lock), &mut [])? != 0 {
        return Err(-3);
    }
    Ok(())
}
impl Shadow {
    pub fn shadow_new(ram: &memory::Memory) -> Result<Self, i32> {
        let services = ram.services_handle()?;
        shadow_owner(ram, services)?;
        let creator = gamepkg::local_task_status()?.0;
        let mut out = [0u8; 8];
        if gamepkg::local_call(21, &[], &mut out)? != 8 {
            return Err(-3);
        }
        let lock = u64::from_le_bytes(out);
        let mut bytes = std::vec::Vec::new();
        bytes.resize(152, 0);
        for (at, value) in [
            (0usize, 0x3153484bu64),
            (8, ram.call_handle()),
            (16, services),
            (24, lock),
        ] {
            shadow_put(&mut bytes, at, value)?;
        }
        let record = match gamepkg::local_record_create(&bytes) {
            Ok(value) => value,
            Err(code) => {
                let _ = gamepkg::local_call(25, &u64::to_le_bytes(lock), &mut []);
                return Err(code);
            }
        };
        Ok(Self {
            root: ram.call_handle(),
            services,
            record,
            lock,
            creator,
            owned: true,
            cleaning: false,
            pages: std::vec::Vec::new(),
        })
    }
    pub fn shadow_id(&self) -> u64 {
        self.record
    }
    pub fn shadow_closed(&self) -> bool {
        self.record == 0 && self.lock == 0
    }
    pub fn shadow_borrow(ram: &memory::Memory, record: u64) -> Result<Self, i32> {
        let (_, bytes) = gamepkg::local_record_snapshot(record, 152)?;
        if bytes.len() != 152 {
            return Err(-1);
        }
        let view = Self {
            root: ram.call_handle(),
            services: ram.services_handle()?,
            record,
            lock: shadow_word(&bytes, 24)?,
            creator: 0,
            owned: false,
            cleaning: false,
            pages: std::vec::Vec::new(),
        };
        view.shadow_snapshot(ram)?;
        match gamepkg::local_mutex_lock(view.lock, 0) {
            Ok(()) => shadow_unlock(view.lock)?,
            Err(-2) => {}
            Err(code) => return Err(code),
        }
        Ok(view)
    }
    fn shadow_snapshot(&self, ram: &memory::Memory) -> Result<(u64, std::vec::Vec<u8>), i32> {
        if self.record == 0 || self.root != ram.call_handle() {
            return Err(-4);
        }
        shadow_owner(ram, self.services)?;
        let result = shadow_root(self.record, self.root, self.services, self.lock)?;
        if shadow_word(&result.1, 40)? != 0 {
            return Err(-4);
        }
        Ok(result)
    }
    pub fn shadow_lock(&self, ram: &memory::Memory) -> Result<Guard, i32> {
        self.shadow_snapshot(ram)?;
        let task = gamepkg::local_task_status()?.0;
        gamepkg::local_mutex_lock(self.lock, 1000)?;
        let result = self.shadow_enter(ram);
        match result {
            Ok(()) => Ok(Guard {
                root: self.root,
                services: self.services,
                record: self.record,
                lock: self.lock,
                task,
                held: true,
                cleanup: false,
            }),
            Err(code) => {
                let _ = shadow_unlock(self.lock);
                Err(code)
            }
        }
    }
    // Creator-only deletion lane after service revocation. It cannot insert
    // tokens and still requires the actual mutex and root/schema identities.
    pub fn shadow_cleanup_lock(&self, ram: &memory::Memory) -> Result<Guard, i32> {
        let (task, cancelled) = gamepkg::local_task_status()?;
        if !self.owned || self.root != ram.call_handle() || self.creator != task || cancelled {
            return Err(-4);
        }
        gamepkg::local_mutex_lock(self.lock, 1000)?;
        let result = self.shadow_cleanup_enter();
        match result {
            Ok(()) => Ok(Guard {
                root: self.root,
                services: self.services,
                record: self.record,
                lock: self.lock,
                task,
                held: true,
                cleanup: true,
            }),
            Err(code) => {
                let _ = shadow_unlock(self.lock);
                Err(code)
            }
        }
    }
    fn shadow_cleanup_enter(&self) -> Result<(), i32> {
        let (version, mut bytes) = shadow_root(self.record, self.root, self.services, self.lock)?;
        if shadow_word(&bytes, 40)? != 0 {
            return Err(-4);
        }
        let active = shadow_word(&bytes, 32)?.checked_add(1).ok_or(-2)?;
        shadow_put(&mut bytes, 32, active)?;
        shadow_publish(self.record, version, &bytes)
    }
    fn shadow_enter(&self, ram: &memory::Memory) -> Result<(), i32> {
        let (version, mut bytes) = self.shadow_snapshot(ram)?;
        let active = shadow_word(&bytes, 32)?.checked_add(1).ok_or(-2)?;
        shadow_put(&mut bytes, 32, active)?;
        shadow_publish(self.record, version, &bytes)
    }
    pub fn shadow_close(&mut self, ram: &memory::Memory) -> Result<(), i32> {
        if self.shadow_closed() {
            return Ok(());
        }
        let (task, cancelled) = gamepkg::local_task_status()?;
        if !self.owned || self.root != ram.call_handle() || task != self.creator || cancelled {
            return Err(-4);
        }
        let validation = shadow_owner(ram, self.services);
        gamepkg::local_mutex_lock(self.lock, 0)?;
        let result = self.shadow_cleanup();
        let unlocked = shadow_unlock(self.lock);
        result?;
        unlocked?;
        gamepkg::local_call(25, &u64::to_le_bytes(self.lock), &mut [])?;
        self.lock = 0;
        self.owned = false;
        validation?;
        Ok(())
    }
    fn shadow_cleanup(&mut self) -> Result<(), i32> {
        if !self.cleaning {
            let (version, mut bytes) =
                shadow_root(self.record, self.root, self.services, self.lock)?;
            if shadow_word(&bytes, 32)? != 0 {
                return Err(-2);
            }
            if shadow_word(&bytes, 40)? != 0 {
                return Err(-4);
            }
            let mut pages = std::vec::Vec::new();
            for index in 0..13usize {
                let page = shadow_word(&bytes, 48 + index * 8)?;
                if page != 0 {
                    let (_, contents) = shadow_page(self.record, page, index)?;
                    for slot in 0..if index == 12 { 16usize } else { 340 } {
                        if shadow_u32(&contents, 16 + slot * 12)? != 0 {
                            return Err(-2);
                        }
                    }
                    pages.push(page);
                }
            }
            shadow_put(&mut bytes, 40, 1)?;
            shadow_publish(self.record, version, &bytes)?;
            self.pages = pages;
            self.cleaning = true;
        }
        for page in &mut self.pages {
            if *page != 0 {
                match gamepkg::local_record_release(*page) {
                    Ok(()) | Err(-4) => *page = 0,
                    Err(code) => return Err(code),
                }
            }
        }
        if self.record != 0 {
            match gamepkg::local_record_release(self.record) {
                Ok(()) | Err(-4) => self.record = 0,
                Err(code) => return Err(code),
            }
        }
        Ok(())
    }
}
impl Guard {
    fn shadow_guard_owner(&self, ram: &memory::Memory) -> Result<(), i32> {
        if !self.held
            || self.root != ram.call_handle()
            || gamepkg::local_task_status()?.0 != self.task
        {
            return Err(-4);
        }
        Ok(())
    }
    fn shadow_guard_snapshot(&self, ram: &memory::Memory) -> Result<(u64, std::vec::Vec<u8>), i32> {
        self.shadow_guard_owner(ram)?;
        if !self.cleanup {
            shadow_owner(ram, self.services)?;
        }
        let result = shadow_root(self.record, self.root, self.services, self.lock)?;
        if shadow_word(&result.1, 40)? != 0 || shadow_word(&result.1, 32)? == 0 {
            return Err(-4);
        }
        Ok(result)
    }
    // [page index, slot index, old token]. A zero old token is the first free slot.
    fn shadow_find(&self, ram: &memory::Memory, guest: u32) -> Result<[u64; 3], i32> {
        self.shadow_guard_snapshot(ram)?;
        if guest == 0 {
            return Err(-1);
        }
        let (_, root) = shadow_root(self.record, self.root, self.services, self.lock)?;
        let mut free = [13u64, 0, 0];
        for index in 0..13usize {
            let page = shadow_word(&root, 48 + index * 8)?;
            if page == 0 {
                if free[0] == 13 {
                    free = [index as u64, 0, 0];
                }
                continue;
            }
            let (_, bytes) = shadow_page(self.record, page, index)?;
            for slot in 0..if index == 12 { 16usize } else { 340 } {
                let va = shadow_u32(&bytes, 16 + slot * 12)?;
                if va == guest {
                    return Ok([
                        index as u64,
                        slot as u64,
                        shadow_word(&bytes, 20 + slot * 12)?,
                    ]);
                }
                if va == 0 && free[0] == 13 {
                    free = [index as u64, slot as u64, 0];
                }
            }
        }
        Ok(free)
    }
    pub fn shadow_lookup(&self, ram: &memory::Memory, guest: u32) -> Result<Option<u64>, i32> {
        self.shadow_guard_snapshot(ram)?;
        if guest == 0 {
            return Ok(None);
        }
        let found = self.shadow_find(ram, guest)?;
        if found[2] == 0 {
            Ok(None)
        } else {
            Ok(Some(found[2]))
        }
    }
    // This only publishes port metadata. The typed caller closes a displaced
    // resource before publication, while this guard remains held.
    pub fn shadow_insert(
        &self,
        ram: &memory::Memory,
        guest: u32,
        token: u64,
    ) -> Result<Option<u64>, i32> {
        if self.cleanup {
            return Err(-4);
        }
        if token == 0 {
            return Err(-1);
        }
        let found = self.shadow_find(ram, guest)?;
        if found[0] == 13 {
            return Err(-2);
        }
        let index = found[0] as usize;
        let slot = found[1] as usize;
        let (root_version, mut root) = self.shadow_guard_snapshot(ram)?;
        let mut page = shadow_word(&root, 48 + index * 8)?;
        if page == 0 {
            let mut bytes = std::vec::Vec::new();
            bytes.resize(if index == 12 { 208 } else { 4096 }, 0);
            shadow_put32(&mut bytes, 0, 0x3150534b)?;
            shadow_put32(&mut bytes, 4, index as u32)?;
            shadow_put(&mut bytes, 8, self.record)?;
            page = gamepkg::local_record_create(&bytes)?;
            shadow_put(&mut root, 48 + index * 8, page)?;
            if let Err(code) = shadow_publish(self.record, root_version, &root) {
                let _ = gamepkg::local_record_release(page);
                return Err(code);
            }
        }
        let (version, mut bytes) = shadow_page(self.record, page, index)?;
        if shadow_u32(&bytes, 16 + slot * 12)? != if found[2] == 0 { 0 } else { guest }
            || shadow_word(&bytes, 20 + slot * 12)? != found[2]
        {
            return Err(-3);
        }
        shadow_put32(&mut bytes, 16 + slot * 12, guest)?;
        shadow_put(&mut bytes, 20 + slot * 12, token)?;
        shadow_publish(page, version, &bytes)?;
        if found[2] == 0 {
            Ok(None)
        } else {
            Ok(Some(found[2]))
        }
    }
    pub fn shadow_remove(&self, ram: &memory::Memory, guest: u32) -> Result<Option<u64>, i32> {
        self.shadow_guard_snapshot(ram)?;
        if guest == 0 {
            return Ok(None);
        }
        let found = self.shadow_find(ram, guest)?;
        if found[2] == 0 {
            return Ok(None);
        }
        let (_, root) = self.shadow_guard_snapshot(ram)?;
        let index = found[0] as usize;
        let slot = found[1] as usize;
        let page = shadow_word(&root, 48 + index * 8)?;
        let (version, mut bytes) = shadow_page(self.record, page, index)?;
        if shadow_u32(&bytes, 16 + slot * 12)? != guest
            || shadow_word(&bytes, 20 + slot * 12)? != found[2]
        {
            return Err(-3);
        }
        shadow_put32(&mut bytes, 16 + slot * 12, 0)?;
        shadow_put(&mut bytes, 20 + slot * 12, 0)?;
        shadow_publish(page, version, &bytes)?;
        Ok(Some(found[2]))
    }
    pub fn shadow_entries(&self, ram: &memory::Memory) -> Result<std::vec::Vec<[u64; 2]>, i32> {
        let (_, root) = self.shadow_guard_snapshot(ram)?;
        let mut result = std::vec::Vec::new();
        for index in 0..13usize {
            let page = shadow_word(&root, 48 + index * 8)?;
            if page == 0 {
                continue;
            }
            let (_, bytes) = shadow_page(self.record, page, index)?;
            for slot in 0..if index == 12 { 16usize } else { 340 } {
                let guest = shadow_u32(&bytes, 16 + slot * 12)?;
                if guest != 0 {
                    result.push([u64::from(guest), shadow_word(&bytes, 20 + slot * 12)?]);
                }
            }
        }
        Ok(result)
    }
    // Copied slot order, including holes. Values are port metadata, not
    // permission to use an arbitrary SDK handle or native address.
    pub fn shadow_slots(&self, ram: &memory::Memory) -> Result<std::vec::Vec<[u64; 2]>, i32> {
        let (_, root) = self.shadow_guard_snapshot(ram)?;
        let mut result = std::vec::Vec::new();
        for index in 0..13usize {
            let count = if index == 12 { 16usize } else { 340 };
            let page = shadow_word(&root, 48 + index * 8)?;
            if page == 0 {
                for _ in 0..count {
                    result.push([0, 0]);
                }
            } else {
                let (_, bytes) = shadow_page(self.record, page, index)?;
                for slot in 0..count {
                    result.push([
                        u64::from(shadow_u32(&bytes, 16 + slot * 12)?),
                        shadow_word(&bytes, 20 + slot * 12)?,
                    ]);
                }
            }
        }
        Ok(result)
    }
    pub fn shadow_guard_close(&mut self, ram: &memory::Memory) -> Result<(), i32> {
        if !self.held {
            return Ok(());
        }
        self.shadow_guard_owner(ram)?;
        let result = self.shadow_leave();
        let unlocked = shadow_unlock(self.lock);
        if unlocked.is_ok() {
            self.held = false;
        }
        result?;
        unlocked?;
        Ok(())
    }
    fn shadow_leave(&self) -> Result<(), i32> {
        let (version, mut root) = shadow_root(self.record, self.root, self.services, self.lock)?;
        let active = shadow_word(&root, 32)?;
        if active == 0 {
            return Err(-3);
        }
        shadow_put(&mut root, 32, active - 1)?;
        shadow_publish(self.record, version, &root)
    }
    pub fn shadow_guard_closed(&self) -> bool {
        !self.held
    }
}
