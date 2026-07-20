//! Build-time gazetteer preparation: fetch GeoNames `cities500` (+ admin1 and
//! country display names), strip to the columns the app needs, and write a
//! compressed TSV into OUT_DIR for `include_bytes!`.
//!
//! Network is touched at most once: sources are cached under
//! `~/.cache/astro-geonames/` (override with `ASTRO_GEONAMES_DIR`, which is
//! also the escape hatch for fully-offline builds — place `cities500.zip` or
//! `cities500.txt`, `admin1CodesASCII.txt`, and `countryInfo.txt` there).
//!
//! GeoNames data is CC-BY 4.0 — attribution lives in the README.

use std::collections::HashMap;
use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::PathBuf;

const BASE: &str = "https://download.geonames.org/export/dump";

fn main() {
    println!("cargo:rerun-if-env-changed=ASTRO_GEONAMES_DIR");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("places.tsv.gz");
    // A cached output is only valid when no override is in play — with
    // ASTRO_GEONAMES_DIR set, always regenerate from the override sources
    // (this script only reruns when the env value or the sources change).
    if std::env::var("ASTRO_GEONAMES_DIR").is_err() && out.exists() {
        return;
    }

    let cities = source_text("cities500.txt", Some("cities500.zip"));
    let admin1 = source_text("admin1CodesASCII.txt", None);
    let countries = source_text("countryInfo.txt", None);

    // admin1CodesASCII: "DE.16\tThuringia\tThuringia\t<geonameid>"
    let admin1_names: HashMap<&str, &str> = admin1
        .lines()
        .filter_map(|l| {
            let mut f = l.split('\t');
            Some((f.next()?, f.next()?))
        })
        .collect();
    // countryInfo: comment lines start with '#'; ISO code is col 0, name col 4.
    let country_names: HashMap<&str, &str> = countries
        .lines()
        .filter(|l| !l.starts_with('#'))
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            Some((*f.first()?, *f.get(4)?))
        })
        .collect();

    // Main geoname table columns (readme.txt): 0 geonameid, 1 name, 2 asciiname,
    // 3 alternatenames, 4 lat, 5 lon, 6 fclass, 7 fcode, 8 country code, 9 cc2,
    // 10 admin1 code, 11-13 admin2-4, 14 population, 15 elevation, 16 dem,
    // 17 timezone, 18 modification date.
    let mut rows: Vec<(u64, String)> = cities
        .lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            if f.len() < 18 {
                return None;
            }
            let cc = f[8];
            let admin_key = format!("{cc}.{}", f[10]);
            let admin_name = admin1_names.get(admin_key.as_str()).copied().unwrap_or("");
            let country = country_names.get(cc).copied().unwrap_or(cc);
            let pop: u64 = f[14].parse().unwrap_or(0);
            let row = format!(
                "{id}\t{name}\t{ascii}\t{lat}\t{lon}\t{admin}\t{country}\t{cc}\t{pop}\t{tz}",
                id = f[0], name = f[1], ascii = f[2], lat = f[4], lon = f[5],
                admin = admin_name, country = country, cc = cc, pop = pop, tz = f[17],
            );
            Some((pop, row))
        })
        .collect();
    rows.sort_by_key(|(pop, _)| std::cmp::Reverse(*pop));

    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    for (_, row) in &rows {
        enc.write_all(row.as_bytes()).unwrap();
        enc.write_all(b"\n").unwrap();
    }
    let gz = enc.finish().unwrap();
    fs::write(&out, &gz).unwrap();
    println!(
        "cargo:warning=gazetteer: embedded {} places ({} KB compressed)",
        rows.len(),
        gz.len() / 1024
    );
}

/// Return the text of a GeoNames source file: from `ASTRO_GEONAMES_DIR`, then
/// the local cache, then the network (cached for subsequent builds).
fn source_text(txt_name: &str, zip_name: Option<&str>) -> String {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Ok(d) = std::env::var("ASTRO_GEONAMES_DIR") {
        dirs.push(PathBuf::from(d));
    }
    let cache = std::env::var("HOME")
        .map(|h| PathBuf::from(h).join(".cache/astro-geonames"))
        .unwrap_or_else(|_| PathBuf::from(".astro-geonames-cache"));
    dirs.push(cache.clone());

    let override_active = std::env::var("ASTRO_GEONAMES_DIR").is_ok();
    for (i, dir) in dirs.iter().enumerate() {
        let from_override = override_active && i == 0;
        let txt = dir.join(txt_name);
        if txt.exists() {
            if from_override {
                println!("cargo:rerun-if-changed={}", txt.display());
            }
            return fs::read_to_string(&txt).unwrap_or_else(|e| panic!("read {txt:?}: {e}"));
        }
        if let Some(z) = zip_name {
            let zip_path = dir.join(z);
            if zip_path.exists() {
                if from_override {
                    println!("cargo:rerun-if-changed={}", zip_path.display());
                }
                return unzip_one(&fs::read(&zip_path).unwrap(), txt_name);
            }
        }
    }

    // Fetch (once) into the cache.
    let fetch_name = zip_name.unwrap_or(txt_name);
    let url = format!("{BASE}/{fetch_name}");
    println!("cargo:warning=gazetteer: downloading {url} (first build only)");
    let bytes = download(&url).unwrap_or_else(|e| {
        panic!(
            "failed to download {url}: {e}\n\
             For offline builds, put {fetch_name} into a directory and set ASTRO_GEONAMES_DIR to it."
        )
    });
    fs::create_dir_all(&cache).ok();
    fs::write(cache.join(fetch_name), &bytes).ok();
    match zip_name {
        Some(_) => unzip_one(&bytes, txt_name),
        None => String::from_utf8(bytes).expect("geonames file is not utf-8"),
    }
}

fn download(url: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let resp = ureq::get(url).call()?;
    let bytes = resp
        .into_body()
        .with_config()
        .limit(200 * 1024 * 1024)
        .read_to_vec()?;
    Ok(bytes)
}

fn unzip_one(zip_bytes: &[u8], member: &str) -> String {
    let mut archive =
        zip::ZipArchive::new(Cursor::new(zip_bytes)).expect("invalid geonames zip");
    let mut file = archive.by_name(member).expect("member missing from zip");
    let mut s = String::new();
    file.read_to_string(&mut s).expect("zip member not utf-8");
    s
}
