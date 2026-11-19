//! Java exceptions as values: code paths that throw in Java return `Err(JavaException)` here, so callers that
//! catch (and often turn `getMessage()` into a parse message) behave identically.

use std::fmt;

/// A thrown Java exception as a value: its class, `getMessage()` and `getCause()`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JavaException {
    /// Fully-qualified class, e.g. `java.lang.ClassCastException`
    pub class: String,
    /// `getMessage()` (Java may return null)
    pub message: Option<String>,
    /// `getCause()`, when it matters for messages
    pub cause: Option<Box<JavaException>>,
}

/// The result of a call that throws a [`JavaException`] in Java.
pub type JResult<T> = Result<T, JavaException>;

impl JavaException {
    /// `new <class>(message)`: an exception of the given fully-qualified class with a message.
    pub fn new(class: &str, message: impl Into<String>) -> JavaException {
        JavaException { class: class.to_string(), message: Some(message.into()), cause: None }
    }

    /// `new <class>()`: an exception of the given fully-qualified class whose `getMessage()` is null.
    pub fn without_message(class: &str) -> JavaException {
        JavaException { class: class.to_string(), message: None, cause: None }
    }

    /// `initCause(cause)`: this exception with `cause` set as its `getCause()`.
    pub fn with_cause(mut self, cause: JavaException) -> JavaException {
        self.cause = Some(Box::new(cause));
        self
    }

    /// `new RuntimeException(cause)`: message is `cause.toString()`
    pub fn runtime_wrapping(cause: JavaException) -> JavaException {
        JavaException { class: "java.lang.RuntimeException".into(), message: Some(cause.to_java_string()), cause: Some(Box::new(cause)) }
    }

    /// `getMessage()`
    pub fn get_message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    /// `toString()`: `class: message` or just the class
    pub fn to_java_string(&self) -> String {
        match &self.message {
            Some(m) => format!("{}: {}", self.class, m),
            None => self.class.clone(),
        }
    }

    /// `ClassCastException` as HotSpot words it for application classes.
    pub fn class_cast(from: &str, to: &str) -> JavaException {
        JavaException::new(
            "java.lang.ClassCastException",
            format!("class {from} cannot be cast to class {to} ({from} and {to} are in unnamed module of loader 'app')"),
        )
    }

    /// A helpful `NullPointerException`.
    pub fn npe(message: impl Into<String>) -> JavaException {
        JavaException::new("java.lang.NullPointerException", message)
    }

    /// `new IllegalArgumentException(message)`.
    pub fn illegal_argument(message: impl Into<String>) -> JavaException {
        JavaException::new("java.lang.IllegalArgumentException", message)
    }
}

impl fmt::Display for JavaException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_java_string())
    }
}

impl std::error::Error for JavaException {}
