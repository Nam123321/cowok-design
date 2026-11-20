// @story 06.3
// @implements AC-01, AC-03, UT-01, UT-02
use presentation_cms::{
    estimate_svg_primitives, patch_slide_slots_lol_html, validate_chart_json_str,
    validate_chart_spec_v12, ChartDataPoint, ChartKind2D, ChartSeries, ChartSpecV12,
    ChartValidationError, CmsPatcherError, CmsSlotKind, SlotPatchInput,
    CHART_MAX_POINTS_PER_SERIES, CHART_MAX_SERIES,
};

fn make_spec(kind: ChartKind2D, values: &[f64]) -> ChartSpecV12 {
    ChartSpecV12 {
        kind,
        title: "Q4 Revenue Mix".to_string(),
        unit: Some("%".to_string()),
        aria_summary: None,
        series: vec![ChartSeries {
            name: "Primary".to_string(),
            points: values
                .iter()
                .enumerate()
                .map(|(i, &v)| ChartDataPoint {
                    label: format!("Seg {i}"),
                    value: v,
                    color_token: None,
                })
                .collect(),
        }],
    }
}

#[test]
fn ut_01_proportional_tolerance_and_cartesian_signed_values() {
    for kind in [
        ChartKind2D::Pie,
        ChartKind2D::Donut,
        ChartKind2D::OneHundredStacked,
        ChartKind2D::Funnel,
    ] {
        let valid_low = make_spec(kind, &[49.96, 50.0]);
        assert!(
            validate_chart_spec_v12(&valid_low).is_ok(),
            "{kind:?} 99.96 should pass"
        );

        let valid_high = make_spec(kind, &[50.04, 50.0]);
        assert!(
            validate_chart_spec_v12(&valid_high).is_ok(),
            "{kind:?} 100.04 should pass"
        );

        let invalid_sum = make_spec(kind, &[44.0, 50.0]);
        let err = validate_chart_spec_v12(&invalid_sum).unwrap_err();
        assert_eq!(err.code(), "E_CHART_SUM_NOT_100");
        assert!(matches!(err, ChartValidationError::SumNot100 { .. }));

        let invalid_neg = make_spec(kind, &[-10.0, 110.0]);
        let err_neg = validate_chart_spec_v12(&invalid_neg).unwrap_err();
        assert_eq!(err_neg.code(), "E_CHART_NEGATIVE_PROPORTIONAL");
        assert!(matches!(
            err_neg,
            ChartValidationError::NegativeProportional { .. }
        ));
    }

    for kind in [ChartKind2D::Bar, ChartKind2D::Line, ChartKind2D::Area] {
        let signed = make_spec(kind, &[-12.5, 45.0, -999_999.0, 1_000_000.0]);
        let out = validate_chart_spec_v12(&signed).expect("cartesian signed values must pass");
        assert_eq!(out.normalized_spec.kind, kind);
        assert!(out.estimated_svg_primitives > 0);
        assert!(out.aria_label.contains("Q4 Revenue Mix"));
    }
}

#[test]
fn ut_02_dos_caps_unknown_fields_and_cms_patcher_fail_closed() {
    // Unknown field rejected by #[serde(deny_unknown_fields)]
    let unknown_field_json = r#"{
        "kind": "bar",
        "title": "Revenue",
        "extraEvil": true,
        "series": [{"name": "S1", "points": [{"label": "A", "value": 10}]}]
    }"#;
    let err = validate_chart_json_str(unknown_field_json).unwrap_err();
    assert!(matches!(err, ChartValidationError::SchemaInvalid(_)));

    // NaN / Infinity / 1e308 / >1e6 rejected with E_CHART_DOS_LIMIT
    for bad_num in [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        1e308,
        1_000_001.0,
        -1_000_001.0,
    ] {
        let spec = make_spec(ChartKind2D::Bar, &[bad_num]);
        let err = validate_chart_spec_v12(&spec).unwrap_err();
        assert_eq!(
            err.code(),
            "E_CHART_DOS_LIMIT",
            "expected DoS cap for {bad_num:?}"
        );
    }
    let raw_nan = r#"{"kind":"bar","title":"X","series":[{"name":"S","points":[{"label":"A","value":NaN}]}]}"#;
    assert_eq!(
        validate_chart_json_str(raw_nan).unwrap_err().code(),
        "E_CHART_DOS_LIMIT"
    );
    let raw_inf = r#"{"kind":"bar","title":"X","series":[{"name":"S","points":[{"label":"A","value":1e308}]}]}"#;
    assert_eq!(
        validate_chart_json_str(raw_inf).unwrap_err().code(),
        "E_CHART_DOS_LIMIT"
    );

    // > 12 series
    let mut too_many_series = make_spec(ChartKind2D::Bar, &[10.0]);
    too_many_series.series = (0..=CHART_MAX_SERIES)
        .map(|i| ChartSeries {
            name: format!("S{i}"),
            points: vec![ChartDataPoint {
                label: "A".into(),
                value: 1.0,
                color_token: None,
            }],
        })
        .collect();
    assert_eq!(
        validate_chart_spec_v12(&too_many_series)
            .unwrap_err()
            .code(),
        "E_CHART_DOS_LIMIT"
    );

    // > 200 points/series
    let many_pts: Vec<f64> = (0..=CHART_MAX_POINTS_PER_SERIES)
        .map(|i| i as f64)
        .collect();
    let too_many_pts = make_spec(ChartKind2D::Line, &many_pts);
    assert_eq!(
        validate_chart_spec_v12(&too_many_pts).unwrap_err().code(),
        "E_CHART_DOS_LIMIT"
    );

    // > 2000 SVG primitives (e.g., 6 series * 170 points = 1020 points -> 2060 primitives > 2000)
    let mut heavy_svg = make_spec(ChartKind2D::Line, &[1.0]);
    heavy_svg.series = (0..6)
        .map(|i| ChartSeries {
            name: format!("S{i}"),
            points: (0..170)
                .map(|j| ChartDataPoint {
                    label: format!("P{j}"),
                    value: 1.0,
                    color_token: None,
                })
                .collect(),
        })
        .collect();
    assert!(estimate_svg_primitives(&heavy_svg) > 2000);
    assert_eq!(
        validate_chart_spec_v12(&heavy_svg).unwrap_err().code(),
        "E_CHART_DOS_LIMIT"
    );

    // > 120 char label
    let mut long_label = make_spec(ChartKind2D::Bar, &[10.0]);
    long_label.series[0].points[0].label = "x".repeat(121);
    assert_eq!(
        validate_chart_spec_v12(&long_label).unwrap_err().code(),
        "E_CHART_DOS_LIMIT"
    );

    // cms_patcher enforces validate_chart_spec_v12 on CmsSlotKind::ChartJson
    let html = r#"<section class="slide active"><div data-ff-id="ff-chart" data-charts-json="[]">Chart</div></section>"#;
    let bad_patch = SlotPatchInput {
        ff_id: "ff-chart".into(),
        slot_kind: CmsSlotKind::ChartJson,
        text_value: "Bad Pie".into(),
        bullet_items: None,
        charts_json: Some(r#"{"kind":"pie","title":"Bad Pie","series":[{"name":"S","points":[{"label":"A","value":94}]}]}"#.into()),
        safe_styles_json: None,
    };
    let patch_err = patch_slide_slots_lol_html(html, None, &[bad_patch]).unwrap_err();
    assert!(matches!(
        patch_err,
        CmsPatcherError::ChartValidationFailed {
            code: "E_CHART_SUM_NOT_100",
            ..
        }
    ));
}
