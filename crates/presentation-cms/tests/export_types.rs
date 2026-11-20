use presentation_cms::protocol::DeckProtocolMessage;
use ts_rs::TS;

#[test]
fn export_presentation_cms_typescript_bindings() {
    DeckProtocolMessage::export().expect("Failed to export DeckProtocolMessage");
}
