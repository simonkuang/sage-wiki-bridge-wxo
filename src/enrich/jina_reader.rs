use std::net::IpAddr;

use url::{Host, Url};

use crate::error::BridgeError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JinaReaderOptions {
    pub endpoint: String,
}

impl JinaReaderOptions {
    pub fn build_reader_url(&self, target_url: &str) -> Result<Url, BridgeError> {
        validate_public_http_url(target_url)?;

        let base = self.endpoint.trim_end_matches('/');
        let reader_url = format!("{base}/{target_url}");
        Url::parse(&reader_url).map_err(|err| BridgeError::Config(err.to_string()))
    }
}

pub fn validate_public_http_url(target_url: &str) -> Result<(), BridgeError> {
    let url = Url::parse(target_url).map_err(|err| BridgeError::UrlNotAllowed(err.to_string()))?;

    match url.scheme() {
        "http" | "https" => {}
        scheme => return Err(BridgeError::UrlNotAllowed(format!("scheme {scheme}"))),
    }

    let host = url
        .host()
        .ok_or_else(|| BridgeError::UrlNotAllowed("missing host".to_string()))?;

    match host {
        Host::Domain(domain) => {
            let lower = domain.to_ascii_lowercase();
            if lower == "localhost" || lower.ends_with(".localhost") {
                return Err(BridgeError::UrlNotAllowed(domain.to_string()));
            }
        }
        Host::Ipv4(ip) => reject_private_ip(IpAddr::V4(ip))?,
        Host::Ipv6(ip) => reject_private_ip(IpAddr::V6(ip))?,
    }

    Ok(())
}

fn reject_private_ip(ip: IpAddr) -> Result<(), BridgeError> {
    let rejected = match ip {
        IpAddr::V4(ip) => {
            ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_broadcast()
                || ip.is_documentation()
                || ip.octets()[0] == 0
                || ip.octets()[0] == 169 && ip.octets()[1] == 254
        }
        IpAddr::V6(ip) => {
            ip.is_loopback()
                || ip.is_unspecified()
                || ip.segments()[0] & 0xfe00 == 0xfc00
                || ip.segments()[0] & 0xffc0 == 0xfe80
        }
    };

    if rejected {
        Err(BridgeError::UrlNotAllowed(ip.to_string()))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_jina_reader_url() {
        let options = JinaReaderOptions {
            endpoint: "https://r.jina.ai/".to_string(),
        };

        let url = options
            .build_reader_url("https://example.com/article")
            .unwrap();

        assert_eq!(
            url.as_str(),
            "https://r.jina.ai/https://example.com/article"
        );
    }

    #[test]
    fn rejects_localhost_url() {
        let err = validate_public_http_url("http://localhost:3000").unwrap_err();
        assert!(matches!(err, BridgeError::UrlNotAllowed(_)));
    }

    #[test]
    fn rejects_private_ip_url() {
        let err = validate_public_http_url("http://192.168.1.10/page").unwrap_err();
        assert!(matches!(err, BridgeError::UrlNotAllowed(_)));
    }

    #[test]
    fn rejects_non_http_scheme() {
        let err = validate_public_http_url("file:///etc/passwd").unwrap_err();
        assert!(matches!(err, BridgeError::UrlNotAllowed(_)));
    }
}
