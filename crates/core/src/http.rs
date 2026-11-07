//! Minimal blocking HTTP layer.
//!
//! Everything that talks to the network goes through [`HttpClient`], so tests can swap in
//! [`fixture::FixtureHttp`] and never touch the network.

use std::time::Duration;

use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use serde::de::DeserializeOwned;

use crate::error::{Error, Result};

/// Sent with every request. Wikimedia's policy asks for an identifiable client with a way to
/// reach the maintainers; it deliberately contains nothing about the user or the device.
pub const USER_AGENT: &str = concat!(
    "dynamic-themed-wallpapers/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/spacer-man/dynamic-themed-wallpapers)"
);

/// JSON API responses are small; anything bigger is a bug or an attack.
pub const MAX_JSON_BYTES: u64 = 8 * 1024 * 1024;
/// Upper bound for a single downloaded image.
pub const MAX_IMAGE_BYTES: u64 = 40 * 1024 * 1024;

pub trait HttpClient: Send + Sync {
    /// GET `url` with extra request headers and return the body. Non-2xx statuses are errors.
    fn get_with(&self, url: &str, headers: &[(&str, &str)], max_bytes: u64) -> Result<Vec<u8>>;

    fn get(&self, url: &str, max_bytes: u64) -> Result<Vec<u8>> {
        self.get_with(url, &[], max_bytes)
    }

    /// The first `len` bytes of the body (a `Range` request; a server that ignores it costs
    /// only the bytes read). Enough to read an image's dimensions from its header.
    fn get_prefix(&self, url: &str, len: usize) -> Result<Vec<u8>> {
        let range = format!("bytes=0-{}", len.saturating_sub(1));
        let mut body = self.get_with(url, &[("Range", &range)], MAX_IMAGE_BYTES)?;
        body.truncate(len);
        Ok(body)
    }

    /// Like [`get_with`](Self::get_with) but for availability checks: a short timeout and no
    /// retry, so a dead host is reported in seconds instead of minutes.
    fn get_quick(&self, url: &str, headers: &[(&str, &str)], max_bytes: u64) -> Result<Vec<u8>> {
        self.get_with(url, headers, max_bytes)
    }

    /// One hop of a redirect: the `Location` of a 3xx answer to GET `url` (made absolute), or
    /// `None` when the server answers without redirecting. Used to open short links.
    fn redirect_target(&self, _url: &str) -> Result<Option<String>> {
        Ok(None)
    }
}

/// Timeout of [`HttpClient::get_quick`].
pub const QUICK_TIMEOUT: Duration = Duration::from_secs(8);

pub fn get_json<T: DeserializeOwned>(
    http: &dyn HttpClient,
    url: &str,
    what: &'static str,
) -> Result<T> {
    let body = http.get(url, MAX_JSON_BYTES)?;
    serde_json::from_slice(&body).map_err(|e| Error::parse(what, e))
}

/// Characters left as-is in query values (RFC 3986 unreserved set).
const QUERY_VALUE: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'~');

/// `base?k1=v1&k2=v2` with values percent-encoded.
pub fn build_url(base: &str, params: &[(&str, &str)]) -> String {
    let mut url = String::with_capacity(base.len() + 64 * params.len());
    url.push_str(base);
    for (i, (k, v)) in params.iter().enumerate() {
        url.push(if i == 0 { '?' } else { '&' });
        url.push_str(k);
        url.push('=');
        url.extend(utf8_percent_encode(v, QUERY_VALUE));
    }
    url
}

/// `location` from a redirect of `base`, made absolute (`/path` and `//host/path` forms).
pub fn absolute_url(base: &str, location: &str) -> String {
    if location.contains("://") {
        return location.to_owned();
    }
    let scheme_end = base.find("://").map_or(0, |i| i + 3);
    let scheme = &base[..scheme_end];
    if let Some(rest) = location.strip_prefix("//") {
        return format!("{scheme}{rest}");
    }
    let host_end = base[scheme_end..]
        .find('/')
        .map_or(base.len(), |i| scheme_end + i);
    format!("{}/{}", &base[..host_end], location.trim_start_matches('/'))
}

/// Real client backed by `ureq`.
pub struct UreqClient {
    agent: ureq::Agent,
}

impl UreqClient {
    pub fn new() -> Self {
        let config = ureq::Agent::config_builder();
        #[cfg(all(feature = "tls-graviola", not(feature = "tls-ring")))]
        let config = config.tls_config(
            ureq::tls::TlsConfig::builder()
                .provider(ureq::tls::TlsProvider::Rustls)
                .root_certs(ureq::tls::RootCerts::WebPki)
                .unversioned_rustls_crypto_provider(std::sync::Arc::new(
                    rustls_graviola::default_provider(),
                ))
                .build(),
        );
        let config = config
            .user_agent(USER_AGENT)
            .timeout_global(Some(Duration::from_secs(60)))
            .timeout_connect(Some(Duration::from_secs(15)))
            .http_status_as_error(false)
            .build();
        Self {
            agent: config.into(),
        }
    }
}

impl Default for UreqClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Pause before the single retry of a transient failure (connection reset, 429, 5xx).
const RETRY_DELAY: Duration = Duration::from_millis(1500);

impl HttpClient for UreqClient {
    fn get_with(&self, url: &str, headers: &[(&str, &str)], max_bytes: u64) -> Result<Vec<u8>> {
        match self.get_once(url, headers, max_bytes, None) {
            Err(e) if e.is_transient() => {
                std::thread::sleep(RETRY_DELAY);
                self.get_once(url, headers, max_bytes, None)
            }
            r => r,
        }
    }

    fn get_quick(&self, url: &str, headers: &[(&str, &str)], max_bytes: u64) -> Result<Vec<u8>> {
        self.get_once(url, headers, max_bytes, Some(QUICK_TIMEOUT))
    }

    fn redirect_target(&self, url: &str) -> Result<Option<String>> {
        match self.redirect_once(url) {
            Err(e) if e.is_transient() => {
                std::thread::sleep(RETRY_DELAY);
                self.redirect_once(url)
            }
            r => r,
        }
    }

    fn get_prefix(&self, url: &str, len: usize) -> Result<Vec<u8>> {
        match self.prefix_once(url, len) {
            Err(e) if e.is_transient() => {
                std::thread::sleep(RETRY_DELAY);
                self.prefix_once(url, len)
            }
            r => r,
        }
    }
}

impl UreqClient {
    fn redirect_once(&self, url: &str) -> Result<Option<String>> {
        let resp = self
            .agent
            .get(url)
            .config()
            .max_redirects(0)
            .timeout_global(Some(QUICK_TIMEOUT))
            .build()
            .call()
            .map_err(|e| Error::Network(e.to_string()))?;
        let status = resp.status().as_u16();
        if (300..400).contains(&status) {
            let location = resp
                .headers()
                .get("location")
                .and_then(|v| v.to_str().ok())
                .ok_or_else(|| Error::Network(format!("redirect without Location from {url}")))?;
            return Ok(Some(absolute_url(url, location)));
        }
        if !(200..300).contains(&status) {
            return Err(Error::Status {
                status,
                url: url.to_owned(),
            });
        }
        Ok(None)
    }

    fn prefix_once(&self, url: &str, len: usize) -> Result<Vec<u8>> {
        use std::io::Read;
        let range = format!("bytes=0-{}", len.saturating_sub(1));
        let mut resp = self
            .agent
            .get(url)
            .header("Range", &range)
            .call()
            .map_err(|e| Error::Network(e.to_string()))?;
        let status = resp.status().as_u16();
        if !(200..300).contains(&status) {
            return Err(Error::Status {
                status,
                url: url.to_owned(),
            });
        }
        let mut out = Vec::with_capacity(len.min(1 << 16));
        resp.body_mut()
            .as_reader()
            .take(len as u64)
            .read_to_end(&mut out)
            .map_err(|e| Error::Network(e.to_string()))?;
        Ok(out)
    }

    fn get_once(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        max_bytes: u64,
        timeout: Option<Duration>,
    ) -> Result<Vec<u8>> {
        let mut req = self.agent.get(url);
        if let Some(t) = timeout {
            req = req.config().timeout_global(Some(t)).build();
        }
        for (k, v) in headers {
            req = req.header(*k, *v);
        }
        let mut resp = req.call().map_err(|e| Error::Network(e.to_string()))?;
        let status = resp.status().as_u16();
        if !(200..300).contains(&status) {
            return Err(Error::Status {
                status,
                url: url.to_owned(),
            });
        }
        resp.body_mut()
            .with_config()
            .limit(max_bytes)
            .read_to_vec()
            .map_err(|e| Error::Network(e.to_string()))
    }
}

pub mod fixture {
    //! Offline [`HttpClient`] for tests: routes are matched by "all of these substrings occur in
    //! the URL", first match wins. Unmatched URLs fail loudly so tests can't silently pass.

    use std::sync::Mutex;

    use super::HttpClient;
    use crate::error::{Error, Result};

    type Route = (Vec<String>, Vec<u8>);
    type Redirect = (String, String);
    type Logged = (String, Vec<(String, String)>);

    #[derive(Default)]
    pub struct FixtureHttp {
        routes: Vec<Route>,
        redirects: Vec<Redirect>,
        log: Mutex<Vec<Logged>>,
    }

    impl FixtureHttp {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn route(mut self, needles: &[&str], body: impl Into<Vec<u8>>) -> Self {
            self.routes
                .push((needles.iter().map(|s| s.to_string()).collect(), body.into()));
            self
        }

        /// A 3xx answer from every URL that starts with `from`, pointing to `to`.
        pub fn redirect(mut self, from: &str, to: &str) -> Self {
            self.redirects.push((from.to_owned(), to.to_owned()));
            self
        }

        /// URLs requested so far, in order.
        pub fn requests(&self) -> Vec<String> {
            self.log
                .lock()
                .unwrap()
                .iter()
                .map(|(u, _)| u.clone())
                .collect()
        }

        /// Headers sent with the `i`-th request.
        pub fn headers(&self, i: usize) -> Vec<(String, String)> {
            self.log.lock().unwrap()[i].1.clone()
        }
    }

    impl HttpClient for FixtureHttp {
        fn redirect_target(&self, url: &str) -> Result<Option<String>> {
            self.log.lock().unwrap().push((url.to_owned(), Vec::new()));
            Ok(self
                .redirects
                .iter()
                .find(|(from, _)| url.starts_with(from.as_str()))
                .map(|(_, to)| to.clone()))
        }

        fn get_with(&self, url: &str, headers: &[(&str, &str)], max_bytes: u64) -> Result<Vec<u8>> {
            let headers = headers
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            self.log.lock().unwrap().push((url.to_owned(), headers));
            let decoded = percent_encoding::percent_decode_str(url).decode_utf8_lossy();
            for (needles, body) in &self.routes {
                if needles.iter().all(|n| decoded.contains(n.as_str())) {
                    if body.len() as u64 > max_bytes {
                        return Err(Error::Network("body exceeds limit".into()));
                    }
                    return Ok(body.clone());
                }
            }
            Err(Error::Status {
                status: 404,
                url: format!("[no fixture] {decoded}"),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_url_encodes_values() {
        let url = build_url("https://x.org/api", &[("a", "b c"), ("q", "Киото|P31=Q5")]);
        assert_eq!(
            url,
            "https://x.org/api?a=b%20c&q=%D0%9A%D0%B8%D0%BE%D1%82%D0%BE%7CP31%3DQ5"
        );
    }

    #[test]
    fn redirect_locations_become_absolute() {
        let base = "https://pin.it/abc";
        assert_eq!(absolute_url(base, "https://x.org/a"), "https://x.org/a");
        assert_eq!(absolute_url(base, "//x.org/a"), "https://x.org/a");
        assert_eq!(absolute_url(base, "/a/b?c"), "https://pin.it/a/b?c");
        assert_eq!(absolute_url("https://h.com", "a"), "https://h.com/a");
    }

    #[test]
    fn user_agent_has_no_personal_data() {
        assert!(USER_AGENT.starts_with("dynamic-themed-wallpapers/"));
        assert!(!USER_AGENT.contains('@'));
    }
}
