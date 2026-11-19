//! Java runtime behaviour shared by the swagger-parser crates: exceptions as values and `java.net.URI` /
//! `java.net.URL` / `java.net.InetAddress` with the JDK's parsing rules and messages.

#![warn(missing_docs)]

pub mod error;
pub mod idn;
pub mod inet;
pub mod net;

pub use error::{JResult, JavaException};
