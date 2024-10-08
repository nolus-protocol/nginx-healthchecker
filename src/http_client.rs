use std::time::Duration;

use anyhow::Result;
use reqwest::Client as ReqwestClient;

pub(crate) fn http_client() -> Result<ReqwestClient> {
    ReqwestClient::builder()
        .brotli(true)
        .deflate(true)
        .gzip(true)
        .http1_allow_obsolete_multiline_headers_in_responses(false)
        .http1_allow_spaces_after_header_name_in_responses(false)
        .http1_ignore_invalid_headers_in_responses(false)
        .http2_keep_alive_interval(Some(Duration::from_secs(10)))
        .http2_keep_alive_while_idle(true)
        .https_only(false)
        .zstd(true)
        .build()
        .map_err(From::from)
}
