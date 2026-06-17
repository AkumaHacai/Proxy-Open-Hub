use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::adapter::{CoreAdapter, ImportInput, ImportMatch};
use crate::model::{CoreId, CoreManifest};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CoreDetection {
    pub core_id: CoreId,
    pub display_name: String,
    pub import_match: ImportMatch,
}

#[derive(Default)]
pub struct CoreRegistry {
    adapters: BTreeMap<CoreId, Box<dyn CoreAdapter>>,
}

impl CoreRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<A>(&mut self, adapter: A) -> Option<Box<dyn CoreAdapter>>
    where
        A: CoreAdapter + 'static,
    {
        let id = adapter.manifest().id.clone();
        self.adapters.insert(id, Box::new(adapter))
    }

    pub fn adapter(&self, core_id: &CoreId) -> Option<&dyn CoreAdapter> {
        self.adapters.get(core_id).map(Box::as_ref)
    }

    pub fn manifests(&self) -> Vec<&CoreManifest> {
        self.adapters
            .values()
            .map(|adapter| adapter.manifest())
            .collect()
    }

    pub fn detect_import(&self, input: &ImportInput) -> Vec<CoreDetection> {
        let mut matches = self
            .adapters
            .values()
            .map(|adapter| {
                let manifest = adapter.manifest();
                CoreDetection {
                    core_id: manifest.id.clone(),
                    display_name: manifest.display_name.clone(),
                    import_match: adapter.detect_import(input),
                }
            })
            .filter(|detection| detection.import_match.matched())
            .collect::<Vec<_>>();

        matches.sort_by(|left, right| {
            right
                .import_match
                .confidence
                .cmp(&left.import_match.confidence)
                .then_with(|| left.display_name.cmp(&right.display_name))
        });
        matches
    }
}
