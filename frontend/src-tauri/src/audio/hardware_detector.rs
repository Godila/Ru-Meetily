use std::path::Path;
use std::sync::OnceLock;
use log::info;

/// Hardware capabilities for audio processing optimization
#[derive(Debug, Clone, PartialEq)]
pub struct HardwareProfile {
    pub cpu_cores: u8,
    pub has_gpu_acceleration: bool,
    pub gpu_type: GpuType,
    pub memory_gb: u8,
    pub performance_tier: PerformanceTier,
    /// Detected GPU marketing name (e.g. "NVIDIA GeForce RTX 3060").
    /// `None` when detection failed or there is no GPU.
    pub gpu_name: Option<String>,
    /// Discrete GPU VRAM in GB. `None` when unknown (e.g. shared/iGPU memory,
    /// or detection unavailable). For Metal/unified memory this stays `None`
    /// because the sidecar computes the working-set estimate separately.
    pub gpu_vram_gb: Option<f32>,
    /// Whether a Vulkan/Metal/CUDA runtime is available to the bundled
    /// llama-helper sidecar. Mirrors `has_gpu_acceleration` for non-Metal
    /// builds but is kept as a distinct field for UI clarity.
    pub vulkan_available: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuType {
    None,
    Metal,      // Apple Silicon
    Cuda,       // NVIDIA
    Vulkan,     // AMD/Intel
    OpenCL,     // Generic GPU compute
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerformanceTier {
    Low,      // CPU-only, limited resources
    Medium,   // CPU-only but powerful, or basic GPU
    High,     // Dedicated GPU with good compute
    Ultra,    // High-end hardware with fast GPU
}

/// Adaptive Whisper configuration based on hardware
#[derive(Debug, Clone)]
pub struct AdaptiveWhisperConfig {
    pub beam_size: usize,
    pub temperature: f32,
    pub use_gpu: bool,
    pub max_threads: Option<usize>,
    pub chunk_size_preference: ChunkSizePreference,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChunkSizePreference {
    Fast,       // Smaller chunks for responsiveness
    Balanced,   // Medium chunks for balance
    Quality,    // Larger chunks for accuracy
}

static HARDWARE_PROFILE: OnceLock<HardwareProfile> = OnceLock::new();

impl HardwareProfile {
    /// Get the detected hardware profile (cached after first call)
    pub fn detect() -> &'static HardwareProfile {
        HARDWARE_PROFILE.get_or_init(|| {
            let profile = Self::detect_hardware();
            info!("Detected hardware profile: {:?}", profile);
            profile
        })
    }

    /// Perform hardware detection
    fn detect_hardware() -> HardwareProfile {
        let cpu_cores = Self::detect_cpu_cores();
        let (has_gpu_acceleration, gpu_type) = Self::detect_gpu();
        let memory_gb = Self::detect_memory_gb();
        let performance_tier = Self::calculate_performance_tier(cpu_cores, &gpu_type, memory_gb);

        // GPU name/VRAM detection is best-effort: every external call returns
        // `Option` and never panics. Missing tools or parse failures simply
        // leave the fields `None` and the UI shows "—".
        let (gpu_name, gpu_vram_gb) = if has_gpu_acceleration {
            Self::detect_gpu_name_vram()
        } else {
            (None, None)
        };

        // Vulkan/Metal/CUDA runtime is available iff any GPU was detected.
        // On Apple Silicon we always report vulkan_available=false because the
        // shipped sidecar uses Metal there, not Vulkan.
        let vulkan_available =
            has_gpu_acceleration && !matches!(gpu_type, GpuType::Metal);

        HardwareProfile {
            cpu_cores,
            has_gpu_acceleration,
            gpu_type,
            memory_gb,
            performance_tier,
            gpu_name,
            gpu_vram_gb,
            vulkan_available,
        }
    }

    /// Detect number of CPU cores
    fn detect_cpu_cores() -> u8 {
        std::thread::available_parallelism()
            .map(|n| n.get().min(255) as u8)
            .unwrap_or(4) // Default to 4 cores
    }

    /// Detect GPU acceleration capabilities
    fn detect_gpu() -> (bool, GpuType) {
        // Check for Metal (Apple Silicon)
        #[cfg(target_os = "macos")]
        {
            if Self::has_metal_support() {
                return (true, GpuType::Metal);
            }
        }

        // Check for CUDA (NVIDIA)
        if Self::has_cuda_support() {
            return (true, GpuType::Cuda);
        }

        // Check for Vulkan (AMD/Intel/others)
        if Self::has_vulkan_support() {
            return (true, GpuType::Vulkan);
        }

        // Fallback to CPU-only
        (false, GpuType::None)
    }

    /// Detect available system memory in GB
    fn detect_memory_gb() -> u8 {
        // Simple memory detection - could be enhanced with system-specific calls
        match std::env::var("MEMORY_GB") {
            Ok(mem_str) => mem_str.parse().unwrap_or(8),
            Err(_) => {
                // Default estimates based on common configurations
                8 // Conservative default
            }
        }
    }

    /// Calculate performance tier based on hardware
    fn calculate_performance_tier(cpu_cores: u8, gpu_type: &GpuType, memory_gb: u8) -> PerformanceTier {
        match gpu_type {
            GpuType::Metal => {
                if memory_gb >= 16 && cpu_cores >= 8 {
                    PerformanceTier::Ultra
                } else {
                    PerformanceTier::High
                }
            }
            GpuType::Cuda => {
                if memory_gb >= 16 && cpu_cores >= 8 {
                    PerformanceTier::Ultra
                } else {
                    PerformanceTier::High
                }
            }
            GpuType::Vulkan | GpuType::OpenCL => {
                if memory_gb >= 12 && cpu_cores >= 6 {
                    PerformanceTier::High
                } else {
                    PerformanceTier::Medium
                }
            }
            GpuType::None => {
                if cpu_cores >= 8 && memory_gb >= 16 {
                    PerformanceTier::Medium
                } else {
                    PerformanceTier::Low
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    fn has_metal_support() -> bool {
        // Simple check for Apple Silicon (Metal is available on Intel Macs too, but less optimal for ML)
        std::env::consts::ARCH == "aarch64"
    }

    fn has_cuda_support() -> bool {
        // Check for CUDA environment or libraries
        std::env::var("CUDA_PATH").is_ok() ||
        std::env::var("CUDA_HOME").is_ok() ||
        std::path::Path::new("/usr/local/cuda").exists()
    }

    fn has_vulkan_support() -> bool {
        if std::env::var("VULKAN_SDK").is_ok() ||
            std::path::Path::new("/usr/lib/x86_64-linux-gnu/libvulkan.so").exists() ||
            std::path::Path::new("/usr/lib/libvulkan.so").exists()
        {
            return true;
        }

        #[cfg(target_os = "windows")]
        {
            return Self::has_windows_vulkan_runtime();
        }

        #[cfg(not(target_os = "windows"))]
        {
            false
        }
    }

    #[cfg(target_os = "windows")]
    fn has_windows_vulkan_runtime() -> bool {
        for env_var in ["SystemRoot", "WINDIR"] {
            if let Ok(system_root) = std::env::var(env_var) {
                if Self::has_windows_vulkan_loader(Path::new(&system_root)) {
                    return true;
                }
            }
        }

        Self::has_windows_vulkan_loader(Path::new(r"C:\Windows"))
    }

    fn has_windows_vulkan_loader(system_root: &Path) -> bool {
        system_root.join("System32").join("vulkan-1.dll").is_file()
    }

    /// Best-effort detection of the discrete GPU marketing name and VRAM.
    ///
    /// Strategy (in priority order):
    /// 1. NVIDIA `nvidia-smi` — accurate name + total VRAM, works on every OS.
    /// 2. Platform-specific fallback:
    ///    - Windows: PowerShell `Get-CimInstance Win32_VideoController`
    ///      (`AdapterRAM` is DWORD-capped at ~4 GiB, so treat as approximate).
    ///    - macOS: `sysctl machdep.cpu.brand_string` for the name; VRAM stays
    ///      `None` because Metal uses unified memory.
    ///    - Linux: parse `/sys/class/drm/card*/device/mem_info_vram_total`
    ///      (accurate for amdgpu); name from `/proc/driver/nvidia/...` if NVIDIA.
    ///
    /// Every call returns `Option` and never panics — failures degrade to
    /// `(None, None)` and the UI renders "—".
    fn detect_gpu_name_vram() -> (Option<String>, Option<f32>) {
        // 1. nvidia-smi covers NVIDIA cards on every OS even when the sidecar
        //    was built with the Vulkan feature (the driver still exposes it).
        if let Some(out) = std::process::Command::new("nvidia-smi")
            .args([
                "--query-gpu=name,memory.total",
                "--format=csv,noheader,nounits",
            ])
            .output()
            .ok()
        {
            if out.status.success() {
                let stdout = String::from_utf8_lossy(&out.stdout);
                // Format: "NVIDIA GeForce RTX 3060, 12288 MiB"
                let line = stdout.lines().next().unwrap_or("").trim();
                let mut parts = line.split(',').map(str::trim);
                let name = parts.next().map(|s| s.to_string()).filter(|s| !s.is_empty());
                let vram = parts
                    .next()
                    .and_then(|s| s.split_whitespace().next())
                    .and_then(|n| n.parse::<f32>().ok())
                    .map(|mb| mb / 1024.0);
                if name.is_some() {
                    return (name, vram);
                }
            }
        }

        // 2. Platform fallbacks for non-NVIDIA GPUs.
        #[cfg(target_os = "windows")]
        {
            if let Some((name, vram)) = Self::detect_gpu_name_vram_windows() {
                return (Some(name), vram);
            }
        }

        #[cfg(target_os = "macos")]
        {
            if let Some(name) = Self::detect_gpu_name_macos() {
                // Unified memory: VRAM is unknown here, sidecar estimates it.
                return (Some(name), None);
            }
        }

        #[cfg(all(unix, not(target_os = "macos")))]
        {
            if let Some((name, vram)) = Self::detect_gpu_name_vram_linux() {
                return (Some(name), vram);
            }
        }

        (None, None)
    }

    /// Windows fallback: PowerShell CIM. `AdapterRAM` is a u32, so values
    /// above ~4 GiB wrap; we cap the parsed result at 16 GiB and treat it as
    /// best-effort. Picks the first non-Microsoft-Basic adapter.
    #[cfg(target_os = "windows")]
    fn detect_gpu_name_vram_windows() -> Option<(String, Option<f32>)> {
        let out = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Get-CimInstance Win32_VideoController | \
                 Where-Object { $_.Name -notlike '*Basic Display*' } | \
                 Select-Object -First 1 -Property Name,AdapterRAM | \
                 ForEach-Object { \"$($_.Name)|$($_.AdapterRAM)\" }",
            ])
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let stdout = String::from_utf8_lossy(&out.stdout);
        let line = stdout.lines().next()?.trim();
        if line.is_empty() {
            return None;
        }
        let mut parts = line.split('|');
        let name = parts.next()?.trim().to_string();
        if name.is_empty() {
            return None;
        }
        let vram = parts
            .next()
            .and_then(|s| s.trim().parse::<f32>().ok())
            // AdapterRAM is bytes; clamp the known-broken u32 wraparound.
            .map(|bytes| (bytes / 1024.0 / 1024.0 / 1024.0).min(16.0));
        Some((name, vram))
    }

    /// macOS fallback: CPU/GPU brand string via sysctl. Unified memory means
    /// VRAM is not reported here.
    #[cfg(target_os = "macos")]
    fn detect_gpu_name_macos() -> Option<String> {
        let out = std::process::Command::new("sysctl")
            .arg("machdep.cpu.brand_string")
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let stdout = String::from_utf8_lossy(&out.stdout);
        let name = stdout
            .split(':')
            .nth(1)?
            .trim()
            .trim_end_matches("Apple Silicon")
            .trim();
        if name.is_empty() {
            None
        } else {
            Some(name.to_string())
        }
    }

    /// Linux fallback: NVIDIA driver sysfs (when present) or amdgpu sysfs.
    #[cfg(all(unix, not(target_os = "macos")))]
    fn detect_gpu_name_vram_linux() -> Option<(String, Option<f32>)> {
        // NVIDIA driver exposes per-GPU info files.
        if let Ok(entries) = std::fs::read_dir("/proc/driver/nvidia/gpus") {
            for entry in entries.flatten() {
                let info = entry.path().join("information");
                if let Ok(content) = std::fs::read_to_string(&info) {
                    let name = content
                        .lines()
                        .find(|l| l.starts_with("Model:"))
                        .and_then(|l| l.split(':').nth(1))
                        .map(|s| s.trim().trim_matches('"').to_string());
                    if let Some(name) = name {
                        return Some((name, None));
                    }
                }
            }
        }
        // amdgpu exposes VRAM in bytes via sysfs.
        if let Ok(entries) = std::fs::read_dir("/sys/class/drm") {
            for entry in entries.flatten() {
                let name_path = entry.path().join("device").join("mem_info_vram_total");
                if let Ok(bytes_str) = std::fs::read_to_string(&name_path) {
                    if let Ok(bytes) = bytes_str.trim().parse::<f32>() {
                        let vram = bytes / 1024.0 / 1024.0 / 1024.0;
                        // Name lookup is unreliable here; return what we have.
                        return Some((format!("GPU ({:.0} GB VRAM)", vram), Some(vram)));
                    }
                }
            }
        }
        None
    }

    /// Generate adaptive Whisper configuration based on hardware
    pub fn get_whisper_config(&self) -> AdaptiveWhisperConfig {
        // Windows-specific override: Always use beam size 2 for stability
        #[cfg(target_os = "windows")]
        {
            return AdaptiveWhisperConfig {
                beam_size: 2,
                temperature: 0.2,
                use_gpu: self.has_gpu_acceleration,
                max_threads: Some(self.cpu_cores.min(8) as usize),
                chunk_size_preference: ChunkSizePreference::Balanced,
            };
        }

        // Platform-adaptive configuration for non-Windows systems
        #[cfg(not(target_os = "windows"))]
        {
            match self.performance_tier {
                PerformanceTier::Ultra => AdaptiveWhisperConfig {
                    beam_size: 5,  // Maximum quality
                    temperature: 0.1,
                    use_gpu: self.has_gpu_acceleration,
                    max_threads: Some(self.cpu_cores.min(8) as usize),
                    chunk_size_preference: ChunkSizePreference::Quality,
                },
                PerformanceTier::High => AdaptiveWhisperConfig {
                    beam_size: 3,  // High quality
                    temperature: 0.2,
                    use_gpu: self.has_gpu_acceleration,
                    max_threads: Some(self.cpu_cores.min(6) as usize),
                    chunk_size_preference: ChunkSizePreference::Balanced,
                },
                PerformanceTier::Medium => AdaptiveWhisperConfig {
                    beam_size: 2,  // Balanced
                    temperature: 0.3,
                    use_gpu: self.has_gpu_acceleration,
                    max_threads: Some(self.cpu_cores.min(4) as usize),
                    chunk_size_preference: ChunkSizePreference::Balanced,
                },
                PerformanceTier::Low => AdaptiveWhisperConfig {
                    beam_size: 1,  // Fast processing
                    temperature: 0.4,
                    use_gpu: false, // Force CPU to avoid GPU overhead on weak hardware
                    max_threads: Some(2),
                    chunk_size_preference: ChunkSizePreference::Fast,
                },
            }
        }
    }

    /// Get recommended chunk duration in milliseconds based on performance tier
    pub fn get_recommended_chunk_duration_ms(&self) -> u32 {
        match self.performance_tier {
            PerformanceTier::Ultra => 25000,   // 25 seconds for maximum accuracy
            PerformanceTier::High => 20000,    // 20 seconds for high quality
            PerformanceTier::Medium => 15000,  // 15 seconds for balance
            PerformanceTier::Low => 10000,     // 10 seconds for responsiveness
        }
    }

    /// Check if hardware can handle real-time processing of given sample rate
    pub fn can_handle_realtime(&self, sample_rate: u32, channels: u16) -> bool {
        let data_rate = sample_rate * channels as u32;

        match self.performance_tier {
            PerformanceTier::Ultra => data_rate <= 192000, // Up to 192kHz stereo
            PerformanceTier::High => data_rate <= 96000,   // Up to 96kHz stereo or 192kHz mono
            PerformanceTier::Medium => data_rate <= 48000, // Up to 48kHz stereo
            PerformanceTier::Low => data_rate <= 22050,    // Up to 22kHz stereo or 48kHz mono
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hardware_detection() {
        let profile = HardwareProfile::detect();
        assert!(profile.cpu_cores > 0);
        // Performance optimization: remove println! from tests
        log::debug!("Detected profile: {:?}", profile);
    }

    #[test]
    fn test_whisper_config_generation() {
        let profile = HardwareProfile::detect();
        let config = profile.get_whisper_config();

        assert!(config.beam_size >= 1 && config.beam_size <= 5);
        assert!(config.temperature >= 0.0 && config.temperature <= 1.0);

        // Performance optimization: remove println! from tests
        log::debug!("Generated config: {:?}", config);
    }

    #[test]
    fn test_performance_tier_logic() {
        // Test different hardware combinations
        let low_tier = HardwareProfile::calculate_performance_tier(2, &GpuType::None, 4);
        assert_eq!(low_tier, PerformanceTier::Low);

        let high_tier = HardwareProfile::calculate_performance_tier(8, &GpuType::Metal, 16);
        assert_eq!(high_tier, PerformanceTier::Ultra);
    }

    #[test]
    fn hardware_detector_finds_windows_vulkan_loader_in_system32() {
        let temp_dir = tempfile::tempdir().unwrap();
        let system32 = temp_dir.path().join("System32");
        std::fs::create_dir(&system32).unwrap();
        std::fs::write(system32.join("vulkan-1.dll"), []).unwrap();

        assert!(HardwareProfile::has_windows_vulkan_loader(temp_dir.path()));
    }

    #[test]
    fn hardware_detector_rejects_missing_windows_vulkan_loader() {
        let temp_dir = tempfile::tempdir().unwrap();

        assert!(!HardwareProfile::has_windows_vulkan_loader(temp_dir.path()));
    }
}
