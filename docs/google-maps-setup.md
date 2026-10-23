# Google Maps Platform Setup Guide

## Overview

NightKnight Explore supports Google Maps Platform for enhanced place discovery, including ratings, reviews, and photos. This is **optional** — the application works fully with OpenStreetMap (Overpass) without any Google configuration.

## Prerequisites

1. **Google Cloud Project** — Create a project at [console.cloud.google.com](https://console.cloud.google.com)
2. **Billing** — Google Maps Platform requires a billing account. As of March 2025, the flat $200/month credit was replaced by SKU-specific free usage caps. Check current pricing at https://developers.google.com/maps/billing-and-pricing/pricing
3. **API Keys** — Create separate keys for browser (Maps JS API) and server (Places API)

## Required APIs

Enable these APIs in your Google Cloud project:

- **Maps JavaScript API** — For the embedded map
- **Places API (New)** — For place search and details
- **Maps Embed API** — Optional, for basic embedded map viewing

## Environment Variables

### Frontend (browser key)
Set in `.env` or via Vite:

```
VITE_GOOGLE_MAPS_API_KEY=your-browser-restricted-key
```

### Backend (server key)
Set as environment variable:

```
GOOGLE_MAPS_API_KEY=your-server-restricted-key
```

### Optional
```
GOOGLE_CLOUD_PROJECT=your-project-id
```

## API Key Restrictions

### Browser Key (VITE_GOOGLE_MAPS_API_KEY)
- **Application restrictions**: HTTP referrers — add your domains (e.g., `localhost:5173`, `yourdomain.com`)
- **API restrictions**: Maps JavaScript API only

### Server Key (GOOGLE_MAPS_API_KEY)
- **Application restrictions**: IP addresses or none (for server-side use)
- **API restrictions**: Places API (New) only
- **Never expose in frontend bundles**

## Cost Control

### Free Tier
As of March 2025, Google Maps Platform replaced the flat $200/month credit with SKU-specific free usage caps.

Typical free usage and costs:

| Service | Free Tier (monthly) | Cost Beyond |
|---------|---------------------|-------------|
| Maps JavaScript API | 28,000 loads | $7/1000 loads |
| Places API (Nearby Search) | 5,000 requests | $32/1000 requests |
| Places API (Details) | 5,000 requests | $17/1000 requests |
| Places API (Photos) | 5,000 requests | $7/1000 requests |

**Note**: These are approximate values. Check [Google Maps Platform Pricing](https://developers.google.com/maps/billing-and-pricing/pricing) for current exact rates.

### Field Mask Audit (Cost Impact)

NightKnight Explore uses the following field masks to minimize costs:

**Nearby Search** (`/api/google/search`):
- `places.id,places.displayName,places.types,places.rating,places.userRatingCount,places.formattedAddress,places.websiteUri,places.internationalPhoneNumber,places.priceLevel,places.photos`
- Triggers: Nearby Search (Basic) + optional Advanced/Enterprise SKUs based on fields requested

**Place Details** (`/api/google/details`):
- `id,displayName,rating,userRatingCount,formattedAddress,websiteUri,internationalPhoneNumber,priceLevel,regularOpeningHours,photos,reviews`
- Triggers: Place Details (Basic) + Atmosphere/Contact SKUs for reviews/hours/phone

**Cost Reduction**:
- Photos are only fetched when user opens details (not in search results)
- Reviews are only fetched in details (max 5 per Places API policy)
- Field masks exclude expensive fields like `editorialSummary`, `currentOpeningHours` unless needed

### Best Practices
1. **Field selection**: Request only needed fields to reduce cost
2. **Lazy loading**: Fetch details only when user opens a place
3. **No auto-fetch**: Don't request photos/reviews for all results
4. **Caching**: Cache place details in memory (not persistent storage)
5. **Debouncing**: Avoid repeated requests on map movement
6. **Quota limits**: Set daily quotas in Google Cloud Console
7. **Billing alerts**: Set alerts but note they do NOT guarantee a hard spending cap

### Billing Alerts
- Set billing alerts in Google Cloud Console
- **Note**: Billing alerts do NOT guarantee a hard spending cap
- Use API quotas for hard limits

## Provider Modes

### Overpass (Default)
- OpenStreetMap via Overpass API
- No billing required
- Hidden Gems discovery
- Fully functional without Google

### Google
- Google Maps Platform
- Requires API key and billing
- Ratings, reviews, photos
- Place details with opening hours, phone, website

### Combined
- Google first, Overpass fallback
- **Not recommended** for production due to licensing complexities

## Licensing & Compliance

### Google Maps Platform Terms
- Google map tiles and data are separately licensed
- Do NOT represent Google content as open-source assets
- Do NOT build offline Google Maps databases
- Do NOT cache Google reviews/photos persistently
- Attribute review and photo authors
- Respect EEA-specific terms (Bulgaria billing address)

### AGPL-3.0-or-later
- Application code is AGPL-3.0-or-later
- Google Maps Platform is a separately licensed provider service
- Comply with both licenses simultaneously

## Security

- Never commit API keys to version control
- Use environment variables for all credentials
- Browser keys must have HTTP referrer restrictions
- Server keys must have IP/API restrictions
- The Rust backend validates all Google API requests
- No Google account data is read without explicit user consent

## Troubleshooting

### "Google Maps API key not configured"
- Set `VITE_GOOGLE_MAPS_API_KEY` in your `.env` file
- Restart the dev server

### "Google Maps Platform is not configured"
- Set `GOOGLE_MAPS_API_KEY` environment variable for the backend
- Ensure the key has Places API (New) enabled

### Billing errors (403)
- Verify billing is enabled in Google Cloud
- Check API key restrictions
- Verify the required APIs are enabled

### Quota exceeded (429)
- Check Google Cloud Console quotas
- Increase daily limits if needed
- Implement client-side rate limiting

## Architecture

```
Frontend (React + TypeScript)
├── GoogleMap.tsx — Maps JS API integration
├── App.tsx — Provider mode switching
├── OSM Map (MapLibre) — Existing OSM mode
└── Provider selector — Google/OSM/Combined

Backend (Rust + Axum)
├── /api/health — Service health
├── /api/scan — OSM Overpass discovery
├── /api/geocode — Nominatim geocoding
├── /api/provider — Provider configuration info
├── /api/google/search — Google Places nearby search
└── /api/google/details — Google Place details
```

## References

- [Google Maps Platform Documentation](https://developers.google.com/maps/documentation)
- [Places API (New)](https://developers.google.com/maps/documentation/places/web-service)
- [Maps JavaScript API](https://developers.google.com/maps/documentation/javascript)
- [Google Maps Platform Pricing](https://developers.google.com/maps/billing-and-pricing/pricing)
- [Google Maps Platform Terms](https://cloud.google.com/maps-platform/terms)