// Checked marker entrypoint for a startup which opted into file services.
use crate::gamepkg;
use crate::kernel_directories;
use crate::kernel_file;
use crate::kernel_startup;
use crate::memory;
use crate::registers;
use crate::xbe;

pub struct StartupFiles<'a, 'b> {
    owner: &'a kernel_startup::Startup,
    image: &'a xbe::Image<'b>,
    adapter: kernel_file::RomKernel,
    private_directories: Option<kernel_directories::SaveDirectories>,
}

impl<'a, 'b> StartupFiles<'a, 'b> {
    pub fn startup_files_new(
        owner: &'a kernel_startup::Startup,
        image: &'a xbe::Image<'b>,
        ram: &memory::Memory,
    ) -> Result<Self, i32> {
        let table = owner.startup_rom_table(ram, image)?;
        let adapter = kernel_file::RomKernel::rom_kernel_new(ram, table)?;
        let storage_grant = owner.startup_private_save(ram, image)?;
        let private_directories = if storage_grant == 0 {
            None
        } else {
            Some(kernel_directories::SaveDirectories::save_directories_new(
                ram,
                table,
                storage_grant,
            )?)
        };
        Ok(Self {
            owner,
            image,
            adapter,
            private_directories,
        })
    }

    pub fn startup_files_call(
        &mut self,
        ram: &mut memory::Memory,
        cpu: &mut registers::Registers,
        marker: u32,
    ) -> Result<(), i32> {
        let ordinal = self.owner.startup_rom_ordinal(ram, self.image, marker)?;
        let table = self.owner.startup_rom_table(ram, self.image)?;
        if let Some(directories) = self.private_directories.as_mut() {
            if startup_directory_route(ram, cpu, ordinal, directories)? {
                return directories.save_directory_invoke(table, ram, cpu, ordinal);
            }
        }
        let (_, rom_events) = self.owner.startup_objects(ram)?;
        self.adapter.rom_kernel_invoke(
            self.image.image_rom_source(),
            table,
            &rom_events,
            ram,
            cpu,
            ordinal,
        )
    }

    // The creator finishes file requests before releasing the kernel owner.
    pub fn startup_files_close(&mut self, ram: &memory::Memory) -> Result<(), i32> {
        let table = self.owner.startup_rom_table(ram, self.image)?;
        if let Some(directories) = self.private_directories.as_mut() {
            directories.save_directories_drain(ram, table)?;
        }
        self.adapter.rom_kernel_drain(ram, table)
    }
}

fn startup_directory_route(
    ram: &memory::Memory,
    cpu: &registers::Registers,
    ordinal: u32,
    directories: &kernel_directories::SaveDirectories,
) -> Result<bool, i32> {
    if !cpu.valid() || cpu.esp == 0 {
        return Err(-1);
    }
    match ordinal {
        218 => Ok(true),
        187 => {
            Ok(directories.save_directory_owns(ram.read_u32(cpu.esp.checked_add(4).ok_or(-1)?)?))
        }
        190 | 202 => {
            let attributes = ram.read_u32(cpu.esp.checked_add(12).ok_or(-1)?)?;
            let counted_path = kernel_file::rom_kernel_name(ram, attributes)?;
            let path = gamepkg::text_from_utf8(&counted_path)?;
            let partition = "\\Device\\Harddisk0\\partition1\\";
            if let Some(prefix) = path.get(..partition.len()) {
                return Ok(prefix.eq_ignore_ascii_case(partition));
            }
            Ok(false)
        }
        _ => Ok(false),
    }
}
