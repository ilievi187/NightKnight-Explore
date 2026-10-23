import { describe, expect, it } from 'vitest'
import { addFavorite, clearFavorites, FAVORITES_STORAGE_KEY, FAVORITES_STORAGE_KEY_V1, readFavorites, removeFavorite, type FavoritePlace } from './favorites'

function memoryStorage(): Storage {
  const values = new Map<string, string>()
  return {
    get length() { return values.size },
    clear: () => values.clear(),
    getItem: (key) => values.get(key) ?? null,
    key: (index) => [...values.keys()][index] ?? null,
    removeItem: (key) => values.delete(key),
    setItem: (key, value) => values.set(key, value),
  }
}

const cafe: FavoritePlace = { id: 'node-42', source_url: 'https://www.openstreetmap.org/node/42' }
const namedCafe: FavoritePlace = { ...cafe, name: 'Cosy Corner', category: 'food' }

describe('local favorites', () => {
  it('persists only selected identifiers, source URLs and public names', () => {
    const storage = memoryStorage()
    const detailedPlace = { ...namedCafe, lat: 42.7, lon: 23.3, distance_m: 300, secret: 'nope' }
    addFavorite(detailedPlace, storage)

    const stored = JSON.parse(storage.getItem(FAVORITES_STORAGE_KEY)!)
    expect(stored).toEqual([namedCafe])
    expect(readFavorites(storage)).toEqual([namedCafe])
  })

  it('does not duplicate a saved place', () => {
    const storage = memoryStorage()
    addFavorite(cafe, storage)
    addFavorite(namedCafe, storage)

    expect(readFavorites(storage)).toEqual([cafe])
  })

  it('removes one favorite without touching the others', () => {
    const storage = memoryStorage()
    const museum: FavoritePlace = { id: 'way-9', source_url: 'https://www.openstreetmap.org/way/9', name: 'Old Gate', category: 'history' }
    addFavorite(namedCafe, storage)
    addFavorite(museum, storage)
    removeFavorite(namedCafe.id, storage)

    expect(readFavorites(storage)).toEqual([museum])
  })

  it('clears saved favorites without creating scan history', () => {
    const storage = memoryStorage()
    addFavorite(namedCafe, storage)
    clearFavorites(storage)

    expect(readFavorites(storage)).toEqual([])
    expect(storage.length).toBe(0)
  })

  it('ignores malformed or non-OSM stored records', () => {
    const storage = memoryStorage()
    storage.setItem(FAVORITES_STORAGE_KEY, JSON.stringify([
      cafe,
      { id: 'evil', source_url: 'javascript:alert(1)' },
      { id: 'node-3', source_url: 'https://example.org/place/3', name: 'private label' },
    ]))

    expect(readFavorites(storage)).toEqual([cafe])
  })

  it('reads legacy v1 entries that only stored id and source URL', () => {
    const storage = memoryStorage()
    storage.setItem(FAVORITES_STORAGE_KEY_V1, JSON.stringify([cafe]))

    expect(readFavorites(storage)).toEqual([cafe])
  })

  it('migrates legacy v1 entries to v2 on the next save and removes the old key', () => {
    const storage = memoryStorage()
    storage.setItem(FAVORITES_STORAGE_KEY_V1, JSON.stringify([cafe]))
    const garden: FavoritePlace = { id: 'way-77', source_url: 'https://www.openstreetmap.org/way/77', name: 'City Garden', category: 'nature' }
    addFavorite(garden, storage)

    expect(readFavorites(storage)).toEqual([cafe, garden])
    expect(storage.getItem(FAVORITES_STORAGE_KEY_V1)).toBeNull()
    expect(storage.getItem(FAVORITES_STORAGE_KEY)).toContain('City Garden')
  })

  it('rejects overlong or unknown-category display fields', () => {
    const storage = memoryStorage()
    // Stored (untyped) records must be re-validated at runtime.
    storage.setItem(FAVORITES_STORAGE_KEY, JSON.stringify([
      { ...cafe, name: 'x'.repeat(201) },
      { ...cafe, id: 'node-43', source_url: 'https://www.openstreetmap.org/node/43', category: 'spooky' },
      { ...cafe, id: 'node-44', source_url: 'https://www.openstreetmap.org/node/44', name: 42 },
    ]))

    expect(readFavorites(storage)).toEqual([])
    expect(() => addFavorite({ ...cafe, name: 'x'.repeat(201) }, storage)).toThrow()
    expect(readFavorites(storage)).toEqual([])
  })
})

