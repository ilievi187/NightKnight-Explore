import { describe, expect, it } from 'vitest'
import { CLUSTER_RADIUS_PX, CLUSTER_ZOOM_CUTOFF, clusterPoints, clusterThresholdPx } from './cluster'

const point = (id: string, x: number, y: number) => ({ id, x, y })

describe('cluster threshold', () => {
  it('groups markers while zoomed out', () => {
    expect(clusterThresholdPx(10)).toBe(CLUSTER_RADIUS_PX)
    expect(clusterThresholdPx(14.9)).toBe(CLUSTER_RADIUS_PX)
  })

  it('stops grouping at the close zoom so clusters can split', () => {
    expect(clusterThresholdPx(CLUSTER_ZOOM_CUTOFF)).toBe(0)
    expect(clusterThresholdPx(18)).toBe(0)
    expect(clusterThresholdPx(Number.NaN)).toBe(0)
  })
})

describe('screen-space clustering', () => {
  it('returns nothing for an empty list', () => {
    expect(clusterPoints([], 40)).toEqual([])
  })

  it('keeps every point separate when the threshold is not positive', () => {
    const points = [point('a', 0, 0), point('b', 1, 1)]
    expect(clusterPoints(points, 0).map((group) => group.points.map((item) => item.id))).toEqual([['a'], ['b']])
    expect(clusterPoints(points, -5)).toHaveLength(2)
  })

  it('merges points inside the pixel radius and leaves distant points alone', () => {
    const groups = clusterPoints(
      [point('near-a', 0, 0), point('near-b', 10, 0), point('far', 200, 0)],
      40,
    )
    expect(groups).toHaveLength(2)
    expect(groups[0].points.map((item) => item.id)).toEqual(['near-a', 'near-b'])
    expect(groups[0].x).toBe(5)
    expect(groups[0].y).toBe(0)
    expect(groups[1].points.map((item) => item.id)).toEqual(['far'])
  })

  it('does not pull a far point in through a chain of neighbors', () => {
    const groups = clusterPoints(
      [point('a', 0, 0), point('b', 30, 0), point('c', 60, 0)],
      40,
    )
    expect(groups.map((group) => group.points.map((item) => item.id))).toEqual([['a', 'b'], ['c']])
  })
})
