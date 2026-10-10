// Tracks which pads are present, which slot each holds, and what changed between scans
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashMap;

use crate::{
    backend::DiscoveredPad,
    models::{PadEvent, PadInfo},
};

#[derive(Default)]
pub struct Registry {
    /// The slot each pad held before, kept for the life of the process so a reconnected pad
    /// returns to its player number.
    remembered: HashMap<String, u32>,
    active: Vec<(String, PadInfo)>,
    /// The backend keys of pads that left in the last refresh.
    departed: Vec<String>,
}

fn info(pad: &DiscoveredPad, slot: u32, backend: &str) -> PadInfo {
    PadInfo {
        id: format!("gamepad:{slot}"),
        slot,
        name: pad.name.clone(),
        vendor_id: pad.vendor_id,
        product_id: pad.product_id,
        serial: pad.serial.clone(),
        transport: pad.transport,
        guid: pad.guid.clone(),
        motors: pad.motors,
        triggers: pad.triggers,
        top_tier: pad.top_tier,
        reason: pad.reason.clone(),
        backend: backend.to_string(),
    }
}

impl Registry {
    /// Compares `scan` with what was present and returns the events that describe the difference:
    /// removals first, then additions and changes in scan order.
    pub fn refresh(&mut self, backend: &str, scan: Vec<DiscoveredPad>) -> Vec<PadEvent> {
        let mut events = Vec::new();
        self.departed.clear();

        let mut seen: Vec<&str> = Vec::new();
        let scan: Vec<&DiscoveredPad> = scan
            .iter()
            .filter(|p| {
                let fresh = !seen.contains(&p.key.as_str());
                seen.push(&p.key);
                fresh
            })
            .collect();

        let departed = &mut self.departed;
        self.active.retain(|(key, pad)| {
            let present = scan.iter().any(|p| &p.key == key);
            if !present {
                departed.push(key.clone());
                events.push(PadEvent::Disconnected {
                    id: pad.id.clone(),
                    slot: pad.slot,
                });
            }
            present
        });

        for pad in scan {
            if let Some(existing) = self.active.iter_mut().find(|(key, _)| key == &pad.key) {
                let updated = info(pad, existing.1.slot, backend);
                if updated != existing.1 {
                    existing.1 = updated.clone();
                    events.push(PadEvent::Changed(updated));
                }
                continue;
            }
            let slot = self.slot_for(&pad.key);
            self.remembered.insert(pad.key.clone(), slot);
            let connected = info(pad, slot, backend);
            self.active.push((pad.key.clone(), connected.clone()));
            events.push(PadEvent::Connected(connected));
        }

        self.active.sort_by_key(|(_, pad)| pad.slot);
        events
    }

    fn slot_for(&self, key: &str) -> u32 {
        let taken = |slot: u32| self.active.iter().any(|(_, p)| p.slot == slot);
        match self.remembered.get(key) {
            Some(&slot) if !taken(slot) => slot,
            _ => (0..).find(|slot| !taken(*slot)).unwrap_or(0),
        }
    }

    /// Takes the keys of the pads that left in the last refresh.
    pub fn take_departed(&mut self) -> Vec<String> {
        std::mem::take(&mut self.departed)
    }

    pub fn pads(&self) -> Vec<PadInfo> {
        self.active.iter().map(|(_, pad)| pad.clone()).collect()
    }

    /// The backend key and info for `id`.
    pub fn find(&self, id: &str) -> Option<(String, PadInfo)> {
        self.active
            .iter()
            .find(|(_, pad)| pad.id == id)
            .map(|(key, pad)| (key.clone(), pad.clone()))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::models::Transport;

    pub(crate) fn pad(key: &str) -> DiscoveredPad {
        DiscoveredPad {
            key: key.to_string(),
            name: format!("Pad {key}"),
            vendor_id: 0x054c,
            product_id: 0x05c4,
            serial: Some(key.to_string()),
            transport: Transport::Usb,
            guid: "guid".to_string(),
            motors: 2,
            triggers: false,
            top_tier: 2,
            reason: None,
        }
    }

    fn ids(events: &[PadEvent]) -> Vec<String> {
        events
            .iter()
            .map(|e| match e {
                PadEvent::Connected(p) => format!("+{}", p.id),
                PadEvent::Changed(p) => format!("~{}", p.id),
                PadEvent::Disconnected { id, .. } => format!("-{id}"),
            })
            .collect()
    }

    #[test]
    fn pads_take_the_lowest_free_slot_in_scan_order() {
        let mut registry = Registry::default();
        let events = registry.refresh("mock", vec![pad("a"), pad("b")]);
        assert_eq!(ids(&events), ["+gamepad:0", "+gamepad:1"]);
        assert!(registry
            .refresh("mock", vec![pad("a"), pad("b")])
            .is_empty());
    }

    #[test]
    fn a_reconnected_pad_returns_to_its_slot() {
        let mut registry = Registry::default();
        registry.refresh("mock", vec![pad("a"), pad("b")]);
        let events = registry.refresh("mock", vec![pad("b")]);
        assert_eq!(ids(&events), ["-gamepad:0"]);

        // A newcomer arrives while a is away and takes the free slot 0 ...
        let events = registry.refresh("mock", vec![pad("b"), pad("c")]);
        assert_eq!(ids(&events), ["+gamepad:0"]);

        // ... so a, returning, cannot have it back and takes the next free one.
        let events = registry.refresh("mock", vec![pad("a"), pad("b"), pad("c")]);
        assert_eq!(ids(&events), ["+gamepad:2"]);
    }

    #[test]
    fn a_pad_returning_to_a_free_slot_keeps_it() {
        let mut registry = Registry::default();
        registry.refresh("mock", vec![pad("a"), pad("b")]);
        registry.refresh("mock", vec![pad("b")]);
        let events = registry.refresh("mock", vec![pad("a"), pad("b")]);
        assert_eq!(ids(&events), ["+gamepad:0"]);
    }

    #[test]
    fn removals_come_before_additions_and_changes_are_reported() {
        let mut registry = Registry::default();
        registry.refresh("mock", vec![pad("a"), pad("b")]);
        let mut changed = pad("b");
        changed.top_tier = 0;
        changed.reason = Some("No write access".into());
        let events = registry.refresh("mock", vec![changed, pad("c")]);
        assert_eq!(ids(&events), ["-gamepad:0", "~gamepad:1", "+gamepad:0"]);
    }

    #[test]
    fn duplicate_keys_in_one_scan_count_once() {
        let mut registry = Registry::default();
        registry.refresh("mock", vec![pad("a"), pad("a")]);
        assert_eq!(registry.pads().len(), 1);
    }

    #[test]
    fn find_resolves_an_id_to_its_key() {
        let mut registry = Registry::default();
        registry.refresh("mock", vec![pad("a")]);
        assert_eq!(registry.find("gamepad:0").unwrap().0, "a");
        assert!(registry.find("gamepad:9").is_none());
    }
}
