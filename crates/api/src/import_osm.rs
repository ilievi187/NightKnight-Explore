use osmpbf::ElementReader;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use nightknight_explore_api::osm_db::OsmDatabase;
use serde_json;

fn category_from_tags(tags: &[(String, String)]) -> Option<String> {
    let tag_map: std::collections::HashMap<&str, &str> =
        tags.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();

    if let Some(amenity) = tag_map.get("amenity") {
        match *amenity {
            "restaurant" | "cafe" | "bar" | "pub" | "fast_food" | "food_court" => {
                return Some("food".to_string())
            }
            "hotel" | "guest_house" | "hostel" | "motel" => return Some("stay".to_string()),
            "museum" | "theatre" | "cinema" | "arts_centre" | "gallery" => {
                return Some("see".to_string())
            }
            _ => {}
        }
    }
    if tag_map.contains_key("tourism") {
        return Some("see".to_string());
    }
    if tag_map.contains_key("historic") {
        return Some("history".to_string());
    }
    if tag_map.contains_key("natural")
        || tag_map.contains_key("waterway")
        || tag_map.contains_key("leisure")
    {
        return Some("nature".to_string());
    }
    None
}

fn has_interesting_tags(tags: &[(String, String)]) -> bool {
    let tag_map: std::collections::HashMap<&str, &str> =
        tags.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    tag_map.contains_key("historic")
        || tag_map.contains_key("tourism")
        || tag_map.contains_key("natural")
        || tag_map.contains_key("waterway")
        || tag_map.contains_key("leisure")
        || (tag_map.contains_key("amenity")
            && tag_map
                .get("amenity")
                .map(|a| *a == "fountain" || *a == "viewpoint" || *a == "memorial")
                .unwrap_or(false))
}

fn process_element(
    db: &OsmDatabase,
    osm_type: &str,
    id: i64,
    lat: f64,
    lon: f64,
    tags: &[(String, String)],
) {
    if let Some(cat) = category_from_tags(tags) {
        let name = tags
            .iter()
            .find(|(k, _)| k == "name")
            .map(|(_, v)| v.as_str());
        if name.is_none() && !has_interesting_tags(tags) {
            return;
        }
        let wikidata = tags
            .iter()
            .find(|(k, _)| k == "wikidata")
            .map(|(_, v)| v.as_str());
        let wikipedia = tags
            .iter()
            .find(|(k, _)| k == "wikipedia")
            .map(|(_, v)| v.as_str());
        let website = tags
            .iter()
            .find(|(k, _)| k == "website" || k == "contact:website")
            .map(|(_, v)| v.as_str());
        let phone = tags
            .iter()
            .find(|(k, _)| k == "phone" || k == "contact:phone")
            .map(|(_, v)| v.as_str());
        let opening_hours = tags
            .iter()
            .find(|(k, _)| k == "opening_hours")
            .map(|(_, v)| v.as_str());
        let tags_json = {
            let map: std::collections::HashMap<&str, &str> =
                tags.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
            serde_json::to_string(&map).unwrap_or_default()
        };

        let _ = db.insert_place(
            osm_type,
            id,
            lat,
            lon,
            name,
            &cat,
            &tags_json,
            wikidata,
            wikipedia,
            website,
            phone,
            opening_hours,
        );
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: import-osm <input.pbf> <output.db>");
        std::process::exit(1);
    }
    let input_path = Path::new(&args[1]);
    let output_path = Path::new(&args[2]);

    // Don't delete existing DB until replacement is ready
    let tmp_path = output_path.with_extension("db.tmp");
    let _ = std::fs::remove_file(&tmp_path);

    let db = Arc::new(OsmDatabase::open(&tmp_path).expect("Failed to open database"));

    // Pass 1: Collect node coordinates
    println!("Pass 1: Collecting node coordinates...");
    let mut node_coords: HashMap<i64, (f64, f64)> = HashMap::new();
    let reader = ElementReader::from_path(input_path).expect("Failed to open PBF");
    reader
        .for_each(|element| match element {
            osmpbf::Element::Node(node) => {
                node_coords.insert(node.id(), (node.lat(), node.lon()));
            }
            osmpbf::Element::DenseNode(node) => {
                node_coords.insert(node.id(), (node.lat(), node.lon()));
            }
            _ => {}
        })
        .expect("Failed to read PBF pass 1");
    println!("Collected {} node coordinates", node_coords.len());

    // Pass 2: Process nodes, ways, and relations
    println!("Pass 2: Processing elements...");
    let reader = ElementReader::from_path(input_path).expect("Failed to open PBF");
    let start = Instant::now();
    let mut processed = 0u64;

    reader
        .for_each(|element| {
            let db = &db;
            match element {
                osmpbf::Element::Node(node) => {
                    let tags: Vec<(String, String)> = node
                        .tags()
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                        .collect();
                    process_element(db, "node", node.id(), node.lat(), node.lon(), &tags);
                    processed += 1;
                }
                osmpbf::Element::DenseNode(node) => {
                    let tags: Vec<(String, String)> = node
                        .tags()
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                        .collect();
                    process_element(db, "node", node.id(), node.lat(), node.lon(), &tags);
                    processed += 1;
                }
                osmpbf::Element::Way(way) => {
                    let tags: Vec<(String, String)> = way
                        .tags()
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                        .collect();
                    // Compute centroid from node references
                    let refs: Vec<i64> = way.refs().collect();
                    if refs.is_empty() {
                        return;
                    }
                    let mut lat_sum = 0.0;
                    let mut lon_sum = 0.0;
                    let mut count = 0;
                    for r in &refs {
                        if let Some((lat, lon)) = node_coords.get(r) {
                            lat_sum += lat;
                            lon_sum += lon;
                            count += 1;
                        }
                    }
                    if count > 0 {
                        let lat = lat_sum / count as f64;
                        let lon = lon_sum / count as f64;
                        process_element(db, "way", way.id(), lat, lon, &tags);
                        processed += 1;
                    }
                }
                osmpbf::Element::Relation(rel) => {
                    let tags: Vec<(String, String)> = rel
                        .tags()
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                        .collect();
                    // Relations: use member nodes for centroid if available
                    // For simplicity, skip relations without geometry
                    // (most relations are administrative boundaries)
                    let _ = rel;
                    let _ = tags;
                }
                _ => {}
            }
        })
        .expect("Failed to read PBF pass 2");

    let elapsed = start.elapsed();
    println!(
        "Processed {} elements in {:.1}s",
        processed,
        elapsed.as_secs_f64()
    );
    let total = db.count().unwrap_or(0);
    println!("Places of interest in DB: {}", total);

    // Validate: only replace existing DB if new one has data
    if total > 0 {
        let _ = std::fs::remove_file(output_path);
        std::fs::rename(&tmp_path, output_path).expect("Failed to rename temp DB");
        println!("Database: {}", output_path.display());
    } else {
        eprintln!("ERROR: No places imported. Keeping existing database.");
        let _ = std::fs::remove_file(&tmp_path);
        std::process::exit(1);
    }
}
