use map_model::Map;
use std::fs::File;
use std::io::Write;

// Test the map pipeline by importing simple, handcrafted .osm files, then emitting goldenfiles
// that summarize part of the generated map. Keep the goldenfiles under version control to notice
// when they change. The goldenfiles (and changes to them) themselves aren't easy to understand,
// but the test maps are.
fn main() -> Result<(), std::io::Error> {
    // TODO It's kind of a hack to reference the crate's directory relative to the data dir.
    for path in abstutil::list_dir(abstutil::path("../map_tests/input")) {
        let map = import_map(path);
        // Enable to debug the result wih the normal GUI
        if false {
            map.save();
        }
        println!("Producing goldenfiles for {}", map.get_name());
        dump_turn_goldenfile(&map)?;
    }
    Ok(())
}

// Run the contents of a .osm through the full map importer with default options.
fn import_map(path: String) -> Map {
    let mut timer = abstutil::Timer::new("convert synthetic map");
    let raw = convert_osm::convert(
        convert_osm::Options {
            name: abstutil::basename(&path),
            osm_input: path,
            city_name: "oneshot".to_string(),
            clip: None,
            map_config: map_model::MapConfig {
                driving_side: map_model::DrivingSide::Right,
                bikes_can_use_bus_lanes: true,
            },
            onstreet_parking: convert_osm::OnstreetParking::JustOSM,
            public_offstreet_parking: convert_osm::PublicOffstreetParking::None,
            private_offstreet_parking: convert_osm::PrivateOffstreetParking::FixedPerBldg(0),
            elevation: None,
            include_railroads: true,
        },
        &mut timer,
    );
    let map = Map::create_from_raw(raw, true, &mut timer);
    map
}

// Verify what turns are generated by writing (from lane, to lane, turn type).
fn dump_turn_goldenfile(map: &Map) -> Result<(), std::io::Error> {
    let path = abstutil::path(format!("../map_tests/goldenfiles/{}.txt", map.get_name()));
    let mut f = File::create(path)?;
    for (_, t) in map.all_turns() {
        writeln!(f, "{} is a {:?}", t.id, t.turn_type)?;
    }
    Ok(())
}
