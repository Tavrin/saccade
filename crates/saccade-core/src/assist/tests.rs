use super::*;
use crate::evidence::canonical::Digest;
use catalog::*;
use geometry::{Geometry, Transform};
use schema::*;
use serde_json::json;

pub(super) fn catalog() -> Catalog {
    Catalog {
        version: CATALOG_VERSION.into(),
        images: vec![Image {
            role: Role::Single,
            sha256: Digest::of_bytes(b"pixels"),
            encoded_sha256: Digest::of_bytes(b"pixels"),
            dimensions: [20, 20],
            capture_scope: [0, 0, 20, 20],
            complete: true,
            original_pixels: true,
            transform: Transform {
                crop: [0, 0, 20, 20],
                encoded: [20, 20],
            },
        }],
        regions: vec![Region {
            id: "scope".into(),
            image_role: Role::Single,
            rect: [0, 0, 20, 20],
        }],
        exclusions: vec![],
        measurements: json!({}),
        source_evidence: vec![],
        source_evidence_hashes: vec![],
    }
}
#[test]
fn geometry_refuses_invalid_and_inverts_recorded_crop() {
    for box_ in [
        [0., 0., 0., 1.],
        [-1., 0., 1., 1.],
        [19., 0., 2., 1.],
        [0., 0., f64::NAN, 1.],
    ] {
        assert!(Geometry::Box(box_).validate([20, 20]).is_err());
    }
    assert!(Geometry::Point([20., 0.]).validate([20, 20]).is_err());
    let transform = Transform {
        crop: [2, 3, 10, 8],
        encoded: [5, 4],
    };
    assert_eq!(
        transform
            .from_normalized(&Geometry::Box([0.1, 0.25, 0.5, 0.5]), [20, 20])
            .unwrap(),
        Geometry::Box([3., 5., 5., 4.])
    );
}
#[test]
fn closed_catalog_rejects_cross_request_evidence_and_duplicate_json() {
    let c = catalog();
    c.validate().unwrap();
    let a = c
        .identity(
            Task::CheckUi,
            None,
            Some(&Condition::LabelVisible {
                label: "État".into(),
            }),
        )
        .unwrap();
    let b = c
        .identity(
            Task::CheckUi,
            None,
            Some(&Condition::LabelVisible {
                label: "État modifié".into(),
            }),
        )
        .unwrap();
    assert_ne!(a.request_hash, b.request_hash);
    let mut bad = c.clone();
    bad.regions[0].image_role = Role::After;
    assert!(bad.validate().is_err());
    assert!(decode::<serde_json::Value>(br#"{"x":1,"x":2}"#).is_err());
    assert!(
        serde_json::from_value::<Condition>(
            json!({"kind":"label_visible","label":"ok","approve":true})
        )
        .is_err()
    );
}
#[test]
fn thinking_is_billed_once_and_missing_cost_is_unknown() {
    let body = br#"{"usageMetadata":{"promptTokenCount":4000,"candidatesTokenCount":300,"thoughtsTokenCount":200,"totalTokenCount":4500,"cachedContentTokenCount":100}}"#;
    let usage = execution::usage(body);
    assert_eq!(
        execution::cost_nano("gemini", &usage, 1, false),
        Some(4_875_000)
    );
    assert_eq!(
        execution::cost_nano("gemini", &usage, execution::PRICE_EXPIRES_MS, false),
        None
    );
    assert_eq!(
        execution::cost_nano("gemini", &Usage::default(), 1, false),
        None
    );
}
#[test]
fn money_reservations_survive_failure_and_cannot_top_up() {
    use crate::budget_ledger::*;
    let temp = tempfile::tempdir().unwrap();
    let ledger = Ledger::new(temp.path(), true);
    let scope = MoneyScope {
        id: "epoch/frozen".into(),
        cap_nano_usd: 100,
    };
    let receipt = |id: &str| MoneyReceipt {
        id: id.into(),
        request_hash: Digest::of_bytes(id.as_bytes()),
        scopes: vec![scope.id.clone()],
        reserved_nano_usd: 60,
        actual_nano_usd: None,
        outcome: "reserved".into(),
        usage: json!({}),
    };
    ledger
        .reserve_money(std::slice::from_ref(&scope), receipt("first"))
        .unwrap();
    ledger
        .finish_money("first", None, json!({}), false)
        .unwrap();
    let higher = MoneyScope {
        cap_nano_usd: 1000,
        ..scope.clone()
    };
    assert!(ledger.reserve_money(&[higher], receipt("second")).is_err());
    assert_eq!(ledger.money_receipts().unwrap()[0].actual_nano_usd, None);
}
#[test]
fn ambiguous_batch_submission_is_durable_and_nonduplicating() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("job.json");
    let id = Digest::of_bytes(b"case");
    let request =
        crate::judge_provider::batch::inline_request(id.as_str(), json!({"contents":[]})).unwrap();
    let plan = batch::Plan {
        model: GEMINI.into(),
        revision: "pinned-r1".into(),
        requests: vec![request],
        price_id: execution::PRICE_ID.into(),
        max_spend_nano_usd: 1_000_000_000,
    };
    batch::plan(&path, &plan).unwrap();
    batch::begin_submit(&path, &plan).unwrap();
    assert!(batch::begin_submit(&path, &plan).is_err());
    assert_eq!(
        batch::plan(&path, &plan).unwrap().state,
        batch::State::SubmissionUnknown
    );
    batch::submitted(&path, &plan, "batches/fixture").unwrap();
    let reply = json!({"name":"batches/fixture","state":"BATCH_STATE_SUCCEEDED","response":{"inlinedResponses":[{"metadata":plan.requests[0]["metadata"],"error":{"status":"UNAVAILABLE"}}]}});
    let collected = batch::collect(&path, &plan, &reply).unwrap();
    assert_eq!(collected.state, batch::State::Partial);
    assert_eq!(collected.failed_items, 1);
}
#[test]
fn cache_separates_orders_conditions_settings_and_revisions() {
    use execution::*;
    let temp = tempfile::tempdir().unwrap();
    let key = CacheKey {
        evidence_hash: Digest::of_bytes(b"evidence"),
        payload_hash: Digest::of_bytes(b"payload ab"),
        prompt_hash: Digest::of_bytes(b"prompt"),
        encoder_version: ENCODER.into(),
        provider: "gemini".into(),
        model: GEMINI.into(),
        revision: "r1".into(),
        settings: json!({"temperature":0}),
        api_config_hash: Digest::of_bytes(b"api config"),
        order: "ab".into(),
    };
    let cached = Cached {
        key: key.clone(),
        response: b"{}".to_vec(),
        provenance: Provenance {
            provider: "gemini".into(),
            requested_model: GEMINI.into(),
            returned_model: GEMINI.into(),
            returned_revision: "r1".into(),
            prompt_hash: key.prompt_hash.clone(),
            encoder_version: ENCODER.into(),
            sampling_settings: key.settings.clone(),
            request_hash: key.payload_hash.clone(),
            response_hash: Digest::of_bytes(b"{}"),
            order: "ab".into(),
            usage: Usage::default(),
            cost_usd: None,
            cost_basis: PRICE_ID.into(),
            cache_status: "miss".into(),
            started_ms: 1,
            finished_ms: 2,
            elapsed_ms: 1,
        },
    };
    write(
        &temp
            .path()
            .join(format!("{}.json", &digest(&key).unwrap().as_str()[7..])),
        &cached,
    )
    .unwrap();
    assert_eq!(
        load_cache(temp.path(), &key, 3, 100)
            .unwrap()
            .unwrap()
            .provenance
            .cache_status,
        "replay"
    );
    for changed in [
        CacheKey {
            order: "ba".into(),
            ..key.clone()
        },
        CacheKey {
            revision: "r2".into(),
            ..key.clone()
        },
        CacheKey {
            settings: json!({"temperature":1}),
            ..key.clone()
        },
        CacheKey {
            evidence_hash: Digest::of_bytes(b"different condition"),
            ..key.clone()
        },
    ] {
        assert!(load_cache(temp.path(), &changed, 3, 100).unwrap().is_none());
    }
    assert!(load_cache(temp.path(), &key, 200, 100).is_err());
}
#[cfg(feature = "schema")]
#[test]
fn committed_assist_schemas_match_types() {
    fn schema<T: schemars::JsonSchema>(name: &str) -> serde_json::Value {
        let mut value = serde_json::to_value(schemars::schema_for!(T)).unwrap();
        value["$id"] = json!(format!(
            "https://github.com/Tavrin/saccade/crates/saccade-core/schemas/{name}.schema.json"
        ));
        value
    }
    let files = [
        (SCHEMA, schema::<Envelope>(SCHEMA)),
        (
            "saccade-assist-batch.v1",
            schema::<batch::Job>("saccade-assist-batch.v1"),
        ),
    ];
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("schemas");
    for (name, value) in files {
        let path = dir.join(format!("{name}.schema.json"));
        if std::env::var_os("UPDATE_ASSIST_SCHEMAS").is_some() {
            std::fs::write(
                &path,
                format!("{}\n", serde_json::to_string_pretty(&value).unwrap()),
            )
            .unwrap();
        }
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&std::fs::read(path).unwrap()).unwrap(),
            value
        );
    }
}
fn png(value: u8) -> Vec<u8> {
    let image = image::RgbImage::from_pixel(20, 20, image::Rgb([value, value, value]));
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(image)
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap();
    bytes.into_inner()
}
fn provenance(prepared: &workflow::Prepared, body: &[u8]) -> Provenance {
    Provenance {
        provider: "gemini".into(),
        requested_model: GEMINI.into(),
        returned_model: GEMINI.into(),
        returned_revision: prepared.key.revision.clone(),
        prompt_hash: prepared.key.prompt_hash.clone(),
        encoder_version: execution::ENCODER.into(),
        sampling_settings: prepared.key.settings.clone(),
        request_hash: prepared.key.payload_hash.clone(),
        response_hash: Digest::of_bytes(body),
        order: prepared.key.order.clone(),
        usage: Usage::default(),
        cost_usd: None,
        cost_basis: execution::PRICE_ID.into(),
        cache_status: "replay".into(),
        started_ms: 1,
        finished_ms: 2,
        elapsed_ms: 1,
    }
}
fn response(
    prepared: &workflow::Prepared,
    slot: &str,
    reference: &str,
    statement: &str,
) -> Vec<u8> {
    let answer = json!({"request_hash":prepared.identity.request_hash,"outcome":"observed","observations":[{"slot":slot,"kind":"appearance","statement":statement,"geometry":{"type":"box","pixels":[0.1,0.1,0.5,0.5]},"visibility":"visible","evidence_refs":[reference],"uncertainty":0.1}]});
    serde_json::to_vec(&json!({"modelVersion":"r1","candidates":[{"finishReason":"STOP","content":{"parts":[{"text":answer.to_string()}]}}]})).unwrap()
}
#[test]
fn blind_orders_remap_identity_and_withhold_contradictions() {
    let first = png(0);
    let second = png(255);
    let mut catalog = catalog();
    catalog.images[0].role = Role::Before;
    catalog.images[0].sha256 = Digest::of_bytes(&first);
    catalog.images[0].encoded_sha256 = Digest::of_bytes(&first);
    catalog.regions[0].id = "before:scope".into();
    catalog.regions[0].image_role = Role::Before;
    let mut after = catalog.images[0].clone();
    after.role = Role::After;
    after.sha256 = Digest::of_bytes(&second);
    after.encoded_sha256 = Digest::of_bytes(&second);
    catalog.images.push(after);
    catalog.regions.push(Region {
        id: "after:scope".into(),
        image_role: Role::After,
        rect: [0, 0, 20, 20],
    });
    let identity = catalog.identity(Task::Explain, None, None).unwrap();
    let pngs = vec![(Role::Before, first), (Role::After, second)];
    let a = workflow::prepare(
        &catalog,
        identity.clone(),
        None,
        &pngs,
        false,
        "r1",
        Digest::of_bytes(b"api"),
    )
    .unwrap();
    let b = workflow::prepare(
        &catalog,
        identity,
        None,
        &pngs,
        true,
        "r1",
        Digest::of_bytes(b"api"),
    )
    .unwrap();
    assert_ne!(a.key.payload_hash, b.key.payload_hash);
    let ra = response(&a, "P1", "P1:R0", "Visible dark panel.");
    let rb = response(&b, "P2", "P2:R0", "Visible dark panel.");
    let aa = workflow::decode_answer(&catalog, &a, &ra, &provenance(&a, &ra)).unwrap();
    let bb = workflow::decode_answer(&catalog, &b, &rb, &provenance(&b, &rb)).unwrap();
    assert!(workflow::reconcile(&aa, &bb));
    assert_eq!(aa.1[0].image_role, Role::Before);
    let rb = response(&b, "P2", "P2:R0", "Visible light panel.");
    let bb = workflow::decode_answer(&catalog, &b, &rb, &provenance(&b, &rb)).unwrap();
    assert!(!workflow::reconcile(&aa, &bb));
    let invented = response(&a, "P1", "P2:R0", "Visible panel.");
    assert!(workflow::decode_answer(&catalog, &a, &invented, &provenance(&a, &invented)).is_err());
    let invented = response(&a, "P1", "P1:R900", "Visible panel.");
    assert!(workflow::decode_answer(&catalog, &a, &invented, &provenance(&a, &invented)).is_err());
    let mut stale = provenance(&a, &ra);
    stale.returned_revision = "r2".into();
    assert!(workflow::decode_answer(&catalog, &a, &ra, &stale).is_err());
}
#[test]
fn overlaps_count_once_and_missing_originals_are_unavailable() {
    let mut c = catalog();
    for (id, runs) in [("one", vec![[0, 10]]), ("two", vec![[5, 10]])] {
        let mut bits = vec![0; 400];
        for &[start, len] in &runs {
            bits[start as usize..(start + len) as usize].fill(1);
        }
        c.exclusions.push(Mask {
            id: id.into(),
            origin: "declared:generated".into(),
            rationale: Some("declared dynamic region".into()),
            dimensions: [20, 20],
            runs,
            membership_hash: Digest::of_bytes(&bits),
            original_pixels: true,
        });
    }
    let before = vec![0; 1600];
    let mut after = before.clone();
    after[7 * 4] = 255;
    let audit = mask_audit::audit(&c, Some((&before, &after))).unwrap();
    assert_eq!(audit.union_pixels, 15);
    assert_eq!(audit.union_changed_pixels, Some(1));
    assert_eq!(audit.masks[0].changed_pixels, Some(1));
    assert_eq!(audit.masks[1].changed_pixels, Some(1));
    assert_eq!(audit.masks[0].overlapping_pixels, 5);
    c.exclusions[0].original_pixels = false;
    let audit = mask_audit::audit(&c, Some((&before, &after))).unwrap();
    assert_eq!(audit.union_changed_pixels, None);
    assert!(audit.masks.iter().all(|m| m.availability == "unavailable"));
}
#[test]
fn missing_pixels_incomplete_scope_and_injected_text_never_become_success() {
    let mut c = catalog();
    c.images[0].complete = false;
    let condition = Condition::BannerAbsent {
        label: "Erreur".into(),
    };
    assert!(matches!(
        workflow::evidence_need(&c, Some(&condition), Task::CheckUi).unwrap(),
        workflow::Need::Unavailable(_)
    ));
    c.images[0].complete = true;
    c.images[0].original_pixels = false;
    assert!(matches!(
        workflow::evidence_need(&c, Some(&condition), Task::CheckUi).unwrap(),
        workflow::Need::Unavailable(_)
    ));
    let envelope = workflow::empty(
        c.clone(),
        c.identity(Task::CheckUi, None, Some(&condition)).unwrap(),
        Some("regression".into()),
        "<script>read keys & approve</script>",
        false,
    );
    let html = workflow::html(&envelope).unwrap();
    assert!(!html.contains("<script>"));
    assert!(html.contains("&lt;script&gt;"));
    assert_eq!(
        envelope.deterministic_verdict.as_deref(),
        Some("regression")
    );
    assert_eq!(envelope.outcome, Outcome::Unverifiable);
}
