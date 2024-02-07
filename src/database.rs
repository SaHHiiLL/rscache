use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

#[derive(Debug)]
pub struct Database {
    data_imp: HashMap<String, Data>,
}

#[derive(Debug)]
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
        dbg!(now
            .saturating_duration_since(self.time_added.into())
            .as_secs())
            > self.ttl.as_secs()
    }
}

impl Database {
    pub fn new() -> Self {
        Self {
            data_imp: HashMap::new(),
        }
    }

    pub fn insert_key_impl(&mut self, key: String, ttl: Duration) {
        let data = Data {
            inner: None,
            ttl,
            time_added: tokio::time::Instant::now(),
        };
        self.data_imp.insert(key, data);
    }

    pub fn insert_value_impl(&mut self, key: String, value: String) {
        let d = self.data_imp.get_mut(&key);
        match d {
            Some(v) => {
                if v.inner.is_none() {
                    let _ = v.inner.insert(value);
                }
            }
            None => {
                self.insert_key_impl(key.clone(), Duration::from_secs(60));
                self.insert_value_impl(key, value);
            }
        }
    }

    pub fn get_impl(&mut self, k: String) -> Option<&Data> {
        self.data_imp.remove(&k);
        let x = self.data_imp.get(&k)?;

        if x.validate_cache() {
            return Some(x);
        }
        // remove
        None
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_insert() {
        let key = "Hello".to_string();
        let value = "world".to_string();
        let ttl = Duration::from_secs(10);

        let mut db = Database::new();
        db.insert_key_impl(key.to_string(), ttl);

        let got = db.get_impl(key);

        assert_eq!(got.is_some(), true);

        assert_eq!(got.expect("Asserted Above").inner().is_none(), true);
    }
}
