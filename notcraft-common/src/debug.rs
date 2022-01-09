#[cfg(feature = "debug")]
mod inner {
    use super::DebugEvent;
    use crate::util::ChannelPair;
    use std::{
        any::{Any, TypeId},
        collections::HashSet,
    };

    lazy_static::lazy_static! {
        static ref DEBUG_EVENTS: flurry::HashMap<TypeId, Box<dyn DebugEventChannel>> = Default::default();
    }

    pub trait DebugEventChannel: Any + Send + Sync + 'static {
        fn clear(&self);
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

    impl<E: Send + Sync + 'static> DebugEventChannel for ChannelPair<E> {
        fn clear(&self) {
            self.rx.try_iter().for_each(drop);
        }
    }

    pub fn register_debug_event<E: DebugEvent>(enabled: Option<&HashSet<String>>) {
        enable_debug_event::<E>(enabled.map_or(true, |set| set.contains(E::name())));
    }

    pub fn enable_debug_event<E: DebugEvent>(enable: bool) {
        let id = TypeId::of::<E>();
        let channel = Box::new(ChannelPair::<E>::new()) as Box<dyn DebugEventChannel>;
        match enable {
            true => drop(DEBUG_EVENTS.pin().try_insert(id, channel.into())),
            false => drop(DEBUG_EVENTS.pin().remove(&id)),
        }
    }

    pub fn send_debug_event<E: DebugEvent>(event: E) {
        if let Some(channel) = DEBUG_EVENTS.pin().get(&TypeId::of::<E>()) {
            channel
                .downcast_ref::<ChannelPair<E>>()
                .unwrap()
                .tx
                .send(event)
                .unwrap();
        }
    }

    pub fn drain_debug_events<E: DebugEvent, F>(func: F)
    where
        F: FnMut(E),
    {
        if let Some(channel) = DEBUG_EVENTS.pin().get(&TypeId::of::<E>()) {
            let rx = &channel.downcast_ref::<ChannelPair<E>>().unwrap().rx;
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
