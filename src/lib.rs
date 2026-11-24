//! WIM metadata only: no filesystem extraction, registry reads or build guessing.
use roxmltree::{Document, Node, ParsingOptions};
use serde::Serialize;
use std::{collections::HashSet, fmt};

pub const HEADER_SIZE: usize = 208;
pub const MAX_XML_BYTES: u64 = 32 * 1024 * 1024;
const MAX_IMAGES: u32 = 100_000;
const JS_MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(pub String);
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for Error {}
fn fail(message: &str) -> Error {
    Error(message.into())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub image_count: u32,
    pub xml_offset: u64,
    pub xml_size: u64,
    pub container: ContainerInfo,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WimInfo {
    pub image_count: u32,
    pub images: Vec<ImageInfo>,
    pub container: ContainerInfo,
    pub total_bytes: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ImageInfo {
    pub index: u32,
    /// XML NAME. No DISPLAYNAME fallback or OS branding inference.
    pub os_name: Option<String>,
    /// XML DISPLAYNAME, separately preserved.
    pub display_name: Option<String>,
    pub edition: Option<String>,
    /// Raw XML ARCH, e.g. "9"; never converted to "x64".
    pub architecture: Option<String>,
    /// XML BUILD and SPBUILD joined with '.', only for decimal text values.
    pub build_version: Option<String>,
    pub build: Option<String>,
    pub sp_build: Option<String>,
    pub description: Option<String>,
    pub display_description: Option<String>,
    pub image_size: Option<String>,
    pub hard_link_bytes: Option<String>,
    pub product_name: Option<String>,
    pub installation_type: Option<String>,
    pub hal: Option<String>,
    pub product_type: Option<String>,
    pub product_suite: Option<String>,
    pub major_version: Option<String>,
    pub minor_version: Option<String>,
    /// Derived MAJOR.MINOR.BUILD, corresponding to DISM Version.
    pub version: Option<String>,
    pub sp_level: Option<String>,
    /// WIM header boot index designation; not a bootability test.
    pub bootable: bool,
    /// Legacy WIMBoot flag is distinct from the boot index designation.
    pub wim_boot: Option<String>,
    pub flags: Option<String>,
    pub system_root: Option<String>,
    pub languages: Option<Languages>,
    pub file_count: Option<String>,
    pub directory_count: Option<String>,
    pub created_time: Option<ImageTime>,
    pub modified_time: Option<ImageTime>,
}

/// Decimal strings avoid loss of precision for 64-bit offsets/sizes in JavaScript.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResourceInfo {
    pub offset: String,
    pub size: String,
    pub original_size: String,
    pub flags: u8,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ContainerInfo {
    pub image_type: String,
    pub file_size: String,
    pub header_size: u32,
    pub format_version: u32,
    pub flags: u32,
    pub compression_chunk_size: u32,
    pub guid: String,
    pub part_number: u16,
    pub total_parts: u16,
    pub boot_index: u32,
    pub lookup_table: ResourceInfo,
    pub xml: ResourceInfo,
    pub boot_metadata: ResourceInfo,
    pub integrity: ResourceInfo,
    pub has_integrity_table: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Languages {
    pub values: Vec<String>,
    pub count: u32,
    pub default: Option<String>,
    /// Zero-based index, like DISM DefaultLanguageIndex. None if unmatched.
    pub default_index: Option<u32>,
    pub fallbacks: Vec<LanguageFallback>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LanguageFallback {
    pub language: Option<String>,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ImageTime {
    pub high_part: Option<String>,
    pub low_part: Option<String>,
    /// Derived UTC time; original high/low strings remain unchanged.
    pub utc: Option<String>,
}

fn u16_at(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}
fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}
fn u64_at(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

/// Parses the standard 208-byte WIM header and validates the XML read range.
/// Supports standard WIM versions 0x10D00 and 0xE00 (including XML in ESD).
pub fn parse_header(bytes: &[u8], file_size: u64) -> Result<Header, Error> {
    if bytes.len() < HEADER_SIZE || file_size < HEADER_SIZE as u64 {
        return Err(fail("Truncated WIM header"));
    }
    if &bytes[..8] != b"MSWIM\0\0\0" {
        return Err(fail("Invalid WIM signature"));
    }
    if u32_at(bytes, 8) != HEADER_SIZE as u32 {
        return Err(fail("Unsupported WIM header size"));
    }
    if !matches!(u32_at(bytes, 12), 0x10d00 | 0xe00) {
        return Err(fail("Unsupported WIM format version"));
    }
    let part = u16_at(bytes, 40);
    let parts = u16_at(bytes, 42);
    if part == 0 || parts == 0 || part > parts {
        return Err(fail("Invalid WIM part numbers"));
    }
    // This metadata reader deliberately accepts only a standalone container.
    if part != 1 || parts != 1 {
        return Err(fail("Split WIM files are not supported"));
    }
    let image_count = u32_at(bytes, 44);
    if image_count == 0 || image_count > MAX_IMAGES {
        return Err(fail("Invalid or excessive image count"));
    }

    // RESHDR_DISK at 0x48: packed size (low 56 bits), flags (high 8),
    // file offset (u64), original size (u64).
    let packed = u64_at(bytes, 72);
    let xml_size = packed & 0x00ff_ffff_ffff_ffff;
    let flags = (packed >> 56) as u8;
    let xml_offset = u64_at(bytes, 80);
    let original_size = u64_at(bytes, 88);
    // 0x02 is the metadata flag. XML must not be compressed, spanned or solid.
    if flags & !0x02 != 0 {
        return Err(fail("Unsupported XML resource flags"));
    }
    if xml_size == 0 || xml_size > MAX_XML_BYTES || !xml_size.is_multiple_of(2) {
        return Err(fail("Invalid or excessive XML resource size"));
    }
    if original_size != xml_size {
        return Err(fail("XML resource size mismatch"));
    }
    let end = xml_offset
        .checked_add(xml_size)
        .ok_or_else(|| fail("XML range overflow"))?;
    if xml_offset < HEADER_SIZE as u64 || end > file_size || end > JS_MAX_SAFE_INTEGER {
        return Err(fail("XML resource is outside the readable file range"));
    }
    let boot_index = u32_at(bytes, 120);
    if boot_index > image_count {
        return Err(fail("Invalid WIM boot image index"));
    }
    let container = ContainerInfo {
        image_type: "WIM".into(),
        file_size: file_size.to_string(),
        header_size: u32_at(bytes, 8),
        format_version: u32_at(bytes, 12),
        flags: u32_at(bytes, 16),
        compression_chunk_size: u32_at(bytes, 20),
        guid: format!(
            "{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            u32_at(bytes, 24),
            u16_at(bytes, 28),
            u16_at(bytes, 30),
            bytes[32],
            bytes[33],
            bytes[34],
            bytes[35],
            bytes[36],
            bytes[37],
            bytes[38],
            bytes[39]
        ),
        part_number: part,
        total_parts: parts,
        boot_index,
        lookup_table: resource(bytes, 48),
        xml: resource(bytes, 72),
        boot_metadata: resource(bytes, 96),
        integrity: resource(bytes, 124),
        has_integrity_table: u64_at(bytes, 124) & 0x00ff_ffff_ffff_ffff != 0,
    };
    Ok(Header {
        image_count,
        xml_offset,
        xml_size,
        container,
    })
}

fn resource(bytes: &[u8], offset: usize) -> ResourceInfo {
    let packed = u64_at(bytes, offset);
    ResourceInfo {
        offset: u64_at(bytes, offset + 8).to_string(),
        size: (packed & 0x00ff_ffff_ffff_ffff).to_string(),
        original_size: u64_at(bytes, offset + 16).to_string(),
        flags: (packed >> 56) as u8,
    }
}

fn child<'a, 'input>(
    node: Node<'a, 'input>,
    name: &str,
) -> Result<Option<Node<'a, 'input>>, Error> {
    let mut found = node.children().filter(|n| n.has_tag_name(name));
    let result = found.next();
    if found.next().is_some() {
        return Err(Error(format!("Duplicate XML field: {name}")));
    }
    Ok(result)
}

/// Preserve decoded XML text (including whitespace and leading zeroes).
/// Entities/CDATA are decoded by XML rules; this is not a byte-for-byte XML copy.
fn text(node: Option<Node<'_, '_>>, name: &str) -> Result<Option<String>, Error> {
    let Some(parent) = node else {
        return Ok(None);
    };
    let Some(value) = child(parent, name)? else {
        return Ok(None);
    };
    Ok(Some(node_text(value)?))
}

fn node_text(value: Node<'_, '_>) -> Result<String, Error> {
    if value.children().any(|n| n.is_element()) {
        return Err(Error(format!(
            "Non-text XML field: {}",
            value.tag_name().name()
        )));
    }
    Ok(value
        .children()
        .filter(|n| n.is_text())
        .filter_map(|n| n.text())
        .collect())
}

fn languages(windows: Option<Node<'_, '_>>) -> Result<Option<Languages>, Error> {
    let Some(windows) = windows else {
        return Ok(None);
    };
    let Some(node) = child(windows, "LANGUAGES")? else {
        return Ok(None);
    };
    let values = node
        .children()
        .filter(|n| n.has_tag_name("LANGUAGE"))
        .map(node_text)
        .collect::<Result<Vec<_>, _>>()?;
    let default = text(Some(node), "DEFAULT")?;
    let default_index = default
        .as_ref()
        .and_then(|d| values.iter().position(|v| v == d))
        .map(|i| i as u32);
    let fallbacks = node
        .children()
        .filter(|n| n.has_tag_name("FALLBACK"))
        .map(|n| {
            Ok(LanguageFallback {
                language: n.attribute("LANGUAGE").map(str::to_owned),
                value: node_text(n)?,
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    Ok(Some(Languages {
        count: values.len() as u32,
        values,
        default,
        default_index,
        fallbacks,
    }))
}

fn image_time(image: Node<'_, '_>, name: &str) -> Result<Option<ImageTime>, Error> {
    let Some(node) = child(image, name)? else {
        return Ok(None);
    };
    let high_part = text(Some(node), "HIGHPART")?;
    let low_part = text(Some(node), "LOWPART")?;
    fn word(raw: &str) -> Option<u32> {
        if let Some(hex) = raw.strip_prefix("0x").or_else(|| raw.strip_prefix("0X")) {
            u32::from_str_radix(hex, 16).ok()
        } else if decimal(raw) {
            raw.parse().ok()
        } else {
            None
        }
    }
    let utc = high_part
        .as_deref()
        .and_then(word)
        .zip(low_part.as_deref().and_then(word))
        .and_then(|(high, low)| {
            let ticks = ((high as u64) << 32) | low as u64;
            let nanos = (ticks as i128 - 116_444_736_000_000_000) * 100;
            time::OffsetDateTime::from_unix_timestamp_nanos(nanos)
                .ok()
                .and_then(|date| {
                    date.format(&time::format_description::well_known::Rfc3339)
                        .ok()
                })
        });
    Ok(Some(ImageTime {
        high_part,
        low_part,
        utc,
    }))
}

fn decimal(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit())
}

pub fn parse_xml(header: &Header, bytes: &[u8]) -> Result<WimInfo, Error> {
    if bytes.len() as u64 != header.xml_size
        || !bytes.len().is_multiple_of(2)
        || bytes.len() as u64 > MAX_XML_BYTES
    {
        return Err(fail("Unexpected XML byte length"));
    }
    let units: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .collect();
    let xml = String::from_utf16(&units).map_err(|_| fail("Invalid UTF-16LE XML"))?;
    let xml = xml.strip_prefix('\u{feff}').unwrap_or(&xml);
    let doc = Document::parse_with_options(
        xml,
        ParsingOptions {
            allow_dtd: false,
            nodes_limit: 1_000_000,
            ..Default::default()
        },
    )
    .map_err(|e| Error(format!("Invalid WIM XML: {e}")))?;
    let root = doc.root_element();
    if !root.has_tag_name("WIM") {
        return Err(fail("Expected WIM XML root"));
    }
    let mut images = Vec::new();
    let mut indexes = HashSet::new();
    for image in root.children().filter(|n| n.has_tag_name("IMAGE")) {
        let raw_index = image
            .attribute("INDEX")
            .ok_or_else(|| fail("Missing image INDEX"))?;
        if !decimal(raw_index) {
            return Err(fail("Invalid image INDEX"));
        }
        let index: u32 = raw_index.parse().map_err(|_| fail("Invalid image INDEX"))?;
        if index == 0 || index > header.image_count || !indexes.insert(index) {
            return Err(fail("Duplicate or out-of-range image INDEX"));
        }
        let windows = child(image, "WINDOWS")?;
        let version = windows.map(|w| child(w, "VERSION")).transpose()?.flatten();
        let major_version = text(version, "MAJOR")?;
        let minor_version = text(version, "MINOR")?;
        let build = text(version, "BUILD")?;
        let version_text = match (&major_version, &minor_version, &build) {
            (Some(a), Some(b), Some(c)) if decimal(a) && decimal(b) && decimal(c) => {
                Some(format!("{a}.{b}.{c}"))
            }
            _ => None,
        };
        let sp_build = text(version, "SPBUILD")?;
        let build_version = match (&build, &sp_build) {
            (Some(b), Some(s)) if decimal(b) && decimal(s) => Some(format!("{b}.{s}")),
            _ => None,
        };
        images.push(ImageInfo {
            index,
            os_name: text(Some(image), "NAME")?,
            display_name: text(Some(image), "DISPLAYNAME")?,
            edition: text(windows, "EDITIONID")?,
            architecture: text(windows, "ARCH")?,
            build_version,
            build,
            sp_build,
            description: text(Some(image), "DESCRIPTION")?,
            display_description: text(Some(image), "DISPLAYDESCRIPTION")?,
            image_size: text(Some(image), "TOTALBYTES")?,
            hard_link_bytes: text(Some(image), "HARDLINKBYTES")?,
            product_name: text(windows, "PRODUCTNAME")?,
            installation_type: text(windows, "INSTALLATIONTYPE")?,
            hal: text(windows, "HAL")?,
            product_type: text(windows, "PRODUCTTYPE")?,
            product_suite: text(windows, "PRODUCTSUITE")?,
            major_version,
            minor_version,
            version: version_text,
            sp_level: text(version, "SPLEVEL")?,
            bootable: header.container.boot_index == index,
            wim_boot: text(Some(image), "WIMBOOT")?,
            flags: text(Some(image), "FLAGS")?,
            system_root: text(windows, "SYSTEMROOT")?,
            languages: languages(windows)?,
            file_count: text(Some(image), "FILECOUNT")?,
            directory_count: text(Some(image), "DIRCOUNT")?,
            created_time: image_time(image, "CREATIONTIME")?,
            modified_time: image_time(image, "LASTMODIFICATIONTIME")?,
        });
    }
    if images.len() != header.image_count as usize {
        return Err(fail("Header/XML image count mismatch"));
    }
    Ok(WimInfo {
        image_count: header.image_count,
        images,
        container: header.container.clone(),
        total_bytes: text(Some(root), "TOTALBYTES")?,
    })
}

#[cfg(target_arch = "wasm32")]
mod browser {
    use super::*;
    use js_sys::Uint8Array;
    use serde::Serialize;
    use wasm_bindgen::prelude::*;
    use wasm_bindgen_futures::JsFuture;
    use web_sys::File;

    fn js_error(error: Error) -> JsValue {
        js_sys::Error::new(&error.0).into()
    }

    async fn read_range(file: &File, offset: u64, size: u64) -> Result<Vec<u8>, JsValue> {
        let blob = file.slice_with_f64_and_f64(offset as f64, (offset + size) as f64)?;
        let buffer = JsFuture::from(blob.array_buffer()).await?;
        let view = Uint8Array::new(&buffer);
        if view.length() as u64 != size {
            return Err(js_error(fail("Short file read")));
        }
        // One JS -> Wasm copy of only this requested range.
        Ok(view.to_vec())
    }

    /// Accept a browser File directly, including inside a Dedicated Worker.
    #[wasm_bindgen(js_name = parseWim)]
    pub async fn parse_wim(file: File) -> Result<JsValue, JsValue> {
        let size = file.size();
        if !size.is_finite()
            || size.fract() != 0.0
            || size < HEADER_SIZE as f64
            || size > JS_MAX_SAFE_INTEGER as f64
        {
            return Err(js_error(fail("Invalid or unsupported file size")));
        }
        let bytes = read_range(&file, 0, HEADER_SIZE as u64).await?;
        let header = parse_header(&bytes, size as u64).map_err(js_error)?;
        let bytes = read_range(&file, header.xml_offset, header.xml_size).await?;
        let info = parse_xml(&header, &bytes).map_err(js_error)?;
        // Explicit null, rather than undefined, for missing XML fields.
        info.serialize(&serde_wasm_bindgen::Serializer::new().serialize_missing_as_null(true))
            .map_err(|e| js_sys::Error::new(&e.to_string()).into())
    }
}
