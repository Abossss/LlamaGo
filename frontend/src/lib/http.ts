/** `fetch` with an abort timeout. llama-server may accept the TCP connection
 * but never answer while the model is loading, so a bare fetch can hang the
 * probe for tens of seconds. */
export async function fetchWithTimeout(
  input: string,
  init: RequestInit = {},
  timeoutMs = 4000,
): Promise<Response> {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  try {
    return await fetch(input, { ...init, signal: controller.signal });
  } finally {
    clearTimeout(timer);
  }
}

/** True when the rejection came from `fetchWithTimeout` aborting. */
export function isTimeoutError(err: unknown): boolean {
  return err instanceof DOMException && err.name === 'AbortError';
}
