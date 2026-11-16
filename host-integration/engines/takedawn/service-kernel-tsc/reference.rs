#[allow(unused_imports)]
mod gamepkg {
    pub use crate::client::*;
}
unsafe extern "C" {
    fn tsc_reference(family: u32, inputs: *const u64) -> u64;
}
fn main() {
    let mut checks = 0;
    for (family, frequency) in [
        1u64,
        3,
        10000000,
        1000000000,
        3579545,
        733333333,
        i64::MAX as u64,
        1000000,
    ]
    .into_iter()
    .enumerate()
    {
        let origin = if family % 2 == 0 { 0u64 } else { 0x123456u64 };
        for turn in 0..96u64 {
            let delta = [
                0u64,
                1,
                2,
                3,
                frequency / 2,
                frequency - 1,
                frequency,
                frequency.saturating_add(1),
                0x123456789,
                i64::MAX as u64 - origin,
            ][turn as usize % 10]
                .min(i64::MAX as u64 - origin);
            let skew = [
                0u64,
                1,
                99,
                100,
                999999999,
                1000000000,
                1000000001,
                100000000000000,
                u64::MAX,
            ][turn as usize % 9];
            let inputs = [
                origin,
                origin + delta,
                frequency,
                skew,
                u64::from(turn == 0),
            ];
            let expected = unsafe { tsc_reference(family as u32, inputs.as_ptr()) };
            assert_eq!(
                kernel_pool::kernel_tsc_value(inputs[1], origin, frequency, skew),
                Ok(expected),
                "family {family} turn {turn}"
            );
            checks += 1;
        }
    }
    for values in [
        [0u64, 1, 1, 0],
        [0, 0, 0, 0],
        [1, 0, u64::MAX, 0],
        [u64::MAX, 0, 1, 0],
    ] {
        assert_eq!(
            kernel_pool::kernel_tsc_value(values[0], values[1], values[2], values[3]),
            Err(-1)
        );
    }
    println!("PASS: {checks} original TSC calls, eight independent first-use origins/frequencies, fractional/long deltas, unsigned intermediate/result wrap, full skew domain and invalid counter/frequency controls; pure arithmetic only");
}
