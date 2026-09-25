//! Compact in-memory city index over the GeoNames `cities500` dataset.
//!
//! The dataset holds ~236k rows and is kept for the whole process lifetime, so the
//! representation is chosen for footprint rather than convenience:
//!
//! * every row is a 24-byte [`Entry`] — coordinates as `f32`, feature class/code reduced
//!   to a [`kind`](KIND_PARENT), country as two ASCII bytes, and the name kept as an
//!   offset/length pair into a single shared arena (`names`),
//! * lookups use a uniform lat/lon grid with CSR cell buckets ([`CityIndex::build`])
//!   instead of a tree, because the grid is a flat `u32` array per cell: no per-node
//!   slack and no string allocations per row.
//!
//! The whole index is ~24 bytes per city plus the name arena and one `u32` per grid
//! cell, i.e. ~15 MB for `cities500` (measured: 310 MB before this module existed).

use std::fs::File;
use std::io::{BufRead, BufReader};

use log::warn;

/// Edge length of the lookup grid cells in degrees (0.25° ≈ 28 km at the equator).
///
/// Smaller cells shrink the bounding-box scan but grow [`CityIndex::cell_starts`]
/// quadratically: 0.25° costs ~4 MB of cell offsets, 0.1° already ~26 MB.
const CELL_DEG: f64 = 0.25;

/// Mean Earth radius in kilometers, matching `haversine_km`.
const EARTH_RADIUS_KM: f64 = 6371.0088;

/// Kilometers per degree of latitude on the sphere [`EARTH_RADIUS_KM`] describes.
///
/// Derived rather than hard-coded `111.32`: the candidate box must not be narrower than
/// the radius `haversine_km` measures, and a single source of truth for the sphere keeps
/// the two from drifting apart.
const KM_PER_DEG_LAT: f64 = EARTH_RADIUS_KM * std::f64::consts::PI / 180.0;

/// Slack added to the candidate-box spans, in degrees.
///
/// The box must contain every city the radius can reach, while stored coordinates are
/// `f32` (≤ 4e-6° rounding) and the span arithmetic itself rounds. The margin covers
/// both, and shifts the scanned range by at most one cell row/column.
const BOX_SLACK_DEG: f64 = 1e-4;

/// Number of leading tab-separated columns the parser keeps (population is column 14).
const KEPT_COLUMNS: usize = 15;

/// Observed average row size of `cities500` (columns beyond the ones we read dominate it);
/// used only to pre-size parser buffers so a 40 MB file does not reallocate repeatedly.
const BYTES_PER_ROW_ESTIMATE: usize = 176;

/// Average name length share of a row (column 2 is all the arena stores).
const NAME_BYTES_PER_ROW_ESTIMATE: usize = 12;

/// Usable as a parent city: `P` rows with a populated-place code (`PPL`, `PPLA*`, `PPLC`, …).
const KIND_PARENT: u8 = 0;
/// District (`P` + `PPLX`): displayed as `<district>, <parent city>`.
const KIND_DISTRICT: u8 = 1;
/// Anything else (other feature classes or codes): neither district nor parent.
const KIND_OTHER: u8 = 2;

/// One dataset row, packed to 24 bytes; `name` lives in [`CityIndex::names`].
#[derive(Clone, Copy, Debug)]
struct Entry {
    name_start: u32,
    name_len: u16,
    country: [u8; 2],
    kind: u8,
    lat: f32,
    lon: f32,
    population: u32,
}

/// A city resolved by a query, borrowed from the index it came from.
#[derive(Debug)]
pub struct City<'a> {
    name: &'a str,
    country_code: &'a str,
    kind: u8,
    population: u32,
    distance_km: f64,
}

impl City<'_> {
    /// Display name of the city as found in the dataset.
    pub fn name(&self) -> &str {
        self.name
    }

    /// ISO-3166 alpha-2 country code; empty when the dataset row had none.
    pub fn country_code(&self) -> &str {
        self.country_code
    }

    /// Whether the row is a district (`PPLX`) that resolves against a parent city.
    pub fn is_district(&self) -> bool {
        self.kind == KIND_DISTRICT
    }

    /// Population of the row (0 when the dataset has none).
    pub fn population(&self) -> u32 {
        self.population
    }

    /// Haversine distance from the queried point in kilometers.
    pub fn distance_km(&self) -> f64 {
        self.distance_km
    }
}

/// Offline index of all cities in the dataset.
pub struct CityIndex {
    entries: Vec<Entry>,
    names: String,
    /// CSR offsets into `cell_items`, one entry longer than the cell count.
    cell_starts: Vec<u32>,
    /// Entry indices grouped by grid cell.
    cell_items: Vec<u32>,
    lat_cells: usize,
    lon_cells: usize,
}

impl CityIndex {
    /// Number of indexed cities.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Parses dataset text (tab-separated GeoNames rows) into an index.
    ///
    /// Production code streams the file via [`CityIndex::load`]; this entry point exists for
    /// tests that need an index from a literal dataset.
    ///
    /// Tolerates the same malformed input as the previous loader: rows with fewer than six
    /// columns, empty names, or unparsable or non-finite coordinates are skipped with a
    /// warning, and a dataset without any valid row yields `None`.
    #[cfg(test)]
    pub fn parse(text: &str) -> Option<CityIndex> {
        let mut builder = Builder::sized_for(text.len() as u64);
        for (index, line) in text.lines().enumerate() {
            builder.push(line, index + 1);
        }
        builder.finish()
    }

    /// Loads the dataset from `path`, streaming it line by line (the file is ~40 MB).
    pub fn load(path: &str) -> Option<CityIndex> {
        let file = match File::open(path) {
            Ok(file) => file,
            Err(e) => {
                warn!(
                    "cities500 data not found at {}; city resolution disabled: {}",
                    path, e
                );
                return None;
            }
        };
        let expected_bytes = file.metadata().map(|meta| meta.len()).unwrap_or(0);

        let mut builder = Builder::sized_for(expected_bytes);
        for (index, line) in BufReader::new(file).lines().enumerate() {
            match line {
                Ok(line) => builder.push(&line, index + 1),
                Err(e) => {
                    warn!("cities500 read error at line {}: {}", index + 1, e);
                    break;
                }
            }
        }

        match builder.finish() {
            Some(index) => Some(index),
            None => {
                warn!(
                    "cities500 data at {} contained no valid entries; city resolution disabled",
                    path
                );
                None
            }
        }
    }

    /// Nearest city within `max_distance_km` (exact haversine distance).
    ///
    /// Equal distances are resolved deterministically by the higher population, then by name,
    /// so the result does not depend on dataset order (the dataset contains duplicate positions).
    pub fn nearest_within(
        &self,
        latitude: f64,
        longitude: f64,
        max_distance_km: f64,
    ) -> Option<City<'_>> {
        if !latitude.is_finite() || !longitude.is_finite() || !max_distance_km.is_finite() {
            return None;
        }
        let mut best: Option<(&Entry, f64)> = None;
        self.for_each_candidate(
            latitude,
            longitude,
            max_distance_km,
            &mut |entry, distance| {
                if distance > max_distance_km {
                    return;
                }
                let is_better = match best {
                    None => true,
                    Some((current, current_distance)) => {
                        distance < current_distance
                            || (distance == current_distance
                                && entry.population > current.population)
                            || (distance == current_distance
                                && entry.population == current.population
                                && self.name(entry) < self.name(current))
                    }
                };
                if is_better {
                    best = Some((entry, distance));
                }
            },
        );
        best.map(|(entry, distance)| self.city(entry, distance))
    }

    /// Most populous parent city within `max_distance_km`.
    ///
    /// Districts (`PPLX`) are displayed as `<district>, <parent>`, and the parent is expected to
    /// be the metro area rather than the closest village — hence population first, distance as
    /// tie-breaker (and name last, for determinism).
    pub fn most_populous_parent_within(
        &self,
        latitude: f64,
        longitude: f64,
        max_distance_km: f64,
    ) -> Option<City<'_>> {
        if !latitude.is_finite() || !longitude.is_finite() || !max_distance_km.is_finite() {
            return None;
        }
        let mut best: Option<(&Entry, f64)> = None;
        self.for_each_candidate(
            latitude,
            longitude,
            max_distance_km,
            &mut |entry, distance| {
                if distance > max_distance_km || entry.kind != KIND_PARENT {
                    return;
                }
                let is_better = match best {
                    None => true,
                    Some((current, current_distance)) => {
                        entry.population > current.population
                            || (entry.population == current.population
                                && distance < current_distance)
                            || (entry.population == current.population
                                && distance == current_distance
                                && self.name(entry) < self.name(current))
                    }
                };
                if is_better {
                    best = Some((entry, distance));
                }
            },
        );
        best.map(|(entry, distance)| self.city(entry, distance))
    }

    /// Visits every entry whose grid cell overlaps the bounding box of `max_distance_km`
    /// around the point. The box is a strict superset of the radius — both spans come from
    /// the `haversine_km` sphere plus [`BOX_SLACK_DEG`] — so callers filter by distance.
    fn for_each_candidate<'a>(
        &'a self,
        latitude: f64,
        longitude: f64,
        max_distance_km: f64,
        visit: &mut impl FnMut(&'a Entry, f64),
    ) {
        let lat_span = max_distance_km / KM_PER_DEG_LAT + BOX_SLACK_DEG;
        let min_lat = (latitude - lat_span).max(-90.0);
        let max_lat = (latitude + lat_span).min(90.0);
        if max_lat < -90.0 || min_lat > 90.0 {
            return;
        }

        // A degree of longitude covers fewer kilometers the further the point is from the
        // equator, so the box's extremal latitude — not the query's — bounds the needed span.
        let box_lat = min_lat.abs().max(max_lat.abs()).to_radians();
        let cos_lat = box_lat.cos().abs().max(1e-6);
        let lon_span =
            (max_distance_km / (KM_PER_DEG_LAT * cos_lat) + BOX_SLACK_DEG / cos_lat).min(180.0);

        let (windows, window_count) = self.longitude_cells(longitude, lon_span);
        for lat_cell in self.lat_cell(min_lat)..=self.lat_cell(max_lat) {
            let row = lat_cell * self.lon_cells;
            for (lon_from, lon_to) in windows.iter().take(window_count) {
                for cell in row + lon_from..=row + lon_to {
                    let start = self.cell_starts[cell] as usize;
                    let end = self.cell_starts[cell + 1] as usize;
                    for &index in &self.cell_items[start..end] {
                        let entry = &self.entries[index as usize];
                        let distance =
                            haversine_km(latitude, longitude, entry.lat as f64, entry.lon as f64);
                        visit(entry, distance);
                    }
                }
            }
        }
    }

    /// Longitude cell ranges covering `longitude ± lon_span`; two ranges when the box crosses
    /// the antimeridian, in which case they are clipped at ±180°.
    fn longitude_cells(&self, longitude: f64, lon_span: f64) -> ([(usize, usize); 2], usize) {
        let west = longitude - lon_span;
        let east = longitude + lon_span;
        let last = self.lon_cells - 1;
        if lon_span >= 180.0 {
            ([(0, last), (0, 0)], 1)
        } else if west < -180.0 {
            (
                [
                    (self.lon_cell(west + 360.0), last),
                    (0, self.lon_cell(east)),
                ],
                2,
            )
        } else if east > 180.0 {
            (
                [
                    (self.lon_cell(west), last),
                    (0, self.lon_cell(east - 360.0)),
                ],
                2,
            )
        } else {
            ([(self.lon_cell(west), self.lon_cell(east)), (0, 0)], 1)
        }
    }

    fn lat_cell(&self, latitude: f64) -> usize {
        (((latitude + 90.0) / CELL_DEG).floor() as i64).clamp(0, self.lat_cells as i64 - 1) as usize
    }

    fn lon_cell(&self, longitude: f64) -> usize {
        (((longitude + 180.0) / CELL_DEG).floor() as i64).clamp(0, self.lon_cells as i64 - 1)
            as usize
    }

    fn name(&self, entry: &Entry) -> &str {
        let start = entry.name_start as usize;
        &self.names[start..start + entry.name_len as usize]
    }

    fn country_code<'a>(&self, entry: &'a Entry) -> &'a str {
        let bytes = &entry.country;
        let len = if bytes[0] == 0 {
            0
        } else if bytes[1] == 0 {
            1
        } else {
            2
        };
        std::str::from_utf8(&bytes[..len]).unwrap_or("")
    }

    fn city<'a>(&'a self, entry: &'a Entry, distance_km: f64) -> City<'a> {
        City {
            name: self.name(entry),
            country_code: self.country_code(entry),
            kind: entry.kind,
            population: entry.population,
            distance_km,
        }
    }

    /// Groups all entries into their grid cell (counting sort over cells).
    fn build(entries: Vec<Entry>, names: String) -> CityIndex {
        let lat_cells = (180.0 / CELL_DEG).ceil() as usize;
        let lon_cells = (360.0 / CELL_DEG).ceil() as usize;
        let cell_count = lat_cells * lon_cells;
        let mut index = CityIndex {
            entries,
            names,
            cell_starts: vec![0; cell_count + 1],
            cell_items: Vec::new(),
            lat_cells,
            lon_cells,
        };
        index.cell_items = vec![0; index.entries.len()];

        let mut cell_of = Vec::with_capacity(index.entries.len());
        for entry in &index.entries {
            let cell =
                index.lat_cell(entry.lat as f64) * lon_cells + index.lon_cell(entry.lon as f64);
            index.cell_starts[cell + 1] += 1;
            cell_of.push(cell);
        }
        for cell in 0..cell_count {
            index.cell_starts[cell + 1] += index.cell_starts[cell];
        }

        let mut cursor = index.cell_starts.clone();
        for (entry_index, &cell) in cell_of.iter().enumerate() {
            index.cell_items[cursor[cell] as usize] = entry_index as u32;
            cursor[cell] += 1;
        }

        index
    }
}

/// Accumulates parsed rows; kept separate from [`CityIndex`] so `parse` can stay pure.
struct Builder {
    entries: Vec<Entry>,
    names: String,
}

impl Builder {
    fn sized_for(bytes: u64) -> Self {
        let rows = (bytes as usize / BYTES_PER_ROW_ESTIMATE).max(1);
        Builder {
            entries: Vec::with_capacity(rows),
            names: String::with_capacity(rows * NAME_BYTES_PER_ROW_ESTIMATE),
        }
    }

    /// Parses one dataset row; malformed rows are skipped with a warning.
    fn push(&mut self, line: &str, line_number: usize) {
        if line.trim().is_empty() {
            return;
        }
        let (columns, column_count) = split_row(line);
        if column_count < 6 {
            warn!(
                "skipping cities500 line {}: expected >=6 cols, got {}",
                line_number, column_count
            );
            return;
        }

        let name = columns[1].trim();
        if name.is_empty() {
            warn!("skipping cities500 line {}: empty name", line_number);
            return;
        }
        if name.len() > u16::MAX as usize {
            warn!(
                "skipping cities500 line {}: name longer than {} bytes",
                line_number,
                u16::MAX
            );
            return;
        }

        let Ok(latitude) = columns[4].parse::<f64>() else {
            warn!(
                "skipping cities500 line {}: invalid latitude '{}'",
                line_number, columns[4]
            );
            return;
        };
        if !latitude.is_finite() {
            warn!(
                "skipping cities500 line {}: non-finite latitude '{}'",
                line_number, columns[4]
            );
            return;
        }
        let Ok(longitude) = columns[5].parse::<f64>() else {
            warn!(
                "skipping cities500 line {}: invalid longitude '{}'",
                line_number, columns[5]
            );
            return;
        };
        if !longitude.is_finite() {
            warn!(
                "skipping cities500 line {}: non-finite longitude '{}'",
                line_number, columns[5]
            );
            return;
        }

        let mut country = [0u8; 2];
        for (slot, byte) in country.iter_mut().zip(columns[8].trim().bytes()) {
            *slot = byte.to_ascii_uppercase();
        }
        let population = columns[14]
            .trim()
            .parse::<i64>()
            .map(|value| value.max(0) as u32)
            .unwrap_or(0);

        let name_start = self.names.len() as u32;
        self.names.push_str(name);
        self.entries.push(Entry {
            name_start,
            name_len: name.len() as u16,
            country,
            kind: kind_of(columns[6].trim(), columns[7].trim()),
            lat: latitude as f32,
            lon: longitude as f32,
            population,
        });
    }

    fn finish(mut self) -> Option<CityIndex> {
        if self.entries.is_empty() {
            return None;
        }
        self.entries.shrink_to_fit();
        self.names.shrink_to_fit();
        Some(CityIndex::build(self.entries, self.names))
    }
}

/// Splits the leading `KEPT_COLUMNS` columns and reports how many the row has in total.
/// Avoids one heap allocation per dataset row.
fn split_row(line: &str) -> ([&str; KEPT_COLUMNS], usize) {
    let mut columns = [""; KEPT_COLUMNS];
    let mut count = 0;
    for column in line.split('\t') {
        if count < KEPT_COLUMNS {
            columns[count] = column;
        }
        count += 1;
    }
    (columns, count)
}

/// Reduces GeoNames feature class/code to the three kinds city resolution distinguishes.
fn kind_of(feature_class: &str, feature_code: &str) -> u8 {
    if feature_class != "P" {
        return KIND_OTHER;
    }
    match feature_code {
        "PPL" | "PPLA" | "PPLA2" | "PPLA3" | "PPLA4" | "PPLC" | "PPLG" | "PPLS" => KIND_PARENT,
        "PPLX" => KIND_DISTRICT,
        _ => KIND_OTHER,
    }
}

fn haversine_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let dlat = (lat2 - lat1).to_radians();
    let dlon = (lon2 - lon1).to_radians();
    let a = (dlat / 2.0).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos() * (dlon / 2.0).sin().powi(2);
    let c = 2.0 * a.clamp(0.0, 1.0).sqrt().asin();
    EARTH_RADIUS_KM * c
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a dataset row with population at column 14, mirroring the GeoNames layout.
    fn row(
        name: &str,
        latitude: &str,
        longitude: &str,
        feature_class: &str,
        feature_code: &str,
        country: &str,
        population: &str,
    ) -> String {
        let mut columns = [""; KEPT_COLUMNS];
        columns[0] = "1";
        columns[1] = name;
        columns[2] = name;
        columns[3] = name;
        columns[4] = latitude;
        columns[5] = longitude;
        columns[6] = feature_class;
        columns[7] = feature_code;
        columns[8] = country;
        columns[14] = population;
        columns.join("\t")
    }

    fn city(name: &str, latitude: &str, longitude: &str, code: &str, population: &str) -> String {
        row(name, latitude, longitude, "P", code, "DE", population)
    }

    fn index_of(rows: &[String]) -> CityIndex {
        CityIndex::parse(&rows.join("\n")).expect("test dataset should contain valid rows")
    }

    #[test]
    fn given_city_within_radius_when_resolving_then_nearest_wins() {
        // GIVEN two cities, the farther one much more populous
        let index = index_of(&[
            city("Near", "50.0", "7.0", "PPL", "1000"),
            city("Far", "50.3", "7.0", "PPL", "999999"),
        ]);

        // WHEN resolving a point on top of the nearer city
        let resolved = index.nearest_within(50.0, 7.0, 50.0);

        // THEN the nearest city wins — population only breaks ties
        assert_eq!(
            resolved.map(|c| c.name().to_string()),
            Some("Near".to_string())
        );
    }

    #[test]
    fn given_only_city_beyond_radius_when_resolving_then_none() {
        // GIVEN a single city ~111 km north of the query point
        let index = index_of(&[city("Far", "51.0", "7.0", "PPL", "1000")]);

        // WHEN resolving with a 50 km radius
        let resolved = index.nearest_within(50.0, 7.0, 50.0);

        // THEN nothing is returned (ocean/desert case)
        assert!(
            resolved.is_none(),
            "expected None, got {:?}",
            resolved.map(|c| c.name().to_string())
        );
    }

    #[test]
    fn given_two_cities_at_same_position_when_resolving_then_more_populous_wins() {
        // GIVEN the dataset's duplicate positions (e.g. Zettingen 258 / Gamlen 558)
        let index = index_of(&[
            city("Small", "50.0", "7.0", "PPLA4", "258"),
            city("Big", "50.0", "7.0", "PPLA4", "558"),
        ]);

        // WHEN resolving that position
        let resolved = index.nearest_within(50.0, 7.0, 50.0);

        // THEN the more populous row wins, independent of dataset order
        assert_eq!(
            resolved.map(|c| c.name().to_string()),
            Some("Big".to_string())
        );
    }

    #[test]
    fn given_query_across_antimeridian_when_resolving_then_city_found() {
        // GIVEN a city just east of the date line
        let index = index_of(&[city("Dateline", "0.0", "179.95", "PPL", "600")]);

        // WHEN resolving a point just west of the date line (~11 km away)
        let resolved = index.nearest_within(0.0, -179.95, 50.0);

        // THEN the city is found across the wrap
        assert_eq!(
            resolved.map(|c| c.name().to_string()),
            Some("Dateline".to_string())
        );
    }

    #[test]
    fn given_city_on_grid_cell_boundary_when_resolving_then_found() {
        // GIVEN a city exactly on a 0.25° cell corner
        let index = index_of(&[city("Corner", "0.25", "-0.25", "PPL", "1000")]);

        // WHEN resolving the corner and a point just across the cell edges
        // THEN both find the city (the bounding box spans neighbouring cells)
        for (latitude, longitude) in [(0.25, -0.25), (0.2499, -0.2501), (0.2501, -0.2499)] {
            let resolved = index.nearest_within(latitude, longitude, 50.0);
            assert_eq!(
                resolved.map(|c| c.name().to_string()),
                Some("Corner".to_string()),
                "expected Corner at {}, {}",
                latitude,
                longitude
            );
        }
    }

    #[test]
    fn given_city_at_radius_boundary_to_the_north_when_resolving_then_found() {
        // GIVEN a city 49.999 km due north of the query — 1 m inside a 50 km radius, and in
        // the latitude cell just above the query's latitude span
        let query_latitude = -26.94923;
        let offset_deg = (49.999 / EARTH_RADIUS_KM).to_degrees();
        let index = index_of(&[city(
            "Boundary",
            &(query_latitude + offset_deg).to_string(),
            "17.1599",
            "PPL",
            "1000",
        )]);

        // WHEN resolving the query point with a 50 km radius
        let resolved = index
            .nearest_within(query_latitude, 17.1599, 50.0)
            .expect("a city 49.999 km away lies within a 50 km radius");

        // THEN the scan reaches it — the box is a strict superset of the radius
        assert_eq!(resolved.name(), "Boundary");
        assert!(resolved.distance_km() <= 50.0);
    }

    #[test]
    fn given_city_at_radius_boundary_at_higher_latitude_when_resolving_then_found() {
        // GIVEN a city 2 927 km east of the query but 20° further north, where a longitude
        // span normalised by the query's own latitude ends ~6° short of it
        let index = index_of(&[city("Far", "80.0", "60.0", "PPL", "1000")]);

        // WHEN resolving with a 3 000 km radius
        let resolved = index
            .nearest_within(60.0, 0.0, 3000.0)
            .expect("a city 2 927 km away lies within a 3 000 km radius");

        // THEN the span follows the box's highest latitude, not the query latitude
        assert_eq!(resolved.name(), "Far");
        assert!(resolved.distance_km() <= 3000.0);
    }

    #[test]
    fn given_query_near_pole_when_resolving_then_city_found_and_rest_is_none() {
        // GIVEN a city at the North Cape
        let index = index_of(&[city("Nordkapp", "71.17", "25.78", "PPL", "1000")]);

        // WHEN resolving its position and the poles themselves
        // THEN the city is found and polar queries stay finite (longitude span is clamped)
        assert_eq!(
            index
                .nearest_within(71.17, 25.78, 50.0)
                .map(|c| c.name().to_string()),
            Some("Nordkapp".to_string())
        );
        assert!(index.nearest_within(90.0, 0.0, 50.0).is_none());
        assert!(index.nearest_within(90.0, 180.0, 50.0).is_none());
    }

    #[test]
    fn given_district_when_asking_for_parent_then_most_populous_within_radius_wins() {
        // GIVEN a district with a small town 5.5 km and a large city 22 km away
        let index = index_of(&[
            city("Dorf", "50.0", "7.0", "PPLX", "100"),
            city("SmallTown", "50.05", "7.0", "PPL", "3000"),
            city("BigCity", "50.2", "7.0", "PPLC", "300000"),
        ]);

        // WHEN resolving the nearest city and its parent
        let nearest = index
            .nearest_within(50.0, 7.0, 50.0)
            .expect("district should resolve");
        let parent = index
            .most_populous_parent_within(50.0, 7.0, 30.0)
            .expect("parent should resolve");

        // THEN the district is detected and the metro city wins over the closer village
        assert!(nearest.is_district(), "expected PPLX row to be a district");
        assert_eq!(parent.name(), "BigCity");
    }

    #[test]
    fn given_parent_only_beyond_radius_when_asking_for_parent_then_none() {
        // GIVEN a district whose nearest parent city is ~55 km away
        let index = index_of(&[
            city("Dorf", "50.0", "7.0", "PPLX", "100"),
            city("FarCity", "50.5", "7.0", "PPL", "50000"),
        ]);

        // WHEN asking for a parent within 30 km
        let parent = index.most_populous_parent_within(50.0, 7.0, 30.0);

        // THEN no parent is reported (caller falls back to the district alone)
        assert!(
            parent.is_none(),
            "expected None, got {:?}",
            parent.map(|c| c.name().to_string())
        );
    }

    #[test]
    fn given_only_districts_when_asking_for_parent_then_none() {
        // GIVEN dataset rows that are all districts, none of them a parent city
        let index = index_of(&[
            city("Dorf", "50.0", "7.0", "PPLX", "100"),
            city("OtherDorf", "50.05", "7.0", "PPLX", "500"),
        ]);

        // WHEN asking for a parent within 30 km
        let parent = index.most_populous_parent_within(50.0, 7.0, 30.0);

        // THEN districts are never used as parents
        assert!(
            parent.is_none(),
            "expected None, got {:?}",
            parent.map(|c| c.name().to_string())
        );
    }

    #[test]
    fn given_non_populated_place_rows_when_parsing_then_not_usable_as_city_or_parent() {
        // GIVEN rows of other feature classes and unmapped P codes
        let index = index_of(&[
            row("Mountain", "50.0", "7.0", "T", "MT", "DE", "0"),
            row("Hamlet", "50.01", "7.0", "P", "PPLL", "DE", "900"),
        ]);

        // WHEN resolving and asking for a parent
        // THEN neither row is a parent city (but both are indexed)
        assert_eq!(index.len(), 2);
        assert!(index.most_populous_parent_within(50.0, 7.0, 30.0).is_none());
    }

    #[test]
    fn given_dataset_row_when_resolving_then_country_and_population_are_reported() {
        // GIVEN a row with a lowercase country code
        let index = index_of(&[row("Köln", "50.9", "6.96", "P", "PPLA", "de", "1080000")]);

        // WHEN resolving it
        let resolved = index
            .nearest_within(50.9, 6.96, 50.0)
            .expect("city should resolve");

        // THEN the code is uppercased and the population is kept
        assert_eq!(resolved.country_code(), "DE");
        assert_eq!(resolved.population(), 1_080_000);
        assert!(!resolved.is_district());
    }

    #[test]
    fn given_malformed_rows_when_parsing_then_only_valid_rows_are_indexed() {
        // GIVEN a mix of valid rows and malformed ones
        let index = index_of(&[
            city("Valid", "50.0", "7.0", "PPL", "1000"),
            "junk\tline".to_string(),
            city("EmptyName", "50.1", "7.0", "PPL", "1").replace("EmptyName", "  "),
            city("BadLat", "not-a-latitude", "7.0", "PPL", "1"),
            city("BadLon", "50.2", "not-a-longitude", "PPL", "1"),
            city("Other", "51.0", "7.0", "PPL", "1"),
        ]);

        // WHEN/THEN only the two valid rows are indexed
        assert_eq!(index.len(), 2);
        assert!(index.nearest_within(50.0, 7.0, 50.0).is_some());
        assert!(index.nearest_within(51.0, 7.0, 50.0).is_some());
    }

    #[test]
    fn given_non_finite_coordinates_when_parsing_then_row_is_skipped() {
        // GIVEN a NaN-latitude row ahead of a valid one, plus an infinite-longitude row
        let index = index_of(&[
            city("NaNLat", "NaN", "7.0", "PPL", "999999"),
            city("Valid", "50.0", "7.0", "PPL", "1000"),
            city("InfLon", "50.1", "inf", "PPL", "1"),
        ]);

        // WHEN resolving the valid row's position
        // THEN the non-finite rows are not indexed (a NaN entry wins every distance
        // comparison while `best` is empty and would be returned with a NaN distance)
        assert_eq!(index.len(), 1);
        let resolved = index
            .nearest_within(50.0, 7.0, 50.0)
            .expect("the valid row should resolve");
        assert_eq!(resolved.name(), "Valid");
        assert!(resolved.distance_km().is_finite());
    }

    #[test]
    fn given_row_without_extended_columns_when_parsing_then_population_defaults_to_zero() {
        // GIVEN a six-column row (no feature class/code/country/population)
        let index = index_of(&["1\tTiny\tTiny\tTiny\t50.0\t7.0".to_string()]);

        // WHEN resolving it
        let resolved = index
            .nearest_within(50.0, 7.0, 50.0)
            .expect("city should resolve");

        // THEN it is indexed with empty country and population 0
        assert_eq!(resolved.population(), 0);
        assert_eq!(resolved.country_code(), "");
    }

    #[test]
    fn given_no_valid_rows_when_parsing_then_none() {
        // GIVEN empty text and text without any usable row
        // WHEN/THEN the parser reports "no index"
        assert!(CityIndex::parse("").is_none());
        assert!(CityIndex::parse("\n\t\n").is_none());
        assert!(CityIndex::parse("only\tone").is_none());
    }

    #[test]
    fn given_invalid_query_coordinates_when_resolving_then_none() {
        // GIVEN an index with one city
        let index = index_of(&[city("Koblenz", "50.35", "7.57", "PPL", "1000")]);

        // WHEN querying with out-of-range or non-finite coordinates
        // THEN nothing is returned and no panic occurs
        assert!(index.nearest_within(200.0, 7.57, 50.0).is_none());
        assert!(index.nearest_within(f64::NAN, 7.57, 50.0).is_none());
        assert!(index.nearest_within(50.35, f64::INFINITY, 50.0).is_none());
    }

    #[test]
    fn given_grid_of_cities_when_resolving_each_then_each_city_is_found() {
        // GIVEN 900 cities spread over the globe in a 0.4° grid (~44 km apart)
        let mut rows = Vec::new();
        let mut positions = Vec::new();
        for lat_step in 0..30 {
            for lon_step in 0..30 {
                let latitude = -50.0 + lat_step as f64 * 0.4;
                let longitude = -50.0 + lon_step as f64 * 0.4;
                let name = format!("City-{lat_step}-{lon_step}");
                rows.push(city(
                    &name,
                    &latitude.to_string(),
                    &longitude.to_string(),
                    "PPL",
                    "1000",
                ));
                positions.push((latitude, longitude, name));
            }
        }
        let index = index_of(&rows);

        // WHEN resolving every position
        // THEN each city is found (catches cell-assignment and bucket-fill mistakes)
        assert_eq!(index.len(), positions.len());
        for (latitude, longitude, name) in positions {
            let resolved = index
                .nearest_within(latitude, longitude, 50.0)
                .unwrap_or_else(|| panic!("no city resolved at {latitude}, {longitude}"));
            assert_eq!(
                resolved.name(),
                name,
                "wrong city at {latitude}, {longitude}"
            );
        }
    }

    #[test]
    fn given_index_when_measuring_entry_then_compact() {
        // GIVEN/WHEN the index stores one row per city
        // THEN a row stays at 24 bytes — the memory contract of this module
        // (inline Strings or f64 coordinates would double the footprint per row)
        assert!(
            std::mem::size_of::<Entry>() <= 24,
            "Entry grew to {} bytes",
            std::mem::size_of::<Entry>()
        );
    }

    #[test]
    fn given_loaded_dataset_when_querying_berlin_then_district_chain_resolves_to_berlin() {
        // GIVEN the real dataset is available (CI downloads it, locally it is opt-in)
        let Ok(path) = std::env::var("CITIES500_PATH") else {
            return;
        };
        let Some(index) = CityIndex::load(&path) else {
            return;
        };

        // WHEN resolving a Berlin coordinate
        let resolved = index
            .nearest_within(52.5200, 13.4050, 50.0)
            .expect("Berlin should resolve");
        assert!(
            index.len() > 100_000,
            "expected a full dataset, got {}",
            index.len()
        );
        assert_eq!(resolved.country_code(), "DE");

        // THEN it is Berlin itself, or a district row that resolves against Berlin
        let label = if resolved.is_district() {
            let parent = index
                .most_populous_parent_within(52.5200, 13.4050, 30.0)
                .expect("district row should have a parent");
            format!("{}, {}", resolved.name(), parent.name())
        } else {
            resolved.name().to_string()
        };
        assert!(
            label.contains("Berlin"),
            "expected Berlin (or a district of it) for 52.52, 13.405, got '{label}'"
        );
    }
}
