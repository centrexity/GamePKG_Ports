// Production millisecond clock: preserve the original two separate truncations.
use crate::gamepkg;
use crate::kernel_pool;
use crate::memory;
pub fn timer_ms_value(counter: u64, frequency: u64, skew: u64) -> Result<i64, i32> {
    if counter > i64::MAX as u64 || frequency == 0 || frequency > i64::MAX as u64 {
        return Err(-1);
    }
    let host = (counter / frequency)
        .wrapping_mul(1000)
        .wrapping_add((counter % frequency).wrapping_mul(1000) / frequency);
    Ok(host.wrapping_add(skew / 1000000) as i64)
}
pub fn timer_now_ms(ram: &mut memory::Memory) -> Result<i64, i32> {
    let sample = gamepkg::local_performance_sample()?;
    let skew = kernel_pool::kernel_service_values(ram)?[2];
    timer_ms_value(sample.counter, sample.frequency, skew)
}
