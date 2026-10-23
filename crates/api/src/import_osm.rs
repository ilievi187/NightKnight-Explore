use nightknight_explore_api::osm_db::OsmDatabase;
use osmpbf::ElementReader;
use std::path::Path;
use std::time::Instant;

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
    if tag_map.get("tourism").is_some() {
        return Some("see".to_string());
    }
    if tag_map.get("historic").is_some() {
        return Some("history".to_string());
    }
    if tag_map.get("natural").is_some()
        || tag_map.get("waterway").is_some()
        || tag_map.get("leisure").is_some()
    {
        return Some("nature".to_string());
    }
    None
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: import-osm <input.pbf> <output.db>");
        std::process::exit(1);
    }
    let input_path = Path::new(&args[1]);
    let output_path = Path::new(&args[2]);

    // Remove existing DB
    let _ = std::fs::remove_file(output_path);

    let db = std::sync::Arc::new(OsmDatabase::open(output_path).expect("Failed to open database"));

    let reader = ElementReader::from_path(input_path).expect("Failed to open PBF");
    let start = Instant::now();
    let mut count = 0;

    let db_clone = db.clone();
    reader
        .for_each(move |element| {
            let db = &db_clone;
            match element {
                osmpbf::Element::Node(node) => {
                    let tags: Vec<(String, String)> = node
                        .tags()
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                        .collect();
                    if let Some(cat) = category_from_tags(&tags) {
                        let name = tags
                            .iter()
                            .find(|(k, _)| k == "name")
                            .map(|(_, v)| v.as_str());
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
                        let tags_json = serde_json::to_string(&tags).unwrap_or_default();

                        let _ = db.insert_place(
                            "node",
                            node.id(),
                            node.lat(),
                            node.lon(),
                            name,
                            &cat,
                            &tags_json,
                            wikidata,
                            wikipedia,
                            website,
                            phone,
                            opening_hours,
                        );
                        count += 1;
                    }
                }
                osmpbf::Element::Way(way) => {
                    // Skip ways for now - nodes are sufficient for POI discovery.
                    // Ways (buildings, areas) can be added in a future iteration
                    // by computing centroids from node references.
                    let _ = way;
                }
                _ => {}
            }
        })
        .expect("Failed to read PBF");

    let elapsed = start.elapsed();
    println!("Imported {} places in {:.1}s", count, elapsed.as_secs_f64());
    println!("Database: {}", output_path.display());
    let total = db.count().unwrap_or(0);
    println!("Total places in DB: {}", total);
}
