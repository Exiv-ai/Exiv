use async_trait::async_trait;
use tracing::warn;
use vers_shared::{HttpRequest, HttpResponse, NetworkCapability};

#[derive(Clone)]
pub struct SafeHttpClient {
    client: reqwest::Client,
}

impl SafeHttpClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    fn is_restricted_host(&self, host: &str) -> bool {
        let host = host.to_lowercase();
        if host == "localhost" || host == "127.0.0.1" || host == "::1" {
            return true;
        }

        if let Ok(ip) = host.parse::<std::net::IpAddr>() {
            match ip {
                std::net::IpAddr::V4(v4) => {
                    v4.is_private() || v4.is_loopback() || v4.is_link_local() || v4.is_broadcast() || v4.is_documentation()
                }
                std::net::IpAddr::V6(v6) => {
                    v6.is_loopback() || (v6.segments()[0] & 0xfe00 == 0xfc00) // Unique Local Address (fc00::/7)
                }
            }
        } else {
            false
        }
    }
}

#[async_trait]
impl NetworkCapability for SafeHttpClient {
    async fn send_http_request(&self, request: HttpRequest) -> anyhow::Result<HttpResponse> {
        let url = reqwest::Url::parse(&request.url)?;
        if let Some(host) = url.host_str() {
            if self.is_restricted_host(host) {
                warn!("🚫 Security Violation: Plugin attempted to access restricted host: {}", host);
                return Err(anyhow::anyhow!("Access to restricted host '{}' is denied.", host));
            }
        }

        let method = request.method.parse::<reqwest::Method>()?;
        let mut builder = self.client.request(method, url);

        for (k, v) in request.headers {
            builder = builder.header(k, v);
        }

        if let Some(body) = request.body {
            builder = builder.body(body);
        }

        let resp = builder.send().await?;
        let status = resp.status().as_u16();
        let body = resp.text().await?;

        Ok(HttpResponse { status, body })
    }
}
