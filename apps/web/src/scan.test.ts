import { describe, expect, it } from 'vitest'
import { gemTier, gemTierLabel, prettyDistance, prettyElapsed, scanProgressNote } from './scan'

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
