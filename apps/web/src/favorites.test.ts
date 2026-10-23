import { describe, expect, it } from 'vitest'
import { addFavorite, clearFavorites, readFavorites, removeFavorite } from './favorites'

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

const cafe = { id: 'node-42', source_url: 'https://www.openstreetmap.org/node/42' }

describe('local favorites', () => {
  it('persists only selected place identifiers and source URLs', () => {
    const storage = memoryStorage()
    const detailedPlace = { ...cafe, name: 'Private label', lat: 42.7, lon: 23.3, distance_m: 300 }
    addFavorite(detailedPlace, storage)

    const stored = JSON.parse(storage.getItem('nightknight-explore:favorites:v1')!)
    expect(stored).toEqual([cafe])
    expect(readFavorites(storage)).toEqual([cafe])
  })

  it('does not duplicate a saved place', () => {
    const storage = memoryStorage()
    addFavorite(cafe, storage)
    addFavorite(cafe, storage)

    expect(readFavorites(storage)).toEqual([cafe])
  })

  it('removes one favorite without touching the others', () => {
    const storage = memoryStorage()
    const museum = { id: 'way-9', source_url: 'https://www.openstreetmap.org/way/9' }
    addFavorite(cafe, storage)
    addFavorite(museum, storage)
    removeFavorite(cafe.id, storage)

    expect(readFavorites(storage)).toEqual([museum])
  })

  it('clears saved favorites without creating scan history', () => {
    const storage = memoryStorage()
    addFavorite(cafe, storage)
    clearFavorites(storage)

    expect(readFavorites(storage)).toEqual([])
    expect(storage.length).toBe(0)
  })

  it('ignores malformed or non-OSM stored records', () => {
    const storage = memoryStorage()
    storage.setItem('nightknight-explore:favorites:v1', JSON.stringify([
      cafe,
      { id: 'evil', source_url: 'javascript:alert(1)' },
      { id: 'node-3', source_url: 'https://example.org/place/3', name: 'private label' },
    ]))

    expect(readFavorites(storage)).toEqual([cafe])
  })
})
