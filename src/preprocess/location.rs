use std::path::PathBuf;

use crate::{
    enrich::tencent_lbs::LocationSummary,
    preprocess::{artifact::ProcessedArtifact, artifact::ProcessedArtifactKind, message_key},
    wechat::message::LocationMessage,
};

pub fn process_location(
    message: &LocationMessage,
    summary: &LocationSummary,
    raw_json_path: Option<PathBuf>,
) -> ProcessedArtifact {
    let mut body = String::new();
    body.push_str("## Location\n\n");
    body.push_str(&format!(
        "- Coordinates: {}, {}\n",
        message.latitude, message.longitude
    ));
    if let Some(label) = &message.label {
        body.push_str(&format!("- WeChat Label: {label}\n"));
    }
    if let Some(address) = &summary.address {
        body.push_str(&format!("- Address: {address}\n"));
    }
    let admin = [
        summary.province.as_deref(),
        summary.city.as_deref(),
        summary.district.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" / ");
    if !admin.is_empty() {
        body.push_str(&format!("- Administrative Area: {admin}\n"));
    }
    if let Some(adcode) = &summary.adcode {
        body.push_str(&format!("- Adcode: {adcode}\n"));
    }
    if let Some(city_code) = &summary.city_code {
        body.push_str(&format!("- City Code: {city_code}\n"));
    }

    let mut artifact = ProcessedArtifact::new(
        message_key(&message.common, "location"),
        ProcessedArtifactKind::Location,
        body,
    );
    artifact.external_service = Some("tencent_lbs".to_string());
    if let Some(path) = raw_json_path {
        artifact.raw_payload_paths.push(path);
    }
    artifact.summary = summary.address.clone();
    artifact
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::{
        enrich::tencent_lbs::extract_location_summary,
        wechat::{IncomingMessage, parse_plain_message},
    };

    use super::*;

    #[test]
    fn location_preprocessor_renders_lbs_summary() {
        let xml = include_str!("../../tests/fixtures/wechat/location.xml");
        let message = parse_plain_message(xml).unwrap();
        let IncomingMessage::Location(location) = message else {
            panic!("expected location");
        };
        let summary = extract_location_summary(include_str!(
            "../../tests/fixtures/external/tencent_lbs_success.json"
        ))
        .unwrap();

        let artifact = process_location(
            &location,
            &summary,
            Some(PathBuf::from("data/raw/msg/location.tencent-lbs.json")),
        );

        assert_eq!(artifact.kind, ProcessedArtifactKind::Location);
        assert_eq!(artifact.external_service.as_deref(), Some("tencent_lbs"));
        assert!(
            artifact
                .markdown_body
                .contains("Coordinates: 23.134521, 113.358803")
        );
        assert!(artifact.markdown_body.contains("Adcode: 440106"));
        assert_eq!(artifact.raw_payload_paths.len(), 1);
    }
}
