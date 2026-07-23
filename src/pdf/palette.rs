//! The engraved palette, pulled on paper: same identities as the artifact
//! (brass planets, verdigris signs, steel houses, oxblood aspects), darkened
//! for cream ground; hairlines are ink pre-blended onto the paper tone.

pub const PAPER: (u8, u8, u8) = (244, 239, 226);
pub const INK: (u8, u8, u8) = (43, 39, 33);
pub const INK2: (u8, u8, u8) = (87, 80, 63);
pub const INK3: (u8, u8, u8) = (122, 114, 92);
pub const LINE: (u8, u8, u8) = (212, 207, 195);
pub const HAIRLINE: (u8, u8, u8) = (184, 179, 168);
pub const BRASS: (u8, u8, u8) = (138, 106, 28);
pub const VERDIGRIS: (u8, u8, u8) = (30, 111, 82);
pub const STEEL: (u8, u8, u8) = (60, 95, 150);
pub const OXBLOOD: (u8, u8, u8) = (142, 52, 70);

/// Element washes, pre-blended onto the paper tone (the artifact's
/// rgba washes against cream instead of night).
pub fn wash(element: &str) -> (u8, u8, u8) {
    match element {
        "fire" => (239, 224, 209),
        "earth" => (233, 224, 203),
        "air" => (238, 232, 213),
        _ => (229, 229, 224), // water
    }
}
