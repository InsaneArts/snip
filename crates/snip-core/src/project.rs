//! The project file (ANN-15, ANN-16; architecture §5.3): `.snip`, a zip with
//! nothing compressed (the images already are): manifest.json, source.png
//! (the original pixels), document.json (the objects), preview.png (the
//! flattened image), thumb.webp, and the files the document draws from
//! (assets/<name>: added images, a rotated base).

use std::io::{Cursor, Read, Write};
use std::path::Path;

#[derive(Debug)]
pub struct Project {
    pub source: Vec<u8>,
    pub document: String,
    pub preview: Vec<u8>,
    pub assets: Vec<(String, Vec<u8>)>,
}

/// The format's version: readers refuse newer ones they don't know.
pub const VERSION: u32 = 1;

/// The project as zip bytes; `thumb` is a small WebP of the preview.
pub fn to_bytes(p: &Project, thumb: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let stored = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let manifest = serde_json::json!({ "format": "snip-project", "version": VERSION, "app": env!("CARGO_PKG_VERSION") });
    let mut put = |name: &str, data: &[u8]| -> std::io::Result<()> {
        z.start_file(name, stored).map_err(std::io::Error::other)?;
        z.write_all(data)
    };
    put("manifest.json", manifest.to_string().as_bytes())?;
    put("source.png", &p.source)?;
    put("document.json", p.document.as_bytes())?;
    put("preview.png", &p.preview)?;
    put("thumb.webp", thumb)?;
    for (name, data) in &p.assets {
        put(&format!("assets/{name}"), data)?;
    }
    Ok(z.finish().map_err(std::io::Error::other)?.into_inner())
}

/// A project file's parts (the thumbnail is left out).
pub fn read(path: &Path) -> std::io::Result<Project> {
    read_from(std::fs::File::open(path)?, LARGEST)
}

/// No file in a project is bigger (a shared one may be a zip bomb).
const LARGEST: u64 = 512 << 20;

/// `read`, from any zip, refusing a file in it bigger than `largest` bytes
/// (whatever the zip says its size is) and more than 1000 assets.
fn read_from(r: impl Read + std::io::Seek, largest: u64) -> std::io::Result<Project> {
    let mut z = zip::ZipArchive::new(r).map_err(std::io::Error::other)?;
    let names: Vec<String> = z.file_names().filter_map(|n| n.strip_prefix("assets/")).filter(|n| !n.is_empty() && !n.contains('/')).map(String::from).collect();
    if names.len() > 1000 {
        return Err(std::io::Error::other("not a Snip project: too many files"));
    }
    let mut get = |name: &str| -> std::io::Result<Vec<u8>> {
        let f = z.by_name(name).map_err(|_| std::io::Error::other(format!("not a Snip project: no {name}")))?;
        let mut v = Vec::new();
        f.take(largest + 1).read_to_end(&mut v)?;
        if v.len() as u64 > largest {
            return Err(std::io::Error::other(format!("not a Snip project: {name} is too big")));
        }
        Ok(v)
    };
    let manifest: serde_json::Value = serde_json::from_slice(&get("manifest.json")?).map_err(std::io::Error::other)?;
    if manifest["format"] != "snip-project" || manifest["version"].as_u64().unwrap_or(u64::MAX) > VERSION as u64 {
        return Err(std::io::Error::other("this project was made by a newer Snip"));
    }
    let (source, document, preview) = (get("source.png")?, String::from_utf8_lossy(&get("document.json")?).into_owned(), get("preview.png")?);
    let mut assets = Vec::new();
    for n in names {
        assets.push((n.clone(), get(&format!("assets/{n}"))?));
    }
    Ok(Project { source, document, preview, assets })
}

/// The files a document draws from besides its source: added images and a
/// rotated, flipped or resized base.
pub fn document_files(document: &str) -> Vec<String> {
    let Ok(doc) = serde_json::from_str::<serde_json::Value>(document) else { return vec![] };
    let mut v: Vec<String> = doc["items"]
        .as_array()
        .map(|a| a.iter().filter(|o| o["kind"] == "image").filter_map(|o| o["src"].as_str().map(String::from)).collect())
        .unwrap_or_default();
    if let Some(b) = doc["base"].as_str() {
        v.push(b.into());
    }
    v.retain(|n| !n.is_empty() && !n.contains('/') && n != "..");
    v.sort();
    v.dedup();
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_project_comes_back_as_it_went() {
        let p = Project {
            source: b"\x89PNG source".to_vec(),
            document: r#"{"v":1,"items":[{"kind":"image","src":"img-1.png"}],"base":"base-2.png"}"#.into(),
            preview: b"\x89PNG preview".to_vec(),
            assets: vec![("img-1.png".into(), b"one".to_vec()), ("base-2.png".into(), b"two".to_vec())],
        };
        let dir = std::env::temp_dir().join(format!("snip-project-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a.snip");
        std::fs::write(&path, to_bytes(&p, b"RIFF thumb").unwrap()).unwrap();
        let back = read(&path).unwrap();
        assert_eq!((back.source, back.document, back.preview), (p.source, p.document.clone(), p.preview));
        assert_eq!(back.assets, p.assets);
        assert_eq!(document_files(&p.document), vec!["base-2.png".to_string(), "img-1.png".to_string()]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn other_files_and_newer_projects_are_refused() {
        let dir = std::env::temp_dir().join(format!("snip-project-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("x.snip");
        std::fs::write(&path, b"not a zip").unwrap();
        assert!(read(&path).is_err());
        let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
        z.start_file("manifest.json", zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored)).unwrap();
        z.write_all(br#"{"format":"snip-project","version":99}"#).unwrap();
        std::fs::write(&path, z.finish().unwrap().into_inner()).unwrap();
        assert!(read(&path).unwrap_err().to_string().contains("newer"));
        assert!(document_files(r#"{"items":[{"kind":"image","src":"../etc/passwd"}]}"#).is_empty(), "no paths out of the folder");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_project_too_big_to_be_one_is_refused_not_read() {
        // a file in it bigger than any project's (a zip bomb, shared): refused before it fills memory
        let p = Project { source: vec![7; 4096], document: "{}".into(), preview: b"p".to_vec(), assets: vec![] };
        let bytes = to_bytes(&p, b"t").unwrap();
        assert!(read_from(Cursor::new(bytes.clone()), 1 << 20).is_ok());
        assert!(read_from(Cursor::new(bytes.clone()), 1024).unwrap_err().to_string().contains("too big"));
        crate::fuzz::run(&[&bytes], 3000, |b| {
            let _ = read_from(Cursor::new(b), 1 << 20);
        });
    }
}
