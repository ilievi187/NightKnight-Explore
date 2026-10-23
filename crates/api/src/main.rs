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
/// Local safeguard between this server and shared endpoints.
const COOLDOWN: Duration = Duration::from_secs(8);
/// Overpass query processing timeout; the client must wait at least this long.
const ATTEMPT_TIMEOUT: Duration = Duration::from_secs(35);
/// Connection setup timeout so dead DNS/hosts fail fast instead of hanging.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Small backoff between attempts against the same endpoint.
const SAME_ENDPOINT_RETRY_BACKOFF: Duration = Duration::from_secs(2);
/// 429 Retry-After values longer than this are not waited on inside one scan.
const RATE_LIMIT_WAIT_CAP: Duration = Duration::from_secs(8);
/// After an endpoint fails, avoid it for a while instead of hammering it.
const ENDPOINT_FAILURE_PENALTY: Duration = Duration::from_secs(30);
/// Overall cap on the failover chain so one scan never stalls indefinitely.
const MAX_ATTEMPT_WINDOW: Duration = Duration::from_secs(110);

/// Public Overpass services used in order when no explicit endpoint is
/// configured. Each is an independent shared resource; failover spreads load
/// instead of hammering one dead service.
const DEFAULT_ENDPOINTS: &[&str] = &[
    "https://overpass-api.de/api/interpreter",
    "https://overpass.private.coffee/api/interpreter",
    "https://overpass.kumi.systems/api/interpreter",
];

#[derive(Clone)]
struct AppState {
    http: reqwest::Client,
    endpoints: Vec<String>,
    cache: Arc<Mutex<HashMap<String, CachedScan>>>,
    last_external_request: Arc<Mutex<Option<Instant>>>,
    /// Per-endpoint "not before" time after a failure, so later scans skip
    /// endpoints that are currently known-bad.
    endpoint_backoff: Arc<Mutex<HashMap<String, Instant>>>,
    /// Retry-After hint most recently seen from an upstream 429/503.
    last_retry_after: Arc<Mutex<Option<u64>>>,
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

/// What a single Overpass attempt can fail with.
#[derive(Clone, Copy, Debug, PartialEq)]
enum OverpassFailure {
    /// Upstream says "too many requests"; may carry a bounded Retry-After.
    RateLimited,
    /// The attempt exceeded its timeout.
    TimedOut,
    /// Connection failure, refused, or an unexpected upstream status.
    Unavailable,
    /// Upstream answered but with unusable content.
    InvalidResponse,
}

impl OverpassFailure {
    fn status(self) -> StatusCode {
        match self {
            Self::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            _ => StatusCode::BAD_GATEWAY,
        }
    }
    fn code(self) -> &'static str {
        match self {
            Self::RateLimited => "provider_rate_limited",
            Self::TimedOut => "provider_timeout",
            Self::InvalidResponse => "provider_invalid_response",
            Self::Unavailable => "provider_unavailable",
        }
    }
    fn message(self) -> &'static str {
        match self {
            Self::RateLimited => "Place data provider is rate limiting; try again shortly",
            Self::TimedOut => "Place data provider timed out; try a smaller radius or retry later",
            Self::InvalidResponse => "Invalid place data provider response",
            Self::Unavailable => "Place data provider unavailable",
        }
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter("info,tower_http=warn")
        .init();
    let endpoints = configured_endpoints();
    let bind = env::var("BIND_ADDRESS").unwrap_or_else(|_| "127.0.0.1:8787".to_owned());
    let address: std::net::SocketAddr = bind.parse().expect("Invalid BIND_ADDRESS");
    if !address.ip().is_loopback() {
        panic!("Prototype must bind to localhost only");
    }
    let state = AppState {
        http: reqwest::Client::builder()
            .timeout(ATTEMPT_TIMEOUT)
            .connect_timeout(CONNECT_TIMEOUT)
            .user_agent("NightKnightExplore/0.1 (FOSS local development)")
            .build()
            .expect("HTTP client error"),
        endpoints,
        cache: Arc::new(Mutex::new(HashMap::new())),
        last_external_request: Arc::new(Mutex::new(None)),
        endpoint_backoff: Arc::new(Mutex::new(HashMap::new())),
        last_retry_after: Arc::new(Mutex::new(None)),
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

/// Configured Overpass endpoints, most preferred first. An explicit
/// `OVERPASS_ENDPOINT` replaces the default failover chain with a single
/// operator-selected service.
fn configured_endpoints() -> Vec<String> {
    let configured = env::var("OVERPASS_ENDPOINT").ok().and_then(|v| {
        let v = v.trim().to_owned();
        (!v.is_empty()).then_some(v)
    });
    if let Some(url) = configured {
        validate_endpoint(&url);
        vec![url]
    } else {
        DEFAULT_ENDPOINTS
            .iter()
            .map(|endpoint| {
                validate_endpoint(endpoint);
                (*endpoint).to_owned()
            })
            .collect()
    }
}

fn validate_endpoint(url: &str) {
    if !url.starts_with("https://") && !url.starts_with("http://127.0.0.1:") {
        panic!("OVERPASS_ENDPOINT must use HTTPS or local loopback HTTP");
    }
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
    // Small shared-service safeguard. The lock is held through request
    // initiation to avoid concurrent requests defeating the cooldown.
    let mut guard = state.last_external_request.lock().await;
    if guard.is_some_and(|last| last.elapsed() < COOLDOWN) {
        return Err(provider_error(
            StatusCode::TOO_MANY_REQUESTS,
            OverpassFailure::RateLimited,
            Some(COOLDOWN.as_secs()),
        ));
    }
    *guard = Some(Instant::now());
    drop(guard);

    let query = overpass_query(q.lat, q.lon, radius);
    match fetch_overpass(&state, &query).await {
        Ok(payload) => {
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
        Err(failure) => Err(provider_error(
            failure.status(),
            failure,
            retry_hint(failure),
        )),
    }
}

/// POST form body Overpass query. POST rather than enormous query strings;
/// no browser-to-Overpass CORS required.
fn overpass_query(lat: f64, lon: f64, radius: u32) -> String {
    format!(
        "[out:json][timeout:20];(nwr(around:{radius},{lat},{lon})[\"amenity\"~\"^(restaurant|cafe|bar|pub)$\"];nwr(around:{radius},{lat},{lon})[\"tourism\"~\"^(hotel|guest_house|hostel|attraction|museum|viewpoint|artwork|gallery)$\"];);out center;",
    )
}

/// Try every configured endpoint in order, with bounded retries, honoring
/// upstream rate-limit hints, and skipping recently failed endpoints.
/// Returns the first successful response; otherwise the most informative
/// failure seen.
async fn fetch_overpass(
    state: &AppState,
    query: &str,
) -> Result<OverpassResponse, OverpassFailure> {
    let deadline = Instant::now() + MAX_ATTEMPT_WINDOW;
    let mut last_failure: Option<OverpassFailure> = None;
    'endpoints: for endpoint in &state.endpoints {
        let skip_until = state.endpoint_backoff.lock().await.get(endpoint).copied();
        if skip_until.is_some_and(|until| Instant::now() < until) {
            continue;
        }
        // Up to two attempts per endpoint: one immediate, one after the
        // upstream hint or a small backoff.
        for attempt in 0..2 {
            if attempt > 0 {
                let wait = state
                    .last_retry_after
                    .lock()
                    .await
                    .filter(|seconds| Duration::from_secs(*seconds) <= RATE_LIMIT_WAIT_CAP)
                    .map(Duration::from_secs)
                    .unwrap_or(SAME_ENDPOINT_RETRY_BACKOFF);
                tokio::time::sleep(wait).await;
            }
            if Instant::now() >= deadline {
                break 'endpoints;
            }
            match attempt_overpass(state, endpoint, query).await {
                Ok(response) => {
                    state.endpoint_backoff.lock().await.remove(endpoint);
                    return Ok(response);
                }
                Err(OverpassFailure::RateLimited) => {
                    tracing::warn!(endpoint, "Overpass rate limited this client");
                    last_failure = Some(OverpassFailure::RateLimited);
                }
                Err(failure) => {
                    tracing::warn!(
                        endpoint,
                        failure = failure.code(),
                        "Overpass attempt failed"
                    );
                    state
                        .endpoint_backoff
                        .lock()
                        .await
                        .insert(endpoint.clone(), Instant::now() + ENDPOINT_FAILURE_PENALTY);
                    last_failure = Some(failure);
                    break;
                }
            }
        }
    }
    Err(last_failure.unwrap_or(OverpassFailure::TimedOut))
}

/// One HTTP attempt against one endpoint.
async fn attempt_overpass(
    state: &AppState,
    endpoint: &str,
    query: &str,
) -> Result<OverpassResponse, OverpassFailure> {
    let response = state
        .http
        .post(endpoint)
        .form(&[("data", query)])
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                OverpassFailure::TimedOut
            } else {
                OverpassFailure::Unavailable
            }
        })?;
    if !response.status().is_success() {
        return Err(match response.status().as_u16() {
            429 | 503 => {
                if let Some(seconds) = response
                    .headers()
                    .get(header::RETRY_AFTER)
                    .and_then(|value| value.to_str().ok())
                    .and_then(|value| value.trim().parse::<u64>().ok())
                {
                    *state.last_retry_after.lock().await = Some(seconds);
                }
                OverpassFailure::RateLimited
            }
            _ => OverpassFailure::Unavailable,
        });
    }
    let payload: OverpassResponse = response.json().await.map_err(|_| {
        tracing::warn!(endpoint, "Overpass returned unparseable content");
        OverpassFailure::InvalidResponse
    })?;
    Ok(payload)
}

fn retry_hint(failure: OverpassFailure) -> Option<u64> {
    match failure {
        OverpassFailure::RateLimited => Some(COOLDOWN.as_secs()),
        _ => None,
    }
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
    failure: OverpassFailure,
    retry_after_seconds: Option<u64>,
) -> (StatusCode, Json<ApiError>) {
    api_error(
        status,
        failure.code(),
        failure.message(),
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
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicUsize, Ordering};

    const CAFE_FIXTURE: &str = r#"{"elements":[{"type":"node","id":101,"lat":42.6980,"lon":23.3225,"tags":{"name":"Failover Cafe","amenity":"cafe"}}]}"#;

    fn test_state(endpoints: Vec<String>, timeout_millis: u64) -> AppState {
        AppState {
            http: reqwest::Client::builder()
                .timeout(Duration::from_millis(timeout_millis))
                .connect_timeout(Duration::from_millis(500))
                .user_agent("NightKnightExplore-tests/0.1")
                .build()
                .unwrap(),
            endpoints,
            cache: Arc::new(Mutex::new(HashMap::new())),
            last_external_request: Arc::new(Mutex::new(None)),
            endpoint_backoff: Arc::new(Mutex::new(HashMap::new())),
            last_retry_after: Arc::new(Mutex::new(None)),
        }
    }

    /// One scripted upstream response. Empty-body entries stand in for
    /// "this hit fails".
    #[derive(Clone)]
    struct ScriptEntry {
        status: u16,
        retry_after: Option<u64>,
        body: &'static str,
        delay: Option<u64>,
    }
    impl ScriptEntry {
        fn ok() -> Self {
            Self {
                status: 200,
                retry_after: None,
                body: CAFE_FIXTURE,
                delay: None,
            }
        }
        fn status(status: u16) -> Self {
            Self {
                status,
                retry_after: None,
                body: "",
                delay: None,
            }
        }
        fn rate_limited(retry_after: u64) -> Self {
            Self {
                status: 429,
                retry_after: Some(retry_after),
                body: "",
                delay: None,
            }
        }
        /// Succeeds, but slowly enough to trip a short client timeout.
        fn slow_ok(delay_millis: u64) -> Self {
            Self {
                status: 200,
                retry_after: None,
                body: CAFE_FIXTURE,
                delay: Some(delay_millis),
            }
        }
    }

    /// Local test double: an axum server that replays the scripted responses
    /// in order, then replays the last one forever. Returns the endpoint URL
    /// and a hit counter.
    async fn mock_overpass(scripts: Vec<ScriptEntry>) -> (String, Arc<AtomicUsize>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let replay: Arc<Mutex<VecDeque<ScriptEntry>>> =
            Arc::new(Mutex::new(scripts.into_iter().collect()));
        let hits = Arc::new(AtomicUsize::new(0));
        let hit_counter = hits.clone();
        let app = Router::new().route(
            "/api/interpreter",
            axum::routing::post(move || {
                let replay = replay.clone();
                let hit_counter = hit_counter.clone();
                async move {
                    hit_counter.fetch_add(1, Ordering::SeqCst);
                    let mut queue = replay.lock().await;
                    if let Some(entry) = queue.pop_front() {
                        queue.push_back(entry.clone());
                        drop(queue);
                        if let Some(delay) = entry.delay {
                            tokio::time::sleep(Duration::from_millis(delay)).await;
                        }
                        let mut response = axum::http::Response::builder()
                            .status(entry.status)
                            .body(entry.body.to_owned())
                            .unwrap();
                        if let Some(seconds) = entry.retry_after {
                            response.headers_mut().insert(
                                header::RETRY_AFTER,
                                axum::http::HeaderValue::from(seconds),
                            );
                        }
                        response
                    } else {
                        drop(queue);
                        axum::http::Response::builder()
                            .status(500)
                            .body(String::new())
                            .unwrap()
                    }
                }
            }),
        );
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        (format!("http://{address}/api/interpreter"), hits)
    }

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
        let (status, Json(payload)) = provider_error(
            StatusCode::TOO_MANY_REQUESTS,
            OverpassFailure::RateLimited,
            Some(8),
        );

        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(payload.code, "provider_rate_limited");
        assert_eq!(payload.provider_id, Some("openstreetmap-overpass"));
        assert_eq!(payload.retry_after_seconds, Some(8));
    }

    #[test]
    fn timeout_failure_maps_to_distinct_code() {
        let (status, Json(payload)) =
            provider_error(StatusCode::BAD_GATEWAY, OverpassFailure::TimedOut, None);
        assert_eq!(status, StatusCode::BAD_GATEWAY);
        assert_eq!(payload.code, "provider_timeout");
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

    #[tokio::test]
    async fn failover_recovers_from_primary_endpoint_failure() {
        let (broken, broken_hits) = mock_overpass(vec![ScriptEntry::status(500)]).await;
        let (working, _) = mock_overpass(vec![ScriptEntry::ok()]).await;
        let state = test_state(vec![broken.clone(), working], 20_000);

        let response = fetch_overpass(&state, "[out:json];out;").await.unwrap();

        assert!(!response.elements.is_empty());
        assert!(broken_hits.load(Ordering::SeqCst) >= 1);
        assert_eq!(state.endpoints.first().unwrap(), &broken);
    }

    #[tokio::test]
    async fn rate_limit_hint_is_honored_with_retried_attempt_under_cap() {
        let (endpoint, hits) =
            mock_overpass(vec![ScriptEntry::rate_limited(1), ScriptEntry::ok()]).await;
        let state = test_state(vec![endpoint], 20_000);

        let response = fetch_overpass(&state, "[out:json];out;").await.unwrap();

        assert!(!response.elements.is_empty());
        assert_eq!(
            hits.load(Ordering::SeqCst),
            2,
            "retry after the hint counts"
        );
    }

    #[tokio::test]
    async fn upstream_failure_is_not_an_empty_success() {
        let (broken, _) = mock_overpass(vec![ScriptEntry::status(500)]).await;
        let state = test_state(vec![broken], 20_000);

        let result = fetch_overpass(&state, "[out:json];out;").await;

        assert!(matches!(result, Err(OverpassFailure::Unavailable)));
    }

    #[tokio::test]
    async fn failed_endpoint_is_skipped_for_a_short_penalties_window() {
        let (broken, hits) = mock_overpass(vec![ScriptEntry::status(500)]).await;
        let state = test_state(vec![broken], 20_000);

        let _ = fetch_overpass(&state, "[out:json];out;").await;
        let hits_after_first = hits.load(Ordering::SeqCst);
        let _ = fetch_overpass(&state, "[out:json];out;").await;
        let hits_after_second = hits.load(Ordering::SeqCst);

        assert!(hits_after_first >= 1);
        assert_eq!(
            hits_after_second, hits_after_first,
            "a just-failed endpoint must not be hammered by the next scan"
        );
    }

    #[tokio::test]
    async fn timeout_is_reported_as_provider_timeout() {
        let (slow, _) = mock_overpass(vec![ScriptEntry::slow_ok(1_000)]).await;
        // The mock replies slowly; the short client timeout simulates a
        // stalled upstream without slowing the test suite down much.
        let state = test_state(vec![slow], 200);

        let result = fetch_overpass(&state, "[out:json];out;").await;

        assert!(matches!(result, Err(OverpassFailure::TimedOut)));
    }
}
