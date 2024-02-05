use std::{collections::HashMap, time::Duration};

#[derive(Debug)]
pub struct Database {
    data: HashMap<String, Option<String>>,
    #[allow(dead_code)]
    data_imp: HashMap<String, Data>,
}

#[derive(Debug)]
pub struct Data {
    #[allow(dead_code)]
    data: Option<String>,
    // time to live in seconds
    #[allow(dead_code)]
    ttl: Duration,
    #[allow(dead_code)]
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
}

impl Database {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
            data_imp: HashMap::new(),
        }
    }

    pub fn insert_key_no_value(&mut self, k: String) {
        self.data.insert(k, None);
    }

    pub fn insert_key_impl(&mut self, key: String, ttl: Duration) {
        let data = Data {
            data: None,
            ttl,
            time_added: tokio::time::Instant::now(),
        };
        self.data_imp.insert(key, data);
    }

    pub fn insert_value_impl(&mut self, key: String, value: String) {
        let d = self.data_imp.get_mut(&key);
        match d {
            Some(v) => {
                if v.data.is_none() {
                    let _ = v.data.insert(value);
                }
            }
            None => {
                self.insert_key_impl(key.clone(), Duration::from_secs(60));
                self.insert_value_impl(key, value);
            }
        }
    }

    pub fn insert_value(&mut self, k: String, value: String) {
        match self.data.get_mut(&k) {
            Some(v) => {
                if v.is_none() {
                    let _ = v.insert(value);
                }
            }
            None => {
                // if theres no value add it with key
                self.data.insert(k, Some(value));
            }
        }
    }

    pub fn get_impl(&self, k: &str) -> Option<&Data> {
        self.data_imp.get(k)
    }

    pub fn get(&self, k: &str) -> Option<&Option<String>> {
        self.data.get(k)
    }
}

#[test]
fn test_database_map() {
    let mut db = Database::new();

    let key = "world".to_string();
    let value = "Hello".to_string();

    db.insert_key_no_value(key.clone());
    let expected = Some(String::from("some string"));
    assert_ne!(db.get(&key), Some(&expected));
    assert_eq!(db.get(&key), Some(&None));

    db.insert_value(key.clone(), value.clone());

    let expected = Some(value.clone());
    assert_eq!(db.get(&key), Some(&expected));

    let diff_val = String::from("This should not appear");
    db.insert_value(key.clone(), diff_val.clone());

    let expected = Some(value.clone());
    assert_eq!(db.get(&key), Some(&expected));
}
