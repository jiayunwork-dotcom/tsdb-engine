use dashmap::DashMap;

pub struct GlobalDictionary {
    key_ids: DashMap<String, u64>,
    next_key_id: parking_lot::Mutex<u64>,
    value_ids: DashMap<String, u64>,
    next_value_id: parking_lot::Mutex<u64>,
    reverse_keys: DashMap<u64, String>,
    reverse_values: DashMap<u64, String>,
    tag_value_counts: DashMap<String, usize>,
}

impl GlobalDictionary {
    pub fn new() -> Self {
        Self {
            key_ids: DashMap::new(),
            next_key_id: parking_lot::Mutex::new(1),
            value_ids: DashMap::new(),
            next_value_id: parking_lot::Mutex::new(1),
            reverse_keys: DashMap::new(),
            reverse_values: DashMap::new(),
            tag_value_counts: DashMap::new(),
        }
    }

    pub fn get_key_id(&self, key: &str) -> u64 {
        if let Some(id) = self.key_ids.get(key) {
            return *id.value();
        }
        let mut next = self.next_key_id.lock();
        let id = *next;
        *next += 1;
        drop(next);

        self.key_ids.insert(key.to_string(), id);
        self.reverse_keys.insert(id, key.to_string());
        id
    }

    pub fn get_or_create_id(&self, key: &str, value: &str) -> u64 {
        let composite = format!("{}:{}", key, value);
        if let Some(id) = self.value_ids.get(&composite) {
            return *id.value();
        }

        let mut count = self.tag_value_counts.entry(key.to_string()).or_insert(0);
        *count.value_mut() += 1;

        let mut next = self.next_value_id.lock();
        let id = *next;
        *next += 1;
        drop(next);

        self.value_ids.insert(composite.clone(), id);
        self.reverse_values.insert(id, value.to_string());
        id
    }

    pub fn get_value_by_id(&self, id: u64) -> Option<String> {
        self.reverse_values.get(&id).map(|v| v.value().clone())
    }

    pub fn get_key_by_id(&self, id: u64) -> Option<String> {
        self.reverse_keys.get(&id).map(|v| v.value().clone())
    }

    pub fn value_count_for_key(&self, key: &str) -> usize {
        self.tag_value_counts.get(key).map(|c| *c.value()).unwrap_or(0)
    }
}
