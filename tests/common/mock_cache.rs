//! A recording stand-in for `@Injectable ResolverCache cache`: every call is recorded; recorded
//! expectations (`cache.loadRef(ref, format, T.class); result = x`) come back in order (JMockit's
//! `returns(a, b)`: the last value repeats), and unrecorded calls return the mock defaults (`null` /
//! `false`).

use std::cell::RefCell;
use std::rc::Rc;
use swagger_parser::core::ParseOptions;
use swagger_parser::java::JResult;
use swagger_parser::parser::ref_utils::RefFormat;
use swagger_parser::parser::resolver_cache::{Cache, Component, ComponentKind, ComponentType};

#[derive(Clone, Debug, PartialEq)]
pub enum CacheCall {
    LoadRef(String, RefFormat, ComponentKind),
    GetRenamedRef(Option<String>),
    PutRenamedRef(String, String),
    HasReferencedKey(String),
    AddReferencedKey(String),
    RefsAreEquivalent(Option<String>, Option<String>),
    GetParseOptions,
}

type Recorded<K, V> = Vec<(K, Vec<V>, usize)>;

/// Recorded `refsAreEquivalent` answers: the two refs and the answer.
type Equivalences = Vec<((Option<String>, Option<String>), bool)>;

#[derive(Default)]
pub struct MockCache {
    pub calls: RefCell<Vec<CacheCall>>,
    load_ref: RefCell<Recorded<(String, RefFormat, ComponentKind), Option<Component>>>,
    renamed: RefCell<Recorded<Option<String>, Option<String>>>,
    equivalent: RefCell<Equivalences>,
    parse_options: RefCell<Option<ParseOptions>>,
}

fn next<K: PartialEq, V: Clone>(rec: &RefCell<Recorded<K, V>>, key: &K) -> Option<V> {
    let mut rec = rec.borrow_mut();
    let (_, values, pos) = rec.iter_mut().find(|(k, _, _)| k == key)?;
    let i = (*pos).min(values.len() - 1);
    *pos += 1;
    Some(values[i].clone())
}

impl MockCache {
    pub fn new() -> Rc<MockCache> {
        Rc::new(MockCache::default())
    }

    /// `cache.loadRef(ref, format, T.class); result = value`
    pub fn expect_load_ref<T: ComponentType>(&self, r: &str, format: RefFormat, value: Option<T>) {
        self.load_ref.borrow_mut().push(((r.to_string(), format, T::KIND), vec![value.map(T::into_component)], 0));
    }

    /// `cache.getRenamedRef(ref); returns(values...)`
    pub fn expect_renamed_ref(&self, r: &str, values: &[Option<&str>]) {
        self.renamed.borrow_mut().push((Some(r.to_string()), values.iter().map(|v| v.map(str::to_string)).collect(), 0));
    }

    /// `cache.refsAreEquivalent(first, second); result = value`
    pub fn expect_refs_are_equivalent(&self, first: &str, second: &str, value: bool) {
        self.equivalent.borrow_mut().push(((Some(first.to_string()), Some(second.to_string())), value));
    }

    /// `cache.getParseOptions(); result = options`
    pub fn expect_parse_options(&self, options: ParseOptions) {
        *self.parse_options.borrow_mut() = Some(options);
    }

    pub fn count(&self, call: &CacheCall) -> usize {
        self.calls.borrow().iter().filter(|c| *c == call).count()
    }

    pub fn calls(&self) -> Vec<CacheCall> {
        self.calls.borrow().clone()
    }
}

impl Cache for MockCache {
    fn load_ref_dyn(&self, r: &str, format: RefFormat, kind: ComponentKind) -> JResult<Option<Component>> {
        let key = (r.to_string(), format, kind);
        self.calls.borrow_mut().push(CacheCall::LoadRef(key.0.clone(), format, kind));
        Ok(next(&self.load_ref, &key).flatten())
    }

    fn get_renamed_ref(&self, original_ref: Option<&str>) -> Option<String> {
        let key = original_ref.map(str::to_string);
        self.calls.borrow_mut().push(CacheCall::GetRenamedRef(key.clone()));
        next(&self.renamed, &key).flatten()
    }

    fn put_renamed_ref(&self, original_ref: &str, new_ref: &str) {
        self.calls.borrow_mut().push(CacheCall::PutRenamedRef(original_ref.into(), new_ref.into()));
    }

    fn has_referenced_key(&self, key: &str) -> bool {
        self.calls.borrow_mut().push(CacheCall::HasReferencedKey(key.into()));
        false
    }

    fn add_referenced_key(&self, key: &str) {
        self.calls.borrow_mut().push(CacheCall::AddReferencedKey(key.into()));
    }

    fn refs_are_equivalent(&self, first: Option<&str>, second: Option<&str>) -> bool {
        let key = (first.map(str::to_string), second.map(str::to_string));
        self.calls.borrow_mut().push(CacheCall::RefsAreEquivalent(key.0.clone(), key.1.clone()));
        self.equivalent.borrow().iter().find(|(k, _)| *k == key).map(|(_, v)| *v).unwrap_or(false)
    }

    fn get_parse_options(&self) -> Option<ParseOptions> {
        self.calls.borrow_mut().push(CacheCall::GetParseOptions);
        self.parse_options.borrow().clone()
    }
}

/// `Rc<MockCache>` as the `Rc<dyn Cache>` processors take
pub fn dyn_cache(c: &Rc<MockCache>) -> Rc<dyn Cache> {
    c.clone()
}
