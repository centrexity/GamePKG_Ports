// Checked marker entrypoint for a startup which opted into optical files.
use crate::kernel_file;
use crate::kernel_startup;
use crate::memory;
use crate::registers;
use crate::xbe;

pub struct StartupFiles<'a, 'b> {
    owner: &'a kernel_startup::Startup,
    image: &'a xbe::Image<'b>,
    adapter: kernel_file::RomKernel,
}

impl<'a, 'b> StartupFiles<'a, 'b> {
    pub fn startup_files_new(
        owner: &'a kernel_startup::Startup,
        image: &'a xbe::Image<'b>,
        ram: &memory::Memory,
    ) -> Result<Self, i32> {
        let table = owner.startup_rom_table(ram, image)?;
        let adapter = kernel_file::RomKernel::rom_kernel_new(ram, table)?;
        Ok(Self {
            owner,
            image,
            adapter,
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
        self.adapter
            .rom_kernel_invoke(self.image.image_rom_source(), table, ram, cpu, ordinal)
    }

    // The creator finishes file requests before releasing the kernel owner.
    pub fn startup_files_close(&mut self, ram: &memory::Memory) -> Result<(), i32> {
        let table = self.owner.startup_rom_table(ram, self.image)?;
        self.adapter.rom_kernel_drain(ram, table)
    }
}
