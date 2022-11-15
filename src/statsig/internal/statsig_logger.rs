use std::sync::{Arc, RwLock};
use std::thread;
use std::thread::JoinHandle;
use std::mem::replace;
use std::time::Duration;
use crate::statsig::internal::statsig_network::StatsigNetwork;
use crate::{StatsigEvent, StatsigOptions};
use crate::statsig::statsig_event::StatsigEventInternal;

pub struct StatsigLogger {
    network: Arc<StatsigNetwork>,
    events: Arc<RwLock<Vec<StatsigEventInternal>>>,
    max_queue_size: u32,
    flush_interval_ms: u32,
    bg_thread_handle: Option<JoinHandle<()>>,
}

impl StatsigLogger {
    pub fn new(network: Arc<StatsigNetwork>, options: &StatsigOptions) -> Self {
        let mut inst = Self {
            network,
            events: Arc::from(RwLock::from(vec![])),
            max_queue_size: options.logger_max_queue_size,
            flush_interval_ms: options.logger_flush_interval_ms,
            bg_thread_handle: None,
        };
        inst.spawn_bg_thread();
        inst
    }

    pub fn spawn_bg_thread(&mut self) {
        let events = self.events.clone();
        let network = self.network.clone();
        let interval = Duration::from_millis(self.flush_interval_ms as u64);

        self.bg_thread_handle = Some(thread::spawn(move || loop {
            Self::flush_impl(&network, &events); // TODO: await this
            thread::sleep(interval)
        }));
    }

    pub fn enqueue(&self, event: StatsigEventInternal) {
        if let Some(mut mut_events) = self.events.write().ok() {
            mut_events.push(event);
        };
    }

    pub async fn flush(&self) {
        Self::flush_impl(&self.network, &self.events).await;
    }
    
    async fn flush_impl(network: &StatsigNetwork, events: &RwLock<Vec<StatsigEventInternal>>) {
        let count = match events.read().ok() {
            Some(e) => e.len(),
            _ => return,
        };

        if count == 0 {
            return;
        }

        let mut mut_events = events.write().ok().unwrap();
        let local_events = replace(&mut *mut_events, Vec::new());
        drop(mut_events);
        network.send_events(&local_events).await
    }
}