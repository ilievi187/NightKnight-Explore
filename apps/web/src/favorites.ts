export type FavoritePlace = {
  id: string
  source_url: string
}

export const FAVORITES_STORAGE_KEY = 'nightknight-explore:favorites:v1'

export interface KeyValueStorage {
  getItem(key: string): string | null
  setItem(key: string, value: string): void
  removeItem(key: string): void
}

function isFavoritePlace(value: unknown): value is FavoritePlace {
  if (!value || typeof value !== 'object') return false
  const record = value as Record<string, unknown>
  if (typeof record.id !== 'string' || typeof record.source_url !== 'string') return false

  const idMatch = /^(node|way|relation)-([1-9]\d*)$/.exec(record.id)
  if (!idMatch) return false

  try {
    const url = new URL(record.source_url)
    return url.protocol === 'https:'
      && url.hostname === 'www.openstreetmap.org'
      && url.pathname === `/${idMatch[1]}/${idMatch[2]}`
      && !url.search
      && !url.hash
  } catch {
    return false
  }
}

export function readFavorites(storage?: KeyValueStorage): FavoritePlace[] {
  try {
    const target = storage ?? window.localStorage
    const stored: unknown = JSON.parse(target.getItem(FAVORITES_STORAGE_KEY) ?? '[]')
    if (!Array.isArray(stored)) return []
    return stored.filter(isFavoritePlace).map(({ id, source_url }) => ({ id, source_url }))
  } catch {
    return []
  }
}

export function addFavorite(place: FavoritePlace, storage?: KeyValueStorage): FavoritePlace[] {
  if (!isFavoritePlace(place)) throw new Error('Only canonical OpenStreetMap place links can be saved.')
  const favorites = readFavorites(storage)
  if (favorites.some((favorite) => favorite.id === place.id)) return favorites

  const updated = [...favorites, { id: place.id, source_url: place.source_url }]
  const target = storage ?? window.localStorage
  target.setItem(FAVORITES_STORAGE_KEY, JSON.stringify(updated))
  return updated
}

export function removeFavorite(id: string, storage?: KeyValueStorage): FavoritePlace[] {
  const updated = readFavorites(storage).filter((favorite) => favorite.id !== id)
  const target = storage ?? window.localStorage
  if (updated.length === 0) target.removeItem(FAVORITES_STORAGE_KEY)
  else target.setItem(FAVORITES_STORAGE_KEY, JSON.stringify(updated))
  return updated
}

export function clearFavorites(storage?: KeyValueStorage): void {
  const target = storage ?? window.localStorage
  target.removeItem(FAVORITES_STORAGE_KEY)
}
