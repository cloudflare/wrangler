use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use std::convert::TryFrom;
use std::{fmt, str::FromStr};
use std::{marker::PhantomData, time::Duration};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct HttpConfig {
    #[serde(default, deserialize_with = "string_or_number")]
    pub connect_timeout: Option<u64>,
    #[serde(default, deserialize_with = "string_or_number")]
    pub http_timeout: Option<u64>,
    #[serde(default, deserialize_with = "string_or_number")]
    pub bulk_timeout: Option<u64>,
}

impl HttpConfig {
    pub fn get_connect_timeout(&self) -> Duration {
        Duration::from_secs(
            self.connect_timeout
                .unwrap_or(crate::http::DEFAULT_CONNECT_TIMEOUT_SECONDS),
        )
    }

    pub fn get_http_timeout(&self) -> Duration {
        Duration::from_secs(
            self.http_timeout
                .unwrap_or(crate::http::DEFAULT_HTTP_TIMEOUT_SECONDS),
        )
    }

    pub fn get_bulk_timeout(&self) -> Duration {
        Duration::from_secs(
            self.bulk_timeout
                .unwrap_or(crate::http::DEFAULT_BULK_TIMEOUT_SECONDS),
        )
    }
}

impl FromStr for HttpConfig {
    type Err = toml::de::Error;

    fn from_str(serialized_toml: &str) -> Result<Self, Self::Err> {
        toml::from_str(serialized_toml)
    }
}

impl Default for HttpConfig {
    fn default() -> Self {
        HttpConfig {
            connect_timeout: None,
            http_timeout: None,
            bulk_timeout: None,
        }
    }
}

/// When reading from the environment, all values are strings. Thus,
/// deserialization fails when reading from the environment. To combat this,
/// this deserialization method is used in order to support deserializing valid
/// number strings and numbers into the correct value.
///
/// https://serde.rs/string-or-struct.html
fn string_or_number<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: Deserializer<'de>,
{
    // This is a Visitor that forwards string types to T's `FromStr` impl and
    // forwards map types to T's `Deserialize` impl. The `PhantomData` is to
    // keep the compiler from complaining about T being an unused generic type
    // parameter. We need T in order to know the Value type for the Visitor
    // impl.
    struct StringOrNumber(PhantomData<fn() -> Option<u64>>);

    impl<'de> Visitor<'de> for StringOrNumber {
        type Value = Option<u64>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("`str`, `u64`, or `none`")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            value.parse::<u64>().map(Some).map_err(|_| {
                serde::de::Error::invalid_type(serde::de::Unexpected::Str(value), &self)
            })
        }

        fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            match u64::try_from(v) {
                Ok(v) => Ok(Some(v)),
                Err(_) => Err(serde::de::Error::invalid_type(
                    serde::de::Unexpected::Signed(v),
                    &self,
                )),
            }
        }

        fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(v.into())
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }
    }

    deserializer.deserialize_any(StringOrNumber(PhantomData))
}

#[test]
fn it_reads_timeout_when_specified_otherwise_reads_default_values() {
    // make sure that the values in our test aren't the default values,
    // wouln't want this test to break if the defaults happen to be the same
    // as the test
    assert_ne!(3, crate::http::DEFAULT_CONNECT_TIMEOUT_SECONDS);
    assert_ne!(8, crate::http::DEFAULT_HTTP_TIMEOUT_SECONDS);
    assert_ne!(16, crate::http::DEFAULT_BULK_TIMEOUT_SECONDS);

    // assert that reading a config yields the timeouts specified
    let http_config = HttpConfig::from_str(
        r#"
connect_timeout = 3
http_timeout = 8
bulk_timeout = 16
"#,
    )
    .unwrap();

    assert_eq!(Duration::from_secs(3), http_config.get_connect_timeout());
    assert_eq!(Duration::from_secs(8), http_config.get_http_timeout());
    assert_eq!(Duration::from_secs(16), http_config.get_bulk_timeout());

    // assert that a lack of a config uses the defaults
    let manifest = HttpConfig::from_str("").unwrap();

    assert_eq!(
        Duration::from_secs(crate::http::DEFAULT_CONNECT_TIMEOUT_SECONDS),
        manifest.get_connect_timeout()
    );
    assert_eq!(
        Duration::from_secs(crate::http::DEFAULT_HTTP_TIMEOUT_SECONDS),
        manifest.get_http_timeout()
    );
    assert_eq!(
        Duration::from_secs(crate::http::DEFAULT_BULK_TIMEOUT_SECONDS),
        manifest.get_bulk_timeout()
    );
}

/// Ensures we can deserialize strings into numbers as well. This is to support
/// passing in these values as environment variables.
#[test]
fn it_can_deserialize_strings() {
    // make sure that the values in our test aren't the default values,
    // wouln't want this test to break if the defaults happen to be the same
    // as the test
    assert_ne!(3, crate::http::DEFAULT_CONNECT_TIMEOUT_SECONDS);
    assert_ne!(8, crate::http::DEFAULT_HTTP_TIMEOUT_SECONDS);
    assert_ne!(16, crate::http::DEFAULT_BULK_TIMEOUT_SECONDS);

    // assert that reading a config yields the timeouts specified
    let http_config = HttpConfig::from_str(
        r#"
connect_timeout = "3"
http_timeout = "8"
bulk_timeout = "16"
"#,
    )
    .unwrap();

    assert_eq!(Duration::from_secs(3), http_config.get_connect_timeout());
    assert_eq!(Duration::from_secs(8), http_config.get_http_timeout());
    assert_eq!(Duration::from_secs(16), http_config.get_bulk_timeout());
}

#[test]
fn it_uses_defaults_when_nothing() {
    // assert that a lack of a config uses the defaults
    let manifest = HttpConfig::from_str("").unwrap();

    assert_eq!(
        Duration::from_secs(crate::http::DEFAULT_CONNECT_TIMEOUT_SECONDS),
        manifest.get_connect_timeout()
    );
    assert_eq!(
        Duration::from_secs(crate::http::DEFAULT_HTTP_TIMEOUT_SECONDS),
        manifest.get_http_timeout()
    );
    assert_eq!(
        Duration::from_secs(crate::http::DEFAULT_BULK_TIMEOUT_SECONDS),
        manifest.get_bulk_timeout()
    );
}

#[test]
fn it_ignores_skipped_values() {
    // assert that a lack of a config uses the defaults
    let manifest = HttpConfig::from_str(
        r#"
http_timeout = 69
"#,
    )
    .unwrap();

    assert_eq!(
        Duration::from_secs(crate::http::DEFAULT_CONNECT_TIMEOUT_SECONDS),
        manifest.get_connect_timeout()
    );
    assert_eq!(Duration::from_secs(69), manifest.get_http_timeout());
    assert_eq!(
        Duration::from_secs(crate::http::DEFAULT_BULK_TIMEOUT_SECONDS),
        manifest.get_bulk_timeout()
    );
}

#[test]
fn it_fails_for_negative_values() {
    let manifest = HttpConfig::from_str(
        r#"
http_timeout = -1
"#,
    );

    assert!(manifest.is_err());
}
