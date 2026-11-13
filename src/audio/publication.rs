//! RCU pequeño: lectores acotados sin locks; publicación y destrucción solo en el escritor.
use std::sync::{
    Mutex,
    atomic::{AtomicPtr, AtomicUsize, Ordering::SeqCst},
};
pub(crate) struct Publication<T> {
    current: AtomicPtr<T>,
    readers: AtomicUsize,
    retired: Mutex<Vec<Box<T>>>,
}
// Todos los accesos al contenido son compartidos; publicación serializada por retired.
unsafe impl<T: Send + Sync> Send for Publication<T> {}
unsafe impl<T: Send + Sync> Sync for Publication<T> {}
impl<T> Publication<T> {
    pub fn new(value: T) -> Self {
        Self {
            current: AtomicPtr::new(Box::into_raw(Box::new(value))),
            readers: AtomicUsize::new(0),
            retired: Mutex::new(Vec::new()),
        }
    }
    pub fn read<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        self.readers.fetch_add(1, SeqCst);
        struct Guard<'a>(&'a AtomicUsize);
        impl Drop for Guard<'_> {
            fn drop(&mut self) {
                self.0.fetch_sub(1, SeqCst);
            }
        }
        let _guard = Guard(&self.readers);
        // SeqCst: un lector del puntero anterior es visible antes de que el escritor
        // compruebe readers; un lector posterior solo obtiene el puntero nuevo.
        f(unsafe { &*self.current.load(SeqCst) })
    }
    pub fn publish(&self, value: T) {
        let mut retired = self.retired.lock().unwrap();
        let previous = self.current.swap(Box::into_raw(Box::new(value)), SeqCst);
        retired.push(unsafe { Box::from_raw(previous) });
        if self.readers.load(SeqCst) == 0 {
            retired.clear();
        }
    }
}
impl<T> Drop for Publication<T> {
    fn drop(&mut self) {
        drop(unsafe { Box::from_raw(*self.current.get_mut()) });
    }
}
