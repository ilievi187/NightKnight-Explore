const MAX_NAME_LENGTH = 200
const CATEGORY_KEYS = ['food', 'stay', 'see', 'history', 'nature'] as const
type FavoriteCategory = (typeof CATEGORY_KEYS)[number]

export type FavoritePlace = {
  id: string
  source_url: string
  /** Public OSM record name for display only; optional for v1 compatibility. */
  name?: string
  /** Public OSM category key for display only; optional for v1 compatibility. */
  category?: FavoriteCategory
}

export const FAVORITES_STORAGE_KEY = 'nightknight-explore:favorites:v2'
/** Storage key of the first favorites format (id + source_url only). */
export const FAVORITES_STORAGE_KEY_V1 = 'nightknight-explore:favorites:v1'

export interface KeyValueStorage {
  getItem(key: string): string | null
  setItem(key: string, value: string): void
  removeItem(key: string): void
}

function osmIdMatch(id: unknown): RegExpExecArray | null {
  if (typeof id !== 'string') return null
  return /^(node|way|relation)-([1-9]\d*)$/.exec(id)
}

function isCanonicalRecordUrl(url: unknown, idMatch: RegExpExecArray): boolean {
  if (typeof url !== 'string') return false
  try {
    const parsed = new URL(url)
    return parsed.protocol === 'https:'
      && parsed.hostname === 'www.openstreetmap.org'
      && parsed.pathname === `/${idMatch[1]}/${idMatch[2]}`
      && !parsed.search
      && !parsed.hash
  } catch {
    return false
  }
}

function isFavoritePlace(value: unknown): value is FavoritePlace {
  if (!value || typeof value !== 'object') return false
  const record = value as Record<string, unknown>
  const idMatch = osmIdMatch(record.id)
  if (!idMatch) return false
  if (!isCanonicalRecordUrl(record.source_url, idMatch)) return false

  if (record.name !== undefined) {
    if (typeof record.name !== 'string') return false
    if (record.name.length > MAX_NAME_LENGTH) return false
    if (!record.name.trim()) return false
  }
  if (record.category !== undefined && !CATEGORY_KEYS.includes(record.category as (typeof CATEGORY_KEYS)[number])) {
    return false
  }
  return true
}

function storedEntries(raw: string | null): unknown[] {
  if (!raw) return []
  try {
    const parsed: unknown = JSON.parse(raw)
    return Array.isArray(parsed) ? parsed : []
  } catch {
    return []
  }
}

export function readFavorites(storage?: KeyValueStorage): FavoritePlace[] {
  try {
    const target = storage ?? window.localStorage
    // v2 first; fall back to v1 records (id + source_url only) so an
    // existing browser's saved links survive the format upgrade.
    const v2 = storedEntries(target.getItem(FAVORITES_STORAGE_KEY))
      .filter(isFavoritePlace)
      .map(({ id, source_url, name, category }) => ({ id, source_url, name, category }))
    if (v2.length > 0) return v2
    return storedEntries(target.getItem(FAVORITES_STORAGE_KEY_V1))
      .filter(isFavoritePlace)
      .map(({ id, source_url }) => ({ id, source_url }))
  } catch {
    return []
  }
}

export function addFavorite(place: FavoritePlace, storage?: KeyValueStorage): FavoritePlace[] {
  if (!isFavoritePlace(place)) throw new Error('Only canonical OpenStreetMap place links can be saved.')
  const favorites = readFavorites(storage)
  if (favorites.some((favorite) => favorite.id === place.id)) return favorites

  const updated = [...favorites, {
    id: place.id,
    source_url: place.source_url,
    name: place.name?.trim(),
    category: place.category,
  }]
  const target = storage ?? window.localStorage
  target.setItem(FAVORITES_STORAGE_KEY, JSON.stringify(updated))
  target.removeItem(FAVORITES_STORAGE_KEY_V1)
  return updated
}

export function removeFavorite(id: string, storage?: KeyValueStorage): FavoritePlace[] {
  const updated = readFavorites(storage).filter((favorite) => favorite.id !== id)
  const target = storage ?? window.localStorage
  if (updated.length === 0) {
    target.removeItem(FAVORITES_STORAGE_KEY)
    target.removeItem(FAVORITES_STORAGE_KEY_V1)
  } else {
    target.setItem(FAVORITES_STORAGE_KEY, JSON.stringify(updated))
  }
  return updated
}

export function clearFavorites(storage?: KeyValueStorage): void {
  const target = storage ?? window.localStorage
  target.removeItem(FAVORITES_STORAGE_KEY)
  target.removeItem(FAVORITES_STORAGE_KEY_V1)
}
