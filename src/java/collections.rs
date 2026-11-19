//! Shared, mutable Java collections (`ArrayList`, `LinkedHashMap`/`HashMap`/`TreeMap`, `LinkedHashSet`/`HashSet`)
//! with reference semantics: cloning a handle aliases the same collection, exactly as passing a Java reference.
//! `HashMap`/`HashSet` iterate in Java's bucket order (String keys), so algorithms that depend on it port 1:1.

use indexmap::IndexMap;
use java_compat::lang::string_hash;
use parking_lot::RwLock;
use std::sync::Arc;

/// Which `java.util.Map` implementation a map emulates (only iteration order differs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapKind {
    /// `LinkedHashMap`: insertion order
    Linked,
    /// `HashMap`: bucket order of `String.hashCode()` spread, for the current table capacity
    Hash,
    /// `TreeMap`: `String.compareTo` order
    Tree,
}

#[derive(Debug)]
struct MapData<V> {
    kind: MapKind,
    entries: IndexMap<String, V>,
    /// HashMap table capacity (0 = not yet allocated) and resize threshold
    capacity: usize,
    threshold: usize,
}

/// Stands for a `null` key (`LinkedHashMap` accepts one; serializing such a map fails in Jackson).
pub const NULL_KEY: &str = "\u{0}<null key>";

/// A `Map<String, V>` shared by reference.
#[derive(Debug)]
pub struct JMap<V>(Arc<RwLock<MapData<V>>>);

impl<V> Clone for JMap<V> {
    fn clone(&self) -> Self {
        JMap(self.0.clone())
    }
}

fn table_size_for(cap: usize) -> usize {
    if cap <= 1 { 1 } else { cap.next_power_of_two().min(1 << 30) }
}

fn spread(h: i32) -> u32 {
    let h = h as u32;
    h ^ (h >> 16)
}

impl<V: Clone> Default for JMap<V> {
    fn default() -> Self {
        JMap::new()
    }
}

impl<V: Clone> JMap<V> {
    /// `new LinkedHashMap<>()`
    pub fn new() -> JMap<V> {
        JMap::with_kind(MapKind::Linked)
    }

    /// `new HashMap<>()`
    pub fn new_hash() -> JMap<V> {
        JMap::with_kind(MapKind::Hash)
    }

    /// `new TreeMap<>()`
    pub fn new_tree() -> JMap<V> {
        JMap::with_kind(MapKind::Tree)
    }

    /// An empty map that iterates like `kind`.
    pub fn with_kind(kind: MapKind) -> JMap<V> {
        JMap(Arc::new(RwLock::new(MapData { kind, entries: IndexMap::new(), capacity: 0, threshold: 0 })))
    }

    /// `new HashMap<>(initialCapacity)`
    pub fn new_hash_with_capacity(initial: usize) -> JMap<V> {
        let m = JMap::with_kind(MapKind::Hash);
        m.0.write().threshold = table_size_for(initial);
        m
    }

    /// `new LinkedHashMap<>(other)` / `new HashMap<>(other)` / `new TreeMap<>(other)`: a copy.
    pub fn copy_of(kind: MapKind, other: &JMap<V>) -> JMap<V> {
        let m = JMap::with_kind(kind);
        if kind == MapKind::Hash {
            let s = other.len();
            if s > 0 {
                let ft = s as f32 / 0.75 + 1.0;
                let t = if ft < (1 << 30) as f32 { ft as usize } else { 1 << 30 };
                m.0.write().threshold = table_size_for(t);
            }
        }
        for (k, v) in other.entries() {
            m.put(k, v);
        }
        m
    }

    /// A map of `kind` filled from `entries` in order, later duplicates replacing earlier values.
    pub fn from_entries(kind: MapKind, entries: impl IntoIterator<Item = (String, V)>) -> JMap<V> {
        let m = JMap::with_kind(kind);
        for (k, v) in entries {
            m.put(k, v);
        }
        m
    }

    /// Which `Map` implementation this map emulates.
    pub fn kind(&self) -> MapKind {
        self.0.read().kind
    }

    /// Whether both handles alias the same map (Java `==`).
    pub fn ptr_eq(&self, other: &JMap<V>) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// The number of entries.
    pub fn len(&self) -> usize {
        self.0.read().entries.len()
    }

    /// `size()`: same as [`JMap::len`].
    pub fn size(&self) -> usize {
        self.len()
    }

    /// `isEmpty()`.
    pub fn is_empty(&self) -> bool {
        self.0.read().entries.is_empty()
    }

    /// `get(key)`: a clone of the value, `None` when absent.
    pub fn get(&self, key: &str) -> Option<V> {
        self.0.read().entries.get(key).cloned()
    }

    /// `containsKey(key)`.
    pub fn contains_key(&self, key: &str) -> bool {
        self.0.read().entries.contains_key(key)
    }

    /// `put`: returns the previous value. An existing key keeps its position.
    pub fn put(&self, key: impl Into<String>, value: V) -> Option<V> {
        let mut d = self.0.write();
        let key = key.into();
        if let Some(slot) = d.entries.get_mut(&key) {
            return Some(std::mem::replace(slot, value));
        }
        d.entries.insert(key, value);
        if d.kind == MapKind::Hash {
            if d.capacity == 0 {
                let cap = if d.threshold > 0 { d.threshold } else { 16 };
                d.capacity = cap;
                d.threshold = (cap as f32 * 0.75) as usize;
            }
            if d.entries.len() > d.threshold {
                d.capacity *= 2;
                d.threshold *= 2;
            }
        }
        None
    }

    /// `put(null, value)`
    pub fn put_null_key(&self, value: V) -> Option<V> {
        self.put(NULL_KEY, value)
    }

    /// `put(key, null)`: modelled as removing the entry (null values are skipped when serializing and read
    /// back as absent).
    pub fn put_null_value(&self, key: &str) -> Option<V> {
        self.remove(key)
    }

    /// `putIfAbsent`
    pub fn put_if_absent(&self, key: impl Into<String>, value: V) -> Option<V> {
        let key = key.into();
        if let Some(v) = self.get(&key) {
            return Some(v);
        }
        self.put(key, value);
        None
    }

    /// `putAll(other)`: puts every entry of `other`, in its iteration order.
    pub fn put_all(&self, other: &JMap<V>) {
        for (k, v) in other.entries() {
            self.put(k, v);
        }
    }

    /// `remove(key)`: the removed value, `None` when absent.
    pub fn remove(&self, key: &str) -> Option<V> {
        self.0.write().entries.shift_remove(key)
    }

    /// `clear()`.
    pub fn clear(&self) {
        self.0.write().entries.clear();
    }

    /// Entries in Java iteration order (a snapshot).
    pub fn entries(&self) -> Vec<(String, V)> {
        let d = self.0.read();
        match d.kind {
            MapKind::Linked => d.entries.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
            MapKind::Tree => {
                let mut v: Vec<(String, V)> = d.entries.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                v.sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
                v
            }
            MapKind::Hash => {
                let cap = d.capacity.max(1) as u32;
                let mut v: Vec<(u32, usize, String, V)> = d
                    .entries
                    .iter()
                    .enumerate()
                    .map(|(i, (k, v))| (spread(string_hash(k)) & (cap - 1), i, k.clone(), v.clone()))
                    .collect();
                v.sort_by_key(|e| (e.0, e.1));
                v.into_iter().map(|e| (e.2, e.3)).collect()
            }
        }
    }

    /// `keySet()`: the keys in Java iteration order (a snapshot).
    pub fn keys(&self) -> Vec<String> {
        self.entries().into_iter().map(|(k, _)| k).collect()
    }

    /// `values()`: the values in Java iteration order (a snapshot).
    pub fn values(&self) -> Vec<V> {
        self.entries().into_iter().map(|(_, v)| v).collect()
    }

    /// Mutates values in place (e.g. `entry.setValue(...)` while iterating).
    pub fn replace_value(&self, key: &str, value: V) {
        if let Some(slot) = self.0.write().entries.get_mut(key) {
            *slot = value;
        }
    }

    /// Keeps entries for which `f` is true (`entrySet().removeIf(...)` / iterator removal).
    pub fn retain(&self, mut f: impl FnMut(&str, &V) -> bool) {
        self.0.write().entries.retain(|k, v| f(k, v));
    }
}

/// `AbstractMap.equals`: same key set and equal values, order-insensitive.
impl<V: Clone + PartialEq> PartialEq for JMap<V> {
    fn eq(&self, other: &Self) -> bool {
        if self.ptr_eq(other) {
            return true;
        }
        let a = self.0.read();
        let b = other.0.read();
        a.entries.len() == b.entries.len() && a.entries.iter().all(|(k, v)| b.entries.get(k).is_some_and(|w| v == w))
    }
}

/// A `List<T>` shared by reference (`ArrayList`).
#[derive(Debug)]
pub struct JList<T>(Arc<RwLock<Vec<T>>>);

impl<T> Clone for JList<T> {
    fn clone(&self) -> Self {
        JList(self.0.clone())
    }
}

impl<T: Clone> Default for JList<T> {
    fn default() -> Self {
        JList::new()
    }
}

impl<T: Clone> JList<T> {
    /// `new ArrayList<>()`.
    pub fn new() -> JList<T> {
        JList(Arc::new(RwLock::new(Vec::new())))
    }

    /// A list owning the elements of `v`.
    pub fn from_vec(v: Vec<T>) -> JList<T> {
        JList(Arc::new(RwLock::new(v)))
    }

    /// `new ArrayList<>(other)`
    pub fn copy_of(other: &JList<T>) -> JList<T> {
        JList::from_vec(other.to_vec())
    }

    /// Whether both handles alias the same list (Java `==`).
    pub fn ptr_eq(&self, other: &JList<T>) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// The number of elements.
    pub fn len(&self) -> usize {
        self.0.read().len()
    }

    /// `size()`: same as [`JList::len`].
    pub fn size(&self) -> usize {
        self.len()
    }

    /// `isEmpty()`.
    pub fn is_empty(&self) -> bool {
        self.0.read().is_empty()
    }

    /// `get(i)`: a clone of the element, `None` when `i` is out of bounds.
    pub fn get(&self, i: usize) -> Option<T> {
        self.0.read().get(i).cloned()
    }

    /// `add(v)`: appends.
    pub fn add(&self, v: T) {
        self.0.write().push(v);
    }

    /// `add(i, v)`: inserts at `i`; panics when `i > len`.
    pub fn insert(&self, i: usize, v: T) {
        self.0.write().insert(i, v);
    }

    /// `set(i, v)`: replaces the element at `i` and returns the old one; panics when `i` is out of bounds.
    pub fn set(&self, i: usize, v: T) -> T {
        std::mem::replace(&mut self.0.write()[i], v)
    }

    /// `remove(int)`: removes and returns the element at `i`; panics when `i` is out of bounds.
    pub fn remove_at(&self, i: usize) -> T {
        self.0.write().remove(i)
    }

    /// `addAll(other)`: appends the elements of `other`.
    pub fn add_all(&self, other: &JList<T>) {
        let items = other.to_vec();
        self.0.write().extend(items);
    }

    /// `clear()`.
    pub fn clear(&self) {
        self.0.write().clear();
    }

    /// A snapshot of the elements.
    pub fn to_vec(&self) -> Vec<T> {
        self.0.read().clone()
    }

    /// Keeps the elements for which `f` is true (`removeIf` with the predicate negated).
    pub fn retain(&self, f: impl FnMut(&T) -> bool) {
        self.0.write().retain(f);
    }
}

impl<T: Clone + PartialEq> JList<T> {
    /// `contains(v)`: by `==`.
    pub fn contains(&self, v: &T) -> bool {
        self.0.read().iter().any(|x| x == v)
    }

    /// `indexOf(v)`: the first equal element's index, `None` where Java returns `-1`.
    pub fn index_of(&self, v: &T) -> Option<usize> {
        self.0.read().iter().position(|x| x == v)
    }

    /// `remove(Object)`: first equal element.
    pub fn remove(&self, v: &T) -> bool {
        let mut w = self.0.write();
        match w.iter().position(|x| x == v) {
            Some(i) => {
                w.remove(i);
                true
            }
            None => false,
        }
    }
}

impl<T: Clone + PartialEq> PartialEq for JList<T> {
    fn eq(&self, other: &Self) -> bool {
        self.ptr_eq(other) || *self.0.read() == *other.0.read()
    }
}

/// A `Set<String>` shared by reference (`LinkedHashSet`, `HashSet` or `TreeSet` iteration order).
#[derive(Debug, Clone)]
pub struct JSet(JMap<()>);

impl Default for JSet {
    fn default() -> Self {
        JSet::new()
    }
}

impl JSet {
    /// `new LinkedHashSet<>()`
    pub fn new() -> JSet {
        JSet(JMap::new())
    }

    /// `new HashSet<>()`
    pub fn new_hash() -> JSet {
        JSet(JMap::new_hash())
    }

    /// An empty set that iterates like `kind`.
    pub fn with_kind(kind: MapKind) -> JSet {
        JSet(JMap::with_kind(kind))
    }

    /// A set of `kind` holding `items`, duplicates dropped.
    pub fn from_iter<S: Into<String>>(kind: MapKind, items: impl IntoIterator<Item = S>) -> JSet {
        let s = JSet::with_kind(kind);
        for i in items {
            s.add(i);
        }
        s
    }

    /// Whether both handles alias the same set (Java `==`).
    pub fn ptr_eq(&self, other: &JSet) -> bool {
        self.0.ptr_eq(&other.0)
    }

    /// `add`: true if newly added.
    pub fn add(&self, v: impl Into<String>) -> bool {
        self.0.put(v, ()).is_none()
    }

    /// `contains(v)`.
    pub fn contains(&self, v: &str) -> bool {
        self.0.contains_key(v)
    }

    /// `remove(v)`: true if it was present.
    pub fn remove(&self, v: &str) -> bool {
        self.0.remove(v).is_some()
    }

    /// The number of elements.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// `size()`: same as [`JSet::len`].
    pub fn size(&self) -> usize {
        self.len()
    }

    /// `isEmpty()`.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// `clear()`.
    pub fn clear(&self) {
        self.0.clear();
    }

    /// The elements in Java iteration order (a snapshot).
    pub fn items(&self) -> Vec<String> {
        self.0.keys()
    }
}

impl PartialEq for JSet {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.items().iter().all(|i| other.contains(i))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_map_iteration_follows_java_bucket_order() {
        let m: JMap<i32> = JMap::new_hash();
        for (i, k) in ["banana", "apple", "cherry", "date", "elderberry", "fig", "grape"].iter().enumerate() {
            m.put(*k, i as i32);
        }
        // recorded from java.util.HashMap
        assert_eq!(m.keys(), vec!["banana", "date", "apple", "cherry", "fig", "grape", "elderberry"]);
        let m2: JMap<i32> = JMap::new_hash();
        for i in 0..40 {
            m2.put(format!("k{}", i * 7), i);
        }
        let want = "k91, k70, k77, k98, k35, k56, k14, k161, k182, k140, k189, k266, k147, k224, k168, k245, k105, k126, k203, k0, k84, k42, k63, k21, k7, k28, k49, k196, k273, k154, k231, k175, k252, k112, k133, k210, k259, k217, k238, k119";
        assert_eq!(m2.keys().join(", "), want);
    }
}
