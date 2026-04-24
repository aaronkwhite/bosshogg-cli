//! Per-process name→ID cache.
//!
//! Users pass human-friendly names (flag keys, insight short-IDs, cohort
//! names); the API mostly wants numeric IDs. This cache remembers the
//! mapping for one `bosshogg` invocation.
//!
//! Persistent (cross-invocation) caching is deferred to v1.x. See the
//! design spec's "Open questions resolved" table.

use dashmap::DashMap;

#[derive(Debug, Default)]
pub struct Cache {
    flags_by_key: DashMap<String, i64>,
    insights_by_short_id: DashMap<String, i64>,
    cohorts_by_name: DashMap<String, i64>,
}

impl Cache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn flag_id_for_key(&self, key: &str) -> Option<i64> {
        self.flags_by_key.get(key).map(|v| *v)
    }

    pub fn remember_flag(&self, key: &str, id: i64) {
        self.flags_by_key.insert(key.to_string(), id);
    }

    pub fn insight_id_for_short_id(&self, short: &str) -> Option<i64> {
        self.insights_by_short_id.get(short).map(|v| *v)
    }

    pub fn remember_insight(&self, short: &str, id: i64) {
        self.insights_by_short_id.insert(short.to_string(), id);
    }

    pub fn cohort_id_for_name(&self, name: &str) -> Option<i64> {
        self.cohorts_by_name.get(name).map(|v| *v)
    }

    pub fn remember_cohort(&self, name: &str, id: i64) {
        self.cohorts_by_name.insert(name.to_string(), id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_by_key_roundtrip() {
        let c = Cache::new();
        assert_eq!(c.flag_id_for_key("foo"), None);
        c.remember_flag("foo", 42);
        assert_eq!(c.flag_id_for_key("foo"), Some(42));
    }

    #[test]
    fn cache_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Cache>();
    }
}
