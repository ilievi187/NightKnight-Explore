import { useEffect, useRef, useState } from 'react'

interface GoogleMapProps {
  lat: number
  lon: number
  apiKey: string
  onMapClick?: (lat: number, lon: number) => void
}

declare global {
  interface Window {
    google?: {
      maps: {
        Map: new (element: HTMLElement, options: unknown) => unknown
        Marker: new (options: unknown) => unknown
        LatLng: new (lat: number, lng: number) => unknown
        MapMouseEvent: unknown
      }
    }
  }
}

export default function GoogleMap({ lat, lon, apiKey, onMapClick }: GoogleMapProps) {
  const container = useRef<HTMLDivElement>(null)
  const [loaded, setLoaded] = useState(false)
  const [error, setError] = useState('')
  const mapRef = useRef<unknown>(null)

  useEffect(() => {
    if (!apiKey) {
      setError('Google Maps API key not configured. Set VITE_GOOGLE_MAPS_API_KEY environment variable.')
      return
    }

    // Load Google Maps JS API
    const script = document.createElement('script')
    script.src = `https://maps.googleapis.com/maps/api/js?key=${apiKey}&libraries=places`
    script.async = true
    script.defer = true
    script.onload = () => setLoaded(true)
    script.onerror = () => setError('Failed to load Google Maps. Check your API key and network connection.')
    document.head.appendChild(script)

    return () => {
      document.head.removeChild(script)
    }
  }, [apiKey])

  useEffect(() => {
    if (!loaded || !container.current || !window.google?.maps) return

    const map = new window.google.maps.Map(container.current, {
      center: { lat, lng: lon },
      zoom: 12,
    })

    mapRef.current = map

    if (onMapClick) {
      (map as { addListener: (event: string, handler: unknown) => void }).addListener('click', (e: { latLng?: { lat: () => number; lng: () => number } }) => {
        if (e.latLng) {
          onMapClick(e.latLng.lat(), e.latLng.lng())
        }
      })
    }

    return () => {
      mapRef.current = null
    }
  }, [loaded, lat, lon, onMapClick])

  if (error) {
    return <div className="map-error" role="alert">{error}</div>
  }

  if (!loaded) {
    return <div className="map-loading" role="status">Loading Google Maps…</div>
  }

  return <div className="google-map" ref={container} aria-label="Google Maps" />
}