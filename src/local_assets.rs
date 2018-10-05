// local storage of thread-local data
// way to convert non-thread-local to thread-local
// way to refer to the thread-local data

// way to submit non-thread-local representation
impl<V, I> Asset for Mesh<V, I> {
    type Local = GlMesh<V, I>;

    fn to_local(self) -> Self::Local {
        mesh.to_gl_mesh(UsageType::StaticDraw).unwrap()
    }
}






pub trait Asset: Send + Sync {
    type Local;

    fn to_local(self) -> Self::Local;
}

pub struct Handle<A: ThreadLocalAsset> {
    id: usize,
    _marker: PhantomData<*const A>,
}

#[derive(Clone, Debug, Eq, PartialEq, Default)]
struct LocalStorageInner<A: ThreadLocalAsset> {
    counter: usize,
    map: HashMap<usize, A::Local>,
}

#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct SharedLocalStorage<A: ThreadLocalAsset> {
    inner: Rc<LocalStorageInner<A>>,
    recv: Receiver<A>,
    // We hold onto a sender so that we can clone it and give out copies to whoever needs one.
    sender_prototype: Sender<A>,
}

#[derive(Debug)]
struct AssetChannel<A> {
    sender: Mutex<Sender<A>>,
}

impl<A: Asset> SharedLocalStorage<A> {
    pub fn new() -> Self { unimplemented!() }
}