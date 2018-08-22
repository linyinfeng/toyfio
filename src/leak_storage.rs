use std::marker::PhantomData;
use log::debug;

pub struct LeakStorage<T> {
    phantom: PhantomData<T>
}

impl<T> LeakStorage<T> {
    /// "Insert" a item
    /// 
    /// The result of this function must be used or the item's memory will leak
    #[must_use = "using key returned by insert is the only way get or remove the resource"]
    pub fn insert(item: T) -> usize {
        let boxed = Box::new(item);
        let key = Box::leak(boxed) as *mut _ as usize;
        debug!("leak item and get key {}", key);
        key
    }

    /// Remove a item
    /// 
    /// Unsafe, key must be valid
    pub unsafe fn remove(key: usize) {
        debug!("clean item at key {}", key);
        Box::from_raw(key as *mut T);
        // box dropped
    }

    /// Move item out of the "container"
    /// 
    /// Unsafe, key must be valid
    pub unsafe fn get(key: usize) -> T {
        debug!("move item at key {}", key);
        *Box::from_raw(key as *mut T)
    }

    /// Get a reference of a item
    /// 
    /// Unsafe, key must be valid
    #[allow(dead_code)] // Maybe unused in this project
    pub unsafe fn get_ref<'a>(key: usize) -> &'static T {
        debug!("access item at key {}", key);
        &*(key as *const T)
    }

    /// Get a mutable reference of a item
    /// 
    /// Unsafe, key must be valid
    pub unsafe fn get_ref_mut<'a>(key: usize) -> &'static mut T {
        debug!("access item at key {}", key);
        &mut *(key as *mut T)
    }
}