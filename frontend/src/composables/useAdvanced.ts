import { ref } from 'vue';
import type { AdvancedParams, ConfigInfo } from '../types';
import { useConfig } from './useConfig';

// ── Module-level shared state (mirrors the original single-setup model) ──
export const advanced = ref<AdvancedParams>(emptyAdvanced());

/** model path -> alias. Persisted in config.json. */
export const aliases = ref<Record<string, string>>({});

export function emptyAdvanced(): AdvancedParams {
  return {
    ctx_size: null,
    ngl: null,
    temperature: null,
    max_tokens: null,
    api_key: null,
  };
}

/** Generate a fresh 32-hex-char API key (64 bits entropy). */
export function generateApiKey(): string {
  const buf = new Uint8Array(16);
  crypto.getRandomValues(buf);
  return Array.from(buf, (b) => b.toString(16).padStart(2, '0')).join('');
}

/** Persist the current `advanced` + `aliases` (via the shared saveSelection). */
export function persistAdvanced() {
  const { saveSelection } = useConfig();
  saveSelection();
}

export function useAdvanced() {
  return {
    advanced,
    aliases,
    generateApiKey,
    persistAdvanced,
  };
}

// Re-export for callers that just need to hydrate from a ConfigInfo object.
export type { ConfigInfo };
