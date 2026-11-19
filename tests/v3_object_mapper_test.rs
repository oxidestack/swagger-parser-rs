//! Port of `io.swagger.v3.parser.ObjectMapperTest`.
//!
//! Upstream asserts that `ObjectMapperFactory.createJson().getRegisteredModuleIds()` contains the type id of
//! Jackson's `JavaTimeModule` — a JVM/Jackson module registry the port does not have (java.time values are
//! part of its value model, `JavaObject::OffsetDateTime`). The expectation is changed to what the port
//! produces for the same concern: with the parser mapper's settings (`WRITE_DATES_AS_TIMESTAMPS` disabled),
//! an `OffsetDateTime` is written as an ISO-8601 string, which is what registering `JavaTimeModule` provides.

mod common;

use jackson::convert::{object_to_json_string, SerConfig};
use jackson::object::OffsetDateTime;
use jackson::JavaObject;

#[test]
fn test_java_time_module() {
    let _g = common::setup(common::V3_MODULE);
    let value = JavaObject::OffsetDateTime(OffsetDateTime { epoch_second: 1_600_000_000, nano: 0, offset_seconds: 0 });
    let json = object_to_json_string(&value, &SerConfig::swagger(), false).expect("JavaTimeModule found?");
    assert_eq!(json, "\"2020-09-13T12:26:40Z\"", "JavaTimeModule found?");
}
