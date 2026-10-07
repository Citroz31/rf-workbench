use crate::{Resource, ResourceManager};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
#[derive(Clone, Debug)]
pub struct Device {
    pub resource: String,
    pub idn: String,
    pub error: Option<String>,
}
pub fn identify(resources: &[String], timeout: Duration, cancelled: &AtomicBool) -> Vec<Device> {
    let mut seen = std::collections::BTreeSet::new();
    resources
        .iter()
        .filter(|r| seen.insert((*r).clone()))
        .take(64)
        .take_while(|_| !cancelled.load(Ordering::Acquire))
        .map(|r| {
            let result = ResourceManager
                .open_resource(r, timeout)
                .and_then(|mut s| s.query("*IDN?"));
            match result {
                Ok(idn) => Device {
                    resource: r.clone(),
                    idn,
                    error: None,
                },
                Err(e) => Device {
                    resource: r.clone(),
                    idn: String::new(),
                    error: Some(e.to_string()),
                },
            }
        })
        .collect()
}
pub fn physical(resources: &[String]) -> Vec<String> {
    resources
        .iter()
        .filter(|r| Resource::parse(r).ok().is_some_and(|x| x != Resource::Sim))
        .cloned()
        .collect()
}
