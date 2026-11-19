//! Java runtime behaviour the port relies on: shared collections with Java iteration orders and
//! nullable-argument conversion for Java-style setters.

pub mod collections;
pub use java_rt::{error, net};
pub mod path;
pub mod classpath;
pub mod system;

pub use collections::{JList, JMap, JSet, MapKind};
pub use error::{JResult, JavaException};

/// Argument conversion for Java-style setters taking a nullable value: accepts `T`, `Option<T>` and,
/// for `String` fields, `&str` / `Option<&str>`.
pub trait IntoNullable<T> {
    /// The value as `Some`, or `None` for a Java `null`.
    fn into_nullable(self) -> Option<T>;
}

impl<T> IntoNullable<T> for T {
    fn into_nullable(self) -> Option<T> {
        Some(self)
    }
}

impl<T> IntoNullable<T> for Option<T> {
    fn into_nullable(self) -> Option<T> {
        self
    }
}

impl IntoNullable<String> for &str {
    fn into_nullable(self) -> Option<String> {
        Some(self.to_string())
    }
}

impl IntoNullable<String> for &String {
    fn into_nullable(self) -> Option<String> {
        Some(self.clone())
    }
}

impl IntoNullable<String> for Option<&str> {
    fn into_nullable(self) -> Option<String> {
        self.map(str::to_string)
    }
}
