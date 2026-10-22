//! Metadata manipulation engine, encoding/decoding codecs, and date utilities for Sheaf.

use std::collections::BTreeMap;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use chrono::{DateTime, FixedOffset, NaiveDate, NaiveDateTime, NaiveTime, Offset, TimeZone, Utc};
use lopdf::{Dictionary, Document, Object, StringFormat};
use serde::{Deserialize, Serialize};

use crate::error::ensure_not_overwriting_inputs;
use crate::fs::{check_output_path, write_atomic};
use crate::load::{LoadOptions, load_pdf};
use crate::ops::split::sanitize_stem;
use crate::{Progress, SheafError};

// ============================================================================
// 1. PDFDocEncoding & UTF-16BE / UTF-8 Codec
// ============================================================================

/// Mapping table for PDFDocEncoding bytes 0x80..=0x9F to Unicode characters.
const PDF_DOC_ENCODING_80_9F: [char; 32] = [
    '\u{2022}', // 0x80 Bullet
    '\u{2020}', // 0x81 Dagger
    '\u{2021}', // 0x82 Double dagger
    '\u{2026}', // 0x83 Ellipsis
    '\u{2014}', // 0x84 Em dash
    '\u{2013}', // 0x85 En dash
    '\u{0192}', // 0x86 Florin / Latin small letter f with hook
    '\u{2044}', // 0x87 Fraction slash
    '\u{2039}', // 0x88 Single left-pointing angle quotation mark
    '\u{203A}', // 0x89 Single right-pointing angle quotation mark
    '\u{2212}', // 0x8A Minus sign
    '\u{2030}', // 0x8B Per mille sign
    '\u{201E}', // 0x8C Double low-9 quotation mark
    '\u{201A}', // 0x8D Single low-9 quotation mark
    '\u{2018}', // 0x8E Left single quotation mark
    '\u{2019}', // 0x8F Right single quotation mark
    '\u{201C}', // 0x90 Left double quotation mark
    '\u{201D}', // 0x91 Right double quotation mark
    '\u{2122}', // 0x92 Trade mark sign
    '\u{FB01}', // 0x93 Latin small ligature fi
    '\u{FB02}', // 0x94 Latin small ligature fl
    '\u{0141}', // 0x95 Latin capital letter L with stroke
    '\u{00D8}', // 0x96 Latin capital letter O with stroke
    '\u{0160}', // 0x97 Latin capital letter S with caron
    '\u{017D}', // 0x98 Latin capital letter Z with caron
    '\u{0131}', // 0x99 Latin small letter dotless i
    '\u{0142}', // 0x9A Latin small letter l with stroke
    '\u{00F8}', // 0x9B Latin small letter o with stroke
    '\u{0161}', // 0x9C Latin small letter s with caron
    '\u{017E}', // 0x9D Latin small letter z with caron
    '\u{20AC}', // 0x9E Euro sign
    '\u{FFFD}', // 0x9F Undefined
];

/// Mapping table for PDFDocEncoding bytes 0x18..=0x1F to Unicode characters.
const PDF_DOC_ENCODING_18_1F: [char; 8] = [
    '\u{02D8}', // 0x18 Breve
    '\u{02C7}', // 0x19 Caron
    '\u{02C6}', // 0x1A Circumflex
    '\u{02D9}', // 0x1B Dot accent
    '\u{02DD}', // 0x1C Double acute
    '\u{02DB}', // 0x1D Ogonek
    '\u{02DA}', // 0x1E Ring
    '\u{02DC}', // 0x1F Small tilde
];

/// Decodes a single PDFDocEncoding byte to a Unicode `char`.
pub fn decode_pdfdoc_byte(b: u8) -> char {
    match b {
        // Standard ASCII controls and printable
        0x09 => '\t',
        0x0A => '\n',
        0x0C => '\u{000C}',
        0x0D => '\r',
        0x18..=0x1F => PDF_DOC_ENCODING_18_1F[(b - 0x18) as usize],
        0x20..=0x7E => b as char,
        0x80..=0x9F => PDF_DOC_ENCODING_80_9F[(b - 0x80) as usize],
        0xA0..=0xFF => b as char, // Matches ISO Latin 1 (U+00A0..=U+00FF)
        _ => '\u{FFFD}',
    }
}

/// Encodes a Unicode `char` into a single PDFDocEncoding byte, if representable.
pub fn encode_pdfdoc_char(c: char) -> Option<u8> {
    match c {
        '\t' => Some(0x09),
        '\n' => Some(0x0A),
        '\u{000C}' => Some(0x0C),
        '\r' => Some(0x0D),
        '\u{02D8}' => Some(0x18),
        '\u{02C7}' => Some(0x19),
        '\u{02C6}' => Some(0x1A),
        '\u{02D9}' => Some(0x1B),
        '\u{02DD}' => Some(0x1C),
        '\u{02DB}' => Some(0x1D),
        '\u{02DA}' => Some(0x1E),
        '\u{02DC}' => Some(0x1F),
        ' '..='~' => Some(c as u8),
        '\u{00A0}'..='\u{00FF}' => Some(c as u8),
        _ => {
            for (idx, &mapped) in PDF_DOC_ENCODING_80_9F.iter().enumerate() {
                if mapped == c && mapped != '\u{FFFD}' {
                    return Some(0x80 + (idx as u8));
                }
            }
            None
        }
    }
}

/// Decodes a PDF text string from raw bytes into a UTF-8 Rust `String`.
///
/// Follows PDF 1.7 specification:
/// 1. UTF-16BE if prefixed with BOM `[0xFE, 0xFF]`.
/// 2. UTF-8 if prefixed with BOM `[0xEF, 0xBB, 0xBF]`.
/// 3. PDFDocEncoding otherwise.
pub fn decode_pdf_string(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0xFE, 0xFF]) {
        // UTF-16BE with BOM
        decode_utf16be(&bytes[2..])
    } else if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        // UTF-8 with BOM
        String::from_utf8_lossy(&bytes[3..]).into_owned()
    } else {
        // PDFDocEncoding
        bytes.iter().copied().map(decode_pdfdoc_byte).collect()
    }
}

/// Decodes raw UTF-16BE bytes (without BOM) into a Rust `String`, properly handling surrogate pairs.
#[allow(clippy::chunks_exact_to_as_chunks)]
pub fn decode_utf16be(bytes: &[u8]) -> String {
    let u16_units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
        .collect();
    char::decode_utf16(u16_units)
        .map(|r| r.unwrap_or('\u{FFFD}'))
        .collect()
}

/// Encodes a Unicode `&str` into PDF string bytes.
///
/// Uses PDFDocEncoding if all characters are representable in PDFDocEncoding.
/// Otherwise, encodes as UTF-16BE prefixed with BOM `[0xFE, 0xFF]` (including surrogate pairs for codepoints > U+FFFF).
pub fn encode_pdf_string(text: &str) -> Vec<u8> {
    // 1. Try encoding entirely in PDFDocEncoding
    let mut pdfdoc_bytes = Vec::with_capacity(text.len());
    let mut all_representable = true;
    for c in text.chars() {
        if let Some(b) = encode_pdfdoc_char(c) {
            pdfdoc_bytes.push(b);
        } else {
            all_representable = false;
            break;
        }
    }

    if all_representable {
        return pdfdoc_bytes;
    }

    // 2. Fall back to UTF-16BE with BOM
    let mut out = Vec::with_capacity(2 + text.len() * 2);
    out.extend_from_slice(&[0xFE, 0xFF]);
    for c in text.chars() {
        let mut buf = [0u16; 2];
        let encoded = c.encode_utf16(&mut buf);
        for unit in encoded.iter().copied() {
            out.extend_from_slice(&unit.to_be_bytes());
        }
    }
    out
}

/// Creates a lopdf `Object::String` with proper encoding.
pub fn create_pdf_string_object(text: &str) -> Object {
    let bytes = encode_pdf_string(text);
    // Use Literal for cleaner text; if contains non-printable or UTF-16BE BOM, lopdf Serializer will escape or write bytes
    Object::String(bytes, StringFormat::Literal)
}

/// Extracts a decoded text string from a lopdf `Object`.
pub fn object_to_decoded_string(obj: &Object) -> Option<String> {
    match obj {
        Object::String(bytes, _) => Some(decode_pdf_string(bytes)),
        _ => None,
    }
}

// ============================================================================
// 2. PDF Date Parser & Formatter
// ============================================================================

/// Parsed or raw PDF date information.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PdfDateInfo {
    /// Formatted ISO 8601 string (e.g. `"2026-10-22T10:00:00+02:00"` or `"2026-10-22T10:00:00Z"`), or `None` if unparseable.
    pub iso: Option<String>,
    /// Exact raw string value stored in the PDF (e.g. `"D:20261022100000+02'00'"`).
    pub raw: Option<String>,
}

/// Determines the current effective timestamp, respecting `SOURCE_DATE_EPOCH` if set.
pub fn get_effective_now() -> DateTime<Utc> {
    if let Ok(epoch_str) = std::env::var("SOURCE_DATE_EPOCH")
        && let Ok(secs) = epoch_str.trim().parse::<i64>()
        && let Some(dt) = DateTime::from_timestamp(secs, 0)
    {
        return dt;
    }
    Utc::now()
}

/// Parses a PDF date string tolerantly according to ISO 32000-1 / PDF 1.7.
///
/// Handles variants such as:
/// - `D:20261022100000+02'00'`
/// - `D:20261022100000Z`
/// - `20261022100000+0200`
/// - `D:20261022`
/// - `D:2026`
/// - `D:20261022100000-05'00'`
/// - Standard ISO-8601 strings (e.g. `2026-10-22T10:00:00+02:00`)
pub fn parse_pdf_date(raw: &str) -> Result<DateTime<FixedOffset>, String> {
    let s = raw.trim();
    if s.is_empty() {
        return Err("Empty date string".to_string());
    }

    // Try standard ISO-8601 / RFC 3339 first (e.g. 2026-10-22T10:00:00Z or +02:00)
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Ok(dt);
    }

    // Strip optional "D:" prefix
    let body = s.strip_prefix("D:").unwrap_or(s).trim();

    // Split numeric date/time prefix from optional timezone suffix
    // Look for '+', '-', or 'Z' after at least 4 digits
    let mut tz_idx = None;
    for (i, b) in body.bytes().enumerate() {
        if i >= 4 && (b == b'+' || b == b'-' || b == b'Z' || b == b'z') {
            tz_idx = Some(i);
            break;
        }
    }

    let (digits_part, tz_part) = match tz_idx {
        Some(idx) => (&body[..idx], &body[idx..]),
        None => (body, ""),
    };

    // Filter to purely ASCII digits for the datetime components
    let digits: String = digits_part.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() < 4 {
        return Err(format!("Date contains less than 4 year digits: '{raw}'"));
    }

    let year: i32 = digits[0..4]
        .parse()
        .map_err(|e| format!("Invalid year: {e}"))?;
    let month: u32 = if digits.len() >= 6 {
        digits[4..6].parse().unwrap_or(1).clamp(1, 12)
    } else {
        1
    };
    let day: u32 = if digits.len() >= 8 {
        digits[6..8].parse().unwrap_or(1).clamp(1, 31)
    } else {
        1
    };
    let hour: u32 = if digits.len() >= 10 {
        digits[8..10].parse().unwrap_or(0).clamp(0, 23)
    } else {
        0
    };
    let min: u32 = if digits.len() >= 12 {
        digits[10..12].parse().unwrap_or(0).clamp(0, 59)
    } else {
        0
    };
    let sec: u32 = if digits.len() >= 14 {
        digits[12..14].parse().unwrap_or(0).clamp(0, 59)
    } else {
        0
    };

    let naive_date = NaiveDate::from_ymd_opt(year, month, day)
        .ok_or_else(|| format!("Invalid date YMD {year}-{month}-{day}"))?;
    let naive_time = NaiveTime::from_hms_opt(hour, min, sec)
        .ok_or_else(|| format!("Invalid time HMS {hour}:{min}:{sec}"))?;
    let naive_dt = NaiveDateTime::new(naive_date, naive_time);

    // Parse timezone offset
    let offset = parse_pdf_timezone(tz_part)?;
    offset
        .from_local_datetime(&naive_dt)
        .single()
        .ok_or_else(|| {
            format!("Ambiguous or invalid local datetime '{naive_dt}' for offset {offset}")
        })
}

/// Parses PDF timezone parts like `+02'00'`, `+0200`, `+02'`, `+02`, `-05'00'`, `Z`, `Z00'00'`.
fn parse_pdf_timezone(tz_part: &str) -> Result<FixedOffset, String> {
    let t = tz_part.trim();
    if t.is_empty() || t.starts_with('Z') || t.starts_with('z') {
        return FixedOffset::east_opt(0)
            .ok_or_else(|| "Failed to construct UTC offset".to_string());
    }

    let sign = if t.starts_with('+') {
        1
    } else if t.starts_with('-') {
        -1
    } else {
        return Err(format!("Invalid timezone prefix in '{t}'"));
    };

    let tz_digits: String = t[1..].chars().filter(|c| c.is_ascii_digit()).collect();
    let (tz_hours, tz_mins) = match tz_digits.len() {
        0 => (0, 0),
        1 | 2 => (tz_digits.parse::<i32>().unwrap_or(0), 0),
        3 => {
            let h = tz_digits[..1].parse::<i32>().unwrap_or(0);
            let m = tz_digits[1..].parse::<i32>().unwrap_or(0);
            (h, m)
        }
        _ => {
            let h = tz_digits[..2].parse::<i32>().unwrap_or(0);
            let m = tz_digits[2..4.min(tz_digits.len())]
                .parse::<i32>()
                .unwrap_or(0);
            (h, m)
        }
    };

    let total_secs = sign * (tz_hours * 3600 + tz_mins * 60);
    FixedOffset::east_opt(total_secs)
        .ok_or_else(|| format!("Timezone offset out of range: {total_secs} seconds"))
}

/// Formats a `DateTime<FixedOffset>` into standard PDF date format: `D:YYYYMMDDHHmmSS+HH'mm'`.
pub fn format_pdf_date(dt: &DateTime<FixedOffset>) -> String {
    let offset_secs = dt.offset().fix().local_minus_utc();
    let sign = if offset_secs >= 0 { '+' } else { '-' };
    let abs_secs = offset_secs.abs();
    let hours = abs_secs / 3600;
    let mins = (abs_secs % 3600) / 60;

    let base = dt.format("%Y%m%d%H%M%S");
    if offset_secs == 0 {
        format!("D:{base}Z00'00'")
    } else {
        format!("D:{base}{sign}{hours:02}'{mins:02}'")
    }
}

/// Formats a `DateTime<FixedOffset>` into standard ISO-8601 string.
pub fn format_iso_date(dt: &DateTime<FixedOffset>) -> String {
    dt.to_rfc3339()
}

// ============================================================================
// 3. Metadata Extraction & Models
// ============================================================================

/// Standard metadata fields in PDF `/Info` dictionary.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetaInfoFields {
    /// Document title.
    pub title: Option<String>,
    /// Document author.
    pub author: Option<String>,
    /// Document subject.
    pub subject: Option<String>,
    /// Keywords describing document content.
    pub keywords: Option<String>,
    /// Tool/application that originally created the document content.
    pub creator: Option<String>,
    /// PDF producer library or engine (e.g. Sheaf, PDFium).
    pub producer: Option<String>,
}

/// Metadata dates (`CreationDate` and `ModDate`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetaDatesFields {
    /// Document creation timestamp (ISO 8601 string and raw PDF value).
    pub creation_date: PdfDateInfo,
    /// Document modification timestamp (ISO 8601 string and raw PDF value).
    pub mod_date: PdfDateInfo,
}

/// XMP metadata packet presence and sync status.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetaXmpInfo {
    /// Whether an XMP metadata packet exists in the Catalog `/Metadata` stream.
    pub present: bool,
    /// Size of the raw XMP metadata stream in bytes (`None` if absent).
    pub size_bytes: Option<usize>,
    /// Whether standard properties in XMP match the Info dictionary (`None` if no XMP).
    pub in_sync: Option<bool>,
    /// Raw XMP XML packet string (only populated if requested via `--xmp`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub packet: Option<String>,
}

/// Complete metadata report for a single PDF file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetaFileResult {
    /// Input file path.
    pub path: String,
    /// Standard metadata fields.
    pub info: MetaInfoFields,
    /// Document creation and modification dates.
    pub dates: MetaDatesFields,
    /// Custom key-value pairs stored in the Info dictionary outside standard fields.
    pub custom: BTreeMap<String, String>,
    /// XMP metadata status and packet details.
    pub xmp: MetaXmpInfo,
    /// Per-file error message (e.g. if encrypted without password or corrupted).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Operation success indicator for this file.
    pub ok: bool,
}

/// Report returned by [`meta_get`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetaGetReport {
    /// Metadata results for each input file.
    pub files: Vec<MetaFileResult>,
}

/// Options for [`meta_get`].
#[derive(Debug, Clone, Default)]
pub struct MetaGetOptions {
    /// Decryption options for password-protected files.
    pub load: LoadOptions,
    /// Whether to include the raw XMP packet content string (up to 1MB).
    pub include_xmp_packet: bool,
}

/// Known standard Info dictionary keys.
pub const STANDARD_INFO_KEYS: &[&[u8]] = &[
    b"Title",
    b"Author",
    b"Subject",
    b"Keywords",
    b"Creator",
    b"Producer",
    b"CreationDate",
    b"ModDate",
    b"Trapped",
];

/// Extracts standard info fields from a lopdf Document.
///
/// Shared single source of truth for both `sheaf info` and `sheaf meta`.
pub fn extract_info_fields(doc: &Document) -> Option<MetaInfoFields> {
    let dict = get_info_dictionary(doc)?;
    Some(MetaInfoFields {
        title: dict_decoded_string(doc, dict, b"Title"),
        author: dict_decoded_string(doc, dict, b"Author"),
        subject: dict_decoded_string(doc, dict, b"Subject"),
        keywords: dict_decoded_string(doc, dict, b"Keywords"),
        creator: dict_decoded_string(doc, dict, b"Creator"),
        producer: dict_decoded_string(doc, dict, b"Producer"),
    })
}

/// Extracts dates from the Info dictionary.
pub fn extract_dates_fields(doc: &Document) -> MetaDatesFields {
    let dict = match get_info_dictionary(doc) {
        Some(d) => d,
        None => {
            return MetaDatesFields {
                creation_date: PdfDateInfo {
                    iso: None,
                    raw: None,
                },
                mod_date: PdfDateInfo {
                    iso: None,
                    raw: None,
                },
            };
        }
    };

    MetaDatesFields {
        creation_date: extract_date_field(doc, dict, b"CreationDate"),
        mod_date: extract_date_field(doc, dict, b"ModDate"),
    }
}

/// Extracts all custom (non-standard) key-value pairs from the Info dictionary.
pub fn extract_custom_fields(doc: &Document) -> BTreeMap<String, String> {
    let mut custom = BTreeMap::new();
    let dict = match get_info_dictionary(doc) {
        Some(d) => d,
        None => return custom,
    };

    for (k, v) in dict.iter() {
        let is_standard = STANDARD_INFO_KEYS.iter().any(|&std_key| std_key == k);
        if !is_standard && let Ok(key_str) = std::str::from_utf8(k) {
            let resolved = match v {
                Object::Reference(id) => doc.get_object(*id).unwrap_or(v),
                other => other,
            };
            if let Some(val_str) = object_to_decoded_string(resolved) {
                custom.insert(key_str.to_string(), val_str);
            }
        }
    }

    custom
}

fn extract_date_field(doc: &Document, dict: &Dictionary, key: &[u8]) -> PdfDateInfo {
    let raw = dict_decoded_string(doc, dict, key);
    let iso = raw
        .as_deref()
        .and_then(|r| parse_pdf_date(r).ok())
        .map(|dt| format_iso_date(&dt));
    PdfDateInfo { iso, raw }
}

fn dict_decoded_string(doc: &Document, dict: &Dictionary, key: &[u8]) -> Option<String> {
    let obj = dict.get(key).ok()?;
    let resolved = match obj {
        Object::Reference(id) => doc.get_object(*id).ok()?,
        other => other,
    };
    object_to_decoded_string(resolved)
}

/// Resolves the trailer `/Info` dictionary if present.
pub fn get_info_dictionary(doc: &Document) -> Option<&Dictionary> {
    let info_obj = doc.trailer.get(b"Info").ok()?;
    match info_obj {
        Object::Dictionary(d) => Some(d),
        Object::Reference(id) => doc.get_dictionary(*id).ok(),
        _ => None,
    }
}

/// Resolves a mutable reference to the trailer `/Info` dictionary, or creates one if absent.
pub fn get_or_create_info_dictionary_mut(
    doc: &mut Document,
) -> Result<lopdf::ObjectId, SheafError> {
    if let Ok(info_obj) = doc.trailer.get(b"Info") {
        match info_obj {
            Object::Reference(id) => return Ok(*id),
            Object::Dictionary(d) => {
                let clone_d = d.clone();
                let id = doc.add_object(Object::Dictionary(clone_d));
                doc.trailer.set("Info", Object::Reference(id));
                return Ok(id);
            }
            _ => {}
        }
    }

    let id = doc.add_object(Object::Dictionary(Dictionary::new()));
    doc.trailer.set("Info", Object::Reference(id));
    Ok(id)
}

// ============================================================================
// 4. XMP Metadata Inspection & Synchronization
// ============================================================================

/// Parsed XMP standard properties.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ParsedXmpProperties {
    title: Option<String>,
    author: Option<String>,
    subject: Option<String>,
    keywords: Option<String>,
    creator_tool: Option<String>,
    producer: Option<String>,
    create_date: Option<String>,
    modify_date: Option<String>,
}

/// Inspects the Catalog `/Metadata` stream and parses standard XMP properties.
pub fn inspect_xmp_metadata(
    doc: &Document,
    info: &MetaInfoFields,
    dates: &MetaDatesFields,
    include_packet: bool,
) -> MetaXmpInfo {
    let meta_stream_obj = match doc.catalog().and_then(|c| c.get(b"Metadata")) {
        Ok(Object::Reference(id)) => doc.get_object(*id).ok(),
        Ok(Object::Stream(s)) => Some(&Object::Stream(s.clone())),
        _ => None,
    };

    let stream = match meta_stream_obj {
        Some(Object::Stream(s)) => s,
        _ => {
            return MetaXmpInfo {
                present: false,
                size_bytes: None,
                in_sync: None,
                packet: None,
            };
        }
    };

    let decompressed = stream.decompressed_content().ok();
    let bytes: &[u8] = decompressed.as_deref().unwrap_or(&stream.content);
    let size_bytes = bytes.len();

    let packet_str = if include_packet {
        // Truncate to 1MB if larger
        let limit = size_bytes.min(1024 * 1024);
        Some(String::from_utf8_lossy(&bytes[..limit]).into_owned())
    } else {
        None
    };

    let parsed_xmp = parse_xmp_packet(bytes);
    let in_sync = match parsed_xmp {
        Some(xmp) => Some(check_xmp_in_sync(info, dates, &xmp)),
        None => Some(false), // Malformed XMP packet
    };

    MetaXmpInfo {
        present: true,
        size_bytes: Some(size_bytes),
        in_sync,
        packet: packet_str,
    }
}

/// Parses an XML byte slice looking for known XMP Dublin Core, PDF, and XMP Basic elements.
fn parse_xmp_packet(bytes: &[u8]) -> Option<ParsedXmpProperties> {
    use quick_xml::events::Event;
    use quick_xml::reader::Reader;

    let mut reader = Reader::from_reader(bytes);
    reader.config_mut().trim_text(true);

    let mut props = ParsedXmpProperties::default();
    let mut current_tag = String::new();
    let mut current_text = String::new();

    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let ln = e.local_name();
                let local_name = ln.as_ref();
                if matches!(
                    local_name,
                    "title"
                        | "creator"
                        | "description"
                        | "Keywords"
                        | "CreatorTool"
                        | "Producer"
                        | "CreateDate"
                        | "ModifyDate"
                ) {
                    current_tag = local_name.to_string();
                    current_text.clear();
                }
            }
            Ok(Event::Text(e)) => {
                if !current_tag.is_empty() {
                    let text = e.as_ref();
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        if !current_text.is_empty() {
                            current_text.push(' ');
                        }
                        current_text.push_str(trimmed);
                    }
                }
            }
            Ok(Event::End(e)) => {
                let ln = e.local_name();
                let local_name = ln.as_ref();
                if local_name == current_tag {
                    let val = current_text.trim().to_string();
                    if !val.is_empty() {
                        match current_tag.as_str() {
                            "title" => props.title = Some(val),
                            "creator" => props.author = Some(val),
                            "description" => props.subject = Some(val),
                            "Keywords" => props.keywords = Some(val),
                            "CreatorTool" => props.creator_tool = Some(val),
                            "Producer" => props.producer = Some(val),
                            "CreateDate" => props.create_date = Some(val),
                            "ModifyDate" => props.modify_date = Some(val),
                            _ => {}
                        }
                    }
                    current_tag.clear();
                    current_text.clear();
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => return None,
            _ => {}
        }
        buf.clear();
    }

    Some(props)
}

fn check_xmp_in_sync(
    info: &MetaInfoFields,
    dates: &MetaDatesFields,
    xmp: &ParsedXmpProperties,
) -> bool {
    let check_opt = |a: &Option<String>, b: &Option<String>| -> bool {
        match (a, b) {
            (Some(s1), Some(s2)) => s1 == s2,
            (None, None) => true,
            _ => false,
        }
    };

    if !check_opt(&info.title, &xmp.title) {
        return false;
    }
    if !check_opt(&info.author, &xmp.author) {
        return false;
    }
    if !check_opt(&info.subject, &xmp.subject) {
        return false;
    }
    if !check_opt(&info.keywords, &xmp.keywords) {
        return false;
    }
    if !check_opt(&info.creator, &xmp.creator_tool) {
        return false;
    }
    if !check_opt(&info.producer, &xmp.producer) {
        return false;
    }

    // Dates comparison
    let check_date = |info_dt: &PdfDateInfo, xmp_raw: &Option<String>| -> bool {
        match (&info_dt.iso, xmp_raw) {
            (Some(iso1), Some(raw2)) => {
                let parsed2 = parse_pdf_date(raw2).ok().map(|dt| format_iso_date(&dt));
                match parsed2 {
                    Some(iso2) => iso1 == &iso2,
                    None => false,
                }
            }
            (None, None) => true,
            _ => false,
        }
    };

    if !check_date(&dates.creation_date, &xmp.create_date) {
        return false;
    }
    if !check_date(&dates.mod_date, &xmp.modify_date) {
        return false;
    }

    true
}

// ============================================================================
// 5. `meta_get` Operation
// ============================================================================

/// Reads metadata from one or more PDF files.
///
/// Encrypted files without a password will produce an error entry with code `encrypted`
/// (or return [`SheafError::Encrypted`] if single file / overall failure).
pub fn meta_get(
    files: &[PathBuf],
    opts: &MetaGetOptions,
    p: &dyn Progress,
) -> Result<MetaGetReport, SheafError> {
    if files.is_empty() {
        return Err(SheafError::EmptySelection);
    }

    let total = files.len() as u64;
    p.report(0, total);

    let mut results = Vec::with_capacity(files.len());
    let mut first_error: Option<SheafError> = None;

    for (idx, path) in files.iter().enumerate() {
        if p.is_cancelled() {
            return Err(SheafError::Cancelled);
        }

        let res = match load_pdf(path, &opts.load) {
            Ok(loaded) => {
                let doc = &loaded.doc;
                let info = extract_info_fields(doc).unwrap_or_default();
                let dates = extract_dates_fields(doc);
                let custom = extract_custom_fields(doc);
                let xmp = inspect_xmp_metadata(doc, &info, &dates, opts.include_xmp_packet);

                MetaFileResult {
                    path: path.to_string_lossy().into_owned(),
                    info,
                    dates,
                    custom,
                    xmp,
                    error: None,
                    ok: true,
                }
            }
            Err(e) => {
                if first_error.is_none() {
                    first_error = Some(match &e {
                        SheafError::Encrypted => SheafError::Encrypted,
                        SheafError::WrongPassword => SheafError::WrongPassword,
                        SheafError::InvalidPdf(msg) => SheafError::InvalidPdf(msg.clone()),
                        SheafError::EmptySelection => SheafError::EmptySelection,
                        SheafError::PageOutOfRange { page, total } => SheafError::PageOutOfRange {
                            page: *page,
                            total: *total,
                        },
                        SheafError::Cancelled => SheafError::Cancelled,
                        _ => SheafError::InvalidPdf(e.to_string()),
                    });
                }
                MetaFileResult {
                    path: path.to_string_lossy().into_owned(),
                    info: MetaInfoFields::default(),
                    dates: MetaDatesFields::default(),
                    custom: BTreeMap::new(),
                    xmp: MetaXmpInfo::default(),
                    error: Some(e.to_string()),
                    ok: false,
                }
            }
        };

        results.push(res);
        p.report(idx as u64 + 1, total);
    }

    // If all files failed and we only had one file, return the specific SheafError
    if files.len() == 1
        && !results[0].ok
        && let Some(err) = first_error
    {
        return Err(err);
    }

    Ok(MetaGetReport { files: results })
}

// ============================================================================
// 6. `meta_set` Operation
// ============================================================================

/// Represents a change to a single metadata field between old and new state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetaDiffEntry {
    /// Field name or key.
    pub field: String,
    /// Old value before update (`None` if unset or missing).
    pub old: Option<String>,
    /// New value after update (`None` if removed).
    pub new: Option<String>,
}

/// XMP processing mode for [`meta_set`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum XmpMode {
    /// Synchronize standard metadata properties with existing XMP (never create new XMP).
    #[default]
    Sync,
    /// Keep existing XMP packet untouched without modification.
    Keep,
    /// Drop/remove Catalog `/Metadata` XMP stream from document.
    Drop,
}

impl XmpMode {
    /// Parses XMP mode string (`"sync"`, `"keep"`, `"drop"`).
    pub fn parse(s: &str) -> Result<Self, SheafError> {
        match s.trim().to_ascii_lowercase().as_str() {
            "sync" => Ok(Self::Sync),
            "keep" => Ok(Self::Keep),
            "drop" => Ok(Self::Drop),
            other => Err(SheafError::InvalidSelection(format!(
                "Invalid xmp mode '{other}'. Supported values: sync, keep, drop."
            ))),
        }
    }

    /// String representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Sync => "sync",
            Self::Keep => "keep",
            Self::Drop => "drop",
        }
    }
}

/// Options for [`meta_set`].
#[derive(Debug, Clone, Default)]
pub struct MetaSetOptions {
    /// Decryption options.
    pub load: LoadOptions,
    /// Update title.
    pub title: Option<String>,
    /// Update author.
    pub author: Option<String>,
    /// Update subject.
    pub subject: Option<String>,
    /// Update keywords.
    pub keywords: Option<String>,
    /// Update creator tool.
    pub creator: Option<String>,
    /// Update PDF producer.
    pub producer: Option<String>,
    /// Update creation date (ISO-8601 string).
    pub creation_date: Option<String>,
    /// Modification date policy: `"now"`, `"keep"`, or explicit ISO-8601 string.
    pub mod_date: String,
    /// Custom key-value pairs to set.
    pub custom: BTreeMap<String, String>,
    /// List of metadata keys to remove.
    pub unset: Vec<String>,
    /// XMP synchronization mode (`Sync`, `Keep`, `Drop`).
    pub xmp: XmpMode,
    /// Target output PDF path (defaults to `{stem}_meta.pdf`).
    pub out: Option<PathBuf>,
    /// Overwrite existing output file if `true`.
    pub force: bool,
    /// Compute diff and plan without writing output file if `true`.
    pub dry_run: bool,
}

/// Report returned by [`meta_set`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetaSetReport {
    /// Input file path.
    pub input: PathBuf,
    /// Output file path.
    pub output: PathBuf,
    /// List of modified fields and their before/after values.
    pub diff: Vec<MetaDiffEntry>,
    /// Applied XMP mode (`"sync"`, `"keep"`, or `"drop"`).
    pub xmp_mode: String,
    /// Whether XMP is in sync after the operation (`None` if no XMP).
    pub xmp_in_sync: Option<bool>,
    /// Whether this was a dry-run execution.
    pub dry_run: bool,
}

/// Validates that a custom metadata key matches `[A-Za-z0-9_-]{1,64}` and does not conflict with standard keys.
pub fn validate_custom_key(key: &str) -> Result<(), SheafError> {
    if key.is_empty() || key.len() > 64 {
        return Err(SheafError::InvalidSelection(format!(
            "Custom metadata key '{key}' must be between 1 and 64 characters long."
        )));
    }
    if !key
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(SheafError::InvalidSelection(format!(
            "Custom metadata key '{key}' contains invalid characters. Only [A-Za-z0-9_-] are allowed."
        )));
    }
    let lower = key.to_ascii_lowercase();
    let std_lower = [
        "title",
        "author",
        "subject",
        "keywords",
        "creator",
        "producer",
        "creationdate",
        "moddate",
        "trapped",
    ];
    if std_lower.contains(&lower.as_str()) {
        return Err(SheafError::InvalidSelection(format!(
            "Custom metadata key '{key}' conflicts with standard metadata key. Use the dedicated option instead."
        )));
    }
    Ok(())
}

/// Updates metadata fields and custom keys in a PDF file, synchronizing XMP.
pub fn meta_set(
    input: &Path,
    opts: &MetaSetOptions,
    p: &dyn Progress,
) -> Result<MetaSetReport, SheafError> {
    if !input.exists() {
        return Err(SheafError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("File not found: {}", input.display()),
        )));
    }

    // Validate non-empty values
    let check_non_empty = |name: &str, opt: &Option<String>| -> Result<(), SheafError> {
        if let Some(s) = opt
            && s.is_empty()
        {
            return Err(SheafError::InvalidSelection(format!(
                "Value for --{name} cannot be empty. Use --unset {name} to remove the field."
            )));
        }
        Ok(())
    };

    check_non_empty("title", &opts.title)?;
    check_non_empty("author", &opts.author)?;
    check_non_empty("subject", &opts.subject)?;
    check_non_empty("keywords", &opts.keywords)?;
    check_non_empty("creator", &opts.creator)?;
    check_non_empty("producer", &opts.producer)?;
    check_non_empty("creation-date", &opts.creation_date)?;

    for (k, v) in &opts.custom {
        validate_custom_key(k)?;
        if v.is_empty() {
            return Err(SheafError::InvalidSelection(format!(
                "Value for custom key '{k}' cannot be empty. Use --unset {k} to remove it."
            )));
        }
    }

    // Determine target output path
    let out_path = match &opts.out {
        Some(path) => path.clone(),
        None => {
            let parent = input.parent().unwrap_or_else(|| Path::new("."));
            let stem = sanitize_stem(input);
            parent.join(format!("{stem}_meta.pdf"))
        }
    };
    ensure_not_overwriting_inputs(&[input.to_path_buf()], &out_path)?;
    if !opts.dry_run {
        check_output_path(&out_path, opts.force)?;
    }

    if p.is_cancelled() {
        return Err(SheafError::Cancelled);
    }
    p.report(10, 100);

    let loaded = load_pdf(input, &opts.load)?;
    let mut doc = loaded.doc;

    // Read current state
    let old_info = extract_info_fields(&doc).unwrap_or_default();
    let old_dates = extract_dates_fields(&doc);
    let old_custom = extract_custom_fields(&doc);

    // Compute planned changes
    let mut diff = Vec::new();

    let mut track_change = |field: &str, old_val: Option<String>, new_val: Option<String>| {
        if old_val != new_val {
            diff.push(MetaDiffEntry {
                field: field.to_string(),
                old: old_val,
                new: new_val,
            });
        }
    };

    // Unset helper (case-insensitive check)
    let is_unset = |name: &str| -> bool { opts.unset.iter().any(|u| u.eq_ignore_ascii_case(name)) };

    // Calculate new values for standard info fields
    let new_title = if is_unset("title") {
        None
    } else {
        opts.title.clone().or(old_info.title.clone())
    };
    track_change("Title", old_info.title.clone(), new_title.clone());

    let new_author = if is_unset("author") {
        None
    } else {
        opts.author.clone().or(old_info.author.clone())
    };
    track_change("Author", old_info.author.clone(), new_author.clone());

    let new_subject = if is_unset("subject") {
        None
    } else {
        opts.subject.clone().or(old_info.subject.clone())
    };
    track_change("Subject", old_info.subject.clone(), new_subject.clone());

    let new_keywords = if is_unset("keywords") {
        None
    } else {
        opts.keywords.clone().or(old_info.keywords.clone())
    };
    track_change("Keywords", old_info.keywords.clone(), new_keywords.clone());

    let new_creator = if is_unset("creator") {
        None
    } else {
        opts.creator.clone().or(old_info.creator.clone())
    };
    track_change("Creator", old_info.creator.clone(), new_creator.clone());

    let new_producer = if is_unset("producer") {
        None
    } else {
        opts.producer.clone().or(old_info.producer.clone())
    };
    track_change("Producer", old_info.producer.clone(), new_producer.clone());

    // Creation date
    let new_creation_date = if is_unset("creationdate") || is_unset("creation-date") {
        None
    } else if let Some(cd) = &opts.creation_date {
        let parsed = parse_pdf_date(cd).map_err(|e| {
            SheafError::InvalidSelection(format!("Invalid --creation-date '{cd}': {e}"))
        })?;
        Some(format_pdf_date(&parsed))
    } else {
        old_dates.creation_date.raw.clone()
    };
    track_change(
        "CreationDate",
        old_dates.creation_date.raw.clone(),
        new_creation_date.clone(),
    );

    // Mod date
    let new_mod_date = if is_unset("moddate") || is_unset("mod-date") {
        None
    } else if opts.mod_date == "keep" {
        old_dates.mod_date.raw.clone()
    } else if opts.mod_date == "now" {
        let eff_now = get_effective_now();
        Some(format_pdf_date(&eff_now.fixed_offset()))
    } else {
        let parsed = parse_pdf_date(&opts.mod_date).map_err(|e| {
            SheafError::InvalidSelection(format!("Invalid --mod-date '{}': {e}", opts.mod_date))
        })?;
        Some(format_pdf_date(&parsed))
    };
    track_change(
        "ModDate",
        old_dates.mod_date.raw.clone(),
        new_mod_date.clone(),
    );

    // Custom fields
    let mut new_custom = old_custom.clone();
    for u in &opts.unset {
        new_custom.remove(u);
    }
    for (k, v) in &opts.custom {
        new_custom.insert(k.clone(), v.clone());
    }

    // Check all old custom keys vs new custom keys for diff
    for (k, old_v) in &old_custom {
        let new_v = new_custom.get(k).cloned();
        track_change(k, Some(old_v.clone()), new_v);
    }
    for (k, new_v) in &new_custom {
        if !old_custom.contains_key(k) {
            track_change(k, None, Some(new_v.clone()));
        }
    }

    // Require at least one change
    if diff.is_empty() && opts.xmp != XmpMode::Drop {
        return Err(SheafError::InvalidSelection(
            "No metadata changes specified. Provide at least one field to set, unset, or update."
                .to_string(),
        ));
    }

    if p.is_cancelled() {
        return Err(SheafError::Cancelled);
    }
    p.report(40, 100);

    let mut xmp_in_sync = None;

    if !opts.dry_run {
        let info_id = get_or_create_info_dictionary_mut(&mut doc)?;
        let mut info_dict = doc
            .get_dictionary(info_id)
            .map_err(|e| SheafError::InvalidPdf(format!("Failed to get Info dict: {e}")))?
            .clone();

        // Helper to update or remove an info key
        let update_dict_key = |d: &mut Dictionary, key: &[u8], val: &Option<String>| match val {
            Some(s) => {
                d.set(key, create_pdf_string_object(s));
            }
            None => {
                d.remove(key);
            }
        };

        update_dict_key(&mut info_dict, b"Title", &new_title);
        update_dict_key(&mut info_dict, b"Author", &new_author);
        update_dict_key(&mut info_dict, b"Subject", &new_subject);
        update_dict_key(&mut info_dict, b"Keywords", &new_keywords);
        update_dict_key(&mut info_dict, b"Creator", &new_creator);
        update_dict_key(&mut info_dict, b"Producer", &new_producer);
        update_dict_key(&mut info_dict, b"CreationDate", &new_creation_date);
        update_dict_key(&mut info_dict, b"ModDate", &new_mod_date);

        // Apply unset to info dict for any custom keys
        for u in &opts.unset {
            info_dict.remove(u.as_bytes());
        }

        // Apply new custom keys
        for (k, v) in &new_custom {
            info_dict.set(k.as_bytes(), create_pdf_string_object(v));
        }

        doc.objects.insert(info_id, Object::Dictionary(info_dict));

        // Handle XMP
        match opts.xmp {
            XmpMode::Drop => {
                if let Ok(catalog) = doc.catalog_mut() {
                    catalog.remove(b"Metadata");
                }
                xmp_in_sync = None;
            }
            XmpMode::Keep => {
                let latest_info = extract_info_fields(&doc).unwrap_or_default();
                let latest_dates = extract_dates_fields(&doc);
                let xmp_info = inspect_xmp_metadata(&doc, &latest_info, &latest_dates, false);
                xmp_in_sync = xmp_info.in_sync;
            }
            XmpMode::Sync => {
                // Synchronize existing XMP if present. Do NOT create new XMP if absent.
                let has_xmp = doc.catalog().and_then(|c| c.get(b"Metadata")).is_ok();
                if has_xmp {
                    let latest_info = extract_info_fields(&doc).unwrap_or_default();
                    let latest_dates = extract_dates_fields(&doc);
                    let sync_res = synchronize_xmp_stream(&mut doc, &latest_info, &latest_dates);
                    xmp_in_sync = Some(sync_res);
                } else {
                    xmp_in_sync = None;
                }
            }
        }

        p.report(70, 100);

        write_atomic(&out_path, opts.force, |temp_file| {
            let mut writer = BufWriter::new(temp_file);
            doc.save_to(&mut writer)
                .map_err(|e| std::io::Error::other(format!("Failed to write PDF: {e}")))?;
            writer.flush()?;
            Ok(())
        })?;
    }

    p.report(100, 100);

    Ok(MetaSetReport {
        input: input.to_path_buf(),
        output: out_path,
        diff,
        xmp_mode: opts.xmp.as_str().to_string(),
        xmp_in_sync,
        dry_run: opts.dry_run,
    })
}

/// Synchronizes standard properties into an existing Catalog `/Metadata` XMP stream.
///
/// Returns `true` on successful synchronization, or `false` if XML parsing failed.
fn synchronize_xmp_stream(
    doc: &mut Document,
    info: &MetaInfoFields,
    dates: &MetaDatesFields,
) -> bool {
    let meta_obj_id = match doc.catalog().and_then(|c| c.get(b"Metadata")) {
        Ok(Object::Reference(id)) => *id,
        _ => return false,
    };

    let stream = match doc.get_object(meta_obj_id) {
        Ok(Object::Stream(s)) => s.clone(),
        _ => return false,
    };

    let decompressed = stream.decompressed_content().ok();
    let bytes: &[u8] = decompressed.as_deref().unwrap_or(&stream.content);
    let xml_str = String::from_utf8_lossy(bytes);

    if let Some(updated_xml) = update_xmp_xml_text(&xml_str, info, dates) {
        let mut new_stream = stream;
        new_stream.set_plain_content(updated_xml.into_bytes());
        doc.objects.insert(meta_obj_id, Object::Stream(new_stream));
        true
    } else {
        false
    }
}

/// Updates known property tags within an XMP XML packet string.
fn update_xmp_xml_text(
    xml: &str,
    info: &MetaInfoFields,
    dates: &MetaDatesFields,
) -> Option<String> {
    // Basic verification of XML packet
    if !xml.contains("<x:xmpmeta") && !xml.contains("<rdf:RDF") {
        return None;
    }

    let mut out = xml.to_string();

    let update_single_tag = |target_xml: &mut String, tag_name: &str, val_opt: &Option<String>| {
        let open_tag = format!("<{tag_name}>");
        let close_tag = format!("</{tag_name}>");
        if let Some(start_idx) = target_xml.find(&open_tag)
            && let Some(end_rel) = target_xml[start_idx..].find(&close_tag)
        {
            let end_idx = start_idx + end_rel + close_tag.len();
            match val_opt {
                Some(v) => {
                    let replacement = format!("<{tag_name}>{v}</{tag_name}>");
                    target_xml.replace_range(start_idx..end_idx, &replacement);
                }
                None => {
                    target_xml.replace_range(start_idx..end_idx, "");
                }
            }
        }
    };

    update_single_tag(&mut out, "pdf:Keywords", &info.keywords);
    update_single_tag(&mut out, "xmp:CreatorTool", &info.creator);
    update_single_tag(&mut out, "pdf:Producer", &info.producer);
    update_single_tag(&mut out, "xmp:CreateDate", &dates.creation_date.iso);
    update_single_tag(&mut out, "xmp:ModifyDate", &dates.mod_date.iso);

    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pdfdoc_encoding_ascii_and_special_symbols() {
        // Standard ASCII
        let s = "Hello, World! 123";
        let encoded = encode_pdf_string(s);
        assert_eq!(encoded, s.as_bytes());
        assert_eq!(decode_pdf_string(&encoded), s);

        // Special symbols in PDFDocEncoding (e.g. Euro, bullet, en-dash, em-dash, quotation marks)
        let special = "€ • – — “ ” ‘ ’ © ®";
        let encoded_special = encode_pdf_string(special);
        // All these are representable in PDFDocEncoding
        assert!(!encoded_special.starts_with(&[0xFE, 0xFF]));
        let decoded = decode_pdf_string(&encoded_special);
        assert_eq!(decoded, special);
    }

    #[test]
    fn test_utf16be_with_bom_and_surrogate_pairs() {
        // Text with emoji (crab 🦀 U+1F980, rocket 🚀 U+1F680) and non-Latin characters (CJK, Cyrillic)
        let unicode_str = "Sheaf 🦀 🚀 Привет 世界";
        let encoded = encode_pdf_string(unicode_str);

        // Must start with UTF-16BE BOM
        assert!(
            encoded.starts_with(&[0xFE, 0xFF]),
            "Non-PDFDocEncoding string must be encoded as UTF-16BE"
        );

        let decoded = decode_pdf_string(&encoded);
        assert_eq!(decoded, unicode_str);
    }

    #[test]
    fn test_decode_utf8_with_bom() {
        let utf8_bom = [0xEF, 0xBB, 0xBF, b'H', b'e', b'l', b'l', b'o'];
        assert_eq!(decode_pdf_string(&utf8_bom), "Hello");
    }

    #[test]
    fn test_pdf_date_parsing_various_formats() {
        // Full PDF format with timezone
        let dt1 = parse_pdf_date("D:20261022100000+02'00'").unwrap();
        assert_eq!(format_iso_date(&dt1), "2026-10-22T10:00:00+02:00");
        assert_eq!(format_pdf_date(&dt1), "D:20261022100000+02'00'");

        // UTC with Z
        let dt2 = parse_pdf_date("D:20261022100000Z").unwrap();
        assert_eq!(format_iso_date(&dt2), "2026-10-22T10:00:00+00:00");
        assert_eq!(format_pdf_date(&dt2), "D:20261022100000Z00'00'");

        // Without D: prefix and colons
        let dt3 = parse_pdf_date("20261022100000-0500").unwrap();
        assert_eq!(format_iso_date(&dt3), "2026-10-22T10:00:00-05:00");

        // Partial date (year only, year+month, year+month+day)
        let dt4 = parse_pdf_date("D:2026").unwrap();
        assert_eq!(format_iso_date(&dt4), "2026-01-01T00:00:00+00:00");

        let dt5 = parse_pdf_date("D:202606").unwrap();
        assert_eq!(format_iso_date(&dt5), "2026-06-01T00:00:00+00:00");

        let dt6 = parse_pdf_date("D:20260615").unwrap();
        assert_eq!(format_iso_date(&dt6), "2026-06-15T00:00:00+00:00");
    }

    #[test]
    fn test_source_date_epoch_override() {
        unsafe {
            std::env::set_var("SOURCE_DATE_EPOCH", "1700000000");
        }
        let eff_now = get_effective_now();
        assert_eq!(eff_now.timestamp(), 1700000000);
        unsafe {
            std::env::remove_var("SOURCE_DATE_EPOCH");
        }
    }
}
