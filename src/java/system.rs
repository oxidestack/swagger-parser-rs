//! `java.lang.System` properties: a process-wide map, as in the JVM (`System.getProperty`,
//! `System.setProperty`, `System.clearProperty`).

use parking_lot::Mutex;
use std::collections::HashMap;

static PROPERTIES: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);

/// `System.getProperty(key)`
pub fn get_property(key: &str) -> Option<String> {
    PROPERTIES.lock().as_ref().and_then(|m| m.get(key).cloned())
}

/// `System.setProperty(key, value)`: returns the previous value
pub fn set_property(key: &str, value: &str) -> Option<String> {
    PROPERTIES.lock().get_or_insert_with(HashMap::new).insert(key.to_string(), value.to_string())
}

/// `System.clearProperty(key)`: returns the previous value
pub fn clear_property(key: &str) -> Option<String> {
    PROPERTIES.lock().as_mut().and_then(|m| m.remove(key))
}
