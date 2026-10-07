//! Acquisition and DSP run on separate workers. Full queues drop the newest
//! frame; counters and a discontinuity on the next accepted frame expose loss.
use crate::{Result, queue, transport::Io};
use rf_core::dsp::IqFrame;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
pub struct Pump {
    consumer: queue::Consumer<IqFrame>,
    stop: Arc<AtomicBool>,
    error: Arc<Mutex<Option<String>>>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Pump {
    /// The read callback must enforce its own timeout. Native HAL transports
    /// and supervised Python adapters meet that contract.
    pub fn spawn(
        mut read: impl FnMut() -> Result<IqFrame> + Send + 'static,
        capacity: usize,
    ) -> Self {
        let (mut producer, consumer) = queue::bounded(capacity);
        let stop = Arc::new(AtomicBool::new(false));
        let error = Arc::new(Mutex::new(None));
        let (s, e) = (stop.clone(), error.clone());
        let thread = std::thread::spawn(move || {
            let mut lost = false;
            while !s.load(Ordering::Acquire) {
                match read() {
                    Ok(mut f) => {
                        if lost {
                            f.time.discontinuity = true;
                        }
                        lost = producer.push(f).is_err();
                    }
                    Err(reason) => {
                        if let Ok(mut slot) = e.lock() {
                            *slot = Some(reason);
                        }
                        break;
                    }
                }
            }
        });
        Self {
            consumer,
            stop,
            error,
            thread: Some(thread),
        }
    }
    pub fn from_io(mut io: Io, capacity: usize) -> Self {
        Self::spawn(move || io.read(), capacity)
    }
    pub fn read(&mut self, timeout: std::time::Duration) -> Result<IqFrame> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            if let Some(f) = self.consumer.pop() {
                return Ok(f);
            }
            if let Ok(slot) = self.error.lock()
                && let Some(e) = slot.as_ref()
            {
                return Err(e.clone());
            }
            if std::time::Instant::now() >= deadline {
                return Err("Streaming : délai dépassé".into());
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
    pub fn overflows(&self) -> usize {
        self.consumer.overflows()
    }
}
impl Drop for Pump {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(h) = self.thread.take() {
            let _ = h.join();
        }
    }
}
pub enum Source {
    Direct(Io),
    Streaming(Pump, std::time::Duration),
}
impl Source {
    pub fn open(c: &rf_core::dsp::Settings) -> Result<Self> {
        let io = Io::open(c, true)?;
        if matches!(
            c.io_backend.as_str(),
            "TCP" | "UDP" | "SOAPY" | "UHD" | "IIO" | "AUDIO" | "ZMQ"
        ) {
            Ok(Self::Streaming(
                Pump::from_io(io, 4),
                std::time::Duration::from_millis(c.timeout_ms),
            ))
        } else {
            Ok(Self::Direct(io))
        }
    }
    pub fn read(&mut self) -> Result<IqFrame> {
        match self {
            Self::Direct(io) => io.read(),
            Self::Streaming(p, t) => p.read(*t),
        }
    }
    pub fn overflows(&self) -> usize {
        match self {
            Self::Direct(_) => 0,
            Self::Streaming(p, _) => p.overflows(),
        }
    }
}
