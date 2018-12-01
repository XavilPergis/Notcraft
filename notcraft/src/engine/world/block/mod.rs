mod registry;

pub use self::registry::*;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default, Serialize, Deserialize)]
pub struct Faces<T> {
    pub top: T,
    pub bottom: T,
    pub right: T,
    pub left: T,
    pub front: T,
    pub back: T,
}

impl<T> Faces<T> {
    fn map<U, F>(self, mut func: F) -> Faces<U>
    where
        F: FnMut(T) -> U,
    {
        Faces {
            top: func(self.top),
            bottom: func(self.bottom),
            left: func(self.left),
            right: func(self.right),
            front: func(self.front),
            back: func(self.back),
        }
    }
}
