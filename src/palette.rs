
pub trait Palette {
    fn rgb(color: u8) -> (u8, u8, u8);
    fn rgba_opaque(color: u8) -> (u8, u8, u8, u8);
    fn rgba_transparent(color: u8) -> (u8, u8, u8, u8);
    fn is_transparent(alpha: u8) -> bool;
    fn transparent_alpha() -> u8;
}