export type ClusterInput = { id: string; x: number; y: number }

export type PointCluster<T extends ClusterInput> = {
  points: T[]
  x: number
  y: number
}

/** Screen-pixel radius used to group markers while the map is zoomed out. */
export const CLUSTER_RADIUS_PX = 46

/**
 * At this zoom and closer, every place is its own marker. Zooming a cluster
 * to this level is what splits it.
 */
export const CLUSTER_ZOOM_CUTOFF = 15

export function clusterThresholdPx(zoom: number): number {
  if (!Number.isFinite(zoom) || zoom >= CLUSTER_ZOOM_CUTOFF) return 0
  return CLUSTER_RADIUS_PX
}

/**
 * Greedy screen-space grouping. Membership is measured from the seed point,
 * not a moving center, so the result stays stable for a fixed zoom.
 * A non-positive threshold returns one group per point.
 */
export function clusterPoints<T extends ClusterInput>(
  points: readonly T[],
  thresholdPx: number,
): PointCluster<T>[] {
  if (points.length === 0) return []
  if (!Number.isFinite(thresholdPx) || thresholdPx <= 0) {
    return points.map((point) => ({ points: [point], x: point.x, y: point.y }))
  }

  const groups: PointCluster<T>[] = []
  const used = new Set<number>()
  const radiusSq = thresholdPx * thresholdPx
  for (let index = 0; index < points.length; index += 1) {
    if (used.has(index)) continue
    const seed = points[index]
    const members: T[] = [seed]
    used.add(index)
    let sumX = seed.x
    let sumY = seed.y
    for (let other = index + 1; other < points.length; other += 1) {
      if (used.has(other)) continue
      const candidate = points[other]
      const dx = candidate.x - seed.x
      const dy = candidate.y - seed.y
      if (dx * dx + dy * dy <= radiusSq) {
        members.push(candidate)
        used.add(other)
        sumX += candidate.x
        sumY += candidate.y
      }
    }
    groups.push({ points: members, x: sumX / members.length, y: sumY / members.length })
  }
  return groups
}
