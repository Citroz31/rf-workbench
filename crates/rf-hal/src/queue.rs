//! Single producer / single consumer bounded queue. Neither endpoint is Clone
//! or Sync; moving either endpoint to its own worker is supported.
use std::{
    cell::{Cell, UnsafeCell},
    marker::PhantomData,
    mem::MaybeUninit,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
struct Ring<T> {
    slots: Box<[UnsafeCell<MaybeUninit<T>>]>,
    head: AtomicUsize,
    tail: AtomicUsize,
    dropped: AtomicUsize,
}
// SAFETY: one producer owns writes, one consumer owns reads. Release/Acquire
// publishes initialized slots and releases consumed slots before reuse.
unsafe impl<T: Send> Sync for Ring<T> {}
impl<T> Drop for Ring<T> {
    fn drop(&mut self) {
        let mut h = *self.head.get_mut();
        let t = *self.tail.get_mut();
        while h != t {
            let i = h % self.slots.len(); // SAFETY: all endpoints are gone; these are exactly the unread initialized slots.
            unsafe { self.slots[i].get_mut().assume_init_drop() };
            h = h.wrapping_add(1);
        }
    }
}
pub struct Producer<T> {
    ring: Arc<Ring<T>>,
    _single: PhantomData<Cell<()>>,
}
pub struct Consumer<T> {
    ring: Arc<Ring<T>>,
    _single: PhantomData<Cell<()>>,
}
pub fn bounded<T: Send>(capacity: usize) -> (Producer<T>, Consumer<T>) {
    assert!(capacity > 0 && capacity <= 65536);
    let capacity = capacity.next_power_of_two();
    let ring = Arc::new(Ring {
        slots: (0..capacity)
            .map(|_| UnsafeCell::new(MaybeUninit::uninit()))
            .collect(),
        head: AtomicUsize::new(0),
        tail: AtomicUsize::new(0),
        dropped: AtomicUsize::new(0),
    });
    (
        Producer {
            ring: ring.clone(),
            _single: PhantomData,
        },
        Consumer {
            ring,
            _single: PhantomData,
        },
    )
}
impl<T> Producer<T> {
    pub fn push(&mut self, value: T) -> std::result::Result<(), T> {
        let t = self.ring.tail.load(Ordering::Relaxed);
        let h = self.ring.head.load(Ordering::Acquire);
        if t.wrapping_sub(h) >= self.ring.slots.len() {
            self.ring.dropped.fetch_add(1, Ordering::Relaxed);
            return Err(value);
        } // SAFETY: this slot is unused and only this producer may initialize it.
        unsafe { (*self.ring.slots[t % self.ring.slots.len()].get()).write(value) };
        self.ring.tail.store(t.wrapping_add(1), Ordering::Release);
        Ok(())
    }
}
impl<T> Consumer<T> {
    pub fn pop(&mut self) -> Option<T> {
        let h = self.ring.head.load(Ordering::Relaxed);
        if h == self.ring.tail.load(Ordering::Acquire) {
            return None;
        } // SAFETY: Acquire observed an initialized slot; only this consumer reads it.
        let value =
            unsafe { (*self.ring.slots[h % self.ring.slots.len()].get()).assume_init_read() };
        self.ring.head.store(h.wrapping_add(1), Ordering::Release);
        Some(value)
    }
    pub fn overflows(&self) -> usize {
        self.ring.dropped.load(Ordering::Relaxed)
    }
}
