//! Adapter registry for managing and looking up target adapters.

use std::collections::HashMap;
use std::sync::Arc;

use crate::TargetAdapter;

use anyhow::bail;

/// Error type for adapter registry operations.
#[derive(Debug, Clone)]
pub struct AdapterError {
    pub target: String,
    pub message: String,
}

impl std::fmt::Display for AdapterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "adapter error for '{}': {}", self.target, self.message)
    }
}

impl std::error::Error for AdapterError {}

/// A boxed adapter that can be stored in the registry.
pub type BoxedAdapter = Box<dyn TargetAdapter>;

/// Reference to a registered adapter.
#[derive(Clone)]
pub struct RegisteredAdapter {
    pub id: &'static str,
    adapter: Arc<BoxedAdapter>,
}

impl std::fmt::Debug for RegisteredAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegisteredAdapter")
            .field("id", &self.id)
            .finish()
    }
}

impl RegisteredAdapter {
    /// Creates a new registered adapter from a target adapter.
    pub fn new(adapter: impl TargetAdapter + 'static) -> Self {
        let id = adapter.id();
        Self {
            id,
            adapter: Arc::new(Box::new(adapter)),
        }
    }

    /// Returns the adapter's ID.
    pub fn id(&self) -> &'static str {
        self.id
    }

    /// Returns a reference to the underlying adapter.
    pub fn adapter(&self) -> &dyn TargetAdapter {
        &**self.adapter
    }

    /// Returns the adapter as a trait object.
    pub fn as_trait(&self) -> &dyn TargetAdapter {
        &**self.adapter
    }
}

/// Thread-safe registry for target adapters.
#[derive(Default)]
pub struct AdapterRegistry {
    adapters: HashMap<&'static str, RegisteredAdapter>,
}

impl AdapterRegistry {
    /// Creates a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers an adapter with the registry.
    ///
    /// # Panics
    ///
    /// Panics if an adapter with the same ID is already registered.
    pub fn register<T: TargetAdapter + 'static>(&mut self, adapter: T) -> &mut Self {
        let id = adapter.id();
        if self.adapters.contains_key(id) {
            panic!("adapter '{}' is already registered", id);
        }
        self.adapters.insert(id, RegisteredAdapter::new(adapter));
        self
    }

    /// Returns true if an adapter with the given ID is registered.
    pub fn contains(&self, id: &str) -> bool {
        self.adapters.contains_key(id)
    }

    /// Returns the registered adapter with the given ID, if present.
    pub fn get(&self, id: &str) -> Option<&RegisteredAdapter> {
        self.adapters.get(id)
    }

    /// Returns the adapter with the given ID, or an error if not found.
    pub fn get_required(&self, id: &str) -> anyhow::Result<&RegisteredAdapter> {
        self.get(id).ok_or_else(|| {
            anyhow::anyhow!(
                "unknown target '{}'. Available targets: {}",
                id,
                self.available_targets().join(", ")
            )
        })
    }

    /// Returns a list of all registered adapter IDs.
    pub fn available_targets(&self) -> Vec<&'static str> {
        self.adapters.keys().copied().collect()
    }

    /// Returns the number of registered adapters.
    pub fn len(&self) -> usize {
        self.adapters.len()
    }

    /// Returns true if no adapters are registered.
    pub fn is_empty(&self) -> bool {
        self.adapters.is_empty()
    }

    /// Returns an iterator over all registered adapters.
    pub fn iter(&self) -> impl Iterator<Item = &RegisteredAdapter> {
        self.adapters.values()
    }

    /// Validates that a scenario exists for the given target.
    pub fn validate_scenario(&self, target: &str, scenario: &str) -> anyhow::Result<()> {
        let adapter = self.get_required(target)?;
        if adapter.adapter().supports_scenario(scenario) {
            Ok(())
        } else {
            let scenarios = adapter
                .adapter()
                .discover_scenarios()
                .iter()
                .map(|s| s.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            bail!(
                "unknown scenario '{}' for target '{}'. Available scenarios: {}",
                scenario,
                target,
                scenarios
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AdapterContext, CollectedData, LaunchedRun, PreparedRun, ScenarioRef, ScenarioSpec,
    };

    struct TestAdapter {
        id: &'static str,
        scenarios: Vec<ScenarioRef>,
    }

    impl TestAdapter {
        fn new(id: &'static str, scenarios: Vec<&str>) -> Self {
            Self {
                id,
                scenarios: scenarios
                    .into_iter()
                    .map(|name| ScenarioRef {
                        name: name.to_string(),
                        description: None,
                    })
                    .collect(),
            }
        }
    }

    impl TargetAdapter for TestAdapter {
        fn id(&self) -> &'static str {
            self.id
        }

        fn discover_scenarios(&self) -> Vec<ScenarioRef> {
            self.scenarios.clone()
        }

        fn prepare(
            &self,
            _ctx: &AdapterContext,
            _spec: &ScenarioSpec,
        ) -> anyhow::Result<PreparedRun> {
            Ok(PreparedRun::default())
        }

        fn launch(
            &self,
            _ctx: &AdapterContext,
            _prepared: &PreparedRun,
        ) -> anyhow::Result<LaunchedRun> {
            Ok(LaunchedRun {
                pid: Some(1),
                window_id: None,
                command: crate::CommandSpec::default(),
                env: std::collections::BTreeMap::new(),
            })
        }

        fn collect(
            &self,
            _ctx: &AdapterContext,
            _run: &LaunchedRun,
        ) -> anyhow::Result<CollectedData> {
            Ok(CollectedData::default())
        }

        fn stop(&self, _ctx: &AdapterContext, _run: &LaunchedRun) -> anyhow::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn registry_empty() {
        let reg = AdapterRegistry::new();
        assert!(reg.is_empty());
        assert_eq!(reg.len(), 0);
        assert!(!reg.contains("test"));
    }

    #[test]
    fn registry_register_adapter() {
        let mut reg = AdapterRegistry::new();
        reg.register(TestAdapter::new("test", vec!["scenario_a", "scenario_b"]));

        assert!(!reg.is_empty());
        assert_eq!(reg.len(), 1);
        assert!(reg.contains("test"));
        assert_eq!(reg.available_targets(), vec!["test"]);
    }

    #[test]
    fn registry_get_adapter() {
        let mut reg = AdapterRegistry::new();
        reg.register(TestAdapter::new("test", vec!["scenario_a"]));

        let adapter = reg.get("test").unwrap();
        assert_eq!(adapter.id(), "test");
    }

    #[test]
    fn registry_get_missing_adapter() {
        let reg = AdapterRegistry::new();
        assert!(reg.get("missing").is_none());
    }

    #[test]
    fn registry_get_required_missing_adapter() {
        let reg = AdapterRegistry::new();
        let result = reg.get_required("missing");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("unknown target 'missing'"));
        assert!(err.to_string().contains("Available targets:"));
    }

    #[test]
    fn registry_multiple_adapters() {
        let mut reg = AdapterRegistry::new();
        reg.register(TestAdapter::new("adapter_a", vec!["a1", "a2"]))
            .register(TestAdapter::new("adapter_b", vec!["b1"]));

        assert_eq!(reg.len(), 2);
        assert!(reg.contains("adapter_a"));
        assert!(reg.contains("adapter_b"));
    }

    #[test]
    fn registry_iter() {
        let mut reg = AdapterRegistry::new();
        reg.register(TestAdapter::new("a", vec![]))
            .register(TestAdapter::new("b", vec![]));

        let ids: Vec<_> = reg.iter().map(|a| a.id()).collect();
        assert!(ids.contains(&"a"));
        assert!(ids.contains(&"b"));
    }

    #[test]
    fn registry_validate_scenario() {
        let mut reg = AdapterRegistry::new();
        reg.register(TestAdapter::new("test", vec!["valid_scenario"]));

        assert!(reg.validate_scenario("test", "valid_scenario").is_ok());
    }

    #[test]
    fn registry_validate_scenario_unknown_target() {
        let reg = AdapterRegistry::new();
        let result = reg.validate_scenario("unknown", "scenario");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("unknown target 'unknown'"));
    }

    #[test]
    fn registry_validate_scenario_unknown_scenario() {
        let mut reg = AdapterRegistry::new();
        reg.register(TestAdapter::new("test", vec!["known_scenario"]));

        let result = reg.validate_scenario("test", "unknown_scenario");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err
            .to_string()
            .contains("unknown scenario 'unknown_scenario' for target 'test'"));
        assert!(err.to_string().contains("known_scenario"));
    }

    #[test]
    #[should_panic(expected = "already registered")]
    fn registry_duplicate_id_panics() {
        let mut reg = AdapterRegistry::new();
        reg.register(TestAdapter::new("duplicate", vec![]));
        reg.register(TestAdapter::new("duplicate", vec![]));
    }
}
