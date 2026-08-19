//! The picture beside an application's name.
//!
//! An icon is decoration. Every row renders without one, every function here
//! returns `Option`, and nothing fails a request because a bundle keeps its
//! artwork somewhere this cannot read.
//!
//! # Why the container is parsed rather than converted
//!
//! macOS ships `sips` and `iconutil`, and either would turn an `.icns` into a
//! PNG in one line. Both are a subprocess per icon, and the dashboard asks for
//! eight at once — eight process spawns to draw a list. An `.icns` is a flat
//! sequence of tagged chunks and the modern ones hold PNG bytes verbatim, so
//! the file is read, the chunk is found, and the bytes are already the answer.
//!
//! What this does not handle: bundles whose icon lives in an asset catalog
//! (`Assets.car`), which is a compiled format with no published layout. Those
//! report no icon, which is the decoration case again.

use std::path::{Path, PathBuf};

/// Encoded bytes an icon may occupy before it is refused.
///
/// The constraint is the IPC payload rather than the pixels: a 1024px icon that
/// compressed to 30KB is cheaper than a 512px one that did not, and the
/// dashboard asks for eight at once. Base64 adds a third, so this is around
/// 256KB on the wire per icon, and a bundle that ships nothing smaller than
/// that shows no icon — which is the decoration case.
const MAX_BYTES: usize = 192 * 1024;

/// The width a caller gets when it does not ask for one.
pub const DEFAULT_WIDTH: u32 = 128;

const ICNS_MAGIC: &[u8] = b"icns";
const PNG_MAGIC: &[u8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

/// An application's icon as a `data:` URL, sized for a list row.
///
/// `target` is a request, not a guarantee: the smallest icon at least that wide
/// is chosen, falling back to the widest available when every icon is smaller.
/// Bundles ship a fixed set of sizes and picking the nearest beats scaling a
/// 1024px icon down in the webview. Anything over [`MAX_BYTES`] is passed over
/// whatever its width.
pub fn data_url(app_path: &Path, target: u32) -> Option<String> {
    let png = icon_png(app_path, target)?;
    let mut url = String::from("data:image/png;base64,");
    url.push_str(&base64(&png));
    Some(url)
}

/// The raw PNG bytes of the closest icon to `target`.
pub fn icon_png(app_path: &Path, target: u32) -> Option<Vec<u8>> {
    let bytes = std::fs::read(icon_file(app_path)?).ok()?;
    let mut best: Option<(u32, &[u8])> = None;

    for (width, png) in embedded_pngs(&bytes) {
        if png.len() > MAX_BYTES {
            continue;
        }
        best = Some(match best {
            // Nothing yet, or this one is a better fit: the smallest at least
            // as wide as asked for, and otherwise the widest there is.
            None => (width, png),
            Some((chosen, _)) if better_fit(width, chosen, target) => (width, png),
            Some(current) => current,
        });
    }

    best.map(|(_, png)| png.to_vec())
}

/// Whether `candidate` is a closer match for `target` than `chosen`.
fn better_fit(candidate: u32, chosen: u32, target: u32) -> bool {
    match (candidate >= target, chosen >= target) {
        // Both big enough: the smaller one carries fewer bytes for the same look.
        (true, true) => candidate < chosen,
        // Only one is big enough.
        (true, false) => true,
        (false, true) => false,
        // Both too small: the larger one is closer to what was asked for.
        (false, false) => candidate > chosen,
    }
}

/// Which `.icns` in the bundle holds the application's icon.
///
/// `CFBundleIconFile` is the declared answer and may or may not carry the
/// extension. Where it is absent — an asset-catalog bundle, or a malformed
/// plist — a single `.icns` in `Resources` is unambiguous enough to use, and
/// several are not, so that case takes the one named after the bundle or gives
/// up rather than picking a document icon at random.
fn icon_file(app_path: &Path) -> Option<PathBuf> {
    let resources = app_path.join("Contents").join("Resources");

    if let Some(declared) = declared_icon_name(app_path) {
        let named = resources.join(&declared);
        if named.extension().is_some() && named.is_file() {
            return Some(named);
        }
        let with_extension = resources.join(format!("{declared}.icns"));
        if with_extension.is_file() {
            return Some(with_extension);
        }
    }

    let mut candidates: Vec<PathBuf> = std::fs::read_dir(&resources)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("icns"))
        })
        .collect();
    candidates.sort();

    if candidates.len() == 1 {
        return candidates.pop();
    }
    let stem = app_path.file_stem()?.to_string_lossy().to_string();
    candidates
        .into_iter()
        .find(|path| path.file_stem().is_some_and(|name| name == stem.as_str()))
}

fn declared_icon_name(app_path: &Path) -> Option<String> {
    let plist = plist::Value::from_file(app_path.join("Contents").join("Info.plist")).ok()?;
    let name = plist
        .as_dictionary()?
        .get("CFBundleIconFile")?
        .as_string()?
        .trim()
        .to_string();
    (!name.is_empty()).then_some(name)
}

/// Every PNG in an `.icns`, with the width it declares.
///
/// The container is a magic word, a total length, and then chunks of a
/// four-byte type and a four-byte length that counts its own header. Older
/// chunk types hold run-length-encoded pixels and newer ones hold a whole PNG
/// or JPEG 2000 file; only the PNGs are of interest, and they announce
/// themselves by their magic rather than by which type tag carries them.
fn embedded_pngs(bytes: &[u8]) -> Vec<(u32, &[u8])> {
    if bytes.len() < 8 || &bytes[0..4] != ICNS_MAGIC {
        return Vec::new();
    }
    // The header's own length, clamped to what was actually read: a truncated
    // file must not send the reader past the end of the buffer.
    let declared = be_u32(&bytes[4..8]) as usize;
    let end = declared.min(bytes.len());

    let mut found = Vec::new();
    let mut cursor = 8;
    while cursor + 8 <= end {
        let length = be_u32(&bytes[cursor + 4..cursor + 8]) as usize;
        // A chunk shorter than its own header, or longer than the file, means
        // the rest cannot be walked: stop rather than guess at a resync point.
        if length < 8 || cursor + length > end {
            break;
        }
        let data = &bytes[cursor + 8..cursor + length];
        if let Some(width) = png_width(data) {
            found.push((width, data));
        }
        cursor += length;
    }

    found
}

/// The width a PNG declares, read from the IHDR chunk that must come first.
fn png_width(data: &[u8]) -> Option<u32> {
    if data.len() < 24 || !data.starts_with(PNG_MAGIC) {
        return None;
    }
    // 8 magic, 4 length, 4 type ("IHDR"), then width.
    if &data[12..16] != b"IHDR" {
        return None;
    }
    Some(be_u32(&data[16..20]))
}

fn be_u32(bytes: &[u8]) -> u32 {
    u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

/// Standard base64, for a `data:` URL.
///
/// Sixteen lines against a dependency, for the one place in the project that
/// needs it. No line wrapping: a data URL is one token.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);

    for chunk in bytes.chunks(3) {
        let bits = (u32::from(chunk[0]) << 16)
            | (u32::from(chunk.get(1).copied().unwrap_or(0)) << 8)
            | u32::from(chunk.get(2).copied().unwrap_or(0));
        for index in 0..4 {
            // Two source bytes encode three characters and one pad, one byte
            // encodes two and two pads.
            if index > chunk.len() {
                out.push('=');
            } else {
                out.push(ALPHABET[((bits >> (18 - index * 6)) & 0x3F) as usize] as char);
            }
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A PNG header long enough to be read: magic, an IHDR length and tag, then
    /// the width and height. Nothing decodes these bytes, so the rest is padding.
    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut out = Vec::from(PNG_MAGIC);
        out.extend_from_slice(&13u32.to_be_bytes());
        out.extend_from_slice(b"IHDR");
        out.extend_from_slice(&width.to_be_bytes());
        out.extend_from_slice(&height.to_be_bytes());
        out.extend_from_slice(&[8, 6, 0, 0, 0]);
        out
    }

    fn icns(chunks: &[(&[u8; 4], Vec<u8>)]) -> Vec<u8> {
        let mut body = Vec::new();
        for (tag, data) in chunks {
            body.extend_from_slice(*tag);
            body.extend_from_slice(&((data.len() + 8) as u32).to_be_bytes());
            body.extend_from_slice(data);
        }
        let mut out = Vec::from(ICNS_MAGIC);
        out.extend_from_slice(&((body.len() + 8) as u32).to_be_bytes());
        out.extend_from_slice(&body);
        out
    }

    /// The RFC 4648 vectors. This is hand-rolled, so it is checked against the
    /// specification rather than against itself.
    #[test]
    fn base64_matches_the_specifications_own_vectors() {
        for (input, expected) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64(input.as_bytes()), expected, "{input}");
        }
        // Every byte value, so the alphabet and the shifts are exercised whole.
        let all: Vec<u8> = (0..=255).collect();
        assert_eq!(base64(&all).len(), 344);
    }

    #[test]
    fn pngs_are_found_among_chunks_that_are_not_pngs() {
        let container = icns(&[
            (b"TOC ", vec![0; 16]),
            (b"is32", vec![1, 2, 3, 4]),
            (b"ic08", png(256, 256)),
            (b"ic07", png(128, 128)),
        ]);

        let found = embedded_pngs(&container);
        let widths: Vec<_> = found.iter().map(|(width, _)| *width).collect();

        assert_eq!(widths, vec![256, 128]);
    }

    /// A truncated or malformed container must stop, not walk off the buffer.
    #[test]
    fn a_malformed_container_yields_nothing_rather_than_panicking() {
        assert!(embedded_pngs(b"").is_empty());
        assert!(embedded_pngs(b"notanicns").is_empty());

        let mut truncated = icns(&[(b"ic08", png(256, 256))]);
        truncated.truncate(20);
        assert!(embedded_pngs(&truncated).is_empty());

        // A chunk claiming to be longer than the file.
        let mut lying = Vec::from(ICNS_MAGIC);
        lying.extend_from_slice(&24u32.to_be_bytes());
        lying.extend_from_slice(b"ic08");
        lying.extend_from_slice(&u32::MAX.to_be_bytes());
        lying.extend_from_slice(&[0; 8]);
        assert!(embedded_pngs(&lying).is_empty());

        // A chunk shorter than its own header would not advance the cursor.
        let mut zero = Vec::from(ICNS_MAGIC);
        zero.extend_from_slice(&16u32.to_be_bytes());
        zero.extend_from_slice(b"ic08");
        zero.extend_from_slice(&0u32.to_be_bytes());
        assert!(embedded_pngs(&zero).is_empty());
    }

    /// The selection rule: the smallest icon that is still big enough, because
    /// a 512px icon drawn into a 128px row is four times the bytes for nothing.
    #[test]
    fn the_smallest_icon_that_is_big_enough_wins() {
        for (target, expected) in [(16, 32), (32, 32), (33, 128), (128, 128), (200, 256)] {
            let mut chosen = 0;
            for width in [32u32, 128, 256] {
                if chosen == 0 || better_fit(width, chosen, target) {
                    chosen = width;
                }
            }
            assert_eq!(chosen, expected, "target {target}");
        }
    }

    /// When everything is smaller than asked for, the widest is the closest.
    #[test]
    fn an_undersized_set_falls_back_to_the_widest() {
        let mut chosen = 0;
        for width in [16u32, 32, 64] {
            if chosen == 0 || better_fit(width, chosen, 512) {
                chosen = width;
            }
        }
        assert_eq!(chosen, 64);
    }

    /// Anything past `MAX_BYTES` is skipped before it is encoded, so a bundle
    /// shipping only an enormous icon reports none rather than a megabyte of
    /// JSON. Width is not the test: a large icon that compressed well is fine.
    #[test]
    fn icons_too_heavy_for_the_wire_are_refused() {
        let scratch = std::env::temp_dir().join(format!("nirmoka-icons-{}", std::process::id()));
        let resources = scratch.join("Huge.app").join("Contents").join("Resources");
        std::fs::create_dir_all(&resources).expect("the temp directory is writable");

        let mut heavy = png(1024, 1024);
        heavy.resize(MAX_BYTES + 1, 0);
        std::fs::write(resources.join("Huge.icns"), icns(&[(b"ic10", heavy)])).expect("written");
        assert_eq!(icon_png(&scratch.join("Huge.app"), DEFAULT_WIDTH), None);

        // The same width, small enough to send.
        std::fs::write(
            resources.join("Huge.icns"),
            icns(&[(b"ic10", png(1024, 1024))]),
        )
        .expect("written");
        assert!(icon_png(&scratch.join("Huge.app"), DEFAULT_WIDTH).is_some());

        let _ = std::fs::remove_dir_all(&scratch);
    }

    #[test]
    fn a_bundle_without_an_icns_reports_no_icon() {
        let scratch =
            std::env::temp_dir().join(format!("nirmoka-icons-none-{}", std::process::id()));
        let resources = scratch.join("Bare.app").join("Contents").join("Resources");
        std::fs::create_dir_all(&resources).expect("the temp directory is writable");

        assert_eq!(icon_file(&scratch.join("Bare.app")), None);
        assert_eq!(data_url(&scratch.join("Bare.app"), DEFAULT_WIDTH), None);
        let _ = std::fs::remove_dir_all(&scratch);
    }

    #[test]
    fn the_whole_path_produces_a_data_url() {
        let scratch = std::env::temp_dir().join(format!("nirmoka-icons-ok-{}", std::process::id()));
        let bundle = scratch.join("Example.app");
        let resources = bundle.join("Contents").join("Resources");
        std::fs::create_dir_all(&resources).expect("the temp directory is writable");
        std::fs::write(
            resources.join("Example.icns"),
            icns(&[(b"ic07", png(128, 128))]),
        )
        .expect("written");

        let url = data_url(&bundle, DEFAULT_WIDTH).expect("the bundle has an icon");

        assert!(url.starts_with("data:image/png;base64,"), "{url}");
        let _ = std::fs::remove_dir_all(&scratch);
    }
}
