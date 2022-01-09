#[cfg(feature = "debug")]
mod inner {
    use super::DebugEvent;
    use crate::util::ChannelPair;
    use std::{
        any::{Any, TypeId},
        collections::HashSet,
        sync::atomic::{AtomicBool, Ordering},
    };

    lazy_static::lazy_static! {
        static ref DEBUG_EVENTS: flurry::HashMap<TypeId, Box<dyn DebugEventChannel>> = Default::default();
    }

    pub trait DebugEventChannel: Any + Send + Sync + 'static {
        fn clear(&self);
        fn set_drained(&self);
    }

    impl dyn DebugEventChannel {
        fn downcast_ref<T: Send + Sync + 'static>(&self) -> Option<&T> {
            if self.type_id() == TypeId::of::<T>() {
                Some(unsafe { &*(self as *const dyn DebugEventChannel as *const T) })
            } else {
                None
            }
        }
    }

    struct DebugChannel<E> {
        inner: ChannelPair<E>,
        drained: AtomicBool,
    }

    impl<E: Send + Sync + 'static> DebugEventChannel for DebugChannel<E> {
        fn clear(&self) {
            // the `self.drained` flag is needed because there might be events that come in
            // after the channel has been drained, but before the channels are cleared. we
            // want to clear a channel if nobody is listening to it, though, to avoid debug
            // events piling up.
            if !self.drained.swap(false, Ordering::SeqCst) {
                self.inner.rx.try_iter().for_each(drop);
            }
        }

        fn set_drained(&self) {
            self.drained.store(true, Ordering::SeqCst);
        }
    }

    pub fn register_debug_event<E: DebugEvent>(enabled: Option<&HashSet<String>>) {
        enable_debug_event::<E>(enabled.map_or(true, |set| set.contains(E::name())));
    }

    pub fn enable_debug_event<E: DebugEvent>(enable: bool) {
        let id = TypeId::of::<E>();
        let channel = Box::new(DebugChannel {
            inner: ChannelPair::<E>::new(),
            drained: AtomicBool::new(false),
        }) as Box<dyn DebugEventChannel>;
        match enable {
            true => drop(DEBUG_EVENTS.pin().try_insert(id, channel.into())),
            false => drop(DEBUG_EVENTS.pin().remove(&id)),
        }
    }

    pub fn send_debug_event<E: DebugEvent>(event: E) {
        if let Some(channel) = DEBUG_EVENTS.pin().get(&TypeId::of::<E>()) {
            let tx = &channel.downcast_ref::<DebugChannel<E>>().unwrap().inner.tx;
            tx.send(event).unwrap();
        }
    }

    pub fn drain_debug_events<E: DebugEvent, F>(func: F)
    where
        F: FnMut(E),
    {
        if let Some(channel) = DEBUG_EVENTS.pin().get(&TypeId::of::<E>()) {
            channel.set_drained();
            let rx = &channel.downcast_ref::<DebugChannel<E>>().unwrap().inner.rx;
            rx.try_iter().for_each(func);
        }
    }

    pub fn clear_debug_events() {
        for value in DEBUG_EVENTS.pin().values() {
            value.clear();
        }
    }
}

// use dummy implementations that do nothing when the debug feature is disabled,
// so we can still call these functions unconditionally, for simplicity.
#[cfg(not(feature = "debug"))]
mod inner {
    use super::DebugEvent;
    use std::collections::HashSet;

    pub fn register_debug_event<E: DebugEvent>(_enabled: Option<&HashSet<String>>) {}

    pub fn enable_debug_event<E: DebugEvent>(_enable: bool) {}

    pub fn send_debug_event<E: DebugEvent>(_event: E) {}

    pub fn drain_debug_events<E: DebugEvent, F>(_func: F)
    where
        F: FnMut(E),
    {
    }

    pub fn clear_debug_events() {}
}

pub use inner::*;

pub trait DebugEvent: Send + Sync + 'static {
    fn name() -> &'static str;
}

#[macro_export]
macro_rules! debug_events {
    ($modname:ident, $($type:path => $name:expr,)*) => {
        pub mod $modname {
            use super::*;

            $(impl $crate::debug::DebugEvent for $type {
                fn name() -> &'static str {
                    $name
                }
            })*

            pub fn enumerate(enabled: Option<&std::collections::HashSet<String>>) {
                $($crate::debug::register_debug_event::<$type>(enabled);)*
            }
        }
    };
}

pub use debug_events;
