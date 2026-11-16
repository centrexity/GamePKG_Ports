pub fn pacing_case(index: u32) -> [u64; 3] {
    let base = 1000000000000u64;
    if index == 0 {
        return [0, 1, 0];
    }
    if index == 1 {
        return [1, base, 8333333];
    }
    if index < 402 {
        let cycle = u64::from((index - 2) / 2);
        let now = base + (cycle + 1 + cycle / 20) * 16666666;
        return [if index & 1 == 0 { 1 } else { 4 }, now, 0];
    }
    if index == 402 {
        return [2, 0, 0];
    }
    if index == 403 {
        return [1, base + 10000000000, 11111111];
    }
    if index < 416 {
        let cycle = u64::from((index - 404) / 2);
        return [
            if index & 1 == 0 { 1 } else { 4 },
            base + 10000000000 + (cycle + 1) * 16666666,
            0,
        ];
    }
    if index == 416 {
        return [1, base + 11000000000, 8333333];
    }
    if index == 417 {
        return [1, base + 11003700000, 8333333];
    }
    if index < 658 {
        return [
            1,
            base + 11003700000 + u64::from(index - 417) * 8333333,
            8333333,
        ];
    }
    if index == 658 {
        return [4, base + 13003700000, 0];
    }
    if index == 659 {
        return [4, base + 14003700000, 0];
    }
    if index == 660 || index == 661 {
        return [3, 0, 0];
    }
    if index == 662 {
        return [0, 0, 0];
    }
    if index == 663 {
        return [1, 0, 1];
    }
    if index == 664 {
        return [0, 1, 0];
    }
    if index == 665 {
        return [2, 0, 0];
    }
    if index == 666 {
        return [1, base + 15000000000, 0];
    }
    if index == 667 {
        return [1, base + 15002000000, 2000000];
    }
    if index == 668 {
        return [1, base + 15005800000, 0];
    }
    if index == 669 {
        return [4, base + 15005800000, 0];
    }
    if index == 670 {
        return [1, base + 16000000000, 50000000];
    }
    if index == 671 {
        return [1, base + 16056000000, 0];
    }
    if index == 672 {
        return [4, base + 16056000000, 0];
    }
    if index == 673 {
        return [2, 0, 0];
    }
    if index == 674 {
        return [1, base + 17000000000, 16666666];
    }
    if (675..=677).contains(&index) {
        return [1, base + 17000000000 + u64::from(index - 674) * 25000000, 0];
    }
    if index == 678 {
        return [4, base + 17075000000, 0];
    }
    if index == 679 {
        return [1, base + 18000000000, 10000000];
    }
    if (680..=687).contains(&index) {
        return [1, base + 18000000000 + u64::from(index - 679) * 3000000, 0];
    }
    if index == 688 {
        return [4, base + 18024000000, 0];
    }
    if (689..=721).contains(&index) {
        let group = (index - 689) / 3;
        let periods = [
            16666666u64,
            16666666,
            16666666,
            16499999,
            16500000,
            16500001,
            16833331,
            16833332,
            16833333,
            8250000,
            8416666,
        ];
        let reference = base + 20000000000 + u64::from(group) * 1000000000;
        return match (index - 689) % 3 {
            0 => [2, 0, 0],
            1 => [1, reference, periods[group as usize]],
            _ => [
                4,
                reference
                    + if group < 3 {
                        249999999 + u64::from(group)
                    } else {
                        10000000
                    },
                0,
            ],
        };
    }
    let now = base + 100000000000 + u64::from(index) * 20000000;
    match index % 12 {
        0 => [1, now, 16666666],
        1 => [1, now, 8333333],
        2 => [1, now, 6944444],
        3 => [1, now, 0],
        4 => [4, now, 0],
        5 => [3, 0, 0],
        6 => [2, 0, 0],
        7 => [0, u64::from(index & 16 != 0), 0],
        8 => [5, 0, 0],
        9 => [1, now, if index & 32 == 0 { 1 } else { 50000001 }],
        10 => [4, now + 2000000000, 0],
        _ => [4, now, 0],
    }
}
pub fn pacing_apply(
    state: &mut crate::pacing::Pacing,
    operation: u64,
    first: u64,
    second: u64,
) -> Result<[u64; 4], i32> {
    let mut result = [0u64; 4];
    match operation {
        0 => {
            state.pacing_auto(first != 0);
            result[0] = u64::from(state.pacing_locked());
        }
        1 => result[0] = u64::from(state.pacing_refresh(first, second)),
        2 => state.pacing_relearn(),
        3 => result[0] = u64::from(state.pacing_rate_changed()),
        4 => {
            let value = state.pacing_step(first)?;
            result[0] = value[0];
            result[1] = value[1];
            result[2] = value[2];
        }
        5 => result[0] = u64::from(state.pacing_locked()),
        _ => return Err(-1),
    }
    Ok(result)
}
