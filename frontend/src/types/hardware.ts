// Mirrors Rust `HardwareProfileInfo` (commands.rs). camelCase matches the
// `#[serde(rename_all = "camelCase")]` on the Rust struct.
export interface HardwareProfileInfo {
  cpuCores: number;
  memoryGb: number;
  /** "none" | "metal" | "cuda" | "vulkan" | "opencl" */
  gpuType: string;
  gpuName: string | null;
  gpuVramGb: number | null;
  vulkanAvailable: boolean;
  /** "low" | "medium" | "high" | "ultra" */
  performanceTier: string;
  recommendedModel: string;
  /** "CPU" | "GPU (Vulkan)" | "GPU (Metal)" | "GPU (CUDA)" */
  recommendedInferenceMode: string;
  hasGpu: boolean;
}
