// Integration tests for the hardware-detection + GPU-toggle feature.
//
// These exercise the public crate API against an in-memory SQLite database
// with all migrations applied (so the `use_gpu` column exists). The
// `#[ignore]` test additionally probes the real hardware on the running
// machine and is meant to be run locally, mirroring the GigaAM real-audio
// test pattern.

use sqlx::SqlitePool;
use app_lib::audio::hardware_detector::{
    GpuType, HardwareProfile, PerformanceTier,
};
use app_lib::database::repositories::setting::SettingsRepository;
use app_lib::summary::summary_engine::commands::{
    decide_inference, recommend_summary_model,
};

/// Build a fresh in-memory SQLite pool with every migration applied. Each
/// test gets its own database so they cannot interfere.
async fn fresh_pool() -> SqlitePool {
    let pool = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite connect");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrations apply cleanly");
    pool
}

/// Construct a `HardwareProfile` for tests (the struct has many fields).
fn profile(gpu: GpuType) -> HardwareProfile {
    HardwareProfile {
        cpu_cores: 8,
        has_gpu_acceleration: !matches!(gpu, GpuType::None),
        gpu_type: gpu,
        memory_gb: 16,
        performance_tier: PerformanceTier::High,
        gpu_name: None,
        gpu_vram_gb: None,
        vulkan_available: !matches!(gpu, GpuType::None | GpuType::Metal),
    }
}

// ============================================================================
// SettingsRepository::use_gpu persistence
// ============================================================================

#[tokio::test]
async fn use_gpu_persistence_round_trip() {
    let pool = fresh_pool().await;

    // true round-trip
    SettingsRepository::set_use_gpu(&pool, true)
        .await
        .expect("set true");
    assert_eq!(
        SettingsRepository::get_use_gpu(&pool).await.expect("get"),
        true,
        "use_gpu=true must round-trip"
    );

    // false round-trip
    SettingsRepository::set_use_gpu(&pool, false)
        .await
        .expect("set false");
    assert_eq!(
        SettingsRepository::get_use_gpu(&pool).await.expect("get"),
        false,
        "use_gpu=false must round-trip"
    );
}

#[tokio::test]
async fn use_gpu_null_defaults_to_hardware() {
    // A fresh DB has never had use_gpu written, so it is NULL. The repository
    // must resolve NULL to the hardware-detected default (ON iff a GPU exists).
    let pool = fresh_pool().await;
    let expected = HardwareProfile::detect().has_gpu_acceleration;
    let resolved = SettingsRepository::get_use_gpu(&pool)
        .await
        .expect("get on NULL row");
    assert_eq!(
        resolved, expected,
        "NULL use_gpu must resolve to hardware default (has_gpu={})",
        expected
    );
}

// ============================================================================
// decide_inference (pure)
// ============================================================================

#[test]
fn decide_inference_no_gpu_forces_cpu() {
    let p = profile(GpuType::None);
    let plan = decide_inference(&p, true);
    assert_eq!(plan.inference_mode, "CPU");
    assert!(plan.force_cpu, "no GPU must force CPU even if user enabled");
}

#[test]
fn decide_inference_gpu_but_user_disabled_forces_cpu() {
    let p = profile(GpuType::Vulkan);
    let plan = decide_inference(&p, false);
    assert_eq!(plan.inference_mode, "CPU");
    assert!(plan.force_cpu, "user-disabled GPU must force CPU");
}

#[test]
fn decide_inference_vulkan_gpu_enabled() {
    let p = profile(GpuType::Vulkan);
    let plan = decide_inference(&p, true);
    assert_eq!(plan.inference_mode, "GPU (Vulkan)");
    assert!(!plan.force_cpu);
}

#[test]
fn decide_inference_metal_gpu_enabled() {
    let p = profile(GpuType::Metal);
    let plan = decide_inference(&p, true);
    assert_eq!(plan.inference_mode, "GPU (Metal)");
    assert!(!plan.force_cpu);
}

// ============================================================================
// recommend_summary_model (RAM-driven)
// ============================================================================

#[test]
fn recommend_summary_model_prefers_4b_on_high_ram() {
    let is_macos = cfg!(target_os = "macos");
    assert_eq!(recommend_summary_model(is_macos, 16), "qwen3.5:4b");
    assert_eq!(recommend_summary_model(is_macos, 32), "qwen3.5:4b");
}

#[test]
fn recommend_summary_model_falls_back_to_2b_on_low_ram() {
    let is_macos = cfg!(target_os = "macos");
    assert_eq!(recommend_summary_model(is_macos, 4), "qwen3.5:2b");
    assert_eq!(recommend_summary_model(is_macos, 8), "qwen3.5:2b");
}

// ============================================================================
// Real-hardware smoke test (run locally, like the GigaAM real-audio test)
// ============================================================================

/// Verifies `HardwareProfile::detect()` runs without panicking on the actual
/// machine and returns a structurally valid profile. Skipped by default; run
/// with: `cargo test --test hardware_integration -- --ignored hardware_detect`
#[tokio::test]
#[ignore]
async fn hardware_detect_runs_on_real_machine() {
    let p = HardwareProfile::detect();
    println!("Detected profile: {:?}", p);
    assert!(p.cpu_cores > 0, "cpu_cores must be positive");
    // gpu_name / gpu_vram_gb are allowed to be None on some platforms; the
    // invariant we care about is that detection did not panic.
    if p.has_gpu_acceleration {
        // When a GPU is reported, the type must be a concrete variant.
        assert!(
            !matches!(p.gpu_type, GpuType::None),
            "has_gpu_acceleration=true but gpu_type=None (inconsistent)"
        );
    }
}
