// Maintainer-only no-pending-interrupt native runtime fixture.
mod gamepkg {
    pub use crate::client::*;
}
fn main() {
    let runtime = local::runtime::Runtime::new(
        &[local::runtime::Window {
            address: 0x20000000,
            bytes: 65536,
        }],
        vec![],
        4 * 1024 * 1024,
        4,
        65536,
    )
    .unwrap();
    let window = runtime.windows().unwrap()[0].0;
    runtime.commit(window, 0, 65536, 2).unwrap();
    local::install(runtime).unwrap();
    for _ in 0..32 {
        let mut ram = memory::Memory::new().unwrap();
        ram.attach(0x740000, 65536, window, 0, true).unwrap();
        let services = kernel_pool::kernel_services_initialize(&ram, 0x10000, 2).unwrap();
        let mut owner = irql::Controller::controller_new(&ram).unwrap();
        let mut task = owner.controller_task(&ram).unwrap();
        assert_eq!(
            task.task_key(),
            owner.controller_task(&ram).unwrap().task_key()
        );
        assert_eq!(task.task_values(&ram).unwrap(), [0; 6]);
        let mut cpu = registers::Registers::new();
        cpu.fs_base = 0x740000;
        assert_eq!(task.irql_raise(&mut ram, &mut cpu, 2), Ok(0));
        assert_eq!(owner.controller_counts(&ram), Ok([1, 0, 1]));
        assert_eq!(ram.read_u8(0x740024), Ok(2));
        assert_eq!(task.irql_raise(&mut ram, &mut cpu, 4), Ok(2));
        assert_eq!(owner.controller_counts(&ram), Ok([1, 0, 1]));
        task.irql_lower(&mut ram, &mut cpu, 0).unwrap();
        assert_eq!(owner.controller_counts(&ram), Ok([0, 0, 2]));
        assert_eq!(ram.read_u8(0x740024), Ok(0));
        task.irql_disable(&mut ram, &mut cpu).unwrap();
        task.irql_disable(&mut ram, &mut cpu).unwrap();
        assert_eq!(owner.controller_counts(&ram), Ok([0, 1, 2]));
        assert_eq!(cpu.eflags & 0x200, 0);
        task.irql_enable_after_shadow(&mut ram, &mut cpu).unwrap();
        assert_eq!(owner.controller_counts(&ram), Ok([0, 0, 2]));
        assert_eq!(cpu.eflags & 0x200, 0x200);
        task.irql_lower(&mut ram, &mut cpu, 8).unwrap();
        assert_eq!(owner.controller_counts(&ram), Ok([0, 0, 2]));
        assert_eq!(task.irql_blocks_dpcs(&ram), Ok(true));
        assert_eq!(task.irql_to_dpc(&mut ram, &mut cpu), Ok(8));
        let saved = task.irql_enter_interrupt(&mut ram, &mut cpu, 16).unwrap();
        assert_eq!(saved, 2);
        task.irql_leave_interrupt(&mut ram, &mut cpu, saved)
            .unwrap();
        task.irql_lower(&mut ram, &mut cpu, 0).unwrap();
        assert_eq!(owner.controller_counts(&ram), Ok([0, 0, 4]));
        owner.controller_close(&ram).unwrap();
        assert!(owner.controller_closed());
        assert_eq!(task.task_values(&ram), Err(-4));
        services.service_close(&ram).unwrap();
        client::local_record_release(ram.call_handle()).unwrap();
    }
    local::shutdown();
    println!("PASS: native IRQL state, actual recursive gate, PCR publication and CLI/STI fields; fixture has no pending interrupts, not delivery qualification");
}
