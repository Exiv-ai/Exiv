use async_trait::async_trait;
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
}

#[async_trait]
impl NetworkCapability for SafeHttpClient {
    async fn send_http_request(&self, request: HttpRequest) -> anyhow::Result<HttpResponse> {
        let method = request.method.parse::<reqwest::Method>()?;
        let mut builder = self.client.request(method, &request.url);

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
