import { useEffect, useMemo, useRef, useState } from 'react'
import { LngLatBounds, Map as MapLibreMap, Marker, NavigationControl } from 'maplibre-gl'
import 'maplibre-gl/dist/maplibre-gl.css'
import { addFavorite, clearFavorites, readFavorites, removeFavorite, type FavoritePlace } from './favorites'
import { gemTier, gemTierLabel, prettyDistance, prettyElapsed, readJsonBody, scanErrorMessage, scanProgressNote } from './scan'

type Category = 'all' | 'food' | 'stay' | 'see' | 'history' | 'nature'
type GemReason = { text: string; field_path: string; observed_value: string }
type GemSignal = { score: number; method: string; reasons: GemReason[] }
type SourceAttribution = { provider_id: string; label: string; text: string; url: string; license: string }
type Place = { id: string; name: string; category: Exclude<Category,'all'>; lat: number; lon: number; distance_m: number; source: string; source_url: string; attribution: SourceAttribution; website: string | null; gem: GemSignal | null }
type ScanResponse = { places: Place[]; scanned_at_unix: number; source: string; cached: boolean }
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
  const [screen,setScreen]=useState<'discover'|'saved'>('discover')
  const [favorites,setFavorites]=useState<FavoritePlace[]>(()=>readFavorites())
  const [lat,setLat]=useState('42.6977')
  const [lon,setLon]=useState('23.3219')
  const [radius,setRadius]=useState(2500)
  const [category,setCategory]=useState<Category>('all')
  const [search,setSearch]=useState('')
  const [sortBy,setSortBy]=useState<'distance'|'gems'>('distance')
  const [data,setData]=useState<ScanResponse|null>(null)
  const [error,setError]=useState('')
  const [busy,setBusy]=useState(false)
  const [startedAt,setStartedAt]=useState<number|null>(null)
  const [elapsed,setElapsed]=useState(0)
  const [scanDuration,setScanDuration]=useState<number|null>(null)
  const [locating,setLocating]=useState(false)
  const [selected,setSelected]=useState<string|null>(null)
  const controller=useRef<AbortController|null>(null)
  const places=useMemo(()=>(data?.places||[]).filter(p=>(category==='all'||p.category===category)&&p.name.toLocaleLowerCase().includes(search.toLocaleLowerCase()))
    .sort((a,b)=>sortBy==='gems'
      ?(b.gem?.score??0)-(a.gem?.score??0)||a.distance_m-b.distance_m
      :a.distance_m-b.distance_m),[data,category,search,sortBy])
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
    setBusy(true);setError('');setSelected(null);setScanDuration(null)
    const start=Date.now();setStartedAt(start);setElapsed(0)
    try{
      const r=await fetch(`/api/scan?lat=${encodeURIComponent(a)}&lon=${encodeURIComponent(b)}&radius_m=${radius}`,{signal:c.signal})
      const payload=await readJsonBody(r)
      if(!r.ok)throw new Error(scanErrorMessage(r,payload))
      if(!payload||typeof payload!=='object'||!Array.isArray((payload as ScanResponse).places)){throw new Error('The API returned an unexpected response shape. Check that the Rust API version matches the web app.')}
      setData(payload as ScanResponse)
      setScanDuration(Date.now()-start)
    }catch(e){if((e as Error).name!=='AbortError'){const networkHint=e instanceof TypeError?'. Check that the Rust API is running.':'';setError(`${(e as Error).message}${networkHint}`)}}
    finally{if(controller.current===c){setBusy(false);setStartedAt(null)}}
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
  useEffect(()=>()=>{controller.current?.abort()},[])
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
      <section className="hero"><div className="hero-bg"/><div className="eyebrow">✦ &nbsp; YOUR WORLD, REVEALED</div><h1>Discover what’s <em>around you.</em></h1><p>One scan. Real restaurants, places to stay, historical landmarks and hidden natural corners — real OpenStreetMap records with transparent, explained gem signals. No invented ratings.</p><div className="hero-metrics"><span><strong>5</strong> DISCOVERY CATEGORIES</span><span><strong>10 KM</strong> MAX RADIUS</span><span><strong>OPEN</strong> SOURCE DATA</span></div></section>
      <section className="workbench">
        <div className="panel controls"><div className="section-heading"><span className="blue-bar"/><div><div className="overline">SCAN PARAMETERS</div><h2>Set your coordinates</h2></div></div>
          <div className="form-grid"><label>LATITUDE<input type="number" value={lat} onChange={e=>setLat(e.target.value)} step="any" min="-90" max="90"/></label><label>LONGITUDE<input type="number" value={lon} onChange={e=>setLon(e.target.value)} step="any" min="-180" max="180"/></label></div>
          <button className="ghost locate" onClick={locate} disabled={locating}>{locating?'Locating…':'◎ Use my location (optional)'}</button>
          <div className="radius-label"><label htmlFor="radius">SCAN RADIUS</label><strong>{prettyDistance(radius)}</strong></div><input id="radius" className="range" type="range" min="500" max="10000" step="500" value={radius} onChange={e=>setRadius(Number(e.target.value))}/>
          <div className="range-hints"><span>500 M</span><span>10 KM</span></div><div className="note">Your coordinates are sent to the configured POI provider when you scan. No background location tracking.</div>
          <button className="primary" onClick={scan} disabled={busy}>{busy?`SCANNING AREA · ${prettyElapsed(elapsed)}…`:'✦  INITIATE SCAN'} <span>→</span></button>
          {busy&&<p role="status" className="scan-progress" aria-live="polite">⌛ {scanProgressNote(elapsed,isCached)} ({prettyElapsed(elapsed)})</p>}
          {error&&<p role="alert" className="error">⚠ {error}</p>}
        </div>
        <div className="panel map-panel"><div className="map-heading"><div><span className="overline">GEOSPATIAL VIEW</span><h2>Area overview</h2></div><span className="map-chip">◉ LIVE MAP</span></div><ExploreMap places={places} lat={Number(lat)} lon={Number(lon)} selected={selected} onSelect={setSelected}/>
        </div>
      </section>
      <section className="results"><div className="results-header"><div><div className="overline">DISCOVERY RESULTS</div><h2>{data?`${places.length} places found`:'Your next discovery starts here'}</h2><p>{data?`Source: ${data.source} • ${data.cached?'cached scan':'fresh scan'}${scanDuration!==null&&!data.cached?` • completed in ${prettyElapsed(scanDuration)}`:''} • ${sortBy==='gems'?'gem signals first':'distance-sorted'} results`:'Choose a location and start a scan to find real nearby places.'}</p></div><div className="results-tools"><button className="ghost sort-toggle" onClick={()=>setSortBy(sortBy==='gems'?'distance':'gems')} disabled={!data||data.places.every(p=>!p.gem)}>{sortBy==='gems'?'Ⓓ Distance first':'✦ Hidden gems first'}</button><input className="search" aria-label="Search scanned places" placeholder="⌕ Search results" value={search} onChange={e=>setSearch(e.target.value)}/></div></div>
      <div className="tabs">{categories.map(c=><button key={c.key} className={c.key===category?'active':''} onClick={()=>{setCategory(c.key);setSelected(null)}}>{c.icon} &nbsp; {c.label}</button>)}</div>
      {selectedPlace&&<div className="selected-note">Selected: <strong>{selectedPlace.name}</strong> — {prettyDistance(selectedPlace.distance_m)} away</div>}
      <div className="places">
        {!data?<div className="empty"><span>✦</span><strong>Ready when you are</strong><p>Scan a region to populate nearby places with source links.</p></div>:
          places.length===0?<div className="empty"><span>⌁</span><strong>No matching places</strong><p>Try another category, query or radius.</p></div>:
          places.map(p=><article className={`place-card ${p.id===selected?'selected':''}`} key={p.id} onClick={()=>setSelected(p.id)}>
            <div className={`place-symbol ${p.category}`}>{symbol(p.category)}</div><div className="place-body">
            <div className="place-type">{categoryLabel(p.category)}</div><h3>{p.name}</h3><p>◎ &nbsp; {prettyDistance(p.distance_m)} away</p>
            {p.gem&&(()=>{const tier=gemTier(p.gem!.score);return <>
            <div className={`gem-badge ${gemClass(tier)}`} title="Transparent OSM tag signals — no ratings or reviews">✦ {gemTierLabel(tier)} · {p.gem!.score}/100</div>
            {p.gem!.reasons.length>0&&<details className="gem-why"><summary>Why this signal?</summary><ul>{p.gem!.reasons.map((reason,i)=><li key={i}><span className="gem-reason-text">{reason.text}</span>{reason.observed_value&&<code className="gem-tag">{reason.field_path} = {reason.observed_value}</code>}{!reason.observed_value&&<code className="gem-tag">{reason.field_path}</code>}</li>)}</ul><p className="gem-note">Derived only from this record's OpenStreetMap tags ({p.gem!.method}). No popularity, ratings or reviews are involved.</p></details>}
            </>})()}
            <div className="place-links"><a href={p.source_url} target="_blank" rel="noopener noreferrer" onClick={e=>e.stopPropagation()}>Open record ↗</a><a href={p.attribution.url} target="_blank" rel="noopener noreferrer" onClick={e=>e.stopPropagation()}>{p.attribution.text} ↗</a>{p.website&&<a href={p.website} target="_blank" rel="noopener noreferrer" onClick={e=>e.stopPropagation()}>Website ↗</a>}<button className="favorite-toggle" aria-pressed={favorites.some(favorite=>favorite.id===p.id)} onClick={e=>{e.stopPropagation();toggleFavorite(p)}}>{favorites.some(favorite=>favorite.id===p.id)?'★ Saved':'☆ Save place'}</button></div></div>
          </article>)}
      </div></section>
      </>}
      {screen==='saved'&&<section className="results saved-results"><div className="results-header"><div><div className="overline">ON THIS DEVICE ONLY</div><h2>Saved place links</h2><p>Only selected OpenStreetMap IDs and source URLs are stored in this browser. Scans and coordinates are not saved.</p></div>{favorites.length>0&&<button className="ghost clear-favorites" onClick={deleteAllFavorites}>Clear all saved places</button>}</div>{error&&<p role="alert" className="error">⚠ {error}</p>}<div className="places">{favorites.length===0?<div className="empty"><span>☆</span><strong>No saved places</strong><p>Save a place from discovery results to keep its OpenStreetMap link here.</p></div>:favorites.map(favorite=><article className="place-card saved-card" key={favorite.id}><div className={`place-symbol ${favorite.category||''}`}>{favorite.category?symbol(favorite.category):'☆'}</div><div className="place-body"><div className="place-type">{favorite.category?categoryLabel(favorite.category):'OpenStreetMap record'}</div><h3 title={favorite.id}>{favorite.name||favorite.id}</h3><div className="place-links"><a href={favorite.source_url} target="_blank" rel="noopener noreferrer">Open record ↗</a><a href="https://www.openstreetmap.org/copyright" target="_blank" rel="noopener noreferrer">© OpenStreetMap contributors ↗</a><button className="favorite-toggle" onClick={()=>deleteFavorite(favorite.id)}>Remove</button></div></div></article>)}</div></section>}
      <footer>NightKnight Explore · Open-source alpha · Places © OpenStreetMap contributors (ODbL) · No reviews or ratings included in this build.</footer>
    </main>
  </div>
}
