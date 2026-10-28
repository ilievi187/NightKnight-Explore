import { useCallback, useEffect, useMemo, useRef, useState, type FormEvent } from 'react'
import type { Map as MapLibreMap, Marker } from 'maplibre-gl'
import 'maplibre-gl/dist/maplibre-gl.css'
import GoogleMap from './GoogleMap'
import { addFavorite, clearFavorites, readFavorites, removeFavorite, type FavoritePlace } from './favorites'
import { appendScanPage, gemTier, gemTierLabel, normalizePlaceQuery, prettyDistance, prettyElapsed, readJsonBody, scanErrorMessage, scanProgressNote } from './scan'

type Category = 'all' | 'food' | 'stay' | 'see' | 'history' | 'nature'
type GemReason = { text: string; field_path: string; observed_value: string }
type GemSignal = { score: number; method: string; reasons: GemReason[] }
type SourceAttribution = { provider_id: string; label: string; text: string; url: string; license: string }
type Place = { id: string; name: string; category: Exclude<Category,'all'>; lat: number; lon: number; distance_m: number; source: string; source_url: string; attribution: SourceAttribution; website: string | null; gem: GemSignal | null }
type ScanResponse = { places: Place[]; scanned_at_unix: number; source: string; cached: boolean; total_matches: number }
type GeocodePlace = { id: string; display_name: string; lat: number; lon: number; source_url: string }
type GeocodeResponse = { results: GeocodePlace[]; source: string; attribution: SourceAttribution; cached: boolean }
type GooglePlace = { id: string; displayName?: { text: string; languageCode?: string }; types?: string[]; rating?: number; userRatingCount?: number; formattedAddress?: string; websiteUri?: string; internationalPhoneNumber?: string; priceLevel?: string; photos?: { name: string; widthPx: number; heightPx: number; authorAttributions?: { displayName: string; uri?: string; photoUri?: string }[] }[] }
type GooglePlaceDetails = { id?: string; displayName?: { text: string; languageCode?: string }; rating?: number; userRatingCount?: number; formattedAddress?: string; websiteUri?: string; internationalPhoneNumber?: string; priceLevel?: string; regularOpeningHours?: { openNow?: boolean; periods?: { open: { day: number; hour: number; minute: number }; close: { day: number; hour: number; minute: number } }[]; weekdayDescriptions?: string[] }; reviews?: { authorAttribution?: { displayName: string; uri?: string; photoUri?: string }; rating?: number; text?: { text: string; languageCode?: string }; relativePublishTimeDescription?: string; publishTime?: string }[]; photos?: { name: string; widthPx: number; heightPx: number; authorAttributions?: { displayName: string; uri?: string; photoUri?: string }[] }[] }
const categories: { key: Category; label: string; icon: string }[] = [
  {key:'all',label:'Everything',icon:'✦'}, {key:'see',label:'Attractions',icon:'⌁'},
  {key:'history',label:'Historical',icon:'⌂'}, {key:'nature',label:'Nature',icon:'❧'},
  {key:'food',label:'Food & coffee',icon:'◈'}, {key:'stay',label:'Hotels & stays',icon:'▣'}
]
const validLat = (v:number) => Number.isFinite(v) && v>=-90 && v<=90
const validLon = (v:number) => Number.isFinite(v) && v>=-180 && v<=180
const categoryLabel = (c: Place['category']) => ({food:'Food & coffee',stay:'Place to stay',see:'Attraction',history:'Historical landmark',nature:'Nature spot'})[c]
const symbol = (c: Place['category']) => ({food:'◈',stay:'▣',see:'⌁',history:'⌂',nature:'❧'})[c]
const gemClass = (tier: string) => tier === 'rare' ? 'gem-high' : tier === 'strong' ? 'gem-mid' : 'gem-low'

function ExploreMap({places, lat, lon, selected, onSelect}:{places: Place[],lat:number,lon:number,selected:string|null,onSelect:(id:string,trigger?:HTMLElement)=>void}) {
  const container=useRef<HTMLDivElement>(null)
  const mapRef=useRef<MapLibreMap|null>(null)
  const maplibreRef=useRef<typeof import('maplibre-gl')|null>(null)
  const markers=useRef<Marker[]>([])
  const [mapReady,setMapReady]=useState(false)
  const [mapError,setMapError]=useState(false)
  useEffect(()=>{
    let cancelled=false
    // MapLibre GL is ~1.3 MB; load it as a separate async chunk so the first
    // paint of the app does not wait on the map engine.
    void import('maplibre-gl').then(maplibre=>{
      if(cancelled||!container.current)return
      maplibreRef.current=maplibre
      const map=new maplibre.Map({
        container:container.current,
        style:{version:8,sources:{osm:{type:'raster',tiles:['https://tile.openstreetmap.org/{z}/{x}/{y}.png'],tileSize:256,attribution:'© <a href="https://www.openstreetmap.org/copyright" target="_blank" rel="noopener noreferrer">OpenStreetMap contributors</a>'}},layers:[{id:'osm',type:'raster',source:'osm'}]},
        center:[lon,lat],zoom:12
      })
      map.addControl(new maplibre.NavigationControl({showCompass:false}),'top-right')
      mapRef.current=map
      setMapReady(true)
    }).catch(()=>{
      if(!cancelled)setMapError(true)
    })
    return ()=>{cancelled=true;markers.current.forEach(m=>m.remove());markers.current=[];mapRef.current?.remove();mapRef.current=null;maplibreRef.current=null}
    // eslint-disable-next-line react-hooks/exhaustive-deps
  },[])
  useEffect(()=>{
    const map=mapRef.current
    const maplibre=maplibreRef.current
    if(!map||!maplibre||!validLat(lat)||!validLon(lon))return
    markers.current.forEach(m=>m.remove());markers.current=[]
    const origin=document.createElement('div');origin.className='map-origin';origin.title='Scan origin'
    markers.current.push(new maplibre.Marker({element:origin}).setLngLat([lon,lat]).addTo(map))
    // Simple clustering: group places within ~50px at current zoom
    const zoom=map.getZoom()
    const clusterThreshold=zoom<11?80:zoom<13?40:0
    if(clusterThreshold>0&&places.length>5){
      // Group into clusters
      const clusters: {center:[number,number],places:Place[]}[]=[]
      const used=new Set<number>()
      for(let i=0;i<places.length;i++){
        if(used.has(i))continue
        const cluster={center:[places[i].lon,places[i].lat] as [number,number],places:[places[i]]}
        used.add(i)
        for(let j=i+1;j<places.length;j++){
          if(used.has(j))continue
          const dx=(places[j].lon-places[i].lon)*111000*Math.cos(lat*Math.PI/180)
          const dy=(places[j].lat-places[i].lat)*111000
          if(Math.sqrt(dx*dx+dy*dy)<clusterThreshold*1000/zoom){
            cluster.places.push(places[j]);used.add(j)
            cluster.center[0]=(cluster.center[0]+places[j].lon)/2
            cluster.center[1]=(cluster.center[1]+places[j].lat)/2
          }
        }
        clusters.push(cluster)
      }
      for(const cluster of clusters){
        if(cluster.places.length===1){
          const place=cluster.places[0]
          const marker=document.createElement('button');marker.type='button';marker.className='place-marker';marker.dataset.placeId=place.id;marker.setAttribute('aria-pressed','false')
          marker.textContent=symbol(place.category);marker.title=place.name;marker.setAttribute('aria-label',`Show ${place.name}`)
          marker.addEventListener('click',()=>onSelect(place.id,marker))
          markers.current.push(new maplibre.Marker({element:marker,anchor:'bottom'}).setLngLat([place.lon,place.lat]).addTo(map))
        } else {
          const marker=document.createElement('button');marker.type='button';marker.className='place-cluster';marker.textContent=String(cluster.places.length);marker.title=`${cluster.places.length} places`
          marker.addEventListener('click',()=>{map.easeTo({center:cluster.center,zoom:zoom+2,duration:300})})
          markers.current.push(new maplibre.Marker({element:marker}).setLngLat(cluster.center).addTo(map))
        }
      }
    } else {
      for(const place of places){
        const marker=document.createElement('button');marker.type='button';marker.className='place-marker';marker.dataset.placeId=place.id;marker.setAttribute('aria-pressed','false')
        marker.textContent=symbol(place.category);marker.title=place.name;marker.setAttribute('aria-label',`Show ${place.name}`)
        marker.addEventListener('click',()=>onSelect(place.id,marker))
        markers.current.push(new maplibre.Marker({element:marker,anchor:'bottom'}).setLngLat([place.lon,place.lat]).addTo(map))
      }
    }
    if(places.length){
      const bounds=new maplibre.LngLatBounds([lon,lat],[lon,lat]);places.forEach(p=>bounds.extend([p.lon,p.lat]));
      map.fitBounds(bounds,{padding:60,maxZoom:14,duration:500})
    }else {map.easeTo({center:[lon,lat],zoom:12,duration:300})}
  },[places,lat,lon,onSelect,mapReady])
  useEffect(()=>{
    markers.current.forEach(marker=>{
      const element=marker.getElement()
      if(!element.classList.contains('place-marker'))return
      const isSelected=element.dataset.placeId===selected
      element.classList.toggle('selected',isSelected)
      element.setAttribute('aria-pressed',String(isSelected))
    })
  },[selected,places,mapReady])
  return <div className="map" ref={container} aria-label="Interactive map of scanned places">{!mapReady&&<div className="map-loading" role={mapError?'alert':'status'}>{mapError?'Map unavailable — reload to try again':'◌ Loading map engine…'}</div>}</div>
}

export default function App(){
  const [screen,setScreen]=useState<'discover'|'saved'>('discover')
  const [providerMode,setProviderMode]=useState<'overpass'|'google'>('overpass')
  const [googleApiKey,setGoogleApiKey]=useState('')
  const [favorites,setFavorites]=useState<FavoritePlace[]>(()=>readFavorites())
  const [lat,setLat]=useState('42.6977')
  const [lon,setLon]=useState('23.3219')
  const [placeSearch,setPlaceSearch]=useState('')
  const [geocodeResults,setGeocodeResults]=useState<GeocodePlace[]>([])
  const [geocodeBusy,setGeocodeBusy]=useState(false)
  const [geocodeError,setGeocodeError]=useState('')
  const [radius,setRadius]=useState(2500)
  const [category,setCategory]=useState<Category>('all')
  const [search,setSearch]=useState('')
  const [sortBy,setSortBy]=useState<'distance'|'gems'>('distance')
  const [data,setData]=useState<ScanResponse|null>(null)
  const [googleFavorites,setGoogleFavorites]=useState<string[]>(()=>{try{return JSON.parse(localStorage.getItem('nightknight-explore:google-favorites')||'[]')}catch{return []}})
  const [googlePlaces,setGooglePlaces]=useState<GooglePlace[]>([])
  const [googleDetails,setGoogleDetails]=useState<GooglePlaceDetails|null>(null)
  const [googleDetailsLoading,setGoogleDetailsLoading]=useState(false)
  const [error,setError]=useState('')
  const [busy,setBusy]=useState(false)
  const [startedAt,setStartedAt]=useState<number|null>(null)
  const [elapsed,setElapsed]=useState(0)
  const [scanDuration,setScanDuration]=useState<number|null>(null)
  const [locating,setLocating]=useState(false)
  const [selected,setSelected]=useState<string|null>(null)
  const [offset,setOffset]=useState(0)
  const loadMore=async()=>{
    const a=Number(lat), b=Number(lon)
    if(!validLat(a)||!validLon(b))return
    setBusy(true)
    try{
      const r=await fetch(`/api/scan?lat=${encodeURIComponent(a)}&lon=${encodeURIComponent(b)}&radius_m=${radius}&offset=${offset+120}`,{signal:controller.current?.signal})
      const payload=await readJsonBody(r)
      if(!r.ok)throw new Error(scanErrorMessage(r,payload))
      if(!payload||typeof payload!=='object'||!Array.isArray((payload as ScanResponse).places)){throw new Error('Unexpected response')}
      const newData=payload as ScanResponse
      setData(prev=>appendScanPage(prev,newData))
      setOffset(prev=>prev+120)
    }catch(e){if((e as Error).name!=='AbortError')setError((e as Error).message)}
    finally{setBusy(false)}
  }
  const selectionTrigger=useRef<HTMLElement|null>(null)
  const controller=useRef<AbortController|null>(null)
  const fetchGoogleDetails=async(placeId:string)=>{
    setGoogleDetailsLoading(true)
    try{
      const r=await fetch('/api/google/details',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({place_id:placeId})})
      const payload=await readJsonBody(r)
      if(!r.ok)throw new Error(scanErrorMessage(r,payload))
      setGoogleDetails(payload as GooglePlaceDetails)
    }catch(e){setError((e as Error).message)}
    finally{setGoogleDetailsLoading(false)}
  }
  const selectPlace=useCallback((id:string,trigger?:HTMLElement)=>{
    if(trigger)selectionTrigger.current=trigger
    setSelected(id)
  },[])
  const places=useMemo(()=>(data?.places||[]).filter(p=>(category==='all'||p.category===category)&&p.name.toLocaleLowerCase().includes(search.toLocaleLowerCase()))
    .sort((a,b)=>sortBy==='gems'
      ?(b.gem?.score??0)-(a.gem?.score??0)||a.distance_m-b.distance_m
      :a.distance_m-b.distance_m||(b.gem?.score??0)-(a.gem?.score??0)),[data,category,search,sortBy])
  const selectedPlace=places.find(p=>p.id===selected)
  const toggleFavorite=(place:Place)=>{
    try{
      const next=favorites.some(favorite=>favorite.id===place.id)
        ? removeFavorite(place.id)
        : addFavorite({id:place.id,source_url:place.source_url,name:place.name,category:place.category})
      setFavorites(next);setError('')
    }catch{setError('Could not update local favorites. Check your browser storage settings.')}
  }
  const deleteFavorite=(id:string)=>{
    try{setFavorites(removeFavorite(id))}catch{setError('Could not remove this local favorite.')}
  }
  const deleteAllFavorites=()=>{
    if(!window.confirm('Remove all saved place links from this browser?'))return
    try{clearFavorites();setFavorites([])}catch{setError('Could not clear local favorites.')}
  }
  const scan=async()=>{
    const a=Number(lat), b=Number(lon)
    if(!validLat(a)||!validLon(b)||!lat.trim()||!lon.trim()){setError('Enter valid latitude (−90 to 90) and longitude (−180 to 180).');return}
    controller.current?.abort()
    const c=new AbortController();controller.current=c
    setBusy(true);setError('');setSelected(null);setScanDuration(null);setOffset(0)
    const start=Date.now();setStartedAt(start);setElapsed(0)
    try{
      if(providerMode==='google'&&googleApiKey){
        // Google Places search
        const r=await fetch('/api/google/search',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({lat:a,lon:b,radius_m:radius,max_results:20}),signal:c.signal})
        const payload=await readJsonBody(r)
        if(!r.ok)throw new Error(scanErrorMessage(r,payload))
        if(!Array.isArray(payload)&&!(payload as any)?.places){throw new Error('Google Places returned an unexpected response. Check that the API version matches the web app.')}
        const places = (payload as any)?.places || payload
        setGooglePlaces(places as GooglePlace[])
        setData(null) // Clear OSM data
        setScanDuration(Date.now()-start)
      } else {
        // OSM Overpass scan
        const r=await fetch(`/api/scan?lat=${encodeURIComponent(a)}&lon=${encodeURIComponent(b)}&radius_m=${radius}`,{signal:c.signal})
        const payload=await readJsonBody(r)
        if(!r.ok)throw new Error(scanErrorMessage(r,payload))
        if(!payload||typeof payload!=='object'||!Array.isArray((payload as ScanResponse).places)){throw new Error('The API returned an unexpected response shape. Check that the Rust API version matches the web app.')}
        setData(payload as ScanResponse)
        setGooglePlaces([]) // Clear Google data
        setScanDuration(Date.now()-start)
      }
    }catch(e){if((e as Error).name!=='AbortError'){const networkHint=e instanceof TypeError?'. Check that the Rust API is running.':'';setError(`${(e as Error).message}${networkHint}`)}}
    finally{if(controller.current===c){setBusy(false);setStartedAt(null)}}
  }
  const searchLocation=async(event:FormEvent<HTMLFormElement>)=>{
    event.preventDefault()
    const query=normalizePlaceQuery(placeSearch)
    if(!query){setGeocodeError('Enter 2–200 characters to search.');return}
    setGeocodeBusy(true);setGeocodeError('');setGeocodeResults([])
    try{
      const response=await fetch('/api/geocode',{method:'POST',headers:{'Content-Type':'application/x-www-form-urlencoded'},body:new URLSearchParams({q:query})})
      const payload=await readJsonBody(response)
      if(!response.ok)throw new Error(scanErrorMessage(response,payload))
      if(!payload||typeof payload!=='object'||!Array.isArray((payload as GeocodeResponse).results))throw new Error('The place search returned an unexpected response. Check that the Rust API version matches the web app.')
      setGeocodeResults((payload as GeocodeResponse).results)
      if((payload as GeocodeResponse).results.length===0)setGeocodeError('No matching places found. Try a more specific name or nearby city.')
    }catch(error){setGeocodeError(error instanceof TypeError?'Place search is unavailable. Check that the Rust API is running.':(error as Error).message)}
    finally{setGeocodeBusy(false)}
  }
  const chooseLocation=(result:GeocodePlace)=>{
    setLat(result.lat.toFixed(6));setLon(result.lon.toFixed(6));setPlaceSearch(result.display_name);setGeocodeResults([]);setGeocodeError('');setSelected(null)
  }
  useEffect(()=>{
    if(startedAt===null)return
    const timer=setInterval(()=>setElapsed(Date.now()-startedAt),1000)
    return ()=>clearInterval(timer)
  },[startedAt])
  const isCached=data?.cached??false
  const locate=()=>{
    if(!navigator.geolocation){setError('Browser geolocation is not available.');return}
    setLocating(true);setError('')
    navigator.geolocation.getCurrentPosition(p=>{
      setLat(p.coords.latitude.toFixed(6));setLon(p.coords.longitude.toFixed(6));setLocating(false)
    },err=>{setError(`Location not available: ${err.message}`);setLocating(false)},
    {enableHighAccuracy:false,timeout:10000,maximumAge:120000})
  }
  useEffect(()=>{setData(null);setGooglePlaces([]);setSelected(null);setError('')},[providerMode])
  useEffect(()=>{
    // Check provider configuration from backend
    fetch('/api/provider').then(r=>r.json()).then(data=>{
      // Google API key for browser is set via Vite env var VITE_GOOGLE_MAPS_API_KEY
      // Backend API key is separate (server-side, restricted)
      if(data?.google_api_configured){
        setGoogleApiKey(import.meta.env.VITE_GOOGLE_MAPS_API_KEY || '')
      }
    }).catch(()=>{})
  },[])
  return <div className="shell">
    <aside className="sidebar">
      <div className="brand"><div className="brand-mark">N<span>✦</span></div><div><strong>NightKnight</strong><small>EXPLORE / 01</small></div></div>
      <div className="side-caption">DISCOVERY SYSTEM</div>
      <div className="side-active">◉ &nbsp; Area scanner <span>01</span></div>
      <div className="side-muted">◎ &nbsp; Saved places <span>LOCAL</span></div>
      <div className="side-muted">◈ &nbsp; Review insights <span>SOON</span></div>
      <div className="side-muted">⌁ &nbsp; Trip planner <span>SOON</span></div>
      <div className="sidebar-foot"><span className="live-dot"/> LOCAL-FIRST DISCOVERY<br/><small>Open data • Transparent sources</small></div>
    </aside>
    <main className="main">
      <header className="topbar"><nav className="view-switch" aria-label="Explore views"><button className={screen==='discover'?'view-active':''} aria-current={screen==='discover'?'page':undefined} onClick={()=>setScreen('discover')}>Discover</button><button className={screen==='saved'?'view-active':''} aria-current={screen==='saved'?'page':undefined} onClick={()=>setScreen('saved')}>Saved places ({favorites.length})</button></nav><div className="status-pill"><span className="live-dot"/> ALPHA 0.1 · FOSS</div></header>
      {screen==='discover'&&<>
      <section className="hero"><div className="hero-bg"/><div className="eyebrow">✦ &nbsp; YOUR WORLD, REVEALED</div><h1>Discover what’s <em>around you.</em></h1><p>One scan. Real restaurants, places to stay, historical landmarks and hidden natural corners — real OpenStreetMap records with transparent, explained gem signals. No invented ratings.</p><div className="hero-metrics"><span><strong>5</strong> DISCOVERY CATEGORIES</span><span><strong>10 KM</strong> MAX RADIUS</span><span><strong>OPEN</strong> SOURCE DATA</span></div><div className="provider-toggle" role="group" aria-label="Select data provider"><button className={providerMode==='overpass'?'active':''} onClick={()=>setProviderMode('overpass')} aria-pressed={providerMode==='overpass'}>🗺 OpenStreetMap</button><button className={providerMode==='google'?'active':''} onClick={()=>setProviderMode('google')} aria-pressed={providerMode==='google'} disabled={!googleApiKey}>🌐 Google Maps{!googleApiKey&&' (not configured)'}</button></div></section>
      <section className="workbench">
        <div className="panel controls"><div className="section-heading"><span className="blue-bar"/><div><div className="overline">SCAN PARAMETERS</div><h2>Set your coordinates</h2></div></div>
          <form className="place-search" onSubmit={searchLocation}>
            <label htmlFor="place-search">SEARCH A PLACE</label>
            <div className="place-search-row"><input id="place-search" type="search" maxLength={200} value={placeSearch} onChange={event=>setPlaceSearch(event.target.value)} placeholder="City, landmark or address" autoComplete="off"/><button className="ghost" type="submit" disabled={geocodeBusy}>{geocodeBusy?'Searching…':'Search'}</button></div>
            <p className="geocode-attribution">Submitted search is sent to the configured provider; query/results cache in API memory up to 24 hours. Do not enter private data. Public Nominatim allows max 1 request/sec app-wide; no autocomplete. <a href="https://operations.osmfoundation.org/policies/nominatim/" target="_blank" rel="noopener noreferrer">Usage policy</a> · © <a href="https://www.openstreetmap.org/copyright" target="_blank" rel="noopener noreferrer">OpenStreetMap contributors</a></p>
            {geocodeError&&<p className="geocode-message" role="status">{geocodeError}</p>}
            {geocodeResults.length>0&&<ul className="geocode-results" aria-label="Place search results">{geocodeResults.map(result=><li key={result.id}><button type="button" className="geocode-choice" onClick={()=>chooseLocation(result)}><span>{result.display_name}</span><small>Use as scan center</small></button><a className="geocode-record" href={result.source_url} target="_blank" rel="noopener noreferrer" aria-label={`Open ${result.display_name} in OpenStreetMap`} onClick={event=>event.stopPropagation()}>↗</a></li>)}</ul>}
          </form>
          <div className="form-grid"><label>LATITUDE<input type="number" value={lat} onChange={e=>setLat(e.target.value)} step="any" min="-90" max="90"/></label><label>LONGITUDE<input type="number" value={lon} onChange={e=>setLon(e.target.value)} step="any" min="-180" max="180"/></label></div>
          <button className="ghost locate" onClick={locate} disabled={locating}>{locating?'Locating…':'◎ Use my location (optional)'}</button>
          <div className="radius-label"><label htmlFor="radius">SCAN RADIUS</label><strong>{prettyDistance(radius)}</strong></div><input id="radius" className="range" type="range" min="500" max="10000" step="500" value={radius} onChange={e=>setRadius(Number(e.target.value))}/>
          <div className="range-hints"><span>500 M</span><span>10 KM</span></div><div className="note">Your coordinates are sent to the configured POI provider when you scan. No background location tracking.</div>
          <button className="primary" onClick={scan} disabled={busy}>{busy?`SCANNING AREA · ${prettyElapsed(elapsed)}…`:'✦  INITIATE SCAN'} <span>→</span></button>
          {busy&&<p role="status" className="scan-progress" aria-live="polite">⌛ {scanProgressNote(elapsed,isCached)} ({prettyElapsed(elapsed)})</p>}
          {error&&<p role="alert" className="error">⚠ {error}</p>}
        </div>
        <div className="panel map-panel"><div className="map-heading"><div><span className="overline">GEOSPATIAL VIEW</span><h2>Area overview</h2></div><span className="map-chip">◉ LIVE MAP</span></div>{providerMode==='google'&&googleApiKey?<GoogleMap lat={Number(lat)} lon={Number(lon)} apiKey={googleApiKey} places={googlePlaces} selectedId={selected} onSelect={(id)=>{fetchGoogleDetails(id);setSelected(id)}}/>:<ExploreMap places={places} lat={Number(lat)} lon={Number(lon)} selected={selected} onSelect={selectPlace}/>}
        </div>
      </section>
      <section className="results" aria-labelledby="discovery-results-title"><div className="results-header"><div><div className="overline">DISCOVERY RESULTS</div><h2 id="discovery-results-title">{data?`${places.length} of ${data.total_matches} places found`:'Your next discovery starts here'}</h2><p>{data?`Source: ${data.source} • ${data.cached?'cached scan':'fresh scan'}${scanDuration!==null&&!data.cached?` • completed in ${prettyElapsed(scanDuration)}`:''} • ${sortBy==='gems'?'gem signals first':'distance-sorted'} results`:'Choose a location and start a scan to find real nearby places.'}</p></div><div className="results-tools"><button className="ghost sort-toggle" aria-pressed={sortBy==='gems'} onClick={()=>setSortBy(sortBy==='gems'?'distance':'gems')} disabled={!data||data.places.every(p=>!p.gem)}>{sortBy==='gems'?'Ⓓ Distance first':'✦ Hidden gems first'}</button><input className="search" aria-label="Search scanned places" placeholder="⌕ Search results" value={search} onChange={e=>setSearch(e.target.value)}/></div></div>
      <div className="tabs" role="group" aria-label="Filter results by category">{categories.map(c=><button key={c.key} type="button" aria-pressed={c.key===category} className={c.key===category?'active':''} onClick={()=>{setCategory(c.key);setSelected(null)}}>{c.icon} &nbsp; {c.label}</button>)}</div>
      <p className="sr-only" role="status" aria-live="polite">{selectedPlace?`Selected ${selectedPlace.name}, ${prettyDistance(selectedPlace.distance_m)} away. Place details are shown above the results.`:''}</p>
      {googleDetails&&<section className="selected-detail" aria-labelledby="google-place-title">
        <div className="selected-detail-body">
          <div className="selected-detail-top"><div><div className="place-type">Google Place</div><h3 id="google-place-title">{googleDetails.displayName?.text||'Unknown'}</h3></div><button className="detail-close" aria-label="Close place details" onClick={()=>setGoogleDetails(null)}>×</button></div>
          {googleDetails.rating&&<p>★ {googleDetails.rating} ({googleDetails.userRatingCount||0} reviews)</p>}
          {googleDetails.formattedAddress&&<p>{googleDetails.formattedAddress}</p>}
          {googleDetails.regularOpeningHours?.openNow!==undefined&&<p>{googleDetails.regularOpeningHours.openNow?'Open now':'Closed'}</p>}
          {googleDetails.websiteUri&&<a href={googleDetails.websiteUri} target="_blank" rel="noopener noreferrer">Website ↗</a>}
          {googleDetails.internationalPhoneNumber&&<p>{googleDetails.internationalPhoneNumber}</p>}
          {googleDetails.priceLevel&&<p>Price level: {googleDetails.priceLevel}</p>}
          {googleDetails.reviews&&googleDetails.reviews.length>0&&<div className="google-reviews"><h4>Reviews (up to 5 from Google)</h4>{googleDetails.reviews.map((r,i)=><div key={i} className="review"><strong>{r.authorAttribution?.displayName||'Anonymous'}</strong> ★{r.rating}<p>{r.text?.text||''}</p><small>{r.relativePublishTimeDescription||''}</small></div>)}</div>}
          {googleDetails.photos&&googleDetails.photos.length>0&&<div className="google-photos"><h4>Photos</h4>{googleDetails.photos.map((p,i)=><div key={i} className="google-photo"><img src={`/api/google/photo?name=${encodeURIComponent(p.name)}&maxWidthPx=400`} alt={`Photo ${i+1} by ${p.authorAttributions?.[0]?.displayName||'Unknown'}`} loading="lazy"/><small>Photo by {p.authorAttributions?.[0]?.displayName||'Unknown'}</small></div>)}</div>}
          <div className="place-links detail-links"><a href={`https://www.google.com/maps/place/?q=place_id:${googleDetails.id}`} target="_blank" rel="noopener noreferrer">Open in Google Maps ↗</a><a href={`https://www.google.com/maps/dir/?api=1&destination=${encodeURIComponent(googleDetails.formattedAddress||googleDetails.displayName?.text||'')}`} target="_blank" rel="noopener noreferrer">Directions ↗</a></div>
        </div>
      </section>}
      {selectedPlace&&<section className="selected-detail" aria-labelledby="selected-place-title">
        <div className={`place-symbol ${selectedPlace.category}`}>{symbol(selectedPlace.category)}</div>
        <div className="selected-detail-body">
          <div className="selected-detail-top"><div><div className="place-type">{categoryLabel(selectedPlace.category)} · {prettyDistance(selectedPlace.distance_m)} away</div><h3 id="selected-place-title">{selectedPlace.name}</h3></div><button className="detail-close" aria-label="Close place details" onClick={()=>{setSelected(null);selectionTrigger.current?.focus();selectionTrigger.current=null}}>×</button></div>
          <div className="place-links detail-links"><a href={selectedPlace.source_url} target="_blank" rel="noopener noreferrer">Open OpenStreetMap record ↗</a><a href={selectedPlace.attribution.url} target="_blank" rel="noopener noreferrer">{selectedPlace.attribution.text} · {selectedPlace.attribution.license} ↗</a>{selectedPlace.website&&<a href={selectedPlace.website} target="_blank" rel="noopener noreferrer">Official website ↗</a>}<button className="favorite-toggle" aria-pressed={favorites.some(favorite=>favorite.id===selectedPlace.id)} onClick={()=>toggleFavorite(selectedPlace)}>{favorites.some(favorite=>favorite.id===selectedPlace.id)?'★ Saved':'☆ Save place'}</button></div>
          {selectedPlace.gem&&<div className="detail-evidence"><div className={`gem-badge ${gemClass(gemTier(selectedPlace.gem.score))}`}>✦ {gemTierLabel(gemTier(selectedPlace.gem.score))} · {selectedPlace.gem.score}/100</div><h4>Why this signal?</h4><ul>{selectedPlace.gem.reasons.map((reason,index)=><li key={`${reason.field_path}-${index}`}><span>{reason.text}</span><code className="gem-tag">{reason.field_path}{reason.observed_value?` = ${reason.observed_value}`:''}</code></li>)}</ul><p className="gem-note">Evidence is from this OpenStreetMap record ({selectedPlace.gem.method}); it is not a popularity or rating score.</p></div>}
        </div>
      </section>}
      <div className="places">
        {providerMode==='google'&&googlePlaces.length>0?googlePlaces.map(gp=><article className="place-card" key={gp.id}>
          <div className="place-symbol see">📍</div><div className="place-body">
          <div className="place-type">{gp.types?.[0]||'Place'}{gp.rating?` · ★ ${gp.rating} (${gp.userRatingCount||0})`:''}</div>
          <h3><button type="button" className="place-select" onClick={()=>{fetchGoogleDetails(gp.id);setSelected(gp.id)}}>{gp.displayName?.text||'Unknown'}</button></h3>
          {gp.formattedAddress&&<p>◎ &nbsp; {gp.formattedAddress}</p>}
          {gp.priceLevel&&<p>Price level: {gp.priceLevel}</p>}
          <div className="place-links">
            <a href={`https://www.google.com/maps/place/?q=place_id:${gp.id}`} target="_blank" rel="noopener noreferrer" onClick={e=>e.stopPropagation()}>Open in Google Maps ↗</a>
            {gp.websiteUri&&<a href={gp.websiteUri} target="_blank" rel="noopener noreferrer" onClick={e=>e.stopPropagation()}>Website ↗</a>}
            <button className="favorite-toggle" onClick={e=>{e.stopPropagation();const next=googleFavorites.includes(gp.id)?googleFavorites.filter(id=>id!==gp.id):[...googleFavorites,gp.id];setGoogleFavorites(next);localStorage.setItem('nightknight-explore:google-favorites',JSON.stringify(next))}}>{googleFavorites.includes(gp.id)?'★ Saved':'☆ Save place'}</button>
          </div>
          </div>
        </article>):
        !data?<div className="empty"><span>✦</span><strong>Ready when you are</strong><p>Scan a region to populate nearby places with source links.</p></div>:
        places.length===0?<div className="empty"><span>⌁</span><strong>No matching places</strong><p>Try another category, query or radius.</p></div>:
        places.map(p=><article className={`place-card ${p.id===selected?'selected':''}`} key={p.id}>
            <div className={`place-symbol ${p.category}`}>{symbol(p.category)}</div><div className="place-body">
            <div className="place-type">{categoryLabel(p.category)}</div><h3><button type="button" className="place-select" aria-label={`Show ${p.name} on map`} aria-pressed={p.id===selected} onClick={event=>selectPlace(p.id,event.currentTarget)}>{p.name}</button></h3><p>◎ &nbsp; {prettyDistance(p.distance_m)} away</p>
            {p.gem&&(()=>{const tier=gemTier(p.gem!.score);return <>
            <div className={`gem-badge ${gemClass(tier)}`} title="Transparent OSM tag signals — no ratings or reviews">✦ {gemTierLabel(tier)} · {p.gem!.score}/100</div>
            {p.gem!.reasons.length>0&&<details className="gem-why"><summary>Why this signal?</summary><ul>{p.gem!.reasons.map((reason,i)=><li key={i}><span className="gem-reason-text">{reason.text}</span>{reason.observed_value&&<code className="gem-tag">{reason.field_path} = {reason.observed_value}</code>}{!reason.observed_value&&<code className="gem-tag">{reason.field_path}</code>}</li>)}</ul><p className="gem-note">Derived only from this record's OpenStreetMap tags ({p.gem!.method}). No popularity, ratings or reviews are involved.</p></details>}
            </>})()}
            <div className="place-links"><a href={p.source_url} target="_blank" rel="noopener noreferrer" onClick={e=>e.stopPropagation()}>Open record ↗</a><a href={p.attribution.url} target="_blank" rel="noopener noreferrer" onClick={e=>e.stopPropagation()}>{p.attribution.text} ↗</a>{p.website&&<a href={p.website} target="_blank" rel="noopener noreferrer" onClick={e=>e.stopPropagation()}>Website ↗</a>}<button className="favorite-toggle" aria-pressed={favorites.some(favorite=>favorite.id===p.id)} onClick={e=>{e.stopPropagation();toggleFavorite(p)}}>{favorites.some(favorite=>favorite.id===p.id)?'★ Saved':'☆ Save place'}</button></div></div>
          </article>)}
      </div></section>
      {data&&data.places.length<data.total_matches&&<div className="load-more"><button className="ghost" onClick={loadMore} disabled={busy}>{busy?'Loading…':`Load more (${data.total_matches-data.places.length} remaining)`}</button></div>}
      </>}
      {screen==='saved'&&<section className="results saved-results"><div className="results-header"><div><div className="overline">ON THIS DEVICE ONLY</div><h2>Saved place links</h2><p>Only selected OpenStreetMap IDs and source URLs are stored in this browser. Scans and coordinates are not saved.</p></div>{favorites.length>0&&<button className="ghost clear-favorites" onClick={deleteAllFavorites}>Clear all saved places</button>}</div>{error&&<p role="alert" className="error">⚠ {error}</p>}<div className="places">{favorites.length===0?<div className="empty"><span>☆</span><strong>No saved places</strong><p>Save a place from discovery results to keep its OpenStreetMap link here.</p></div>:favorites.map(favorite=><article className="place-card saved-card" key={favorite.id}><div className={`place-symbol ${favorite.category||''}`}>{favorite.category?symbol(favorite.category):'☆'}</div><div className="place-body"><div className="place-type">{favorite.category?categoryLabel(favorite.category):'OpenStreetMap record'}</div><h3 title={favorite.id}>{favorite.name||favorite.id}</h3><div className="place-links"><a href={favorite.source_url} target="_blank" rel="noopener noreferrer">Open record ↗</a><a href="https://www.openstreetmap.org/copyright" target="_blank" rel="noopener noreferrer">© OpenStreetMap contributors ↗</a><button className="favorite-toggle" onClick={()=>deleteFavorite(favorite.id)}>Remove</button></div></div></article>)}</div></section>}
      <footer>NightKnight Explore · Open-source alpha · {providerMode==='google'?'Google Maps Platform (separately licensed) · ':'Places © OpenStreetMap contributors (ODbL) · '}Discover real places with transparent sources.</footer>
    </main>
  </div>
}
