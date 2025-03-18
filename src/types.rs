pub type Int = i32;
pub type Float = f32;

pub fn as_int(b: bool) -> Int {
    if b {1} else {0}
}