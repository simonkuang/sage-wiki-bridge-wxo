use serde::Deserialize;
use url::Url;

use crate::error::BridgeError;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LocationCoordinate {
    pub latitude: f64,
    pub longitude: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TencentLbsOptions {
    pub endpoint: String,
    pub key: String,
    pub get_poi: bool,
    pub radius_meters: Option<u32>,
}

impl TencentLbsOptions {
    pub fn build_reverse_geocode_url(
        &self,
        coordinate: LocationCoordinate,
    ) -> Result<Url, BridgeError> {
        let mut url =
            Url::parse(&self.endpoint).map_err(|err| BridgeError::Config(err.to_string()))?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair(
                "location",
                &format!("{},{}", coordinate.latitude, coordinate.longitude),
            );
            query.append_pair("key", &self.key);
            query.append_pair("get_poi", if self.get_poi { "1" } else { "0" });
            if let Some(radius) = self.radius_meters {
                query.append_pair("radius", &radius.to_string());
            }
        }
        Ok(url)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocationSummary {
    pub address: Option<String>,
    pub province: Option<String>,
    pub city: Option<String>,
    pub district: Option<String>,
    pub adcode: Option<String>,
    pub city_code: Option<String>,
}

pub fn extract_location_summary(json: &str) -> Result<LocationSummary, BridgeError> {
    let response: TencentLbsResponse = serde_json::from_str(json)
        .map_err(|err| BridgeError::ExternalPayloadInvalid(err.to_string()))?;

    let result = response
        .result
        .ok_or_else(|| BridgeError::Config("Tencent LBS response missing result".to_string()))?;

    Ok(LocationSummary {
        address: result.address,
        province: result
            .address_component
            .as_ref()
            .and_then(|component| component.province.clone()),
        city: result
            .address_component
            .as_ref()
            .and_then(|component| component.city.clone()),
        district: result
            .address_component
            .as_ref()
            .and_then(|component| component.district.clone()),
        adcode: result
            .ad_info
            .as_ref()
            .and_then(|ad_info| ad_info.adcode.clone()),
        city_code: result
            .ad_info
            .as_ref()
            .and_then(|ad_info| ad_info.city_code.clone()),
    })
}

#[derive(Debug, Deserialize)]
struct TencentLbsResponse {
    result: Option<TencentLbsResult>,
}

#[derive(Debug, Deserialize)]
struct TencentLbsResult {
    address: Option<String>,
    address_component: Option<AddressComponent>,
    ad_info: Option<AdInfo>,
}

#[derive(Debug, Deserialize)]
struct AddressComponent {
    province: Option<String>,
    city: Option<String>,
    district: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AdInfo {
    adcode: Option<String>,
    city_code: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_reverse_geocode_url_with_latitude_first() {
        let options = TencentLbsOptions {
            endpoint: "https://apis.map.qq.com/ws/geocoder/v1/".to_string(),
            key: "test-key".to_string(),
            get_poi: true,
            radius_meters: Some(500),
        };

        let url = options
            .build_reverse_geocode_url(LocationCoordinate {
                latitude: 23.134521,
                longitude: 113.358803,
            })
            .unwrap();

        assert_eq!(url.scheme(), "https");
        assert!(url.as_str().contains("location=23.134521%2C113.358803"));
        assert!(url.as_str().contains("radius=500"));
        assert!(url.as_str().contains("get_poi=1"));
    }

    #[test]
    fn extracts_location_summary() {
        let json = include_str!("../../tests/fixtures/external/tencent_lbs_success.json");

        let summary = extract_location_summary(json).unwrap();

        assert_eq!(
            summary.address.as_deref(),
            Some("广东省广州市天河区示例路1号")
        );
        assert_eq!(summary.province.as_deref(), Some("广东省"));
        assert_eq!(summary.city.as_deref(), Some("广州市"));
        assert_eq!(summary.district.as_deref(), Some("天河区"));
        assert_eq!(summary.adcode.as_deref(), Some("440106"));
        assert_eq!(summary.city_code.as_deref(), Some("020"));
    }
}
