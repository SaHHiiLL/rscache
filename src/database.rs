use std::{collections::HashMap, time::Duration};

#[derive(Debug)]
pub struct Database {
    data: HashMap<String, Option<String>>,
}

pub struct Data {
    data: String,
    // time to live in seconds
    ttl: Duration,
}

impl Database {
    pub async fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    pub fn insert_key_no_value(&mut self, k: String) {
        self.data.insert(k, None);
    }

    pub fn insert_value(&mut self, k: String, value: String) {
        match self.data.get(&k) {
            Some(v) => match v {
                Some(_v) => {}
                None => {
                    let _ = self.data.get_mut(&k).expect("LOL").insert(value);
                }
            },
            None => {
                // if theres no value add it with key
                self.data.insert(k, Some(value));
            }
        }
    }

    pub fn get(&self, k: &str) -> Option<&Option<String>> {
        self.data.get(k)
    }

    pub unsafe fn get_unsafe(&self, k: &str) -> &Option<String> {
        self.get(k).expect("YOU DID THIS TO YOUR SELF")
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
