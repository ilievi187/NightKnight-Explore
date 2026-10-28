use axum::{
    extract::{Form, Query, State},
    http::{header, HeaderValue, Method, StatusCode},
    routing::{get, post},
    Json, Router,
};

use nightknight_explore_api::osm_db::OsmDatabase;
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
const GEOCODER_CACHE_TTL: Duration = Duration::from_secs(86_400);
const MAX_GEOCODER_CACHE_ENTRIES: usize = 500;
const GEOCODER_COOLDOWN: Duration = Duration::from_secs(1);
const GEOCODER_TIMEOUT: Duration = Duration::from_secs(8);
/// Local safeguard between this server and shared endpoints.
const COOLDOWN: Duration = Duration::from_secs(8);
/// Overpass query processing timeout; the client must wait at least this
/// long. The server-side query ([timeout:35]) aborts slightly earlier and
/// reports the timeout via `remark`, enabling faster failover.
const ATTEMPT_TIMEOUT: Duration = Duration::from_secs(40);
/// Connection setup timeout so dead DNS/hosts fail fast instead of hanging.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Small backoff between attempts against the same endpoint.
const SAME_ENDPOINT_RETRY_BACKOFF: Duration = Duration::from_secs(2);
/// 429 Retry-After values longer than this are not waited on inside one scan.
const RATE_LIMIT_WAIT_CAP: Duration = Duration::from_secs(8);
/// After an endpoint fails, avoid it for a while instead of hammering it.
const ENDPOINT_FAILURE_PENALTY: Duration = Duration::from_secs(30);
/// Overall cap on the failover chain so one scan never stalls indefinitely.
const MAX_ATTEMPT_WINDOW: Duration = Duration::from_secs(120);
/// Places returned in one scan response. The full match list stays in the
/// memory cache so later pages do not call Overpass again.
const SCAN_PAGE_SIZE: usize = 120;

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
    geocoder_endpoint: String,
    cache: Arc<Mutex<HashMap<String, CachedScan>>>,
    geocoder_cache: Arc<Mutex<HashMap<String, CachedGeocode>>>,
    last_external_request: Arc<Mutex<Option<Instant>>>,
    last_geocoder_request: Arc<Mutex<Option<Instant>>>,
    /// Per-endpoint "not before" time after a failure, so later scans skip
    /// endpoints that are currently known-bad.
    endpoint_backoff: Arc<Mutex<HashMap<String, Instant>>>,
    /// Retry-After hint most recently seen from an upstream 429/503.
    last_retry_after: Arc<Mutex<Option<u64>>>,
    /// Google Maps API key, if configured. When None, Google features are disabled.
    google_api_key: Option<String>,
    /// Local OSM database (optional, for offline discovery without Overpass).
    osm_db: Option<OsmDatabase>,
}
struct CachedScan {
    created: Instant,
    /// Every matching place for this origin and radius, nearest first.
    places: Vec<Place>,
    scanned_at_unix: u64,
    source: &'static str,
}
struct CachedGeocode {
    created: Instant,
    response: GeocodeResponse,
}

#[derive(Deserialize)]
struct ScanQuery {
    lat: f64,
    lon: f64,
    radius_m: Option<u32>,
    offset: Option<usize>,
}
#[derive(Deserialize)]
struct GeocodeQuery {
    q: String,
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
    /// Present only when the record's own OSM tags support at least one
    /// documented Hidden Gems signal. `None` is an honest "no gem signal",
    /// not a synthesized zero.
    gem: Option<GemSignal>,
}

/// Transparent, evidence-anchored "Hidden Gems" signal for one place.
/// Every score contribution is derived from a concrete OSM tag on the
/// record; nothing is popularity-, review- or ranking-based. See
/// docs/hidden-gems-method.md.
#[derive(Serialize, Clone)]
struct GemSignal {
    /// 0–100 deterministic tag-based interest score (method v1).
    score: u8,
    /// Stable identifier of the scoring method that produced this signal.
    method: &'static str,
    /// Each reason states the tag that produced it and what it means.
    reasons: Vec<GemReason>,
}

/// One explained contribution to a Hidden Gems score, tied to the exact
/// provider field that supports it.
#[derive(Serialize, Clone, PartialEq)]
struct GemReason {
    /// Plain-language, non-definitive explanation of the signal.
    text: String,
    /// Provider field path that supports the claim, e.g. `tags.historic`.
    field_path: &'static str,
    /// The observed tag value.
    observed_value: String,
}
#[derive(Serialize, Clone)]
struct ScanResponse {
    places: Vec<Place>,
    scanned_at_unix: u64,
    source: &'static str,
    cached: bool,
    total_matches: usize,
}
#[derive(Serialize, Clone)]
struct GeocodePlace {
    id: String,
    display_name: String,
    lat: f64,
    lon: f64,
    source_url: String,
}
#[derive(Serialize, Clone)]
struct GeocodeResponse {
    results: Vec<GeocodePlace>,
    source: &'static str,
    attribution: SourceAttribution,
    cached: bool,
}
#[derive(Debug, Serialize)]
struct ApiError {
    error: String,
    code: &'static str,
    provider_id: Option<&'static str>,
    retry_after_seconds: Option<u64>,
}
#[derive(Deserialize)]
struct OverpassResponse {
    elements: Vec<OverpassElement>,
    remark: Option<String>,
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
    let geocoder_endpoint = configured_geocoder_endpoint();
    let bind = env::var("BIND_ADDRESS").unwrap_or_else(|_| "127.0.0.1:8792".to_owned());
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
        geocoder_endpoint,
        cache: Arc::new(Mutex::new(HashMap::new())),
        geocoder_cache: Arc::new(Mutex::new(HashMap::new())),
        last_external_request: Arc::new(Mutex::new(None)),
        last_geocoder_request: Arc::new(Mutex::new(None)),
        endpoint_backoff: Arc::new(Mutex::new(HashMap::new())),
        last_retry_after: Arc::new(Mutex::new(None)),
        google_api_key: env::var("GOOGLE_MAPS_API_KEY").ok(),
        osm_db: env::var("OSM_DB_PATH")
            .ok()
            .and_then(|path| OsmDatabase::open(std::path::Path::new(&path)).ok()),
    };
    let cors = CorsLayer::new()
        .allow_origin("http://127.0.0.1:5173".parse::<HeaderValue>().unwrap())
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([header::CONTENT_TYPE]);
    let app = Router::new()
        .route(
            "/api/health",
            get(|| async {
                Json(serde_json::json!({"status":"ok","service":"NightKnight Explore API"}))
            }),
        )
        .route("/api/scan", get(scan))
        .route("/api/geocode", post(geocode))
        .route("/api/provider", get(provider_info))
        .route("/api/google/search", post(google_search))
        .route("/api/google/details", post(google_details))
        .route("/api/google/photo", get(google_photo))
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
        panic!("Provider endpoints must use HTTPS or local loopback HTTP");
    }
}

/// Endpoint for explicit end-user place searches. The public Nominatim
/// service is a low-volume default for this local-first prototype; operators
/// can switch providers without a software update by setting this value.
fn configured_geocoder_endpoint() -> String {
    let endpoint = env::var("GEOCODER_ENDPOINT")
        .unwrap_or_else(|_| "https://nominatim.openstreetmap.org/search".to_owned());
    validate_endpoint(&endpoint);
    endpoint
}

/// Provider information endpoint. Returns which provider is configured.
async fn provider_info(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    let google_configured = state.google_api_key.is_some();
    Ok(Json(serde_json::json!({
        "name": if google_configured { "google" } else { "overpass" },
        "display_name": if google_configured { "Google Maps" } else { "OpenStreetMap (Overpass)" },
        "description": if google_configured {
            "Google Maps Platform. Requires API key and billing in Google Cloud."
        } else {
            "OpenStreetMap via Overpass API. No billing required."
        },
        "requires_billing": google_configured,
        "google_api_configured": google_configured,
    })))
}

/// Google Places nearby search. Only works when GOOGLE_MAPS_API_KEY is set.
async fn google_search(
    State(state): State<AppState>,
    Json(req): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    let Some(key) = &state.google_api_key else {
        return Err(provider_error(
            StatusCode::BAD_REQUEST,
            OverpassFailure::Unavailable,
            None,
        ));
    };

    let lat = req.get("lat").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let lon = req.get("lon").and_then(|v| v.as_f64()).unwrap_or(0.0);

    if !lat.is_finite()
        || !lon.is_finite()
        || !(-90.0..=90.0).contains(&lat)
        || !(-180.0..=180.0).contains(&lon)
    {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "invalid_coordinates",
            "Invalid latitude or longitude",
            None,
            None,
        ));
    }

    let radius = req
        .get("radius_m")
        .and_then(|v| v.as_u64())
        .unwrap_or(5000)
        .min(50000);
    let max_results = req
        .get("max_results")
        .and_then(|v| v.as_u64())
        .unwrap_or(20)
        .min(20);

    let client = state.http.clone();
    let response = client
        .post("https://places.googleapis.com/v1/places:searchNearby")
        .header("X-Goog-Api-Key", key)
        .header("X-Goog-FieldMask", "places.id,places.displayName,places.types,places.rating,places.userRatingCount,places.formattedAddress,places.websiteUri,places.internationalPhoneNumber,places.priceLevel,places.photos")
        .json(&serde_json::json!({
            "locationRestriction": {
                "circle": {
                    "center": {"latitude": lat, "longitude": lon},
                    "radius": radius
                }
            },
            "maxResultCount": max_results,
        }))
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                api_error(StatusCode::GATEWAY_TIMEOUT, "google_timeout", "Google Places request timed out. Try again later.", None, None)
            } else {
                api_error(StatusCode::BAD_GATEWAY, "google_upstream_error", "Google Places upstream error. Check Google Cloud quotas and billing.", None, None)
            }
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body_text = response.text().await.unwrap_or_default();
        return Err(api_error(
            status,
            "google_api_error",
            &format!("Google Places API returned {}: {}", status, body_text),
            None,
            None,
        ));
    }

    let google_response: serde_json::Value = response.json().await.map_err(|_| {
        api_error(
            StatusCode::BAD_GATEWAY,
            "google_invalid_response",
            "Google Places returned unparseable content.",
            None,
            None,
        )
    })?;

    Ok(Json(google_response))
}

/// Google Place details fetch. Only works when GOOGLE_MAPS_API_KEY is set.
async fn google_details(
    State(state): State<AppState>,
    Json(req): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    let Some(key) = &state.google_api_key else {
        return Err(provider_error(
            StatusCode::BAD_REQUEST,
            OverpassFailure::Unavailable,
            None,
        ));
    };

    let place_id = req.get("place_id").and_then(|v| v.as_str()).unwrap_or("");
    if place_id.is_empty() || place_id.len() > 256 {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "invalid_place_id",
            "place_id is required and must be under 256 characters",
            None,
            None,
        ));
    }

    let client = state.http.clone();
    let response = client
        .get(format!("https://places.googleapis.com/v1/places/{}", place_id))
        .header("X-Goog-Api-Key", key)
        .header("X-Goog-FieldMask", "id,displayName,rating,userRatingCount,formattedAddress,websiteUri,internationalPhoneNumber,priceLevel,regularOpeningHours,photos,reviews")
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                api_error(StatusCode::GATEWAY_TIMEOUT, "google_timeout", "Google Places details request timed out.", None, None)
            } else {
                api_error(StatusCode::BAD_GATEWAY, "google_upstream_error", "Google Places details upstream error.", None, None)
            }
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body_text = response.text().await.unwrap_or_default();
        return Err(api_error(
            status,
            "google_api_error",
            &format!(
                "Google Places details API returned {}: {}",
                status, body_text
            ),
            None,
            None,
        ));
    }

    let details: serde_json::Value = response.json().await.map_err(|_| {
        api_error(
            StatusCode::BAD_GATEWAY,
            "google_invalid_response",
            "Google Places details returned unparseable content.",
            None,
            None,
        )
    })?;

    Ok(Json(details))
}

/// Google Places Photo (New) proxy. Fetches photo media from Google and
/// streams it to the browser without exposing the server API key.
async fn google_photo(
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<impl axum::response::IntoResponse, (StatusCode, Json<ApiError>)> {
    let Some(key) = &state.google_api_key else {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "google_unavailable",
            "Google Maps Platform is not configured.",
            None,
            None,
        ));
    };

    let photo_name = params.get("name").cloned().unwrap_or_default();
    let max_width = params
        .get("maxWidthPx")
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(400)
        .min(4800);

    if photo_name.is_empty() {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "invalid_photo_name",
            "Photo name is required",
            None,
            None,
        ));
    }

    // Validate photo name format (should be places/.../photos/...)
    if !photo_name.starts_with("places/") {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "invalid_photo_name",
            "Invalid photo name format",
            None,
            None,
        ));
    }

    let client = state.http.clone();
    let url = format!(
        "https://places.googleapis.com/v1/{}?key={}&maxWidthPx={}",
        photo_name, key, max_width
    );

    let response = client.get(&url).send().await.map_err(|e| {
        if e.is_timeout() {
            api_error(
                StatusCode::GATEWAY_TIMEOUT,
                "google_timeout",
                "Google photo request timed out.",
                None,
                None,
            )
        } else {
            api_error(
                StatusCode::BAD_GATEWAY,
                "google_upstream_error",
                "Google photo upstream error.",
                None,
                None,
            )
        }
    })?;

    if !response.status().is_success() {
        let status = response.status();
        return Err(api_error(
            status,
            "google_api_error",
            &format!("Google photo API returned {}", status),
            None,
            None,
        ));
    }

    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("image/jpeg")
        .to_owned();

    let bytes = response.bytes().await.map_err(|_| {
        api_error(
            StatusCode::BAD_GATEWAY,
            "google_invalid_response",
            "Failed to read photo data.",
            None,
            None,
        )
    })?;

    Ok((
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, content_type)],
        bytes,
    ))
}

async fn scan(
    State(state): State<AppState>,
    Query(q): Query<ScanQuery>,
) -> Result<Json<ScanResponse>, (StatusCode, Json<ApiError>)> {
    let radius = q.radius_m.unwrap_or(2000);
    let offset = q.offset.unwrap_or(0);
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
    {
        let mut cache = state.cache.lock().await;
        cache.retain(|_, hit| hit.created.elapsed() < OVERPASS_CACHE_TTL);
        if let Some(hit) = cache.get(&key) {
            return Ok(Json(scan_page(
                &hit.places,
                offset,
                true,
                hit.scanned_at_unix,
                hit.source,
            )));
        }
    }
    // If local OSM database is available, use it instead of Overpass.
    if let Some(db) = &state.osm_db {
        let (places, _) = db
            .search_nearby(q.lat, q.lon, radius, usize::MAX, 0)
            .map_err(|e| {
                error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    &format!("Local OSM DB error: {}", e),
                )
            })?;
        let mut places: Vec<Place> = places
            .into_iter()
            .map(|lp| {
                // Convert LocalPlace to Place
                let category = lp.category.clone();
                let name = lp.name.clone().unwrap_or_else(|| format!("OSM {}", lp.id));
                let distance = haversine(q.lat, q.lon, lp.lat, lp.lon) as u32;
                Place {
                    id: format!("{}-{}", lp.osm_type, lp.id),
                    name,
                    category: category.clone(),
                    lat: lp.lat,
                    lon: lp.lon,
                    distance_m: distance,
                    source: "OpenStreetMap (local)",
                    source_url: format!("https://www.openstreetmap.org/{}/{}", lp.osm_type, lp.id),
                    attribution: SourceAttribution {
                        provider_id: "openstreetmap",
                        label: "OpenStreetMap",
                        text: "© OpenStreetMap contributors",
                        url: "https://www.openstreetmap.org/copyright",
                        license: "ODbL",
                    },
                    website: lp.website.clone(),
                    gem: {
                        let tags: HashMap<String, String> = {
                            // Try HashMap format first (new format)
                            if let Ok(map) =
                                serde_json::from_str::<HashMap<String, String>>(&lp.tags)
                            {
                                map
                            } else if let Ok(arr) =
                                serde_json::from_str::<Vec<(String, String)>>(&lp.tags)
                            {
                                // Fall back to array format (old format)
                                arr.into_iter().collect()
                            } else {
                                HashMap::new()
                            }
                        };
                        assess_gem(&tags)
                    },
                }
            })
            .collect();
        sort_places(&mut places);
        let scanned_at_unix = now_unix();
        let response = scan_page(
            &places,
            offset,
            false,
            scanned_at_unix,
            "OpenStreetMap (local DB)",
        );
        state.cache.lock().await.insert(
            key,
            CachedScan {
                created: Instant::now(),
                places,
                scanned_at_unix,
                source: "OpenStreetMap (local DB)",
            },
        );
        return Ok(Json(response));
    }

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
            sort_places(&mut places);
            let scanned_at_unix = now_unix();
            let response = scan_page(&places, offset, false, scanned_at_unix, "OpenStreetMap");
            state.cache.lock().await.insert(
                key,
                CachedScan {
                    created: Instant::now(),
                    places,
                    scanned_at_unix,
                    source: "OpenStreetMap",
                },
            );
            Ok(Json(response))
        }
        Err(failure) => {
            let upstream_hint = *state.last_retry_after.lock().await;
            Err(provider_error(
                failure.status(),
                failure,
                retry_hint(failure, upstream_hint),
            ))
        }
    }
}

/// Explicit, user-submitted place lookup. This is deliberately not an
/// autocomplete endpoint: each call must follow a user action, and the
/// public Nominatim policy is enforced by a shared one-request/second gate.
async fn geocode(
    State(state): State<AppState>,
    Form(q): Form<GeocodeQuery>,
) -> Result<Json<GeocodeResponse>, (StatusCode, Json<ApiError>)> {
    let query = q.q.split_whitespace().collect::<Vec<_>>().join(" ");
    if query.chars().count() < 2 || query.chars().count() > 200 {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "invalid_geocode_query",
            "Enter a place name or location (2–200 characters)",
            None,
            None,
        ));
    }
    let key = query.to_lowercase();
    {
        let mut cache = state.geocoder_cache.lock().await;
        cache.retain(|_, hit| hit.created.elapsed() < GEOCODER_CACHE_TTL);
        if let Some(hit) = cache.get(&key) {
            let mut response = hit.response.clone();
            response.cached = true;
            return Ok(Json(response));
        }
    }

    let mut last_request = state.last_geocoder_request.lock().await;
    if let Some(last) = *last_request {
        if last.elapsed() < GEOCODER_COOLDOWN {
            let retry = GEOCODER_COOLDOWN.as_secs().max(1);
            return Err(api_error(
                StatusCode::TOO_MANY_REQUESTS,
                "provider_rate_limited",
                "Place search is rate limited; wait before searching again",
                Some("openstreetmap-nominatim"),
                Some(retry),
            ));
        }
    }
    *last_request = Some(Instant::now());
    drop(last_request);

    let request = state
        .http
        .get(&state.geocoder_endpoint)
        .query(&[("q", query.as_str()), ("format", "jsonv2"), ("limit", "5")])
        .send();
    let request_deadline = Instant::now() + GEOCODER_TIMEOUT;
    let response = tokio::time::timeout(
        request_deadline.saturating_duration_since(Instant::now()),
        request,
    )
    .await
    .map_err(|_| {
        geocoder_error(
            StatusCode::BAD_GATEWAY,
            "provider_timeout",
            "Place search timed out; try again later",
            None,
        )
    })?
    .map_err(|error| {
        tracing::warn!("Nominatim unavailable: {error}");
        geocoder_error(
            StatusCode::BAD_GATEWAY,
            "provider_unavailable",
            "Place search provider unavailable",
            None,
        )
    })?;

    if !response.status().is_success() {
        let status = response.status();
        if status == StatusCode::TOO_MANY_REQUESTS {
            let retry_after = response
                .headers()
                .get(header::RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.trim().parse::<u64>().ok())
                .or(Some(GEOCODER_COOLDOWN.as_secs()));
            return Err(geocoder_error(
                StatusCode::TOO_MANY_REQUESTS,
                "provider_rate_limited",
                "Place search provider is rate limiting requests",
                retry_after,
            ));
        }
        if status == StatusCode::SERVICE_UNAVAILABLE {
            let retry_after = response
                .headers()
                .get(header::RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.trim().parse::<u64>().ok());
            return Err(geocoder_error(
                StatusCode::BAD_GATEWAY,
                "provider_unavailable",
                "Place search provider is temporarily unavailable",
                retry_after,
            ));
        }
        tracing::warn!(status=%status, "Nominatim returned an error");
        return Err(geocoder_error(
            StatusCode::BAD_GATEWAY,
            "provider_unavailable",
            "Place search provider returned an error",
            None,
        ));
    }

    let payload: Vec<serde_json::Value> = tokio::time::timeout(
        request_deadline.saturating_duration_since(Instant::now()),
        response.json(),
    )
    .await
    .map_err(|_| {
        geocoder_error(
            StatusCode::BAD_GATEWAY,
            "provider_timeout",
            "Place search response timed out; try again later",
            None,
        )
    })?
    .map_err(|_| {
        geocoder_error(
            StatusCode::BAD_GATEWAY,
            "provider_invalid_response",
            "Invalid place search response",
            None,
        )
    })?;
    let results: Vec<GeocodePlace> = payload
        .iter()
        .filter_map(nominatim_result)
        .take(5)
        .collect();
    if !payload.is_empty() && results.is_empty() {
        return Err(geocoder_error(
            StatusCode::BAD_GATEWAY,
            "provider_invalid_response",
            "Place search provider returned no usable records",
            None,
        ));
    }
    let result = GeocodeResponse {
        results,
        source: "OpenStreetMap Nominatim",
        attribution: SourceAttribution {
            provider_id: "openstreetmap-nominatim",
            label: "OpenStreetMap contributors",
            text: "© OpenStreetMap contributors",
            url: "https://www.openstreetmap.org/copyright",
            license: "ODbL-1.0",
        },
        cached: false,
    };
    let mut cache = state.geocoder_cache.lock().await;
    if cache.len() >= MAX_GEOCODER_CACHE_ENTRIES {
        if let Some(oldest_key) = cache
            .iter()
            .min_by_key(|(_, entry)| entry.created)
            .map(|(key, _)| key.clone())
        {
            cache.remove(&oldest_key);
        }
    }
    cache.insert(
        key,
        CachedGeocode {
            created: Instant::now(),
            response: result.clone(),
        },
    );
    Ok(Json(result))
}

fn nominatim_result(value: &serde_json::Value) -> Option<GeocodePlace> {
    let kind = value.get("osm_type")?.as_str()?;
    if !matches!(kind, "node" | "way" | "relation") {
        return None;
    }
    let osm_id = value.get("osm_id")?.as_i64()?;
    if osm_id <= 0 {
        return None;
    }
    let lat = value.get("lat")?.as_str()?.parse::<f64>().ok()?;
    let lon = value.get("lon")?.as_str()?.parse::<f64>().ok()?;
    if !lat.is_finite()
        || !lon.is_finite()
        || !(-90.0..=90.0).contains(&lat)
        || !(-180.0..=180.0).contains(&lon)
    {
        return None;
    }
    let display_name = value.get("display_name")?.as_str()?.trim();
    if display_name.is_empty() {
        return None;
    }
    Some(GeocodePlace {
        id: format!("{kind}-{osm_id}"),
        display_name: display_name.to_owned(),
        lat,
        lon,
        source_url: format!("https://www.openstreetmap.org/{kind}/{osm_id}"),
    })
}

fn geocoder_error(
    status: StatusCode,
    code: &'static str,
    message: &str,
    retry_after_seconds: Option<u64>,
) -> (StatusCode, Json<ApiError>) {
    api_error(
        status,
        code,
        message,
        Some("openstreetmap-nominatim"),
        retry_after_seconds,
    )
}

/// POST form body Overpass query. POST rather than enormous query strings;
/// no browser-to-Overpass CORS required. Selectors cover food/stay/see plus
/// the historical, natural and cultural record types the Hidden Gems signal
/// evaluates.
fn overpass_query(lat: f64, lon: f64, radius: u32) -> String {
    format!(
        "[out:json][timeout:35];(\
nwr(around:{radius},{lat},{lon})[\"amenity\"~\"^(restaurant|cafe|bar|pub)$\"];\
nwr(around:{radius},{lat},{lon})[\"tourism\"~\"^(hotel|guest_house|hostel|attraction|museum|viewpoint|artwork|gallery)$\"];\
nwr(around:{radius},{lat},{lon})[\"historic\"~\"^(monument|memorial|castle|ruins|ruin|archaeological_site|monastery|fort|tower)$\"];\
nwr(around:{radius},{lat},{lon})[\"natural\"~\"^(beach|spring|cave_entrance)$\"];\
nwr(around:{radius},{lat},{lon})[\"waterway\"=\"waterfall\"];\
nwr(around:{radius},{lat},{lon})[\"leisure\"=\"garden\"];\
);out center;",
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
    // Do not let a previous scan's Retry-After value affect this request.
    *state.last_retry_after.lock().await = None;
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
                    // A long upstream Retry-After is not permission to retry
                    // early. Skip this instance for this scan and try the
                    // next configured service instead.
                    let wait_hint = *state.last_retry_after.lock().await;
                    if wait_hint
                        .is_some_and(|seconds| Duration::from_secs(seconds) > RATE_LIMIT_WAIT_CAP)
                    {
                        break;
                    }
                }
                // A stalled upstream is not worth a second wait: move on to
                // the next endpoint immediately.
                Err(failure @ OverpassFailure::TimedOut) => {
                    tracing::warn!(
                        endpoint,
                        failure = failure.code(),
                        "Overpass attempt timed out"
                    );
                    state
                        .endpoint_backoff
                        .lock()
                        .await
                        .insert(endpoint.clone(), Instant::now() + ENDPOINT_FAILURE_PENALTY);
                    last_failure = Some(failure);
                    break;
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
    // Overpass reports query failures inside a 200 response via `remark`.
    // A timed-out server-side query is a provider timeout, not a successful
    // empty scan.
    if let Some(remark) = &payload.remark {
        if remark.contains("timed out") || remark.contains("timeout") {
            tracing::warn!(endpoint, remark, "Overpass server-side query timeout");
            return Err(OverpassFailure::TimedOut);
        }
        if remark.contains("runtime error") || remark.contains("static error") {
            tracing::warn!(endpoint, remark, "Overpass reported a query error");
            return Err(OverpassFailure::InvalidResponse);
        }
    }
    Ok(payload)
}

fn retry_hint(failure: OverpassFailure, upstream_hint: Option<u64>) -> Option<u64> {
    match failure {
        OverpassFailure::RateLimited => Some(upstream_hint.unwrap_or(COOLDOWN.as_secs())),
        _ => None,
    }
}

fn scan_cache_key(lat: f64, lon: f64, radius_m: u32) -> String {
    // Pagination is applied after lookup. One cached scan serves every page,
    // so the key must not include offset. Signed zero is normalized so
    // 0.0 and -0.0 share a cache entry.
    let normalized_lat = if lat == 0.0 { 0.0 } else { lat };
    let normalized_lon = if lon == 0.0 { 0.0 } else { lon };
    format!(
        "{:016x}:{:016x}:{radius_m}",
        normalized_lat.to_bits(),
        normalized_lon.to_bits()
    )
}

fn sort_places(places: &mut [Place]) {
    places.sort_by(|left, right| {
        left.distance_m
            .cmp(&right.distance_m)
            .then_with(|| left.id.cmp(&right.id))
    });
}

/// Return one page without discarding the rest of the match list.
fn scan_page(
    places: &[Place],
    offset: usize,
    cached: bool,
    scanned_at_unix: u64,
    source: &'static str,
) -> ScanResponse {
    let total = places.len();
    let start = offset.min(total);
    let end = start.saturating_add(SCAN_PAGE_SIZE).min(total);
    ScanResponse {
        places: places[start..end].to_vec(),
        scanned_at_unix,
        source,
        cached,
        total_matches: total,
    }
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
    let gem = assess_gem(&tags);
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
        gem,
    })
}
fn category(tags: &HashMap<String, String>) -> &'static str {
    if let Some("restaurant" | "cafe" | "bar" | "pub") = tags.get("amenity").map(String::as_str) {
        return "food";
    }
    match tags.get("tourism").map(String::as_str) {
        Some("hotel" | "guest_house" | "hostel") => "stay",
        Some("attraction" | "museum" | "viewpoint" | "artwork" | "gallery") => "see",
        _ => {
            if matches!(
                tags.get("historic").map(String::as_str),
                Some(
                    "monument"
                        | "memorial"
                        | "castle"
                        | "ruins"
                        | "ruin"
                        | "archaeological_site"
                        | "monastery"
                        | "fort"
                        | "tower"
                )
            ) {
                "history"
            } else if matches!(
                tags.get("natural").map(String::as_str),
                Some("beach" | "spring" | "cave_entrance")
            ) || tags.get("waterway").map(String::as_str) == Some("waterfall")
                || tags.get("leisure").map(String::as_str) == Some("garden")
            {
                "nature"
            } else {
                "other"
            }
        }
    }
}

/// Stable URL-safe Wikipedia/Wikidata-style site flag for documentation
/// signals.
fn documented(tags: &HashMap<String, String>) -> bool {
    ["wikidata", "wikipedia", "image"]
        .iter()
        .any(|key| tags.get(*key).is_some_and(|value| !value.trim().is_empty()))
}

/// Derive the transparent Hidden Gems signal from the record's own tags
/// (method `osm-tag-signals-v1`). Returns `None` when no documented signal
/// applies, so the absence of a gem is never fabricated.
fn assess_gem(tags: &HashMap<String, String>) -> Option<GemSignal> {
    let mut reasons: Vec<GemReason> = Vec::new();
    let mut score: u16 = 0;
    fn add(
        points: u16,
        reasons: &mut Vec<GemReason>,
        score: &mut u16,
        path: &'static str,
        value: String,
        text: String,
    ) {
        *score += points;
        reasons.push(GemReason {
            text,
            field_path: path,
            observed_value: value,
        });
    }

    match tags.get("historic").map(String::as_str) {
        Some("monument" | "memorial") => add(
            25,
            &mut reasons,
            &mut score,
            "tags.historic",
            tags["historic"].clone(),
            "Tagged in OSM as a monument or memorial — a historical landmark.".to_owned(),
        ),
        Some("castle" | "fort" | "monastery" | "tower") => add(
            25,
            &mut reasons,
            &mut score,
            "tags.historic",
            tags["historic"].clone(),
            "Tagged in OSM as a castle, fort, monastery or tower — a historical structure."
                .to_owned(),
        ),
        Some("ruins" | "ruin") => add(
            25,
            &mut reasons,
            &mut score,
            "tags.historic",
            tags["historic"].clone(),
            "Tagged in OSM as ruins — condition varies; verify before visiting.".to_owned(),
        ),
        Some("archaeological_site") => add(
            25,
            &mut reasons,
            &mut score,
            "tags.historic",
            tags["historic"].clone(),
            "Tagged in OSM as an archaeological site.".to_owned(),
        ),
        _ => {}
    }
    match tags.get("tourism").map(String::as_str) {
        Some("viewpoint") => add(
            15,
            &mut reasons,
            &mut score,
            "tags.tourism",
            "viewpoint".to_owned(),
            "Tagged in OSM as a viewpoint — a vantage point noted by mappers.".to_owned(),
        ),
        Some("artwork") => add(
            15,
            &mut reasons,
            &mut score,
            "tags.tourism",
            "artwork".to_owned(),
            "Tagged in OSM as public artwork.".to_owned(),
        ),
        _ => {}
    }
    if matches!(
        tags.get("abandoned").map(String::as_str),
        Some("yes") | Some("true") | Some("1")
    ) {
        add(
            20,
            &mut reasons,
            &mut score,
            "tags.abandoned",
            "yes".to_owned(),
            "Tagged in OSM as abandoned — mapped as derelict; verify on site.".to_owned(),
        );
    } else if matches!(
        tags.get("disused").map(String::as_str),
        Some("yes") | Some("true") | Some("1")
    ) {
        add(
            20,
            &mut reasons,
            &mut score,
            "tags.disused",
            "yes".to_owned(),
            "Tagged in OSM as disused — no longer in operation; verify on site.".to_owned(),
        );
    }
    if let Some("beach" | "spring" | "cave_entrance") = tags.get("natural").map(String::as_str) {
        add(
            10,
            &mut reasons,
            &mut score,
            "tags.natural",
            tags["natural"].clone(),
            format!(
                "Tagged in OSM as a natural {}.",
                tags["natural"].replace('_', " ")
            ),
        );
    }
    if tags.get("waterway").map(String::as_str) == Some("waterfall") {
        add(
            10,
            &mut reasons,
            &mut score,
            "tags.waterway",
            "waterfall".to_owned(),
            "Tagged in OSM as a waterfall.".to_owned(),
        );
    }
    if tags.get("leisure").map(String::as_str) == Some("garden") {
        add(
            10,
            &mut reasons,
            &mut score,
            "tags.leisure",
            "garden".to_owned(),
            "Tagged in OSM as a garden.".to_owned(),
        );
    }
    if documented(tags) {
        add(
            10,
            &mut reasons,
            &mut score,
            "tags.wikidata|wikipedia|image",
            String::new(),
            "Record links documentation (an image, Wikipedia or Wikidata entry).".to_owned(),
        );
    }
    if tags
        .get("description")
        .is_some_and(|value| !value.trim().is_empty())
    {
        add(
            5,
            &mut reasons,
            &mut score,
            "tags.description",
            String::new(),
            "Carries an OSM description field.".to_owned(),
        );
    }
    if reasons.is_empty() {
        return None;
    }
    Some(GemSignal {
        score: score.min(100) as u8,
        method: "osm-tag-signals-v1",
        reasons,
    })
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
            geocoder_endpoint: "https://nominatim.openstreetmap.org/search".into(),
            cache: Arc::new(Mutex::new(HashMap::new())),
            geocoder_cache: Arc::new(Mutex::new(HashMap::new())),
            last_external_request: Arc::new(Mutex::new(None)),
            last_geocoder_request: Arc::new(Mutex::new(None)),
            endpoint_backoff: Arc::new(Mutex::new(HashMap::new())),
            last_retry_after: Arc::new(Mutex::new(None)),
            google_api_key: None,
            osm_db: None,
        }
    }

    fn test_state_with_geocoder(endpoint: String) -> AppState {
        let mut state = test_state(vec![], 20_000);
        state.geocoder_endpoint = endpoint;
        state
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
        /// Overpass-style failure: HTTP 200 with a runtime timeout remark
        /// inside the JSON body.
        fn remark_timeout() -> Self {
            Self {
                status: 200,
                retry_after: None,
                body: r#"{"elements":[],"remark":"runtime error: Query timed out in \"recurse\" at line 1"}"#,
                delay: None,
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

    async fn mock_geocoder() -> (String, Arc<AtomicUsize>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let hits = Arc::new(AtomicUsize::new(0));
        let hit_counter = hits.clone();
        let app = Router::new().route(
            "/search",
            get(move || {
                let hits = hit_counter.clone();
                async move {
                    hits.fetch_add(1, Ordering::SeqCst);
                    Json(serde_json::json!([
                        {
                            "osm_type": "relation",
                            "osm_id": 8311615,
                            "lat": "42.697278",
                            "lon": "23.3217279",
                            "display_name": "Pretorium, Sofia, Bulgaria"
                        },
                        {
                            "osm_type": "bad-type",
                            "osm_id": 9,
                            "lat": "0",
                            "lon": "0",
                            "display_name": "Should be skipped"
                        }
                    ]))
                }
            }),
        );
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        (format!("http://{address}/search"), hits)
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
    fn nominatim_result_uses_only_valid_canonical_osm_records() {
        let valid = serde_json::json!({
            "osm_type":"relation", "osm_id":42, "lat":"43.2", "lon":"27.9",
            "display_name":"Historic Square, Varna"
        });
        let place = nominatim_result(&valid).unwrap();
        assert_eq!(place.id, "relation-42");
        assert_eq!(place.display_name, "Historic Square, Varna");
        assert_eq!(
            place.source_url,
            "https://www.openstreetmap.org/relation/42"
        );

        let invalid_kind = serde_json::json!({
            "osm_type":"area", "osm_id":42, "lat":"43.2", "lon":"27.9",
            "display_name":"Unknown"
        });
        assert!(nominatim_result(&invalid_kind).is_none());
        let invalid_coords = serde_json::json!({
            "osm_type":"node", "osm_id":43, "lat":"91", "lon":"27.9",
            "display_name":"Out of range"
        });
        assert!(nominatim_result(&invalid_coords).is_none());
    }

    #[tokio::test]
    async fn geocoder_normalizes_explicit_search_and_caches_results() {
        let (endpoint, hits) = mock_geocoder().await;
        let state = test_state_with_geocoder(endpoint);

        let Json(first) = geocode(
            State(state.clone()),
            Form(GeocodeQuery {
                q: "  Sofia   Bulgaria  ".into(),
            }),
        )
        .await
        .unwrap();
        let Json(cached) = geocode(
            State(state),
            Form(GeocodeQuery {
                q: "sofia bulgaria".into(),
            }),
        )
        .await
        .unwrap();

        assert_eq!(hits.load(Ordering::SeqCst), 1);
        assert!(!first.cached);
        assert!(cached.cached);
        assert_eq!(
            first.results.len(),
            1,
            "invalid upstream record is filtered"
        );
        assert_eq!(
            first.results[0].source_url,
            "https://www.openstreetmap.org/relation/8311615"
        );
        assert_eq!(first.attribution.provider_id, "openstreetmap-nominatim");
    }

    #[tokio::test]
    async fn geocoder_enforces_one_second_cooldown_without_provider_call() {
        let (endpoint, hits) = mock_geocoder().await;
        let state = test_state_with_geocoder(endpoint);
        *state.last_geocoder_request.lock().await = Some(Instant::now());

        let Err((status, Json(payload))) =
            geocode(State(state), Form(GeocodeQuery { q: "Varna".into() })).await
        else {
            panic!("cooldown should reject the request");
        };

        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(payload.code, "provider_rate_limited");
        assert_eq!(payload.provider_id, Some("openstreetmap-nominatim"));
        assert_eq!(hits.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn geocoder_rejects_too_short_queries_without_provider_call() {
        let (endpoint, hits) = mock_geocoder().await;
        let state = test_state_with_geocoder(endpoint);
        let Err((status, Json(payload))) =
            geocode(State(state.clone()), Form(GeocodeQuery { q: "x".into() })).await
        else {
            panic!("short query should be rejected");
        };

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(payload.code, "invalid_geocode_query");
        assert_eq!(hits.load(Ordering::SeqCst), 0);

        let Err((long_status, Json(long_payload))) =
            geocode(State(state), Form(GeocodeQuery { q: "x".repeat(201) })).await
        else {
            panic!("overlong query should be rejected");
        };
        assert_eq!(long_status, StatusCode::BAD_REQUEST);
        assert_eq!(long_payload.code, "invalid_geocode_query");
        assert_eq!(hits.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn upstream_retry_after_is_preserved_in_the_client_hint() {
        assert_eq!(retry_hint(OverpassFailure::RateLimited, Some(30)), Some(30));
        assert_eq!(retry_hint(OverpassFailure::RateLimited, None), Some(8));
        assert_eq!(retry_hint(OverpassFailure::Unavailable, Some(30)), None);
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
        assert_eq!(
            scan_cache_key(0.0, 0.0, 2500),
            scan_cache_key(-0.0, -0.0, 2500)
        );
    }

    fn sample_place(id: &str, distance_m: u32) -> Place {
        Place {
            id: id.to_owned(),
            name: id.to_owned(),
            category: "food".to_owned(),
            lat: 0.0,
            lon: 0.0,
            distance_m,
            source: "OpenStreetMap",
            source_url: format!("https://www.openstreetmap.org/node/{id}"),
            attribution: SourceAttribution {
                provider_id: "openstreetmap",
                label: "OpenStreetMap contributors",
                text: "© OpenStreetMap contributors",
                url: "https://www.openstreetmap.org/copyright",
                license: "ODbL-1.0",
            },
            website: None,
            gem: None,
        }
    }

    #[test]
    fn scan_page_keeps_matches_beyond_the_first_page() {
        let mut places: Vec<_> = (0..125)
            .map(|index| sample_place(&format!("node-{index}"), 5_000 - (index % 7) as u32))
            .collect();
        sort_places(&mut places);

        let first = scan_page(&places, 0, false, 1, "OpenStreetMap");
        assert_eq!(first.places.len(), SCAN_PAGE_SIZE);
        assert_eq!(first.total_matches, 125);
        assert!(!first.cached);
        assert!(first.places.windows(2).all(|pair| {
            pair[0].distance_m < pair[1].distance_m
                || (pair[0].distance_m == pair[1].distance_m && pair[0].id <= pair[1].id)
        }));

        let second = scan_page(&places, SCAN_PAGE_SIZE, true, 1, "OpenStreetMap");
        assert_eq!(second.places.len(), 5);
        assert_eq!(second.total_matches, 125);
        assert!(second.cached);
        assert_ne!(second.places[0].id, first.places[0].id);

        let past_end = scan_page(&places, 10_000, false, 1, "OpenStreetMap");
        assert!(past_end.places.is_empty());
        assert_eq!(past_end.total_matches, 125);
    }

    #[tokio::test]
    async fn overpass_scan_pages_every_match_from_one_upstream_call() {
        let elements: Vec<_> = (0..125)
            .map(|index| {
                serde_json::json!({
                    "type": "node",
                    "id": index + 1,
                    "lat": 42.0 + (index as f64) * 0.00008,
                    "lon": 23.0,
                    "tags": {"name": format!("Cafe {index}"), "amenity": "cafe"}
                })
            })
            .collect();
        let body = serde_json::json!({ "elements": elements }).to_string();
        let body: &'static str = Box::leak(body.into_boxed_str());
        let (endpoint, hits) = mock_overpass(vec![ScriptEntry {
            status: 200,
            retry_after: None,
            body,
            delay: None,
        }])
        .await;
        let state = test_state(vec![endpoint], 20_000);

        let Json(first) = scan(
            State(state.clone()),
            Query(ScanQuery {
                lat: 42.0,
                lon: 23.0,
                radius_m: Some(10_000),
                offset: Some(0),
            }),
        )
        .await
        .unwrap();
        let Json(second) = scan(
            State(state),
            Query(ScanQuery {
                lat: 42.0,
                lon: 23.0,
                radius_m: Some(10_000),
                offset: Some(SCAN_PAGE_SIZE),
            }),
        )
        .await
        .unwrap();

        assert_eq!(hits.load(Ordering::SeqCst), 1);
        assert_eq!(first.places.len(), SCAN_PAGE_SIZE);
        assert_eq!(first.total_matches, 125);
        assert!(!first.cached);
        assert_eq!(first.places[0].id, "node-1");
        assert_eq!(first.places[119].id, "node-120");
        assert_eq!(second.places.len(), 5);
        assert_eq!(second.total_matches, 125);
        assert!(second.cached);
        assert_eq!(second.places[0].id, "node-121");
        assert_eq!(second.places[4].id, "node-125");
    }

    #[test]
    fn gem_signal_is_absent_without_supporting_tags() {
        let mut m = HashMap::new();
        m.insert("amenity".into(), "cafe".into());
        m.insert("name".into(), "Plain cafe".into());
        assert!(assess_gem(&m).is_none());
    }

    #[test]
    fn gem_signal_is_transparent_and_evidence_anchored() {
        let mut m = HashMap::new();
        m.insert("historic".into(), "castle".into());
        let gem = assess_gem(&m).unwrap();
        assert_eq!(gem.method, "osm-tag-signals-v1");
        assert_eq!(gem.score, 25);
        assert_eq!(gem.reasons.len(), 1);
        assert_eq!(gem.reasons[0].field_path, "tags.historic");
        assert_eq!(gem.reasons[0].observed_value, "castle");
        // The explanation names the tag, not any popularity claim.
        assert!(gem.reasons[0].text.contains("castle"));
    }

    #[test]
    fn gem_dereliction_wording_avoids_definitive_claims() {
        let mut m = HashMap::new();
        m.insert("abandoned".into(), "yes".into());
        let gem = assess_gem(&m).unwrap();
        assert!(gem
            .reasons
            .iter()
            .all(|reason| !reason.text.contains("is abandoned")));
        assert!(gem.reasons[0].text.contains("verify on site"));
    }

    #[test]
    fn gem_score_combines_and_caps_at_one_hundred() {
        let mut m = HashMap::new();
        m.insert("historic".into(), "ruins".into());
        m.insert("abandoned".into(), "yes".into());
        m.insert("disused".into(), "yes".into());
        m.insert("tourism".into(), "viewpoint".into());
        m.insert("natural".into(), "cave_entrance".into());
        m.insert("waterway".into(), "waterfall".into());
        m.insert("leisure".into(), "garden".into());
        m.insert("wikidata".into(), "Q42".into());
        m.insert("description".into(), "A curious spot.".into());
        let gem = assess_gem(&m).unwrap();
        assert!(gem.score <= 100);
        // 25+20+15+10+10+10+10+5 = 105 raw (abandoned and disused share one
        // dereliction signal), capped to 100.
        assert_eq!(gem.score, 100);
        assert_eq!(gem.reasons.len(), 8);
        assert_eq!(gem.reasons[0].observed_value, "ruins");
        assert!(matches!(category(&m), "history" | "nature" | "see"));
    }

    #[test]
    fn expanded_categories_map_records() {
        let historic: HashMap<String, String> = [("historic".to_owned(), "monument".to_owned())]
            .into_iter()
            .collect();
        assert_eq!(category(&historic), "history");

        let natural: HashMap<String, String> = [("waterway".to_owned(), "waterfall".to_owned())]
            .into_iter()
            .collect();
        assert_eq!(category(&natural), "nature");

        let garden: HashMap<String, String> = [("leisure".to_owned(), "garden".to_owned())]
            .into_iter()
            .collect();
        assert_eq!(category(&garden), "nature");

        let tavern: HashMap<String, String> = [
            ("amenity".to_owned(), "restaurant".to_owned()),
            ("historic".to_owned(), "memorial".to_owned()),
        ]
        .into_iter()
        .collect();
        assert_eq!(category(&tavern), "food");
    }

    #[test]
    fn gem_carrying_place_is_returned_by_to_place() {
        let mut tags = HashMap::new();
        tags.insert("name".into(), "Old Bastion".into());
        tags.insert("historic".into(), "tower".into());
        let element = OverpassElement {
            kind: "node".into(),
            id: 77,
            lat: Some(43.0),
            lon: Some(27.0),
            center: None,
            tags: Some(tags),
        };
        let place = to_place(element, 43.0, 27.0, 500).unwrap();
        let gem = place.gem.expect("historic tower must carry a gem signal");
        assert_eq!(gem.score, 25);
        assert_eq!(&gem.reasons[0].field_path, &"tags.historic");
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
    async fn long_retry_after_skips_endpoint_and_is_not_retried_early() {
        let (limited, hits) = mock_overpass(vec![ScriptEntry::rate_limited(30)]).await;
        let state = test_state(vec![limited], 20_000);

        let result = fetch_overpass(&state, "[out:json];out;").await;

        assert!(matches!(result, Err(OverpassFailure::RateLimited)));
        assert_eq!(hits.load(Ordering::SeqCst), 1);
        let upstream_hint = *state.last_retry_after.lock().await;
        assert_eq!(
            retry_hint(OverpassFailure::RateLimited, upstream_hint),
            Some(30)
        );
    }

    #[tokio::test]
    async fn retry_after_from_a_previous_scan_is_not_reused() {
        let (endpoint, hits) = mock_overpass(vec![ScriptEntry::status(500)]).await;
        let state = test_state(vec![endpoint], 20_000);
        *state.last_retry_after.lock().await = Some(45);

        let _ = fetch_overpass(&state, "[out:json];out;").await;

        assert_eq!(*state.last_retry_after.lock().await, None);
        assert_eq!(hits.load(Ordering::SeqCst), 1);
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

    #[tokio::test]
    async fn remark_timeout_is_not_a_successful_empty_scan() {
        let (endpoint, _) = mock_overpass(vec![ScriptEntry::remark_timeout()]).await;
        let state = test_state(vec![endpoint], 20_000);

        let result = fetch_overpass(&state, "[out:json];out;").await;

        assert!(
            matches!(result, Err(OverpassFailure::TimedOut)),
            "a 200 response carrying a server-side timeout remark must surface as provider_timeout, not as an empty success"
        );
    }

    #[tokio::test]
    async fn timed_out_endpoint_is_failed_over_without_waiting_for_a_retry() {
        let (slow, slow_hits) = mock_overpass(vec![ScriptEntry::slow_ok(1_000)]).await;
        let (working, _) = mock_overpass(vec![ScriptEntry::ok()]).await;
        let state = test_state(vec![slow, working], 200);

        let result = fetch_overpass(&state, "[out:json];out;").await;

        assert!(
            result.is_ok(),
            "failover must still reach the healthy endpoint"
        );
        assert_eq!(
            slow_hits.load(Ordering::SeqCst),
            1,
            "a stalled endpoint gets one attempt, not a retry"
        );
    }
}
