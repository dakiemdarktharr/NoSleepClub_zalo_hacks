use nora_core::CoreError;
use nora_core::resources::{BudgetPool, MAX_RESERVATIONS, ResourceBudget, ResourceUsage};

#[test]
fn laptop_admission_rejects_oversubscription_without_changing_state() {
    let mut pool = BudgetPool::new(ResourceBudget::default()).unwrap();
    let resident = ResourceUsage {
        ram_bytes: 1_500_000_000,
        vram_bytes: 2_000_000_000,
        disk_bytes: 8_000_000_000,
        workers: 1,
    };
    pool.try_reserve("resident-model", resident).unwrap();
    for request in [
        ResourceUsage {
            ram_bytes: 500_000_001,
            ..ResourceUsage::default()
        },
        ResourceUsage {
            vram_bytes: 1_000_000_001,
            ..ResourceUsage::default()
        },
        ResourceUsage {
            disk_bytes: 7_000_000_001,
            ..ResourceUsage::default()
        },
        ResourceUsage {
            workers: 2,
            ..ResourceUsage::default()
        },
    ] {
        assert!(matches!(
            pool.try_reserve("too-much", request),
            Err(CoreError::BudgetExceeded(_))
        ));
        assert_eq!(pool.usage(), resident);
        assert_eq!(pool.reservation_count(), 1);
    }
}

#[test]
fn release_is_explicit_and_cannot_be_replayed_to_gain_capacity() {
    let mut pool = BudgetPool::new(ResourceBudget::default()).unwrap();
    let request = ResourceUsage {
        workers: 2,
        ..ResourceUsage::default()
    };
    pool.try_reserve("workers", request).unwrap();
    assert!(matches!(
        pool.try_reserve("workers", request),
        Err(CoreError::DuplicateReservation)
    ));
    assert_eq!(pool.release("workers").unwrap(), request);
    assert_eq!(pool.usage(), ResourceUsage::default());
    assert!(matches!(
        pool.release("workers"),
        Err(CoreError::UnknownReservation)
    ));
    pool.try_reserve("new-workers", request).unwrap();
    assert_eq!(pool.usage().workers, 2);
}

#[test]
fn checked_arithmetic_prevents_wrapping_below_limit() {
    let mut pool = BudgetPool::new(ResourceBudget {
        ram_bytes: u64::MAX,
        ..ResourceBudget::default()
    })
    .unwrap();
    pool.try_reserve(
        "one",
        ResourceUsage {
            ram_bytes: u64::MAX,
            ..ResourceUsage::default()
        },
    )
    .unwrap();
    assert!(matches!(
        pool.try_reserve(
            "overflow",
            ResourceUsage {
                ram_bytes: 1,
                ..ResourceUsage::default()
            }
        ),
        Err(CoreError::Overflow)
    ));
    assert_eq!(pool.usage().ram_bytes, u64::MAX);
}

#[test]
fn reservations_metadata_is_bounded_even_with_tiny_allocations() {
    let mut pool = BudgetPool::new(ResourceBudget::default()).unwrap();
    for i in 0..MAX_RESERVATIONS {
        pool.try_reserve(
            &format!("r{i}"),
            ResourceUsage {
                ram_bytes: 1,
                ..ResourceUsage::default()
            },
        )
        .unwrap();
    }
    assert!(matches!(
        pool.try_reserve(
            "more",
            ResourceUsage {
                ram_bytes: 1,
                ..ResourceUsage::default()
            }
        ),
        Err(CoreError::SizeLimit { .. })
    ));
    assert_eq!(pool.usage().ram_bytes, MAX_RESERVATIONS as u64);
}

#[test]
fn zero_or_invalid_reservations_and_configuration_are_rejected() {
    assert!(
        BudgetPool::new(ResourceBudget {
            workers: 0,
            ..ResourceBudget::default()
        })
        .is_err()
    );
    assert!(
        BudgetPool::new(ResourceBudget {
            workers: 65,
            ..ResourceBudget::default()
        })
        .is_err()
    );
    let mut pool = BudgetPool::new(ResourceBudget {
        vram_bytes: 0,
        ..ResourceBudget::default()
    })
    .unwrap();
    assert!(pool.try_reserve("zero", ResourceUsage::default()).is_err());
    assert!(
        pool.try_reserve(
            " ",
            ResourceUsage {
                workers: 1,
                ..ResourceUsage::default()
            }
        )
        .is_err()
    );
    assert!(
        pool.try_reserve(
            "gpu",
            ResourceUsage {
                vram_bytes: 1,
                ..ResourceUsage::default()
            }
        )
        .is_err()
    );
    assert_eq!(pool.reservation_count(), 0);
    assert!(serde_json::from_str::<ResourceUsage>(r#"{"ram_bytes":-1}"#).is_err());
}
