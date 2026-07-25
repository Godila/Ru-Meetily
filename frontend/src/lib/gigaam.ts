// Types for GigaAM (Sber ai-sage) integration. Mirrors `lib/parakeet.ts`.
export interface GigaamModelInfo {
  name: string;
  path: string;
  size_mb: number;
  accuracy: ModelAccuracy;
  speed: ProcessingSpeed;
  status: ModelStatus;
  description?: string;
  quantization: QuantizationType;
}

export type QuantizationType = 'FP32' | 'Int8';
export type ModelAccuracy = 'High' | 'Good' | 'Decent';
export type ProcessingSpeed = 'Slow' | 'Medium' | 'Fast' | 'Very Fast' | 'Ultra Fast';

export type ModelStatus =
  | 'Available'
  | 'Missing'
  | { Downloading: number }
  | { Error: string }
  | { Corrupted: { file_size: number; expected_min_size: number } };

export interface GigaamEngineState {
  currentModel: string | null;
  availableModels: GigaamModelInfo[];
  isLoading: boolean;
  error: string | null;
}

// User-friendly model display configuration.
export interface ModelDisplayInfo {
  friendlyName: string;
  icon: string;
  tagline: string;
  recommended?: boolean;
  tier: 'fastest' | 'balanced' | 'precise';
}

export const MODEL_DISPLAY_CONFIG: Record<string, ModelDisplayInfo> = {
  'gigaam-v3-rnnt-int8': {
    friendlyName: 'GigaAM RU',
    icon: '🇷🇺',
    tagline: 'Лучшее распознавание русского • RNN-T с пунктуацией, int8 (~227 МБ)',
    recommended: true,
    tier: 'precise'
  }
};

// Model configuration for GigaAM models (matching Rust implementation).
// Source: https://huggingface.co/istupakov/gigaam-v3-onnx
export const GIGAAM_MODEL_CONFIGS: Record<string, Partial<GigaamModelInfo>> = {
  'gigaam-v3-rnnt-int8': {
    description: 'Лучшее распознавание русского, RNN-T с пунктуацией, int8',
    size_mb: 227, // Actual download: ~225 MB encoder + decoder + joint + vocab + config
    accuracy: 'High',
    speed: 'Fast',
    quantization: 'Int8'
  }
};

// Helper functions
export function getModelIcon(accuracy: ModelAccuracy): string {
  switch (accuracy) {
    case 'High': return '🔥';
    case 'Good': return '⚡';
    case 'Decent': return '🚀';
    default: return '📊';
  }
}

export function getModelDisplayName(modelName: string): string {
  const displayInfo = MODEL_DISPLAY_CONFIG[modelName];
  return displayInfo?.friendlyName || modelName;
}

export function getModelDisplayInfo(modelName: string): ModelDisplayInfo | null {
  return MODEL_DISPLAY_CONFIG[modelName] || null;
}

export function getStatusColor(status: ModelStatus): string {
  if (status === 'Available') return 'green';
  if (status === 'Missing') return 'gray';
  if (typeof status === 'object' && 'Downloading' in status) return 'blue';
  if (typeof status === 'object' && 'Error' in status) return 'red';
  return 'gray';
}

export function formatFileSize(sizeMb: number): string {
  if (sizeMb >= 1000) {
    return `${(sizeMb / 1000).toFixed(1)}GB`;
  }
  return `${sizeMb}MB`;
}

export function isQuantizedModel(modelName: string): boolean {
  return modelName.includes('int8');
}

export function getModelPerformanceBadge(quantization: QuantizationType): { label: string; color: string } {
  switch (quantization) {
    case 'FP32':
      return { label: 'Full Precision', color: 'blue' };
    case 'Int8':
      return { label: 'Int8 Quantized', color: 'green' };
    default:
      return { label: 'Standard', color: 'gray' };
  }
}

export function getRecommendedModel(): string {
  return 'gigaam-v3-rnnt-int8';
}

// Tauri command wrappers for GigaAM backend.
import { invoke } from '@tauri-apps/api/core';

export class GigaamAPI {
  static async init(): Promise<void> {
    await invoke('gigaam_init');
  }

  static async getAvailableModels(): Promise<GigaamModelInfo[]> {
    return await invoke('gigaam_get_available_models');
  }

  static async loadModel(modelName: string): Promise<void> {
    await invoke('gigaam_load_model', { modelName });
  }

  static async getCurrentModel(): Promise<string | null> {
    return await invoke('gigaam_get_current_model');
  }

  static async isModelLoaded(): Promise<boolean> {
    return await invoke('gigaam_is_model_loaded');
  }

  static async transcribeAudio(audioData: number[]): Promise<string> {
    return await invoke('gigaam_transcribe_audio', { audioData });
  }

  static async getModelsDirectory(): Promise<string> {
    return await invoke('gigaam_get_models_directory');
  }

  static async downloadModel(modelName: string): Promise<void> {
    await invoke('gigaam_download_model', { modelName });
  }

  static async cancelDownload(modelName: string): Promise<void> {
    await invoke('gigaam_cancel_download', { modelName });
  }

  static async deleteCorruptedModel(modelName: string): Promise<string> {
    return await invoke('gigaam_delete_corrupted_model', { modelName });
  }

  static async hasAvailableModels(): Promise<boolean> {
    return await invoke('gigaam_has_available_models');
  }

  static async validateModelReady(): Promise<string> {
    return await invoke('gigaam_validate_model_ready');
  }

  static async openModelsFolder(): Promise<void> {
    await invoke('open_gigaam_models_folder');
  }
}
