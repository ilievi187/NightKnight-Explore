import { useEffect, useMemo, useRef, useState } from 'react'
import { LngLatBounds, Map as MapLibreMap, Marker, NavigationControl } from 'maplibre-gl'
import 'maplibre-gl/dist/maplibre-gl.css'

type Category = 'all' | 'food' | 'stay' | 'see'
type Place = { id: string; name: string; category: Exclude<Category,'all'>; lat: number; lon: number; distance_m: number; source: string; source_url: string; website: string | null }
type ScanResponse = { places: Place[]; scanned_at_unix: number; source: string; cached: boolean }
const categories: { key: Category; label: string; icon: string }[] = [
  {key:'all',label:'Everything',icon:'✦'}, {key:'food',label:'Food & coffee',icon:'◈'},
  {key:'stay',label:'Hotels & stays',icon:'▣'}, {key:'see',label:'Attractions',icon:'⌁'}
]
const prettyDistance = (m: number) => m < 1000 ? `${m} m` : `${(m / 1000).toFixed(1)} km`
const validLat = (v:number) => Number.isFinite(v) && v>=-90 && v<=90
const validLon = (v:number) => Number.isFinite(v) && v>=-180 && v<=180
const categoryLabel = (c: Place['category']) => ({food:'Food & coffee',stay:'Place to stay',see:'Attraction'})[c]
const symbol = (c: Place['category']) => ({food:'◈',stay:'▣',see:'⌁'})[c]

function ExploreMap({places, lat, lon, selected, onSelect}:{places: Place[],lat:number,lon:number,selected:string|null,onSelect:(id:string)=>void}) {
  const container=useRef<HTMLDivElement>(null)
  const mapRef=useRef<MapLibreMap|null>(null)
  const markers=useRef<Marker[]>([])
  useEffect(()=>{
    if(!container.current) return
    const map=new MapLibreMap({
      container:container.current,
      style:{version:8,sources:{osm:{type:'raster',tiles:['https://tile.openstreetmap.org/{z}/{x}/{y}.png'],tileSize:256,attribution:'© <a href="https://www.openstreetmap.org/copyright" target="_blank" rel="noopener noreferrer">OpenStreetMap contributors</a>'}},layers:[{id:'osm',type:'raster',source:'osm'}]},
      center:[lon,lat],zoom:12
    })
    map.addControl(new NavigationControl({showCompass:false}),'top-right')
    mapRef.current=map
    return ()=>{markers.current.forEach(m=>m.remove());markers.current=[];map.remove();mapRef.current=null}
  },[])
  useEffect(()=>{
    const map=mapRef.current
    if(!map || !validLat(lat) || !validLon(lon)) return
    markers.current.forEach(m=>m.remove());markers.current=[]
    const origin=document.createElement('div');origin.className='map-origin';origin.title='Scan origin'
    markers.current.push(new Marker({element:origin}).setLngLat([lon,lat]).addTo(map))
    for(const place of places){
      const marker=document.createElement('button');marker.type='button';marker.className=`place-marker ${selected===place.id?'selected':''}`
      marker.textContent=symbol(place.category);marker.title=place.name;marker.setAttribute('aria-label',`Show ${place.name}`)
      marker.addEventListener('click',()=>onSelect(place.id))
      markers.current.push(new Marker({element:marker,anchor:'bottom'}).setLngLat([place.lon,place.lat]).addTo(map))
    }
    if(places.length){
      const bounds=new LngLatBounds([lon,lat],[lon,lat]);places.forEach(p=>bounds.extend([p.lon,p.lat]));
      map.fitBounds(bounds,{padding:60,maxZoom:14,duration:500})
    }else {map.easeTo({center:[lon,lat],zoom:12,duration:300})}
  },[places,lat,lon,selected,onSelect])
  return <div className="map" ref={container} aria-label="Interactive map of scanned places" />
}

export default function App(){
  const [lat,setLat]=useState('42.6977')
  const [lon,setLon]=useState('23.3219')
  const [radius,setRadius]=useState(2500)
  const [category,setCategory]=useState<Category>('all')
  const [search,setSearch]=useState('')
  const [data,setData]=useState<ScanResponse|null>(null)
  const [error,setError]=useState('')
  const [busy,setBusy]=useState(false)
  const [locating,setLocating]=useState(false)
  const [selected,setSelected]=useState<string|null>(null)
  const controller=useRef<AbortController|null>(null)
  const places=useMemo(()=>(data?.places||[]).filter(p=>(category==='all'||p.category===category)&&p.name.toLocaleLowerCase().includes(search.toLocaleLowerCase())),[data,category,search])
  const selectedPlace=places.find(p=>p.id===selected)
  const scan=async()=>{
    const a=Number(lat), b=Number(lon)
    if(!validLat(a)||!validLon(b)||!lat.trim()||!lon.trim()){setError('Enter valid latitude (−90 to 90) and longitude (−180 to 180).');return}
    controller.current?.abort()
    const c=new AbortController();controller.current=c
    setBusy(true);setError('');setSelected(null)
    try{
      const r=await fetch(`/api/scan?lat=${encodeURIComponent(a)}&lon=${encodeURIComponent(b)}&radius_m=${radius}`,{signal:c.signal})
      const payload=await r.json()
      if(!r.ok) throw new Error(payload?.error||`Scan failed (${r.status})`)
      setData(payload as ScanResponse)
    }catch(e){if((e as Error).name!=='AbortError'){setError(`${(e as Error).message}. Check that the Rust API is running.`)}}
    finally{if(controller.current===c)setBusy(false)}
  }
  const locate=()=>{
    if(!navigator.geolocation){setError('Browser geolocation is not available.');return}
    setLocating(true);setError('')
    navigator.geolocation.getCurrentPosition(p=>{
      setLat(p.coords.latitude.toFixed(6));setLon(p.coords.longitude.toFixed(6));setLocating(false)
    },err=>{setError(`Location not available: ${err.message}`);setLocating(false)},
    {enableHighAccuracy:false,timeout:10000,maximumAge:120000})
  }
  useEffect(()=>()=>{controller.current?.abort()},[])
  return <div className="shell">
    <aside className="sidebar">
      <div className="brand"><div className="brand-mark">N<span>✦</span></div><div><strong>NightKnight</strong><small>EXPLORE / 01</small></div></div>
      <div className="side-caption">DISCOVERY SYSTEM</div>
      <div className="side-active">◉ &nbsp; Area scanner <span>01</span></div>
      <div className="side-muted">◎ &nbsp; Saved places <span>SOON</span></div>
      <div className="side-muted">◈ &nbsp; Review insights <span>SOON</span></div>
      <div className="side-muted">⌁ &nbsp; Trip planner <span>SOON</span></div>
      <div className="sidebar-foot"><span className="live-dot"/> LOCAL-FIRST DISCOVERY<br/><small>Open data • Transparent sources</small></div>
    </aside>
    <main className="main">
      <header className="topbar"><div className="breadcrumb">NAVIGATION <span>/</span> AREA SCANNER</div><div className="status-pill"><span className="live-dot"/> ALPHA 0.1 · FOSS</div></header>
      <section className="hero"><div className="hero-bg"/><div className="eyebrow">✦ &nbsp; YOUR WORLD, REVEALED</div><h1>Discover what’s <em>around you.</em></h1><p>One scan. Local restaurants, places to stay, and places worth seeing — real OpenStreetMap records, no invented ratings.</p><div className="hero-metrics"><span><strong>3</strong> DISCOVERY CATEGORIES</span><span><strong>10 KM</strong> MAX RADIUS</span><span><strong>OPEN</strong> SOURCE DATA</span></div></section>
      <section className="workbench">
        <div className="panel controls"><div className="section-heading"><span className="blue-bar"/><div><div className="overline">SCAN PARAMETERS</div><h2>Set your coordinates</h2></div></div>
          <div className="form-grid"><label>LATITUDE<input type="number" value={lat} onChange={e=>setLat(e.target.value)} step="any" min="-90" max="90"/></label><label>LONGITUDE<input type="number" value={lon} onChange={e=>setLon(e.target.value)} step="any" min="-180" max="180"/></label></div>
          <button className="ghost locate" onClick={locate} disabled={locating}>{locating?'Locating…':'◎ Use my location (optional)'}</button>
          <div className="radius-label"><label htmlFor="radius">SCAN RADIUS</label><strong>{prettyDistance(radius)}</strong></div><input id="radius" className="range" type="range" min="500" max="10000" step="500" value={radius} onChange={e=>setRadius(Number(e.target.value))}/>
          <div className="range-hints"><span>500 M</span><span>10 KM</span></div><div className="note">Your coordinates are sent to the configured POI provider when you scan. No background location tracking.</div>
          <button className="primary" onClick={scan} disabled={busy}>{busy?'SCANNING AREA…':'✦  INITIATE SCAN'} <span>→</span></button>
          {error&&<p role="alert" className="error">⚠ {error}</p>}
        </div>
        <div className="panel map-panel"><div className="map-heading"><div><span className="overline">GEOSPATIAL VIEW</span><h2>Area overview</h2></div><span className="map-chip">◉ LIVE MAP</span></div><ExploreMap places={places} lat={Number(lat)} lon={Number(lon)} selected={selected} onSelect={setSelected}/>
        </div>
      </section>
      <section className="results"><div className="results-header"><div><div className="overline">DISCOVERY RESULTS</div><h2>{data?`${places.length} places found`:'Your next discovery starts here'}</h2><p>{data?`Source: ${data.source} • ${data.cached?'cached scan':'fresh scan'} • distance-sorted results`:'Choose a location and start a scan to find real nearby places.'}</p></div><input className="search" aria-label="Search scanned places" placeholder="⌕ Search results" value={search} onChange={e=>setSearch(e.target.value)}/></div>
      <div className="tabs">{categories.map(c=><button key={c.key} className={c.key===category?'active':''} onClick={()=>{setCategory(c.key);setSelected(null)}}>{c.icon} &nbsp; {c.label}</button>)}</div>
      {selectedPlace&&<div className="selected-note">Selected: <strong>{selectedPlace.name}</strong> — {prettyDistance(selectedPlace.distance_m)} away</div>}
      <div className="places">
        {!data?<div className="empty"><span>✦</span><strong>Ready when you are</strong><p>Scan a region to populate nearby places with source links.</p></div>:
          places.length===0?<div className="empty"><span>⌁</span><strong>No matching places</strong><p>Try another category, query or radius.</p></div>:
          places.map(p=><article className={`place-card ${p.id===selected?'selected':''}`} key={p.id} onClick={()=>setSelected(p.id)}>
            <div className={`place-symbol ${p.category}`}>{symbol(p.category)}</div><div className="place-body"><div className="place-type">{categoryLabel(p.category)}</div><h3>{p.name}</h3><p>◎ &nbsp; {prettyDistance(p.distance_m)} away</p><div className="place-links"><a href={p.source_url} target="_blank" rel="noopener noreferrer" onClick={e=>e.stopPropagation()}>OpenStreetMap ↗</a>{p.website&&<a href={p.website} target="_blank" rel="noopener noreferrer" onClick={e=>e.stopPropagation()}>Website ↗</a>}</div></div>
          </article>)}
      </div></section>
      <footer>NightKnight Explore · Open-source alpha · Places © OpenStreetMap contributors (ODbL) · No reviews or ratings included in this build.</footer>
    </main>
  </div>
}
