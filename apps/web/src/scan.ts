export type ScanPhase =
  | { kind: 'idle' }
  | { kind: 'running'; startedAt: number }
  | { kind: 'done'; durationMs: number }
  | { kind: 'failed'; durationMs: number }

export const prettyDistance = (m: number): string =>
  m < 1000 ? `${m} m` : `${(m / 1000).toFixed(1)} km`

export const prettyElapsed = (ms: number): string => {
  const total = Math.max(0, Math.floor(ms / 1000))
  const minutes = Math.floor(total / 60)
  const seconds = total % 60
  return minutes > 0 ? `${minutes}m ${String(seconds).padStart(2, '0')}s` : `${seconds}s`
}

/**
 * Reassurance copy shown while a scan runs. Phases are honest: they describe
 * elapsed time and provider expectations, never simulated progress.
 */
export function scanProgressNote(elapsedMs: number, cached: boolean): string {
  if (cached) return 'Reusing a cached scan of this exact location and radius.'
  if (elapsedMs < 8_000) return 'Contacting the open place data services…'
  if (elapsedMs < 25_000) return 'Providers are gathering records — larger radii take longer.'
  if (elapsedMs < 60_000) return 'Shared data services are busy; the scan is still running.'
  return 'Taking a while — shared services are slow right now. The scan times out honestly instead of stalling forever.'
}

export type GemTier = 'rare' | 'strong' | 'notable'

/** Display tier for a transparent gem score — tiers summarize the same evidence, nothing else. */
export function gemTier(score: number): GemTier {
  if (!Number.isFinite(score)) return 'notable'
  if (score >= 75) return 'rare'
  if (score >= 50) return 'strong'
  return 'notable'
}

export const gemTierLabel = (tier: GemTier): string =>
  tier === 'rare' ? 'Rare find' : tier === 'strong' ? 'Hidden gem' : 'Notable'

/**
 * Parse an API response body without ever throwing a raw JSON exception.
 * Local proxies can answer non-2xx statuses with an empty body; a scan
 * failure must surface as a clean message, not "Unexpected end of JSON input".
 * Returns `null` for an empty body and `undefined` for a body that is not
 * valid JSON.
 */
export async function readJsonBody(response: Response): Promise<unknown> {
  let text: string
  try {
    text = await response.text()
  } catch {
    return undefined
  }
  if (!text.trim()) return null
  try {
    return JSON.parse(text)
  } catch {
    return undefined
  }
}

export type ScanApiError = {
  error: string
  code: string
  provider_id: string | null
  retry_after_seconds: number | null
}

export function isScanApiError(value: unknown): value is ScanApiError {
  if (!value || typeof value !== 'object') return false
  const record = value as Record<string, unknown>
  return typeof record.error === 'string' && typeof record.code === 'string'
}

/**
 * Human-readable scan failure text. Distinguishes "the API itself is
 * unreachable" from honest provider errors returned by the API.
 */
export function scanErrorMessage(response: Response, body: unknown): string {
  if (isScanApiError(body)) {
    const provider = body.provider_id ? ` · ${body.provider_id} / ${body.code}` : ''
    const retry = body.retry_after_seconds ? ` (retry in ${body.retry_after_seconds}s)` : ''
    return `${body.error}${provider}${retry}`
  }
  if (response.status >= 500) {
    return `The local API did not respond (HTTP ${response.status}). Check that the Rust API is running.`
  }
  return `Scan failed (HTTP ${response.status}) with an unreadable response.`
}

