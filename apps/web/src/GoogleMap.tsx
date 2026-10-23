import { useEffect, useRef, useState } from 'react'

interface GoogleMapProps {
  lat: number
  lon: number
  apiKey: string
  places?: { id: string; displayName?: { text: string } }[]
  selectedId?: string | null
  onSelect?: (id: string) => void
  onMapClick?: (lat: number, lon: number) => void
}

declare global {
  interface Window {
    google?: {
      maps: {
        Map: new (element: HTMLElement, options: unknown) => {
          addListener: (event: string, handler: (e: { latLng?: { lat: () => number; lng: () => number } }) => void) => void
          setCenter: (center: { lat: number; lng: number }) => void
        }
        Marker: new (options: unknown) => {
          setMap: (map: unknown) => void
          addListener: (event: string, handler: () => void) => void
        }
      }
    }
  }
}

export default function GoogleMap({ lat, lon, apiKey, places = [], selectedId, onSelect, onMapClick }: GoogleMapProps) {
  const container = useRef<HTMLDivElement>(null)
  const [loaded, setLoaded] = useState(false)
  const [error, setError] = useState('')
  const mapRef = useRef<unknown>(null)
  const markersRef = useRef<unknown[]>([])

  useEffect(() => {
    if (!apiKey) {
      setError('Google Maps API key not configured. Set VITE_GOOGLE_MAPS_API_KEY environment variable.')
      return
    }

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
      map.addListener('click', (e: { latLng?: { lat: () => number; lng: () => number } }) => {
        if (e.latLng) {
          onMapClick(e.latLng.lat(), e.latLng.lng())
        }
      })
    }

    return () => {
      mapRef.current = null
    }
  }, [loaded, lat, lon, onMapClick])

  // Add markers for Google places
  useEffect(() => {
    if (!loaded || !mapRef.current || !window.google?.maps) return

    const googleMaps = window.google.maps

    // Clear existing markers
    markersRef.current.forEach((m: unknown) => {
      (m as { setMap: (map: unknown) => void }).setMap(null)
    })
    markersRef.current = []

    places.forEach((place) => {
      const marker = new googleMaps.Marker({
        position: { lat, lng: lon }, // Note: Google search doesn't return individual coords in New API
        map: mapRef.current,
        title: place.displayName?.text || 'Unknown',
      })
      if (onSelect) {
        marker.addListener('click', () => onSelect(place.id))
      }
      markersRef.current.push(marker)
    })
  }, [loaded, places, lat, lon, onSelect])

  if (error) {
    return <div className="map-error" role="alert">{error}</div>
  }

  if (!loaded) {
    return <div className="map-loading" role="status">Loading Google Maps…</div>
  }

  return <div className="google-map" ref={container} aria-label="Google Maps" />
}