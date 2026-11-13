use constraint_engine::{
    ActiveConstraint, AstInjectionRequest, AstInjectionResponse, ConstraintSeverity,
    ConstraintViolation, PatchKind, SurgicalPatchRequest, SurgicalPatchResponse,
};
use ts_rs::TS;

#[test]
fn export_constraint_engine_typescript_bindings() {
    ConstraintSeverity::export().expect("Failed to export ConstraintSeverity");
    ActiveConstraint::export().expect("Failed to export ActiveConstraint");
    ConstraintViolation::export().expect("Failed to export ConstraintViolation");
    PatchKind::export().expect("Failed to export PatchKind");
    SurgicalPatchRequest::export().expect("Failed to export SurgicalPatchRequest");
    SurgicalPatchResponse::export().expect("Failed to export SurgicalPatchResponse");
    AstInjectionRequest::export().expect("Failed to export AstInjectionRequest");
    AstInjectionResponse::export().expect("Failed to export AstInjectionResponse");
}
