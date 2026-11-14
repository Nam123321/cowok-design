// @story 04.1
// @cover AC-03, AC-04, AC-05, AC-06, AC-07, AC-08
use rusqlite::params;
use spec_registry::{SqlitePool, SqliteWalConfig};
use tempfile::TempDir;
use uuid::Uuid;

use flow_graph::{
    batch_update_positions, compute_sugiyama_positions, create_node, create_transition,
    delete_node, deploy_draft, get_snapshot, update_node, update_transition, BatchPositionItem,
    BatchUpdatePositionsInput, CreateFlowNodeInput, CreateFlowTransitionInput,
    DeployFlowDraftInput, UpdateFlowNodeInput, UpdateFlowTransitionInput,
};

fn setup_seeded_pool(tmp: &TempDir) -> (SqlitePool, String) {
    let db_path = tmp.path().join(".cowok-design/cowok-design.sqlite3");
    let pool = SqlitePool::open_wal(SqliteWalConfig::new(&db_path, tmp.path())).expect("open wal");
    let project_id = Uuid::new_v4().to_string();
    let conn = pool.connection().expect("conn");
    conn.execute(
        "INSERT INTO projects (id, tenant_id, slug, name, product_brief, workspace_root, created_at, updated_at) \
         VALUES (?1, 'local-default', 'demo-flow', 'Demo Flow', 'Brief', '.', 1700000000000, 1700000000000)",
        params![project_id],
    )
    .expect("seed project");
    (pool, project_id)
}

#[test]
fn it_dag_01_to_05_orthogonal_occ_cycle_check_batch_layout_and_idempotency() {
    let tmp = TempDir::new().expect("tempdir");
    let (pool, project_id) = setup_seeded_pool(&tmp);

    // Foreign tenant must return uniform 404 PROJECT_NOT_FOUND
    let err_foreign = get_snapshot(&pool, "foreign-tenant", &project_id).expect_err("foreign");
    assert_eq!(err_foreign.status, 404);
    assert_eq!(err_foreign.code, "PROJECT_NOT_FOUND");

    let n1 = create_node(
        &pool,
        "local-default",
        &project_id,
        &CreateFlowNodeInput {
            id: None,
            portal_id: "default".into(),
            screen_key: "login".into(),
            title: "Login".into(),
            node_type: "screen".into(),
            route_path: "/login".into(),
            component_entry_path: "src/Login.tsx".into(),
            fixture_ref: "Default".into(),
            fixture_json: "{}".into(),
            note: "Initial login".into(),
            viewport_preset: "desktop-1440".into(),
            pos_x: 100.0,
            pos_y: 120.0,
        },
    )
    .expect("create n1");
    assert_eq!(n1.version, 1);
    assert_eq!(n1.layout_version, 1);

    // Layout-only update increments layoutVersion ONLY (structural version stays 1!)
    let n1_moved = update_node(
        &pool,
        "local-default",
        &project_id,
        &n1.id,
        &UpdateFlowNodeInput {
            pos_x: Some(250.456),
            pos_y: Some(-0.0),
            expected_layout_version: Some(1),
            ..Default::default()
        },
    )
    .expect("move n1");
    assert_eq!(n1_moved.version, 1);
    assert_eq!(n1_moved.layout_version, 2);
    assert_eq!(n1_moved.pos_x, 250.46);
    assert_eq!(n1_moved.pos_y, 0.0);

    // Self-loop must reject with 400 VALIDATION_ERROR
    let self_err = create_transition(
        &pool,
        "local-default",
        &project_id,
        &CreateFlowTransitionInput {
            id: None,
            source_node_id: n1.id.clone(),
            target_node_id: n1.id.clone(),
            flow_type: "main".into(),
            trigger_event: "onClick".into(),
            guard_condition: None,
        },
    )
    .expect_err("self loop");
    assert_eq!(self_err.status, 400);
    assert_eq!(self_err.code, "VALIDATION_ERROR");

    // Deploy draft creating n2 + edge n1 -> n2, plus idempotent replay & payload mismatch
    let snap_before = get_snapshot(&pool, "local-default", &project_id).expect("snap");
    let draft_id = Uuid::new_v4().to_string();
    let n2_id = Uuid::new_v4().to_string();
    let draft_req = DeployFlowDraftInput {
        draft_id: draft_id.clone(),
        expected_graph_version: snap_before.graph_version,
        created_nodes: vec![CreateFlowNodeInput {
            id: Some(n2_id.clone()),
            portal_id: "default".into(),
            screen_key: "dashboard".into(),
            title: "Dashboard".into(),
            node_type: "screen".into(),
            route_path: "/dashboard".into(),
            component_entry_path: "".into(),
            fixture_ref: "Default".into(),
            fixture_json: "{}".into(),
            note: "".into(),
            viewport_preset: "desktop-1440".into(),
            pos_x: 420.0,
            pos_y: 120.0,
        }],
        updated_nodes: vec![],
        deleted_nodes: vec![],
        updated_node_positions: vec![],
        created_transitions: vec![CreateFlowTransitionInput {
            id: None,
            source_node_id: n1.id.clone(),
            target_node_id: n2_id.clone(),
            flow_type: "if_else".into(),
            trigger_event: "onSubmit".into(),
            guard_condition: Some("auth.ok".into()),
        }],
        updated_transitions: vec![],
        deleted_transitions: vec![],
    };

    let deployed = deploy_draft(&pool, "local-default", &project_id, &draft_req).expect("deploy");
    assert_eq!(deployed.nodes.len(), 2);
    assert_eq!(deployed.transitions.len(), 1);

    // Exact replay with same draftId returns identical snapshot without bumping graphVersion
    let replayed = deploy_draft(&pool, "local-default", &project_id, &draft_req).expect("replay");
    assert_eq!(replayed.graph_version, deployed.graph_version);

    // Mutated payload with same draftId returns 422 DRAFT_ID_PAYLOAD_MISMATCH
    let mut mismatched = draft_req.clone();
    mismatched.created_nodes[0].title = "Tampered".into();
    let mis_err =
        deploy_draft(&pool, "local-default", &project_id, &mismatched).expect_err("mismatch");
    assert_eq!(mis_err.status, 422);
    assert_eq!(mis_err.code, "DRAFT_ID_PAYLOAD_MISMATCH");

    // Cycle check: n2 -> n1 must fail with 422 DAG_CYCLE_DETECTED
    let cyc_err = create_transition(
        &pool,
        "local-default",
        &project_id,
        &CreateFlowTransitionInput {
            id: None,
            source_node_id: n2_id.clone(),
            target_node_id: n1.id.clone(),
            flow_type: "main".into(),
            trigger_event: "onClick".into(),
            guard_condition: None,
        },
    )
    .expect_err("cycle");
    assert_eq!(cyc_err.status, 422);
    assert_eq!(cyc_err.code, "DAG_CYCLE_DETECTED");

    // Sugiyama + batch_update_positions in single transaction
    let coords = compute_sugiyama_positions(
        &[n1.id.clone(), n2_id.clone()],
        &[(n1.id.clone(), n2_id.clone())],
    );
    let batch_res = batch_update_positions(
        &pool,
        "local-default",
        &project_id,
        &BatchUpdatePositionsInput {
            positions: vec![
                BatchPositionItem {
                    node_id: n1.id.clone(),
                    pos_x: coords[&n1.id].0,
                    pos_y: coords[&n1.id].1,
                    expected_layout_version: 2,
                },
                BatchPositionItem {
                    node_id: n2_id.clone(),
                    pos_x: coords[&n2_id].0,
                    pos_y: coords[&n2_id].1,
                    expected_layout_version: 1,
                },
            ],
        },
    )
    .expect("batch layout");
    assert_eq!(batch_res.nodes.len(), 2);

    // Reverse transition n1 -> n2 into n2 -> n1 (acyclic since only 1 edge)
    let edge_id = deployed.transitions[0].id.clone();
    let rev = update_transition(
        &pool,
        "local-default",
        &project_id,
        &edge_id,
        &UpdateFlowTransitionInput {
            reversed: Some(true),
            expected_version: Some(1),
            ..Default::default()
        },
    )
    .expect("reverse edge");
    assert_eq!(rev.source_node_id, n2_id);
    assert_eq!(rev.target_node_id, n1.id);

    // Delete node n2 with stale graphVersion fails 409 STALE_GRAPH_VERSION; fresh succeeds and reports cascade
    let stale_del = delete_node(
        &pool,
        "local-default",
        &project_id,
        &n2_id,
        Some(1),
        Some(1),
    )
    .expect_err("stale gv");
    assert_eq!(stale_del.code, "STALE_GRAPH_VERSION");

    let snap_curr = get_snapshot(&pool, "local-default", &project_id).expect("curr");
    let del_ok = delete_node(
        &pool,
        "local-default",
        &project_id,
        &n2_id,
        Some(1),
        Some(snap_curr.graph_version),
    )
    .expect("delete n2");
    assert_eq!(del_ok.cascade_deleted_edge_ids, vec![edge_id]);

    // FixtureRef canonical uppercase convergence: "Default" normalizes to "DEFAULT"
    assert_eq!(n1.fixture_ref, "DEFAULT");
    let invalid_fixture = create_node(
        &pool,
        "local-default",
        &project_id,
        &CreateFlowNodeInput {
            id: None,
            portal_id: "default".into(),
            screen_key: "invalid-fx".into(),
            title: "Invalid Fixture".into(),
            node_type: "screen".into(),
            route_path: "/bad".into(),
            component_entry_path: "".into(),
            fixture_ref: "INVALID_VARIANT".into(),
            fixture_json: "{}".into(),
            note: "".into(),
            viewport_preset: "desktop-1440".into(),
            pos_x: 0.0,
            pos_y: 0.0,
        },
    )
    .expect_err("invalid fixture_ref must fail validation");
    assert_eq!(invalid_fixture.status, 400);
    assert_eq!(invalid_fixture.code, "VALIDATION_ERROR");

    // ViewportPreset legacy alias normalization & fail-closed validation
    let tablet_node = create_node(
        &pool,
        "local-default",
        &project_id,
        &CreateFlowNodeInput {
            id: None,
            portal_id: "default".into(),
            screen_key: "tablet-alias".into(),
            title: "Tablet Alias".into(),
            node_type: "screen".into(),
            route_path: "/tablet".into(),
            component_entry_path: "".into(),
            fixture_ref: "DEFAULT".into(),
            fixture_json: "{}".into(),
            note: "".into(),
            viewport_preset: "tablet".into(),
            pos_x: 10.0,
            pos_y: 20.0,
        },
    )
    .expect("legacy tablet viewport alias must normalize");
    assert_eq!(tablet_node.viewport_preset, "tablet-768");

    let invalid_viewport = create_node(
        &pool,
        "local-default",
        &project_id,
        &CreateFlowNodeInput {
            id: None,
            portal_id: "default".into(),
            screen_key: "bad-viewport".into(),
            title: "Bad Viewport".into(),
            node_type: "screen".into(),
            route_path: "/bad-vp".into(),
            component_entry_path: "".into(),
            fixture_ref: "DEFAULT".into(),
            fixture_json: "{}".into(),
            note: "".into(),
            viewport_preset: "bogus-vp".into(),
            pos_x: 0.0,
            pos_y: 0.0,
        },
    )
    .expect_err("invalid viewport_preset must fail validation");
    assert_eq!(invalid_viewport.status, 400);
    assert_eq!(invalid_viewport.code, "VALIDATION_ERROR");
}
