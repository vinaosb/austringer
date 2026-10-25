//! M6 executor registry tests: day-one completeness over all 34
//! FormatIds, descriptor/binding sync discipline, builder wiring rules,
//! unknown-format dispatch, and the honest day-one placeholders (gif,
//! hdf5). Hermetic.

use austringer_media::descriptor::{FormatDescriptor, FormatRegistry, OpSupport, Operation};
use austringer_media::error::MediaError;
use austringer_media::executor::{
    AdapterOps, AdapterTier, ExecutorRegistryBuilder, ExecutorWiringError, OpBinding, OpRequest,
};
use austringer_media::format::FormatId;
use austringer_media::probe;

const PNG_MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n";

/// A minimal pure-probe ops table for one format (builder-test helper).
fn pure_probe_ops() -> AdapterOps {
    AdapterOps {
        probe: OpBinding::Pure {
            engine: "test-engine".into(),
            handler: probe_adapter,
        },
        extract: OpBinding::Unsupported {
            reason: "arrives with the adapter todos".into(),
        },
        generate: OpBinding::Unsupported {
            reason: "arrives with the adapter todos".into(),
        },
        verify: OpBinding::Unsupported {
            reason: "arrives with the adapter todos".into(),
        },
    }
}

fn probe_adapter(bytes: &[u8], name_hint: Option<&str>) -> austringer_media::probe::ProbeResult {
    probe::probe(bytes, name_hint)
}

/// The gif descriptor from the M2 table (builder-test base; gif is a
/// PERMANENT typed-unsupported placeholder - the vision-only honesty
/// row - so its claims never shift under the dev feature set. The
/// former docx base retired when M18 flipped its extract claim).
fn stable_descriptor() -> FormatDescriptor {
    probe::registry()
        .lookup(&FormatId::Gif)
        .expect("gif descriptor")
        .clone()
}

#[test]
fn day_one_registry_covers_all_34_formats_with_probe_bound() {
    // Given: the day-one executor registry (M6 wiring).
    let registry = austringer_media::executor::registry();
    let mut seen = 0_usize;
    // When: every registered FormatId is enumerated.
    for id in FormatId::ALL {
        let Ok(entry) = registry.lookup(id) else {
            panic!("{id} has no executor entry on day one");
        };
        // Then: probe is a bound pure handler, the entry declares a tier,
        // every unsupported op carries a NON-EMPTY reason (typed-
        // unsupported placeholder, never a silent blank)...
        assert!(
            matches!(entry.ops.probe, OpBinding::Pure { .. }),
            "{id}: probe must be bound"
        );
        macro_rules! reason_nonempty {
            ($binding:expr) => {
                if let OpBinding::Unsupported { reason } = $binding {
                    assert!(!reason.is_empty(), "{id}: blank unsupported reason");
                }
            };
        }
        reason_nonempty!(&entry.ops.extract);
        reason_nonempty!(&entry.ops.generate);
        reason_nonempty!(&entry.ops.verify);
        // ...and the entry carries its static descriptor (same table as
        // M2 - the executor attaches to it, never replaces it).
        assert_eq!(entry.descriptor.id, *id, "{id}: descriptor mismatch");
        seen += 1;
    }
    assert_eq!(seen, 34, "the slate is exactly 34 formats");
    // And: the executor enumerates exactly the registered set.
    assert_eq!(registry.formats().count(), 34);
}

#[test]
fn descriptor_claims_stay_synced_with_bindings_on_day_one() {
    // Given: the M2 descriptor table and the M6 executor registry.
    let descriptors: FormatRegistry = probe::registry();
    let registry = austringer_media::executor::registry();
    macro_rules! check_sync {
        ($binding:expr, $claim:expr, $id:expr, $op:expr) => {
            match ($binding, $claim) {
                (OpBinding::Pure { .. }, OpSupport::Unsupported { .. }) => {
                    panic!("{} {:?}: handler bound but descriptor refuses", $id, $op);
                }
                (OpBinding::Unsupported { .. }, OpSupport::Supported) => {
                    panic!("{} {:?}: Supported claim has no dispatch path", $id, $op);
                }
                _ => {}
            }
        };
    }
    // When/Then: every op of every format: a bound pure handler exists
    // only where the descriptor claims Supported, and a Supported claim
    // never pairs with an Unsupported binding (README drift is
    // unrepresentable; Subprocess bindings pair with either - decided
    // support, tool wiring pending).
    for id in FormatId::ALL {
        let descriptor = descriptors.lookup(id).expect("descriptor");
        let entry = registry.lookup(id).expect("executor entry");
        for op in [
            Operation::Probe,
            Operation::Extract,
            Operation::Generate,
            Operation::Verify,
        ] {
            let claim = descriptor.ops.get(&op).expect("op claim");
            match op {
                Operation::Probe => check_sync!(&entry.ops.probe, claim, id, Operation::Probe),
                Operation::Extract => {
                    check_sync!(&entry.ops.extract, claim, id, Operation::Extract)
                }
                Operation::Generate => {
                    check_sync!(&entry.ops.generate, claim, id, Operation::Generate)
                }
                Operation::Verify => check_sync!(&entry.ops.verify, claim, id, Operation::Verify),
            }
        }
    }
}

#[test]
fn builder_rejects_an_entry_without_a_bound_probe() {
    // Given: a gif entry whose probe is a typed refusal.
    let mut ops = pure_probe_ops();
    ops.probe = OpBinding::Unsupported {
        reason: "no sniffer".into(),
    };
    // When: registered.
    let error = ExecutorRegistryBuilder::new()
        .register(austringer_media::executor::AdapterEntry {
            descriptor: stable_descriptor(),
            tier: AdapterTier::Pure,
            ops,
        })
        .expect_err("probe-less entry must be rejected");
    // Then: the typed wiring error (the plan's MUST-NOT is constructor-
    // enforced, not convention).
    assert!(
        matches!(error, ExecutorWiringError::ProbeNotBound { format } if format == FormatId::Gif)
    );
}

#[test]
fn builder_rejects_duplicate_and_drifted_entries() {
    // Duplicate: two adapters claiming one format is a wiring bug.
    let first = ExecutorRegistryBuilder::new().register(austringer_media::executor::AdapterEntry {
        descriptor: stable_descriptor(),
        tier: AdapterTier::Pure,
        ops: pure_probe_ops(),
    });
    assert!(first.is_ok());
    let mut builder = ExecutorRegistryBuilder::new();
    builder
        .register(austringer_media::executor::AdapterEntry {
            descriptor: stable_descriptor(),
            tier: AdapterTier::Pure,
            ops: pure_probe_ops(),
        })
        .expect("first registers");
    let duplicate = builder
        .register(austringer_media::executor::AdapterEntry {
            descriptor: stable_descriptor(),
            tier: AdapterTier::Pure,
            ops: pure_probe_ops(),
        })
        .expect_err("duplicate must be rejected");
    assert!(
        matches!(duplicate, ExecutorWiringError::DuplicateAdapter { format } if format == FormatId::Gif)
    );

    // Drift: a descriptor claiming Supported extract while the binding is
    // a typed refusal (README-ahead-of-code is unrepresentable).
    let mut ahead = stable_descriptor();
    ahead.ops.insert(Operation::Extract, OpSupport::Supported);
    let drifted =
        ExecutorRegistryBuilder::new().register(austringer_media::executor::AdapterEntry {
            descriptor: ahead,
            tier: AdapterTier::Pure,
            ops: pure_probe_ops(),
        });
    assert!(matches!(
        drifted,
        Err(ExecutorWiringError::SupportedWithoutDispatch { format, op })
            if format == FormatId::Gif && op == Operation::Extract
    ));
}

#[test]
fn unknown_format_dispatch_fails_typed_unsupported_format() {
    // Given: a registry that registered only gif.
    let mut builder = ExecutorRegistryBuilder::new();
    builder
        .register(austringer_media::executor::AdapterEntry {
            descriptor: stable_descriptor(),
            tier: AdapterTier::Pure,
            ops: pure_probe_ops(),
        })
        .expect("gif entry registers");
    let partial = builder.build();
    // When: png (unregistered) is dispatched.
    let error = partial
        .dispatch(
            FormatId::Png,
            OpRequest::Probe {
                bytes: PNG_MAGIC,
                name_hint: None,
            },
        )
        .expect_err("unregistered format must refuse");
    // Then: the typed UnsupportedFormat - never a guess, never a panic.
    assert!(matches!(error, MediaError::UnsupportedFormat));
}

#[test]
fn gif_and_hdf5_placeholders_are_honest_day_one() {
    // Given: the two plan-named placeholder rows.
    let registry = austringer_media::executor::registry();
    // gif: probe + native vision delivery only - a PERMANENT typed
    // refusal, not a waiting-for-the-adapter placeholder.
    let gif = registry.lookup(&FormatId::Gif).expect("gif entry");
    match &gif.ops.extract {
        OpBinding::Unsupported { reason } => assert!(
            reason.to_lowercase().contains("vision"),
            "gif extract reason names the vision-only delivery lane: {reason}"
        ),
        other => panic!("gif extract must be typed-unsupported, got {other:?}"),
    }
    // hdf5: deferred by digest section 6 - the refusal cites it.
    let hdf5 = registry.lookup(&FormatId::Hdf5).expect("hdf5 entry");
    match &hdf5.ops.extract {
        OpBinding::Unsupported { reason } => assert!(
            reason.contains("deferred") && reason.contains("digest"),
            "hdf5 extract cites the digest deferral: {reason}"
        ),
        other => panic!("hdf5 extract must be typed-unsupported, got {other:?}"),
    }
    match &hdf5.ops.generate {
        OpBinding::Unsupported { reason } => assert!(reason.contains("deferred")),
        other => panic!("hdf5 generate must be typed-unsupported, got {other:?}"),
    }
}
