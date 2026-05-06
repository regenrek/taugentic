use std::sync::OnceLock;
use std::time::Duration;

pub fn shared_client() -> reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(15))
                .tcp_keepalive(Duration::from_secs(30))
                .pool_idle_timeout(Duration::from_secs(90))
                .pool_max_idle_per_host(8)
                .build()
                .expect("provider HTTP client configuration must stay valid")
        })
        .clone()
}

#[cfg(test)]
mod tests {
    #[test]
    fn shared_client_reuses_configured_client() {
        let _first = super::shared_client();
        let _second = super::shared_client();
    }
}
