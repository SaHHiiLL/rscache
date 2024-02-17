use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::interval;

#[derive(Debug)]
pub struct Database {
    inner: Arc<RwLock<HashMap<String, Data>>>,
}

#[derive(Debug, Clone)]
pub struct Data {
    inner: Option<String>,
    // time to live in seconds
    ttl: Duration,
    time_added: tokio::time::Instant,
}

impl Default for Data {
    fn default() -> Self {
        Data::new()
    }
}

impl Data {
    pub fn new() -> Self {
        Self {
            time_added: tokio::time::Instant::now(),
            ..Default::default()
        }
    }

    pub fn inner(&self) -> Option<String> {
        self.inner.to_owned()
    }

    pub fn validate_cache(&self) -> bool {
        let now = Instant::now();
        now.saturating_duration_since(self.time_added.into())
            .as_secs()
            < self.ttl.as_secs()
    }
}

impl Database {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn keep_valid(&mut self) {
        let inner = Arc::clone(&self.inner);
        tokio::task::spawn(async move {
            let mut interval = interval(Duration::from_secs(5));
            loop {
                interval.tick().await;
                tracing::debug!("Tring to remove");
                inner.write().await.retain(|_, v| v.validate_cache());
                tracing::debug!("Removed");
            }
        });
    }

    pub async fn insert_key(&mut self, key: String, ttl: Duration) {
        let data = Data {
            inner: None,
            ttl,
            time_added: tokio::time::Instant::now(),
        };
        let table = Arc::clone(&self.inner);
        table.write().await.insert(key, data);
    }

    // TODO remove expect -- recursion is not allowed in async function
    pub async fn insert_value(&mut self, key: String, value: String) {
        let table = Arc::clone(&self.inner);
        match table.write().await.get_mut(&key) {
            Some(v) => {
                if v.inner.is_none() {
                    let _ = v.inner.insert(value);
                }
            }
            None => {
                self.insert_key(key.clone(), Duration::from_secs(10)).await;

                let _ = table
                    .write()
                    .await
                    .get_mut(&key)
                    .expect("Should Not fail")
                    .inner
                    .insert(value);
            }
        };
    }

    // TODO: remove this
    pub async fn get_or_remove(&mut self, k: String) -> Option<Data> {
        let table = Arc::clone(&self.inner);
        let x = table.write().await.get(&k)?.clone();

        if x.validate_cache() {
            return Some(x);
        }
        table.write().await.remove(&k);
        // remove
        None
    }
}

// TODO:
// #[cfg(test)]
// mod test {
//     use super::*;
//
//     #[test]
//     fn test_insert() {
//         let key = "Hello".to_string();
//         let _value = "world".to_string();
//         let ttl = Duration::from_secs(10);
//
//         let mut db = Database::new();
//         db.insert_key(key.to_string(), ttl);
//
//         let got = db.get_or_remove(key);
//
//         assert!(got.is_some());
//
//         assert!(got.expect("Asserted Above").inner().is_none());
//     }
// }
