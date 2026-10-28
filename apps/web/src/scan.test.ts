import { describe, expect, it } from 'vitest'
import { appendScanPage, gemTier, gemTierLabel, normalizePlaceQuery, prettyDistance, prettyElapsed, readJsonBody, scanErrorMessage, scanProgressNote, type ScanPage } from './scan'

describe('distance formatting', () => {
  it('keeps meters below a kilometer', () => {
    expect(prettyDistance(50)).toBe('50 m')
    expect(prettyDistance(999)).toBe('999 m')
  })

  it('formats kilometres with one decimal', () => {
    expect(prettyDistance(1000)).toBe('1.0 km')
    expect(prettyDistance(2345)).toBe('2.3 km')
    expect(prettyDistance(10000)).toBe('10.0 km')
  })
})

describe('elapsed formatting', () => {
  it('shows seconds under a minute', () => {
    expect(prettyElapsed(0)).toBe('0s')
    expect(prettyElapsed(41_000)).toBe('41s')
  })

  it('shows minutes and zero-padded seconds over a minute', () => {
    expect(prettyElapsed(65_000)).toBe('1m 05s')
    expect(prettyElapsed(125_000)).toBe('2m 05s')
  })

  it('never shows a negative duration', () => {
    expect(prettyElapsed(-3000)).toBe('0s')
  })
})

describe('explicit place search input', () => {
  it('trims and normalizes whitespace only after submission', () => {
    expect(normalizePlaceQuery('  Sofia   Bulgaria ')).toBe('Sofia Bulgaria')
  })

  it('rejects too-short and overlong search terms', () => {
    expect(normalizePlaceQuery(' x ')).toBeNull()
    expect(normalizePlaceQuery('x'.repeat(201))).toBeNull()
  })

  it('counts Unicode code points rather than UTF-16 units', () => {
    expect(normalizePlaceQuery('🏰')).toBeNull()
    expect(normalizePlaceQuery('🏰🏰')).toBe('🏰🏰')
  })
})

describe('scan progress notes', () => {
  it('describes cached scans without implying new provider traffic', () => {
    expect(scanProgressNote(20_000, true)).toContain('cached')
  })

  it('matches elapsed time to honest expectation copy', () => {
    expect(scanProgressNote(2_000, false)).toContain('Contacting')
    expect(scanProgressNote(15_000, false)).toContain('larger radii take longer')
    expect(scanProgressNote(35_000, false)).toContain('still running')
    expect(scanProgressNote(95_000, false)).toContain('times out honestly')
  })
})

describe('gem tiers', () => {
  it('ranks scores into fixed, documented thresholds', () => {
    expect(gemTier(75)).toBe('rare')
    expect(gemTier(74)).toBe('strong')
    expect(gemTier(50)).toBe('strong')
    expect(gemTier(49)).toBe('notable')
    expect(gemTier(0)).toBe('notable')
  })

  it('falls back to the lowest tier for bad input instead of crashing', () => {
    expect(gemTier(Number.NaN)).toBe('notable')
  })

  it('labels every tier', () => {
    expect(gemTierLabel('rare')).toBe('Rare find')
    expect(gemTierLabel('strong')).toBe('Hidden gem')
    expect(gemTierLabel('notable')).toBe('Notable')
  })
})

function fakeResponse(status: number, body: string): Response {
  return {
    status,
    ok: status >= 200 && status < 300,
    text: async () => body,
  } as unknown as Response
}

describe('API response parsing', () => {
  it('parses valid JSON bodies', async () => {
    await expect(readJsonBody(fakeResponse(200, '{"places":[]}'))).resolves.toEqual({ places: [] })
  })

  it('reports an empty body without throwing', async () => {
    await expect(readJsonBody(fakeResponse(500, ''))).resolves.toBeNull()
  })

  it('reports unparseable bodies without throwing', async () => {
    await expect(readJsonBody(fakeResponse(502, '<html>bad gateway</html>'))).resolves.toBeUndefined()
  })
})

describe('scan error messages', () => {
  it('uses structured provider errors verbatim with provider and retry hint', () => {
    const message = scanErrorMessage(fakeResponse(429, ''), {
      error: 'Scan cooldown: please retry after 8 seconds',
      code: 'provider_rate_limited',
      provider_id: 'openstreetmap-overpass',
      retry_after_seconds: 8,
    })
    expect(message).toContain('Scan cooldown')
    expect(message).toContain('openstreetmap-overpass')
    expect(message).toContain('retry in 8s')
  })

  it('explains empty 5xx proxy answers as an unreachable local API', () => {
    const message = scanErrorMessage(fakeResponse(500, ''), null)
    expect(message).toContain('did not respond')
    expect(message).toContain('Rust API')
  })

  it('never surfaces raw JSON parse exceptions', () => {
    const message = scanErrorMessage(fakeResponse(400, ''), undefined)
    expect(message).not.toContain('Unexpected end of JSON')
    expect(message).toContain('unreadable response')
  })
})

describe('scan page append', () => {
  const page = (ids: string[], total: number, cached = false): ScanPage<{ id: string }> => ({
    places: ids.map((id) => ({ id })),
    scanned_at_unix: 1,
    source: 'OpenStreetMap',
    cached,
    total_matches: total,
  })

  it('keeps the first page unchanged when there is no previous scan', () => {
    expect(appendScanPage(null, page(['a'], 1))).toEqual(page(['a'], 1))
  })

  it('appends the next page and preserves the full match count', () => {
    const loaded = appendScanPage(page(['a', 'b'], 4), page(['c', 'd'], 4, true))
    expect(loaded.places.map((place) => place.id)).toEqual(['a', 'b', 'c', 'd'])
    expect(loaded.total_matches).toBe(4)
    expect(loaded.cached).toBe(true)
  })

  it('ignores duplicate ids from a repeated page', () => {
    const loaded = appendScanPage(page(['a', 'b'], 3), page(['b', 'c'], 3))
    expect(loaded.places.map((place) => place.id)).toEqual(['a', 'b', 'c'])
    expect(loaded.total_matches).toBe(3)
  })
})
