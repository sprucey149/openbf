pub(crate) fn step(seed: u32) -> u32 {
    seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223)
}

pub(crate) fn unit(seed: &mut u32) -> f32 {
    *seed = step(*seed);
    let hi = *seed >> 16;
    *seed = step(*seed);
    f32::from_bits(0x3f80_0000 | ((hi | (*seed & 0xffff_0000)) >> 9)) - 1.0
}
