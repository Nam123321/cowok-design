use spec_registry::presentation_repo::{DeckSlideV12, PresentationDeckV12, WordingSlotV12};
use ts_rs::TS;

#[test]
fn export_v12_presentation_typescript_bindings() {
    WordingSlotV12::export().expect("Failed to export WordingSlotV12");
    DeckSlideV12::export().expect("Failed to export DeckSlideV12");
    PresentationDeckV12::export().expect("Failed to export PresentationDeckV12");
}
