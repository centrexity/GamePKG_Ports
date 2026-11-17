// Port-owned authenticated import startup and explicit owner lifecycle.
use crate::gamepkg;
use crate::handles;
use crate::image_imports;
use crate::kernel_data;
use crate::kernel_pool;
use crate::memory;
use crate::typed_events;
use crate::xbe;

pub struct Startup {
    root: u64,
    creator: u64,
    phase: u32,
    setup_error: i32,
    close_error: i32,
    services: Option<kernel_pool::KernelServices>,
    registry: Option<kernel_pool::Connections>,
    context: Option<kernel_pool::KernelContext>,
    shadow: Option<kernel_pool::Shadow>,
    handles: Option<handles::Handles>,
    objects: Option<typed_events::Events>,
    semaphores: Option<kernel_pool::Semaphores>,
    critical_shadow: Option<kernel_pool::Shadow>,
    criticals: Option<kernel_pool::Criticals>,
    map: u64,
    rom_key: Option<(u64, [u8; 32])>,
    private_save: u64,
    regular_worker: u64,
    jobs: std::vec::Vec<u64>,
}
impl Startup {
    // Idle owns no new records. RAM, image, data, heap and banks stay caller-owned.
    pub fn startup_new(ram: &memory::Memory) -> Result<Self, i32> {
        ram.call_snapshot()?;
        let (creator, cancelled) = gamepkg::local_task_status()?;
        if cancelled {
            return Err(-4);
        }
        if ram.kernel_handle() != 0 || ram.services_handle()? != 0 {
            return Err(-1);
        }
        Ok(Self {
            root: ram.call_handle(),
            creator,
            phase: 0,
            setup_error: 0,
            close_error: 0,
            services: None,
            registry: None,
            context: None,
            shadow: None,
            handles: None,
            objects: None,
            semaphores: None,
            critical_shadow: None,
            criticals: None,
            map: 0,
            rom_key: None,
            private_save: 0,
            regular_worker: 0,
            jobs: std::vec::Vec::new(),
        })
    }
    // 0 idle, 1 preparing, 2 ready, 3 failed, 4 closing, 5 closed.
    pub fn startup_phase(&self) -> u32 {
        self.phase
    }
    pub fn startup_error(&self) -> i32 {
        self.setup_error
    }
    pub fn startup_closed(&self) -> bool {
        self.phase == 5
    }
    pub fn startup_jobs(&self) -> u32 {
        self.jobs.len() as u32
    }
    fn startup_owner(&self, ram: &memory::Memory) -> Result<(), i32> {
        if ram.call_handle() != self.root || gamepkg::local_task_status()?.0 != self.creator {
            return Err(-4);
        }
        Ok(())
    }
    // Creator-only cleanup observations. They neither revive a closed owner nor
    // turn copied IDs into leases; shared consumers must match this exact owner.
    pub fn startup_lifetime(&self, ram: &memory::Memory) -> Result<[u64; 2], i32> {
        self.startup_owner(ram)?;
        let services = if let Some(owner) = self.services.as_ref() {
            owner.service_handle()
        } else {
            0
        };
        Ok([services, self.map])
    }
    pub fn startup_has_task(&self, ram: &memory::Memory, task: u64) -> Result<bool, i32> {
        self.startup_owner(ram)?;
        Ok(self.jobs.contains(&task))
    }
    pub fn startup_prepare(
        &mut self,
        ram: &mut memory::Memory,
        image: &xbe::Image,
        data: &kernel_data::Data,
        remap: &[u16],
        video: u32,
        speakers: u32,
    ) -> Result<(), i32> {
        self.startup_prepare_mode(ram, image, data, remap, video, speakers, false, [0; 3])
    }
    pub fn startup_prepare_with_shadow(
        &mut self,
        ram: &mut memory::Memory,
        image: &xbe::Image,
        data: &kernel_data::Data,
        remap: &[u16],
        video: u32,
        speakers: u32,
    ) -> Result<(), i32> {
        self.startup_prepare_mode(ram, image, data, remap, video, speakers, true, [0; 3])
    }
    pub fn startup_prepare_with_objects(
        &mut self,
        ram: &mut memory::Memory,
        image: &xbe::Image,
        data: &kernel_data::Data,
        remap: &[u16],
        video: u32,
        speakers: u32,
        scratch: u64,
        offset: u64,
    ) -> Result<(), i32> {
        if scratch == 0 {
            return Err(-1);
        }
        self.startup_prepare_mode(
            ram,
            image,
            data,
            remap,
            video,
            speakers,
            true,
            [scratch, offset, 0],
        )
    }
    pub fn startup_prepare_with_sync_objects(
        &mut self,
        ram: &mut memory::Memory,
        image: &xbe::Image,
        data: &kernel_data::Data,
        remap: &[u16],
        video: u32,
        speakers: u32,
        scratch: u64,
        offset: u64,
    ) -> Result<(), i32> {
        if scratch == 0 {
            return Err(-1);
        }
        self.startup_prepare_mode(
            ram,
            image,
            data,
            remap,
            video,
            speakers,
            true,
            [scratch, offset, 1],
        )
    }
    pub fn startup_prepare_with_criticals(
        &mut self,
        ram: &mut memory::Memory,
        image: &xbe::Image,
        data: &kernel_data::Data,
        remap: &[u16],
        video: u32,
        speakers: u32,
        scratch: u64,
        offset: u64,
    ) -> Result<(), i32> {
        if scratch == 0 {
            return Err(-1);
        }
        self.startup_prepare_mode(
            ram,
            image,
            data,
            remap,
            video,
            speakers,
            true,
            [scratch, offset, 2],
        )
    }
    pub fn startup_prepare_with_rom(
        &mut self,
        ram: &mut memory::Memory,
        image: &xbe::Image,
        data: &kernel_data::Data,
        remap: &[u16],
        video: u32,
        speakers: u32,
        scratch: u64,
        offset: u64,
    ) -> Result<(), i32> {
        if scratch == 0 {
            return Err(-1);
        }
        self.startup_prepare_mode(
            ram,
            image,
            data,
            remap,
            video,
            speakers,
            true,
            [scratch, offset, 3],
        )
    }
    pub fn startup_prepare_with_private_files(
        &mut self,
        ram: &mut memory::Memory,
        image: &xbe::Image,
        data: &kernel_data::Data,
        remap: &[u16],
        video: u32,
        speakers: u32,
        storage: [u64; 3],
    ) -> Result<(), i32> {
        self.startup_prepare_storage(
            ram,
            image,
            data,
            remap,
            video,
            speakers,
            [storage[0], storage[1], storage[2], 0],
        )
    }

    pub fn startup_prepare_with_regular_files(
        &mut self,
        ram: &mut memory::Memory,
        image: &xbe::Image,
        data: &kernel_data::Data,
        remap: &[u16],
        video: u32,
        speakers: u32,
        storage: [u64; 4],
    ) -> Result<(), i32> {
        if storage[3] == 0 {
            return Err(-4);
        }
        self.startup_prepare_storage(ram, image, data, remap, video, speakers, storage)
    }

    fn startup_prepare_storage(
        &mut self,
        ram: &mut memory::Memory,
        image: &xbe::Image,
        data: &kernel_data::Data,
        remap: &[u16],
        video: u32,
        speakers: u32,
        storage: [u64; 4],
    ) -> Result<(), i32> {
        self.startup_owner(ram)?;
        if self.phase != 0 || storage[0] == 0 {
            return Err(-1);
        }
        if gamepkg::local_task_status()?.1 || gamepkg::save_directory(storage[2], "", false)? != 1 {
            return Err(-4);
        }
        let regular_task = storage[3];
        if regular_task != 0
            && (regular_task == self.creator || gamepkg::local_task_finished(regular_task)?)
        {
            return Err(-4);
        }
        let options = if regular_task == 0 { 4 } else { 5 };
        self.startup_prepare_mode(
            ram,
            image,
            data,
            remap,
            video,
            speakers,
            true,
            [storage[0], storage[1], options],
        )?;
        self.private_save = storage[2];
        self.regular_worker = regular_task;
        if regular_task != 0 {
            self.jobs.push(regular_task);
        }
        Ok(())
    }
    fn startup_prepare_mode(
        &mut self,
        ram: &mut memory::Memory,
        image: &xbe::Image,
        data: &kernel_data::Data,
        remap: &[u16],
        video: u32,
        speakers: u32,
        with_shadow: bool,
        objects: [u64; 3],
    ) -> Result<(), i32> {
        self.startup_owner(ram)?;
        ram.call_snapshot()?;
        if gamepkg::local_task_status()?.1 {
            return Err(-4);
        }
        if self.phase != 0 || ram.kernel_handle() != 0 || ram.services_handle()? != 0 {
            return Err(-1);
        }
        self.phase = 1;
        let outcome = self.startup_prepare_inner(
            ram,
            image,
            data,
            remap,
            video,
            speakers,
            with_shadow,
            objects,
        );
        match outcome {
            Ok(()) => {
                self.phase = 2;
                Ok(())
            }
            Err(code) => {
                self.phase = 3;
                self.setup_error = code;
                Err(code)
            }
        }
    }
    fn startup_prepare_inner(
        &mut self,
        ram: &mut memory::Memory,
        image: &xbe::Image,
        data: &kernel_data::Data,
        remap: &[u16],
        video: u32,
        speakers: u32,
        with_shadow: bool,
        objects: [u64; 3],
    ) -> Result<(), i32> {
        self.services = Some(kernel_pool::kernel_services_initialize(
            ram, video, speakers,
        )?);
        self.registry = Some(kernel_pool::Connections::connections_new(ram)?);
        self.context = if objects[0] != 0 {
            self.shadow = Some(kernel_pool::Shadow::shadow_new(ram)?);
            self.handles = Some(handles::Handles::handles_new(ram)?);
            self.handles
                .as_mut()
                .ok_or(-3)?
                .handles_bind(ram, objects[0], objects[1])?;
            self.objects = Some(typed_events::Events::events_new(
                ram,
                self.handles.as_ref().ok_or(-3)?,
            )?);
            if objects[2] >= 1 {
                self.semaphores = Some(kernel_pool::Semaphores::semaphores_new(
                    ram,
                    self.handles.as_ref().ok_or(-3)?,
                )?);
                if objects[2] >= 2 {
                    self.critical_shadow = Some(kernel_pool::Shadow::shadow_new(ram)?);
                    self.criticals = Some(kernel_pool::Criticals::criticals_new(
                        ram,
                        self.critical_shadow.as_ref().ok_or(-3)?,
                    )?);
                    Some(
                        kernel_pool::KernelContext::kernel_context_new_with_criticals(
                            ram,
                            self.registry.as_ref().ok_or(-3)?,
                            self.shadow.as_ref().ok_or(-3)?,
                            self.handles.as_ref().ok_or(-3)?.handles_id(),
                            self.objects.as_ref().ok_or(-3)?.events_id(),
                            self.semaphores.as_ref().ok_or(-3)?.semaphores_id(),
                            self.criticals.as_ref().ok_or(-3)?,
                        )?,
                    )
                } else {
                    Some(
                        kernel_pool::KernelContext::kernel_context_new_with_sync_objects(
                            ram,
                            self.registry.as_ref().ok_or(-3)?,
                            self.shadow.as_ref().ok_or(-3)?,
                            self.handles.as_ref().ok_or(-3)?.handles_id(),
                            self.objects.as_ref().ok_or(-3)?.events_id(),
                            self.semaphores.as_ref().ok_or(-3)?.semaphores_id(),
                        )?,
                    )
                }
            } else {
                Some(kernel_pool::KernelContext::kernel_context_new_with_objects(
                    ram,
                    self.registry.as_ref().ok_or(-3)?,
                    self.shadow.as_ref().ok_or(-3)?,
                    self.handles.as_ref().ok_or(-3)?.handles_id(),
                    self.objects.as_ref().ok_or(-3)?.events_id(),
                )?)
            }
        } else if with_shadow {
            self.shadow = Some(kernel_pool::Shadow::shadow_new(ram)?);
            Some(kernel_pool::KernelContext::kernel_context_new_with_shadow(
                ram,
                self.registry.as_ref().ok_or(-3)?,
                self.shadow.as_ref().ok_or(-3)?,
            )?)
        } else {
            Some(kernel_pool::KernelContext::kernel_context_new(
                ram,
                self.registry.as_ref().ok_or(-3)?,
            )?)
        };
        let context = self.context.as_ref().ok_or(-3)?;
        let plan = if objects[2] >= 3 {
            image_imports::imports_image_with_rom(
                ram,
                image,
                data,
                remap,
                context,
                (objects[2] - 2) as u32,
            )?
        } else {
            image_imports::imports_image_with_context(ram, image, data, remap, context)?
        };
        plan.imports_publish(ram)?;
        self.map = ram.kernel_handle();
        if self.map == 0 {
            return Err(-3);
        }
        if objects[2] >= 3 {
            self.rom_key = Some((image.image_rom_grant(), image.image_digest()));
        }
        Ok(())
    }
    // Only a ready live owner can issue copied IDs. IDs are not resource leases.
    // [KSV3, ISR1, KIC1/KIC2/KIC3, KPL4]; the existing worker four-ID format is unchanged.
    pub fn startup_ids(&self, ram: &memory::Memory) -> Result<[u64; 4], i32> {
        self.startup_owner(ram)?;
        if self.phase != 2 || self.map == 0 || ram.kernel_handle() != self.map {
            return Err(-4);
        }
        let services = self.services.as_ref().ok_or(-4)?.service_handle();
        if ram.services_handle()? != services {
            return Err(-4);
        }
        let context = self.context.as_ref().ok_or(-4)?;
        let (_, map_bytes) = gamepkg::local_record_snapshot(self.map, 1548)?;
        if map_bytes.len() < 36
            || map_bytes[..4] != u32::to_le_bytes(0x344c504b)
            || map_bytes[8..16] != u64::to_le_bytes(self.root)
            || map_bytes[24..32] != u64::to_le_bytes(context.kernel_context_id())
        {
            return Err(-4);
        }
        let registry = context.kernel_context_registry(ram)?;
        if registry.connections_id() != self.registry.as_ref().ok_or(-4)?.connections_id() {
            return Err(-4);
        }
        if let Some(owned) = self.shadow.as_ref() {
            let map = context.kernel_context_shadow(ram)?;
            if map.shadow_id() != owned.shadow_id() {
                return Err(-4);
            }
        } else if context.kernel_context_shadow(ram).err() != Some(-5) {
            return Err(-4);
        }
        if let Some(owned) = self.handles.as_ref() {
            let ids = context.kernel_context_objects(ram)?;
            if ids
                != [
                    owned.handles_id(),
                    self.objects.as_ref().ok_or(-4)?.events_id(),
                ]
            {
                return Err(-4);
            }
            let map = handles::Handles::handles_borrow(ram, ids[0])?;
            typed_events::Events::events_borrow(ram, &map, ids[1])?;
        } else if context.kernel_context_objects(ram).err() != Some(-5) {
            return Err(-4);
        }
        if let Some(owned) = self.semaphores.as_ref() {
            let ids = context.kernel_context_sync_objects(ram)?;
            if ids
                != [
                    self.handles.as_ref().ok_or(-4)?.handles_id(),
                    self.objects.as_ref().ok_or(-4)?.events_id(),
                    owned.semaphores_id(),
                ]
            {
                return Err(-4);
            }
            let table = handles::Handles::handles_borrow(ram, ids[0])?;
            kernel_pool::Semaphores::semaphores_borrow(ram, &table, ids[2])?;
        } else if context.kernel_context_sync_objects(ram).err() != Some(-5) {
            return Err(-4);
        }
        if let Some(owned) = self.criticals.as_ref() {
            let (critical_map, domain) = context.kernel_context_criticals(ram)?;
            if domain.criticals_id() != owned.criticals_id()
                || critical_map.shadow_id() != self.critical_shadow.as_ref().ok_or(-4)?.shadow_id()
            {
                return Err(-4);
            }
        } else if context.kernel_context_criticals(ram).err() != Some(-5) {
            return Err(-4);
        }
        Ok([
            services,
            registry.connections_id(),
            context.kernel_context_id(),
            self.map,
        ])
    }
    // The owner retains event/map cleanup; callers receive only a live borrow.
    pub fn startup_shadow(&self, ram: &memory::Memory) -> Result<kernel_pool::Shadow, i32> {
        self.startup_ids(ram)?;
        self.context.as_ref().ok_or(-4)?.kernel_context_shadow(ram)
    }
    pub fn startup_objects(
        &self,
        ram: &memory::Memory,
    ) -> Result<(handles::Handles, typed_events::Events), i32> {
        self.startup_ids(ram)?;
        let ids = self
            .context
            .as_ref()
            .ok_or(-4)?
            .kernel_context_objects(ram)?;
        let map = handles::Handles::handles_borrow(ram, ids[0])?;
        let domain = typed_events::Events::events_borrow(ram, &map, ids[1])?;
        Ok((map, domain))
    }
    pub fn startup_sync_objects(
        &self,
        ram: &memory::Memory,
    ) -> Result<
        (
            handles::Handles,
            typed_events::Events,
            kernel_pool::Semaphores,
        ),
        i32,
    > {
        let (table, bound_events) = self.startup_objects(ram)?;
        let ids = self
            .context
            .as_ref()
            .ok_or(-4)?
            .kernel_context_sync_objects(ram)?;
        let bound_semaphores = kernel_pool::Semaphores::semaphores_borrow(ram, &table, ids[2])?;
        Ok((table, bound_events, bound_semaphores))
    }
    pub fn startup_criticals(
        &self,
        ram: &memory::Memory,
    ) -> Result<(kernel_pool::Shadow, kernel_pool::Criticals), i32> {
        self.startup_ids(ram)?;
        self.context
            .as_ref()
            .ok_or(-4)?
            .kernel_context_criticals(ram)
    }
    pub fn startup_rom_table(
        &self,
        ram: &memory::Memory,
        image: &xbe::Image,
    ) -> Result<&handles::Handles, i32> {
        self.startup_ids(ram)?;
        if self.rom_key != Some((image.image_rom_grant(), image.image_digest())) {
            return Err(-5);
        }
        self.handles.as_ref().ok_or(-4)
    }

    pub fn startup_private_save(
        &self,
        ram: &memory::Memory,
        image: &xbe::Image,
    ) -> Result<u64, i32> {
        self.startup_rom_table(ram, image)?;
        Ok(self.private_save)
    }
    pub fn startup_regular_worker(
        &self,
        ram: &memory::Memory,
        image: &xbe::Image,
    ) -> Result<u64, i32> {
        self.startup_rom_table(ram, image)?;
        Ok(self.regular_worker)
    }
    pub fn startup_rom_ordinal(
        &self,
        ram: &memory::Memory,
        image: &xbe::Image,
        marker: u32,
    ) -> Result<u32, i32> {
        self.startup_rom_table(ram, image)?;
        kernel_pool::kernel_file_ordinal(
            ram,
            self.context.as_ref().ok_or(-4)?.kernel_context_id(),
            marker,
            if self.regular_worker != 0 {
                3
            } else if self.private_save != 0 {
                2
            } else {
                1
            },
        )
    }
    // Register every subsystem job before its shared owners can be closed. A
    // completion snapshot, including true, does not replace a consuming join.
    pub fn startup_track(&mut self, ram: &memory::Memory, task: u64) -> Result<(), i32> {
        self.startup_ids(ram)?;
        if task == 0 || task == self.creator || self.jobs.contains(&task) {
            return Err(-1);
        }
        if self.jobs.len() >= 32 {
            return Err(-2);
        }
        gamepkg::local_task_finished(task)?;
        self.jobs.push(task);
        Ok(())
    }
    // Spawn suspended and retain its join obligation before returning the job.
    // The caller supplies the reviewed Plan entry/argument, then resumes it.
    pub fn startup_spawn(
        &mut self,
        ram: &memory::Memory,
        entry: u32,
        argument: u64,
    ) -> Result<u64, i32> {
        self.startup_ids(ram)?;
        if self.jobs.len() >= 32 {
            return Err(-2);
        }
        let mut request = std::vec::Vec::new();
        request.extend(u32::to_le_bytes(entry));
        request.extend(u64::to_le_bytes(argument));
        request.extend(u32::to_le_bytes(1));
        let mut response = [0u8; 8];
        if gamepkg::local_call(10, &request, &mut response)? != 8 {
            return Err(-3);
        }
        let task = u64::from_le_bytes(response);
        self.jobs.push(task);
        Ok(task)
    }
    // Cleanup still accepts actual join evidence after cancellation/revocation.
    pub fn startup_accept_join(
        &mut self,
        ram: &memory::Memory,
        joined: &gamepkg::LocalTaskJoin,
    ) -> Result<(), i32> {
        self.startup_owner(ram)?;
        let mut found = self.jobs.len();
        for at in 0..self.jobs.len() {
            if self.jobs[at] == joined.task_id() {
                found = at;
                break;
            }
        }
        if found == self.jobs.len() {
            return Err(-4);
        }
        for at in found..self.jobs.len() - 1 {
            self.jobs[at] = self.jobs[at + 1];
        }
        let _ = self.jobs.pop();
        Ok(())
    }
    pub fn startup_join_task(
        &mut self,
        ram: &memory::Memory,
        task: u64,
    ) -> Result<gamepkg::LocalTaskJoin, i32> {
        self.startup_owner(ram)?;
        if !self.jobs.contains(&task) {
            return Err(-4);
        }
        let joined = gamepkg::local_task_join(task)?;
        self.startup_accept_join(ram, &joined)?;
        Ok(joined)
    }
    fn startup_close_status(&self) -> Result<(), i32> {
        if self.close_error == 0 {
            Ok(())
        } else {
            Err(self.close_error)
        }
    }
    // Nonblocking: stop/join users first. Never cancel them or confuse finished
    // with joined. Failed/partial startup cannot retry or enter native code.
    pub fn startup_close(&mut self, ram: &mut memory::Memory) -> Result<(), i32> {
        self.startup_owner(ram)?;
        if self.phase == 5 {
            return self.startup_close_status();
        }
        if !self.jobs.is_empty() {
            return Err(-2);
        }
        self.phase = 4;
        if self.map != 0 {
            let current = ram.kernel_handle();
            if current != 0 && current != self.map {
                return Err(-4);
            }
            if current == self.map {
                ram.kernel_clear()?;
            } else {
                match gamepkg::local_record_release(self.map) {
                    Ok(()) | Err(-4) => {}
                    Err(code) => return Err(code),
                }
            }
            self.map = 0;
        }
        if let Some(context) = self.context.as_mut() {
            let outcome = context.kernel_context_close(ram);
            if !context.kernel_context_closed() {
                return outcome;
            }
            if let Err(code) = outcome {
                if self.close_error == 0 {
                    self.close_error = code;
                }
            }
            self.context = None;
        }
        if let Some(domain) = self.criticals.as_mut() {
            let outcome = domain.criticals_close(ram, self.critical_shadow.as_ref().ok_or(-3)?);
            if !domain.criticals_closed() {
                return outcome;
            }
            if let Err(code) = outcome {
                if self.close_error == 0 {
                    self.close_error = code;
                }
            }
            self.criticals = None;
        }
        if let Some(map) = self.critical_shadow.as_mut() {
            let outcome = map.shadow_close(ram);
            if !map.shadow_closed() {
                return outcome;
            }
            if let Err(code) = outcome {
                if self.close_error == 0 {
                    self.close_error = code;
                }
            }
            self.critical_shadow = None;
        }
        if let Some(domain) = self.semaphores.as_mut() {
            let outcome = domain.semaphores_close(ram, self.handles.as_ref().ok_or(-3)?);
            if !domain.semaphores_closed() {
                return outcome;
            }
            if let Err(code) = outcome {
                if self.close_error == 0 {
                    self.close_error = code;
                }
            }
            self.semaphores = None;
        }
        if let Some(domain) = self.objects.as_mut() {
            let outcome = domain.events_close(ram, self.handles.as_ref().ok_or(-3)?);
            if !domain.events_closed() {
                return outcome;
            }
            if let Err(code) = outcome {
                if self.close_error == 0 {
                    self.close_error = code;
                }
            }
            self.objects = None;
        }
        if let Some(map) = self.handles.as_mut() {
            let outcome = map.handles_close(ram);
            if !map.handles_closed() {
                return outcome;
            }
            if let Err(code) = outcome {
                if self.close_error == 0 {
                    self.close_error = code;
                }
            }
            self.handles = None;
        }
        if let Some(map) = self.shadow.as_mut() {
            let outcome = kernel_pool::event_close_all(map, ram);
            match outcome {
                Ok(()) => {}
                Err(-4) => {
                    if self.close_error == 0 {
                        self.close_error = -4;
                    }
                }
                Err(code) => return Err(code),
            }
            let outcome = map.shadow_close(ram);
            if !map.shadow_closed() {
                return outcome;
            }
            if let Err(code) = outcome {
                if self.close_error == 0 {
                    self.close_error = code;
                }
            }
            self.shadow = None;
        }
        if let Some(registry) = self.registry.as_mut() {
            let outcome = registry.connections_close(ram);
            if !registry.connections_closed() {
                return outcome;
            }
            if let Err(code) = outcome {
                if self.close_error == 0 {
                    self.close_error = code;
                }
            }
            self.registry = None;
        }
        if let Some(services) = self.services.as_ref() {
            match services.service_close(ram) {
                Ok(()) => {}
                Err(-4) => {
                    // service_close can report dead root after releasing. Make
                    // the owned record's absence explicit without unlinking a
                    // replacement service belonging to somebody else.
                    match gamepkg::local_record_release(services.service_handle()) {
                        Ok(()) | Err(-4) => {}
                        Err(code) => return Err(code),
                    }
                    if self.close_error == 0 {
                        self.close_error = -4;
                    }
                }
                Err(code) => return Err(code),
            }
            self.services = None;
        }
        self.phase = 5;
        self.startup_close_status()
    }
}
