use axum::{
    extract::{Query, State},
    http::{header, HeaderValue, Method, StatusCode},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    env,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

const OVERPASS_CACHE_TTL: Duration = Duration::from_secs(900);

#[derive(Clone)]
struct AppState {
    http: reqwest::Client,
    endpoint: String,
    cache: Arc<Mutex<HashMap<String, CachedScan>>>,
    last_external_request: Arc<Mutex<Option<Instant>>>,
}
struct CachedScan {
    created: Instant,
    response: ScanResponse,
}

#[derive(Deserialize)]
struct ScanQuery {
    lat: f64,
    lon: f64,
    radius_m: Option<u32>,
}
#[derive(Serialize, Clone)]
struct SourceAttribution {
    provider_id: &'static str,
    label: &'static str,
    text: &'static str,
    url: &'static str,
    license: &'static str,
}
#[derive(Serialize, Clone)]
struct Place {
    id: String,
    name: String,
    category: String,
    lat: f64,
    lon: f64,
    distance_m: u32,
    source: &'static str,
    source_url: String,
    attribution: SourceAttribution,
    website: Option<String>,
}
#[derive(Serialize, Clone)]
struct ScanResponse {
    places: Vec<Place>,
    scanned_at_unix: u64,
    source: &'static str,
    cached: bool,
}
#[derive(Serialize)]
struct ApiError {
    error: String,
    code: &'static str,
    provider_id: Option<&'static str>,
    retry_after_seconds: Option<u64>,
}
#[derive(Deserialize)]
struct OverpassResponse {
    elements: Vec<OverpassElement>,
}
#[derive(Deserialize)]
struct OverpassElement {
    #[serde(rename = "type")]
    kind: String,
    id: i64,
    lat: Option<f64>,
    lon: Option<f64>,
    center: Option<Center>,
    tags: Option<HashMap<String, String>>,
}
#[derive(Deserialize)]
struct Center {
    lat: f64,
    lon: f64,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter("info,tower_http=warn")
        .init();
    let endpoint = env::var("OVERPASS_ENDPOINT")
        .unwrap_or_else(|_| "https://overpass.private.coffee/api/interpreter".to_owned());
    if !endpoint.starts_with("https://") && !endpoint.starts_with("http://127.0.0.1:") {
        panic!("OVERPASS_ENDPOINT must use HTTPS or local loopback HTTP");
    }
    let bind = env::var("BIND_ADDRESS").unwrap_or_else(|_| "127.0.0.1:8787".to_owned());
    let address: std::net::SocketAddr = bind.parse().expect("Invalid BIND_ADDRESS");
    if !address.ip().is_loopback() {
        panic!("Prototype must bind to localhost only");
    }
    let state = AppState {
        http: reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("NightKnightExplore/0.1 (FOSS local development)")
            .build()
            .expect("HTTP client error"),
        endpoint,
        cache: Arc::new(Mutex::new(HashMap::new())),
        last_external_request: Arc::new(Mutex::new(None)),
    };
    let cors = CorsLayer::new()
        .allow_origin("http://127.0.0.1:5173".parse::<HeaderValue>().unwrap())
        .allow_methods([Method::GET])
        .allow_headers([header::CONTENT_TYPE]);
    let app = Router::new()
        .route(
            "/api/health",
            get(|| async {
                Json(serde_json::json!({"status":"ok","service":"NightKnight Explore API"}))
            }),
        )
        .route("/api/scan", get(scan))
        .with_state(state)
        .layer(cors)
        .layer(TraceLayer::new_for_http());
    tracing::info!(%address, "local development server starting");
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .expect("Cannot bind API address");
    axum::serve(listener, app).await.expect("API server error");
}

async fn scan(
    State(state): State<AppState>,
    Query(q): Query<ScanQuery>,
) -> Result<Json<ScanResponse>, (StatusCode, Json<ApiError>)> {
    let radius = q.radius_m.unwrap_or(2000);
    if !q.lat.is_finite()
        || !q.lon.is_finite()
        || !(-90.0..=90.0).contains(&q.lat)
        || !(-180.0..=180.0).contains(&q.lon)
        || !(500..=10_000).contains(&radius)
    {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "Invalid coordinates or radius (500–10000m)",
        ));
    }
    let key = scan_cache_key(q.lat, q.lon, radius);
    let mut cache = state.cache.lock().await;
    cache.retain(|_, hit| hit.created.elapsed() < OVERPASS_CACHE_TTL);
    if let Some(hit) = cache.get(&key) {
        let mut data = hit.response.clone();
        data.cached = true;
        return Ok(Json(data));
    }
    drop(cache);
    // Small shared-service safeguard. The lock is held through request initiation
    // to avoid concurrent requests defeating the cooldown.
    let mut guard = state.last_external_request.lock().await;
    if guard.is_some_and(|last| last.elapsed() < Duration::from_secs(8)) {
        return Err(provider_error(
            StatusCode::TOO_MANY_REQUESTS,
            "provider_rate_limited",
            "Scan cooldown: please retry after 8 seconds",
            Some(8),
        ));
    }
    *guard = Some(Instant::now());
    drop(guard);

    let query = format!(
        "[out:json][timeout:20];(nwr(around:{radius},{lat},{lon})[\"amenity\"~\"^(restaurant|cafe|bar|pub)$\"];nwr(around:{radius},{lat},{lon})[\"tourism\"~\"^(hotel|guest_house|hostel|attraction|museum|viewpoint|artwork|gallery)$\"];);out center;",
        lat=q.lat, lon=q.lon
    );
    // POST rather than enormous query strings, no browser-to-Overpass CORS required.
    let result = state
        .http
        .post(&state.endpoint)
        .form(&[("data", query)])
        .send()
        .await
        .map_err(|e| {
            tracing::warn!("Overpass unavailable: {e}");
            provider_error(
                StatusCode::BAD_GATEWAY,
                "provider_unavailable",
                "Place data provider unavailable",
                None,
            )
        })?;
    if !result.status().is_success() {
        tracing::warn!(status=%result.status(), "Overpass returned error");
        return Err(provider_error(
            StatusCode::BAD_GATEWAY,
            "provider_unavailable",
            "Place data provider returned an error",
            None,
        ));
    }
    let payload: OverpassResponse = result.json().await.map_err(|_| {
        provider_error(
            StatusCode::BAD_GATEWAY,
            "provider_invalid_response",
            "Invalid place data provider response",
            None,
        )
    })?;
    let mut places: Vec<_> = payload
        .elements
        .into_iter()
        .filter_map(|element| to_place(element, q.lat, q.lon, radius))
        .collect();
    places.sort_by_key(|p| p.distance_m);
    places.truncate(120);
    let response = ScanResponse {
        places,
        scanned_at_unix: now_unix(),
        source: "OpenStreetMap",
        cached: false,
    };
    state.cache.lock().await.insert(
        key,
        CachedScan {
            created: Instant::now(),
            response: response.clone(),
        },
    );
    Ok(Json(response))
}

fn scan_cache_key(lat: f64, lon: f64, radius_m: u32) -> String {
    let normalized_lat = if lat == 0.0 { 0.0 } else { lat };
    let normalized_lon = if lon == 0.0 { 0.0 } else { lon };
    format!(
        "{:016x}:{:016x}:{radius_m}",
        normalized_lat.to_bits(),
        normalized_lon.to_bits()
    )
}

fn error(status: StatusCode, msg: &str) -> (StatusCode, Json<ApiError>) {
    let code = match status {
        StatusCode::BAD_REQUEST => "invalid_scan_parameters",
        StatusCode::TOO_MANY_REQUESTS => "request_rate_limited",
        _ => "request_failed",
    };
    api_error(status, code, msg, None, None)
}
fn provider_error(
    status: StatusCode,
    code: &'static str,
    msg: &str,
    retry_after_seconds: Option<u64>,
) -> (StatusCode, Json<ApiError>) {
    api_error(
        status,
        code,
        msg,
        Some("openstreetmap-overpass"),
        retry_after_seconds,
    )
}
fn api_error(
    status: StatusCode,
    code: &'static str,
    msg: &str,
    provider_id: Option<&'static str>,
    retry_after_seconds: Option<u64>,
) -> (StatusCode, Json<ApiError>) {
    (
        status,
        Json(ApiError {
            error: msg.to_owned(),
            code,
            provider_id,
            retry_after_seconds,
        }),
    )
}
fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn to_place(el: OverpassElement, center_lat: f64, center_lon: f64, radius: u32) -> Option<Place> {
    let lat = el.lat.or_else(|| el.center.as_ref().map(|c| c.lat))?;
    let lon = el.lon.or_else(|| el.center.as_ref().map(|c| c.lon))?;
    if !lat.is_finite() || !lon.is_finite() {
        return None;
    }
    let tags = el.tags?;
    let name = tags
        .get("name:en")
        .or_else(|| tags.get("name"))?
        .trim()
        .to_string();
    if name.is_empty() {
        return None;
    }
    let category = category(&tags).to_owned();
    if category == "other" {
        return None;
    }
    let dist = haversine(center_lat, center_lon, lat, lon).round() as u32;
    if dist > radius {
        return None;
    }
    if !matches!(el.kind.as_str(), "node" | "way" | "relation") {
        return None;
    }
    let website = tags
        .get("website")
        .or_else(|| tags.get("contact:website"))
        .and_then(|url| safe_website(url));
    Some(Place {
        id: format!("{}-{}", el.kind, el.id),
        name,
        category,
        lat,
        lon,
        distance_m: dist,
        source: "OpenStreetMap",
        source_url: format!("https://www.openstreetmap.org/{}/{}", el.kind, el.id),
        attribution: SourceAttribution {
            provider_id: "openstreetmap",
            label: "OpenStreetMap contributors",
            text: "© OpenStreetMap contributors",
            url: "https://www.openstreetmap.org/copyright",
            license: "ODbL-1.0",
        },
        website,
    })
}
fn category(tags: &HashMap<String, String>) -> &'static str {
    if let Some("restaurant" | "cafe" | "bar" | "pub") = tags.get("amenity").map(String::as_str) {
        return "food";
    }
    match tags.get("tourism").map(String::as_str) {
        Some("hotel" | "guest_house" | "hostel") => "stay",
        Some("attraction" | "museum" | "viewpoint" | "artwork" | "gallery") => "see",
        _ => "other",
    }
}
fn safe_website(url: &str) -> Option<String> {
    let v = url.trim();
    if v.starts_with("https://") || v.starts_with("http://") {
        Some(v.to_owned())
    } else {
        None
    }
}
fn haversine(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let r = 6_371_000.0;
    let p1 = lat1.to_radians();
    let p2 = lat2.to_radians();
    let dp = (lat2 - lat1).to_radians();
    let dl = (lon2 - lon1).to_radians();
    let a = (dp / 2.0).sin().powi(2) + p1.cos() * p2.cos() * (dl / 2.0).sin().powi(2);
    2.0 * r * a.sqrt().asin()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn distance_zero() {
        assert!(haversine(43.0, 27.0, 43.0, 27.0) < 0.01);
    }
    #[test]
    fn distance_realistic() {
        let d = haversine(43.0, 27.0, 43.01, 27.0);
        assert!((1100.0..1120.0).contains(&d));
    }
    #[test]
    fn categories() {
        let mut m = HashMap::new();
        m.insert("tourism".into(), "hotel".into());
        assert_eq!(category(&m), "stay");
    }
    #[test]
    fn invalid_website_scheme() {
        assert!(safe_website("javascript:alert(1)").is_none());
    }
    #[test]
    fn place_without_name_skipped() {
        let e = OverpassElement {
            kind: "node".into(),
            id: 1,
            lat: Some(43.0),
            lon: Some(27.0),
            center: None,
            tags: Some(HashMap::new()),
        };
        assert!(to_place(e, 43.0, 27.0, 3000).is_none());
    }

    #[test]
    fn place_has_provider_neutral_attribution_and_canonical_record_url() {
        let mut tags = HashMap::new();
        tags.insert("name".into(), "Test Cafe".into());
        tags.insert("amenity".into(), "cafe".into());
        let element = OverpassElement {
            kind: "node".into(),
            id: 42,
            lat: Some(43.0),
            lon: Some(27.0),
            center: None,
            tags: Some(tags),
        };

        let place = to_place(element, 43.0, 27.0, 500).unwrap();

        assert_eq!(place.source_url, "https://www.openstreetmap.org/node/42");
        assert_eq!(place.attribution.provider_id, "openstreetmap");
        assert_eq!(place.attribution.text, "© OpenStreetMap contributors");
        assert_eq!(
            place.attribution.url,
            "https://www.openstreetmap.org/copyright"
        );
        assert_eq!(place.attribution.license, "ODbL-1.0");
    }

    #[test]
    fn provider_rate_limit_error_identifies_source_and_retry_window() {
        let (_, Json(payload)) = provider_error(
            StatusCode::TOO_MANY_REQUESTS,
            "provider_rate_limited",
            "Try again shortly",
            Some(8),
        );

        assert_eq!(payload.code, "provider_rate_limited");
        assert_eq!(payload.provider_id, Some("openstreetmap-overpass"));
        assert_eq!(payload.retry_after_seconds, Some(8));
    }

    #[test]
    fn scan_cache_key_distinguishes_nearby_origins() {
        assert_ne!(
            scan_cache_key(43.21401, 27.915, 2500),
            scan_cache_key(43.21402, 27.915, 2500)
        );
    }

    #[test]
    fn scan_cache_key_reuses_identical_origin_and_radius() {
        assert_eq!(
            scan_cache_key(43.21401, 27.915, 2500),
            scan_cache_key(43.21401, 27.915, 2500)
        );
        assert_ne!(
            scan_cache_key(43.21401, 27.915, 2500),
            scan_cache_key(43.21401, 27.915, 3000)
        );
    }
}
