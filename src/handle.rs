use std::collections::HashMap;
use std::cell::{Cell, RefCell, Ref};
use std::marker::PhantomData;

pub struct Handle<T>(usize, PhantomData<T>);
unsafe impl<T> Send for Handle<T> {}
unsafe impl<T> Sync for Handle<T> {}

#[derive(Clone, Debug)]
pub struct LocalPool<T> {
    unique_counter: Cell<usize>,
    asset_map: RefCell<HashMap<usize, T>>,
}

impl<T> Default for LocalPool<T> {
    fn default() -> Self { LocalPool { unique_counter: Cell::new(0), asset_map: RefCell::new(HashMap::new()) } }
}

impl<T> LocalPool<T> {
    pub fn insert(&self, item: T) -> Handle<T> {
        let counter = self.unique_counter.get();
        self.asset_map.borrow_mut().insert(counter, item);
        let ret = Handle(counter, PhantomData);
        self.unique_counter.set(counter + 1);
        ret
    }

    pub fn fetch(&self, handle: &Handle<T>) -> Ref<T> {
        Ref::map(self.asset_map.borrow(), |asset_map| &asset_map[&handle.0])
    }
}
