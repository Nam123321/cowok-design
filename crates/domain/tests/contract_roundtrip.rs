// @story 01.1
// @cover AC-01, AC-02, AC-03, AC-04, AC-05
use domain::*;
use uuid::Uuid;

#[test]
fn test_safe_integer_roundtrip_and_bounds() {
    let max_safe = SafeJsInteger::new(MAX_SAFE_INTEGER).unwrap();
    let min_safe = SafeJsInteger::new(MIN_SAFE_INTEGER).unwrap();

    let json_max = serde_json::to_string(&max_safe).unwrap();
    assert_eq!(json_max, "9007199254740991");

    let deserialized_max: SafeJsInteger = serde_json::from_str(&json_max).unwrap();
    assert_eq!(deserialized_max, max_safe);

    let json_min = serde_json::to_string(&min_safe).unwrap();
    assert_eq!(json_min, "-9007199254740991");

    // Out of bounds positive
    let overflow_json = "9007199254740992";
    let res: Result<SafeJsInteger, _> = serde_json::from_str(overflow_json);
    assert!(res.is_err(), "Expected overflow to be rejected");

    // Out of bounds negative
    let underflow_json = "-9007199254740992";
    let res: Result<SafeJsInteger, _> = serde_json::from_str(underflow_json);
    assert!(res.is_err(), "Expected underflow to be rejected");
}

#[test]
fn test_canonical_spec_key_strict_whitelist() {
    for key in CanonicalSpecKey::ALL {
        let serialized = serde_json::to_string(&key).unwrap();
        assert_eq!(serialized, format!("\"{}\"", key.as_str()));

        let deserialized: CanonicalSpecKey = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, key);
    }

    // Traversal and unknown rejections
    let invalid_cases = [
        "\"custom-spec.md\"",
        "\"../01-product-overview.md\"",
        "\"01-product-overview.md/../02-database-spec.md\"",
        "\"..\\\\01-product-overview.md\"",
        "\"01-product-overview\"",
        "\"01-product-overview.md\\u0000.txt\"",
    ];

    for raw in invalid_cases {
        let res: Result<CanonicalSpecKey, _> = serde_json::from_str(raw);
        assert!(res.is_err(), "Expected {} to fail deserialization", raw);
        let err_msg = res.unwrap_err().to_string();
        assert!(
            err_msg.contains("INVALID_SPEC_KEY"),
            "Error message must contain normalized code INVALID_SPEC_KEY, got: {}",
            err_msg
        );
    }
}

#[test]
fn test_provenance_status_screaming_snake_case() {
    assert_eq!(
        serde_json::to_string(&ProvenanceStatus::Confirmed).unwrap(),
        "\"CONFIRMED\""
    );
    assert_eq!(
        serde_json::to_string(&ProvenanceStatus::Inferred).unwrap(),
        "\"INFERRED\""
    );
    assert_eq!(
        serde_json::to_string(&ProvenanceStatus::Missing).unwrap(),
        "\"MISSING\""
    );

    // Legacy PascalCase must fail
    assert!(serde_json::from_str::<ProvenanceStatus>("\"Confirmed\"").is_err());
    assert!(serde_json::from_str::<ProvenanceStatus>("\"Inferred\"").is_err());
    assert!(serde_json::from_str::<ProvenanceStatus>("\"Missing\"").is_err());
}

#[test]
fn test_harness_error_tagged_union() {
    let err = HarnessError::InvalidSpecKey {
        key: "hack.md".to_string(),
    };
    let json = serde_json::to_string(&err).unwrap();
    assert_eq!(
        json,
        r#"{"kind":"INVALID_SPEC_KEY","payload":{"key":"hack.md"}}"#
    );

    let roundtrip: HarnessError = serde_json::from_str(&json).unwrap();
    assert_eq!(roundtrip, err);

    // Lossless UNSAFE_INTEGER string serialization for >53-bit i64
    let unsafe_err = HarnessError::UnsafeInteger {
        value: 9_007_199_254_740_993,
    };
    let unsafe_json = serde_json::to_string(&unsafe_err).unwrap();
    assert_eq!(
        unsafe_json,
        r#"{"kind":"UNSAFE_INTEGER","payload":{"value":"9007199254740993"}}"#
    );
    let unsafe_roundtrip: HarnessError = serde_json::from_str(&unsafe_json).unwrap();
    assert_eq!(unsafe_roundtrip, unsafe_err);

    // Reject safe integer or raw numeric literal inside UNSAFE_INTEGER (F-04 parity)
    assert!(serde_json::from_str::<HarnessError>(
        r#"{"kind":"UNSAFE_INTEGER","payload":{"value":"42"}}"#
    )
    .is_err());
    assert!(serde_json::from_str::<HarnessError>(
        r#"{"kind":"UNSAFE_INTEGER","payload":{"value":9007199254740993}}"#
    )
    .is_err());
}

#[test]
fn test_p1_persistence_aggregates_roundtrip() {
    let project_id = Uuid::new_v4();
    let now = SafeJsInteger::new(1730500000).unwrap();

    let project = Project {
        id: project_id,
        name: "FlowForge OS".to_string(),
        slug: "flowforge-os".to_string(),
        created_at: now,
        updated_at: now,
    };

    let spec = ProjectSpec {
        id: Uuid::new_v4(),
        project_id,
        spec_key: CanonicalSpecKey::ProductOverview,
        status: ProvenanceStatus::Confirmed,
        updated_at: now,
    };

    let revision = SpecRevision {
        id: Uuid::new_v4(),
        spec_id: spec.id,
        revision_number: 1,
        content_hash: "sha256-e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
            .to_string(),
        created_at: now,
    };

    let proj_json = serde_json::to_string(&project).unwrap();
    assert_eq!(
        serde_json::from_str::<Project>(&proj_json).unwrap(),
        project
    );

    let spec_json = serde_json::to_string(&spec).unwrap();
    assert_eq!(
        serde_json::from_str::<ProjectSpec>(&spec_json).unwrap(),
        spec
    );

    let rev_json = serde_json::to_string(&revision).unwrap();
    assert_eq!(
        serde_json::from_str::<SpecRevision>(&rev_json).unwrap(),
        revision
    );

    // Deserialization must reject revision_number = 0 or non-SHA256 content_hash (F-01 parity)
    assert!(serde_json::from_str::<SpecRevision>(
        r#"{"id":"123e4567-e89b-12d3-a456-426614174000","spec_id":"123e4567-e89b-12d3-a456-426614174000","revision_number":0,"content_hash":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","created_at":0}"#
    )
    .is_err());
    assert!(serde_json::from_str::<SpecRevision>(
        r#"{"id":"123e4567-e89b-12d3-a456-426614174000","spec_id":"123e4567-e89b-12d3-a456-426614174000","revision_number":1,"content_hash":"not-a-hash","created_at":0}"#
    )
    .is_err());

    // Deserialization must reject unknown injected fields across aggregates (F-02 parity)
    assert!(serde_json::from_str::<Project>(
        r#"{"id":"123e4567-e89b-12d3-a456-426614174000","name":"x","slug":"x","created_at":0,"updated_at":0,"unknown_injected_field":"x"}"#
    )
    .is_err());

    // Serialization must also reject invalid struct literals (F-07 parity)
    let invalid_proj = Project {
        id: Uuid::nil(),
        name: String::new(),
        slug: "x".to_string(),
        created_at: SafeJsInteger::ZERO,
        updated_at: SafeJsInteger::ZERO,
    };
    assert!(serde_json::to_string(&invalid_proj).is_err());

    let invalid_rev = SpecRevision {
        id: Uuid::nil(),
        spec_id: Uuid::nil(),
        revision_number: 0,
        content_hash: "invalid-hash".to_string(),
        created_at: SafeJsInteger::ZERO,
    };
    assert!(serde_json::to_string(&invalid_rev).is_err());
}

#[test]
fn test_contract_v1_fixtures_parity() {
    use std::fs;
    use std::path::Path;

    let fixtures_dir = Path::new("tests/fixtures/contract-v1");

    // 1. Valid project fixture
    let proj_raw = fs::read_to_string(fixtures_dir.join("valid-project.json")).unwrap();
    let proj_json: serde_json::Value = serde_json::from_str(&proj_raw).unwrap();
    let proj: Project = serde_json::from_value(proj_json["data"].clone()).unwrap();
    assert_eq!(proj.slug, "cowok-design");

    // 2. Valid project-spec fixture
    let spec_raw = fs::read_to_string(fixtures_dir.join("valid-project-spec.json")).unwrap();
    let spec_json: serde_json::Value = serde_json::from_str(&spec_raw).unwrap();
    let spec: ProjectSpec = serde_json::from_value(spec_json["data"].clone()).unwrap();
    assert_eq!(spec.spec_key, CanonicalSpecKey::ProductOverview);
    assert_eq!(spec.status, ProvenanceStatus::Confirmed);

    // 3. Valid spec-revision fixture
    let rev_raw = fs::read_to_string(fixtures_dir.join("valid-spec-revision.json")).unwrap();
    let rev_json: serde_json::Value = serde_json::from_str(&rev_raw).unwrap();
    let rev: SpecRevision = serde_json::from_value(rev_json["data"].clone()).unwrap();
    assert_eq!(rev.revision_number, 1);

    // 4. Legacy PascalCase must fail in Rust deserialization
    let legacy_raw =
        fs::read_to_string(fixtures_dir.join("legacy-pascal-case-rejected.json")).unwrap();
    let legacy_json: serde_json::Value = serde_json::from_str(&legacy_raw).unwrap();
    let res: Result<ProjectSpec, _> = serde_json::from_value(legacy_json["invalid_data"].clone());
    assert!(
        res.is_err(),
        "Expected legacy PascalCase to fail deserialization"
    );

    // 5. Error fixtures
    let err_key_raw = fs::read_to_string(fixtures_dir.join("error-invalid-spec-key.json")).unwrap();
    let err_key_json: serde_json::Value = serde_json::from_str(&err_key_raw).unwrap();
    let err_key: HarnessError = serde_json::from_value(err_key_json["data"].clone()).unwrap();
    assert_eq!(err_key.kind_str(), "INVALID_SPEC_KEY");

    let err_stale_raw = fs::read_to_string(fixtures_dir.join("error-stale-file.json")).unwrap();
    let err_stale_json: serde_json::Value = serde_json::from_str(&err_stale_raw).unwrap();
    let err_stale: HarnessError = serde_json::from_value(err_stale_json["data"].clone()).unwrap();
    assert_eq!(err_stale.kind_str(), "STALE_FILE");
}
