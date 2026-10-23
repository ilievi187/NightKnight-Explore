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
