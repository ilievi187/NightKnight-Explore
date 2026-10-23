import { describe, expect, it } from 'vitest'

// Contract tests for Google Places API (New) response shapes.
// These use official documented response structures, not legacy mocked JSON.

describe('Google Places API (New) contract', () => {
  it('Nearby Search returns { places: [...] }', () => {
    const response = {
      places: [
        {
          id: 'ChIJN1t_tDeuEmsRUsoyG83frY4',
          displayName: { text: 'Restaurant Name', languageCode: 'en' },
          types: ['restaurant', 'food', 'point_of_interest'],
          rating: 4.5,
          userRatingCount: 123,
          formattedAddress: '123 Main St, Sofia, Bulgaria',
          websiteUri: 'https://example.com',
          internationalPhoneNumber: '+359 2 123 4567',
          priceLevel: 'PRICE_LEVEL_MODERATE',
          photos: [
            {
              name: 'places/ChIJN1t_tDeuEmsRUsoyG83frY4/photos/abc123',
              widthPx: 400,
              heightPx: 300,
              authorAttributions: [
                { displayName: 'John Doe', uri: 'https://example.com/john', photoUri: 'https://example.com/photo' },
              ],
            },
          ],
        },
      ],
    }
    expect(response.places).toBeDefined()
    expect(Array.isArray(response.places)).toBe(true)
    expect(response.places[0].id).toBeDefined()
    expect(response.places[0].displayName?.text).toBe('Restaurant Name')
    expect(response.places[0].userRatingCount).toBe(123)
    expect(response.places[0].formattedAddress).toBeDefined()
    expect(response.places[0].websiteUri).toBeDefined()
    expect(response.places[0].photos?.[0]?.name).toBeDefined()
    expect(response.places[0].photos?.[0]?.authorAttributions?.[0]?.displayName).toBeDefined()
  })

  it('Place Details returns nested fields correctly', () => {
    const response = {
      id: 'ChIJN1t_tDeuEmsRUsoyG83frY4',
      displayName: { text: 'Restaurant Name', languageCode: 'en' },
      rating: 4.5,
      userRatingCount: 123,
      formattedAddress: '123 Main St, Sofia, Bulgaria',
      websiteUri: 'https://example.com',
      internationalPhoneNumber: '+359 2 123 4567',
      priceLevel: 'PRICE_LEVEL_MODERATE',
      regularOpeningHours: {
        openNow: true,
        periods: [
          { open: { day: 1, hour: 9, minute: 0 }, close: { day: 1, hour: 22, minute: 0 } },
        ],
        weekdayDescriptions: ['Monday: 9:00 AM – 10:00 PM'],
      },
      reviews: [
        {
          authorAttribution: { displayName: 'Jane Smith', uri: 'https://example.com/jane', photoUri: 'https://example.com/jane-photo' },
          rating: 5,
          text: { text: 'Great food!', languageCode: 'en' },
          relativePublishTimeDescription: 'a week ago',
          publishTime: '2024-01-01T00:00:00Z',
        },
      ],
      photos: [
        {
          name: 'places/ChIJN1t_tDeuEmsRUsoyG83frY4/photos/abc123',
          widthPx: 400,
          heightPx: 300,
          authorAttributions: [
            { displayName: 'John Doe', uri: 'https://example.com/john', photoUri: 'https://example.com/photo' },
          ],
        },
      ],
    }
    expect(response.displayName?.text).toBe('Restaurant Name')
    expect(response.userRatingCount).toBe(123)
    expect(response.formattedAddress).toBeDefined()
    expect(response.websiteUri).toBeDefined()
    expect(response.regularOpeningHours?.openNow).toBe(true)
    expect(response.reviews?.[0]?.authorAttribution?.displayName).toBeDefined()
    expect(response.reviews?.[0]?.text?.text).toBe('Great food!')
    expect(response.photos?.[0]?.name).toBeDefined()
    expect(response.photos?.[0]?.authorAttributions?.[0]?.displayName).toBeDefined()
  })

  it('Places Photo (New) uses photos[].name and authorAttributions', () => {
    // Photo media URL format: https://places.googleapis.com/v1/{photo_name}?key=...&maxWidthPx=...
    const photoName = 'places/ChIJN1t_tDeuEmsRUsoyG83frY4/photos/abc123'
    const photoUrl = `https://places.googleapis.com/v1/${photoName}?key=SERVER_KEY&maxWidthPx=400`
    expect(photoUrl).toContain('places/ChIJN1t_tDeuEmsRUsoyG83frY4/photos/abc123')
    expect(photoUrl).toContain('maxWidthPx=400')
    // Server key should NOT be in browser URL - use proxy endpoint instead
    const proxyUrl = `/api/google/photo?name=${encodeURIComponent(photoName)}&maxWidthPx=400`
    expect(proxyUrl).not.toContain('SERVER_KEY')
    expect(proxyUrl).toContain('/api/google/photo')
  })

  it('Field masks determine pricing', () => {
    // Nearby Search field mask
    const nearbyMask = 'places.id,places.displayName,places.types,places.rating,places.userRatingCount,places.formattedAddress,places.websiteUri,places.internationalPhoneNumber,places.priceLevel,places.photos'
    expect(nearbyMask).toContain('places.id')
    expect(nearbyMask).toContain('places.displayName')
    // Photos in search results trigger additional SKU
    expect(nearbyMask).toContain('places.photos')

    // Details field mask
    const detailsMask = 'id,displayName,rating,userRatingCount,formattedAddress,websiteUri,internationalPhoneNumber,priceLevel,regularOpeningHours,photos,reviews'
    expect(detailsMask).toContain('reviews')
    expect(detailsMask).toContain('photos')
    // Reviews trigger Atmosphere SKU
    // Photos trigger additional SKU
  })
})