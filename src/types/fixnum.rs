#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FixNum(i32);

impl FixNum {
    pub fn new(val: i32) -> Self {
        Self(val)
    }

    pub fn into_inner(self) -> i32 {
        self.0
    }
}
