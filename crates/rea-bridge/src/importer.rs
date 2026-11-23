// @story 09.1
// @implements AC-03, AC-04 (Lossless .fig/Clipboard/.zip/.pen/.wire Importer, Zip-Bomb Guard, web-clone L1-L6 & Honest Figma Proof Exporter)
use std::collections::BTreeSet;
use std::sync::OnceLock;

use base64::{engine::general_purpose::STANDARD as B64_STD, Engine as _};
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::store::{ProofStatus, ReaReferenceRecord};

pub const MAX_ZIP_UNCOMPRESSED_BYTES: u64 = 16 * 1024 * 1024;
pub const MAX_ZIP_ENTRIES: usize = 128;
pub const MAX_ZIP_EXPANSION_RATIO: f64 = 25.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceFormat {
    Url,
    Github,
    Fig,
    Clipboard,
    Zip,
    Pen,
    Wire,
}

impl SourceFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Url => "url",
            Self::Github => "github",
            Self::Fig => "fig",
            Self::Clipboard => "clipboard",
            Self::Zip => "zip",
            Self::Pen => "pen",
            Self::Wire => "wire",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebCloneLayers {
    pub l1_palette: Vec<String>,
    pub l2_typography: Vec<String>,
    pub l3_spacing_radius: Vec<String>,
    pub l4_layout_sections: Vec<String>,
    pub l5_components: Vec<String>,
    pub l6_interactions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedDesignArtifact {
    pub format: SourceFormat,
    pub design_dna: serde_json::Value,
    pub raw_fields: Option<serde_json::Value>,
    pub auto_summary: String,
    pub extracted_files: Vec<String>,
    #[serde(skip, default)]
    pub extracted_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FigmaProofExport {
    pub reference_id: String,
    pub slug: String,
    pub proof_status: ProofStatus,
    pub verified: bool,
    pub checksum_matched: bool,
    pub computed_sha256: String,
    pub proof_payload_json: String,
    pub verification_note: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ImporterError {
    #[error("Malformed payload for format '{format}': {reason}")]
    MalformedPayload { format: String, reason: String },
    #[error("Zip archive rejected: entry count {count} exceeds max {max}")]
    ZipTooManyEntries { count: usize, max: usize },
    #[error("Zip-bomb guard triggered: uncompressed size {bytes} exceeds max {max} bytes")]
    ZipSizeLimitExceeded { bytes: u64, max: u64 },
    #[error("Zip-bomb guard triggered: expansion ratio {ratio}x exceeds max {max}x")]
    ZipRatioExceeded { ratio: String, max: String },
    #[error("Zip archive rejected unsafe path traversal entry '{0}'")]
    ZipPathTraversal(String),
    #[error("Zip archive rejected symlink entry '{0}'")]
    ZipSymlinkRejected(String),
    #[error("Unsupported ZIP compression method {0}")]
    UnsupportedZipMethod(u16),
}

#[derive(Debug, Clone)]
pub struct ZipEntryData {
    pub name: String,
    pub bytes: Vec<u8>,
}

/// Validates and reads a ZIP archive with strict zip-bomb, symlink, and path-traversal guards.
/// Supports Method 0 (STORED) and Method 8 (DEFLATE) with streaming output budget enforcement.
pub fn read_zip_safely(archive: &[u8]) -> Result<Vec<ZipEntryData>, ImporterError> {
    let eocd_offset = find_eocd(archive).ok_or_else(|| ImporterError::MalformedPayload {
        format: "zip".to_string(),
        reason: "Missing ZIP End-of-Central-Directory record".to_string(),
    })?;

    if eocd_offset + 22 > archive.len() {
        return Err(ImporterError::MalformedPayload {
            format: "zip".to_string(),
            reason: "Truncated ZIP EOCD record".to_string(),
        });
    }

    let total_entries = read_u16(archive, eocd_offset + 10) as usize;
    if total_entries > MAX_ZIP_ENTRIES {
        return Err(ImporterError::ZipTooManyEntries {
            count: total_entries,
            max: MAX_ZIP_ENTRIES,
        });
    }
    let cd_size = read_u32(archive, eocd_offset + 12) as usize;
    let cd_offset = read_u32(archive, eocd_offset + 16) as usize;
    if cd_offset.saturating_add(cd_size) > archive.len() {
        return Err(ImporterError::MalformedPayload {
            format: "zip".to_string(),
            reason: "Central directory bounds exceed archive size".to_string(),
        });
    }

    let mut cursor = cd_offset;
    let mut entries = Vec::with_capacity(total_entries);
    let mut total_uncompressed: u64 = 0;

    for _ in 0..total_entries {
        if cursor + 46 > archive.len() || &archive[cursor..cursor + 4] != b"PK\x01\x02" {
            return Err(ImporterError::MalformedPayload {
                format: "zip".to_string(),
                reason: "Invalid central directory header signature".to_string(),
            });
        }
        let method = read_u16(archive, cursor + 10);
        let compressed_size = read_u32(archive, cursor + 20) as u64;
        let uncompressed_size = read_u32(archive, cursor + 24) as u64;
        let name_len = read_u16(archive, cursor + 28) as usize;
        let extra_len = read_u16(archive, cursor + 30) as usize;
        let comment_len = read_u16(archive, cursor + 32) as usize;
        let external_attr = read_u32(archive, cursor + 38);
        let local_header_offset = read_u32(archive, cursor + 42) as usize;

        let name_start = cursor + 46;
        let name_end = name_start + name_len;
        if name_end + extra_len + comment_len > archive.len() {
            return Err(ImporterError::MalformedPayload {
                format: "zip".to_string(),
                reason: "Central directory entry exceeds archive length".to_string(),
            });
        }
        let name = String::from_utf8_lossy(&archive[name_start..name_end]).to_string();
        cursor = name_end + extra_len + comment_len;

        validate_zip_entry_path(&name)?;

        // Unix symlink mode check: S_IFLNK == 0o120000 in upper 16 bits of external_attr
        let unix_mode = (external_attr >> 16) & 0o170000;
        if unix_mode == 0o120000 {
            return Err(ImporterError::ZipSymlinkRejected(name));
        }

        if name.ends_with('/') {
            continue;
        }

        enforce_zip_quota(compressed_size, uncompressed_size, total_uncompressed)?;

        if local_header_offset + 30 > archive.len()
            || &archive[local_header_offset..local_header_offset + 4] != b"PK\x03\x04"
        {
            return Err(ImporterError::MalformedPayload {
                format: "zip".to_string(),
                reason: format!("Invalid local file header for '{name}'"),
            });
        }
        let lh_name_len = read_u16(archive, local_header_offset + 26) as usize;
        let lh_extra_len = read_u16(archive, local_header_offset + 28) as usize;
        let data_start = local_header_offset + 30 + lh_name_len + lh_extra_len;
        let data_end = data_start.saturating_add(compressed_size as usize);
        if data_end > archive.len() {
            return Err(ImporterError::MalformedPayload {
                format: "zip".to_string(),
                reason: format!("Compressed data out of bounds for '{name}'"),
            });
        }
        let raw_slice = &archive[data_start..data_end];
        let remaining_budget = (MAX_ZIP_UNCOMPRESSED_BYTES - total_uncompressed) as usize;
        let max_by_ratio = if compressed_size == 0 {
            0usize
        } else {
            ((compressed_size as f64) * MAX_ZIP_EXPANSION_RATIO).floor() as usize
        };
        let entry_cap = remaining_budget.min(max_by_ratio.max(if method == 0 {
            compressed_size as usize
        } else {
            0
        }));

        let out_bytes = match method {
            0 => {
                if raw_slice.len() > remaining_budget {
                    return Err(ImporterError::ZipSizeLimitExceeded {
                        bytes: total_uncompressed + raw_slice.len() as u64,
                        max: MAX_ZIP_UNCOMPRESSED_BYTES,
                    });
                }
                raw_slice.to_vec()
            }
            8 => inflate_raw_bounded(raw_slice, entry_cap, compressed_size, total_uncompressed)?,
            other => return Err(ImporterError::UnsupportedZipMethod(other)),
        };

        total_uncompressed = total_uncompressed.saturating_add(out_bytes.len() as u64);
        if total_uncompressed > MAX_ZIP_UNCOMPRESSED_BYTES {
            return Err(ImporterError::ZipSizeLimitExceeded {
                bytes: total_uncompressed,
                max: MAX_ZIP_UNCOMPRESSED_BYTES,
            });
        }
        if compressed_size > 0 {
            let actual_ratio = (out_bytes.len() as f64) / (compressed_size as f64);
            if actual_ratio > MAX_ZIP_EXPANSION_RATIO {
                return Err(ImporterError::ZipRatioExceeded {
                    ratio: format!("{actual_ratio:.1}"),
                    max: format!("{MAX_ZIP_EXPANSION_RATIO:.1}"),
                });
            }
        }

        entries.push(ZipEntryData {
            name,
            bytes: out_bytes,
        });
    }

    Ok(entries)
}

fn validate_zip_entry_path(name: &str) -> Result<(), ImporterError> {
    if name.is_empty()
        || name.contains('\0')
        || name.starts_with('/')
        || name.starts_with('\\')
        || name.contains('\\')
    {
        return Err(ImporterError::ZipPathTraversal(name.to_string()));
    }
    if name.len() >= 2 && name.as_bytes()[1] == b':' {
        return Err(ImporterError::ZipPathTraversal(name.to_string()));
    }
    for segment in name.split('/') {
        if segment == ".." {
            return Err(ImporterError::ZipPathTraversal(name.to_string()));
        }
    }
    Ok(())
}

fn enforce_zip_quota(
    compressed_size: u64,
    uncompressed_size: u64,
    running_total: u64,
) -> Result<(), ImporterError> {
    let projected = running_total.saturating_add(uncompressed_size);
    if projected > MAX_ZIP_UNCOMPRESSED_BYTES {
        return Err(ImporterError::ZipSizeLimitExceeded {
            bytes: projected,
            max: MAX_ZIP_UNCOMPRESSED_BYTES,
        });
    }
    if compressed_size == 0 && uncompressed_size > 0 {
        return Err(ImporterError::ZipRatioExceeded {
            ratio: "inf".to_string(),
            max: format!("{MAX_ZIP_EXPANSION_RATIO:.1}"),
        });
    }
    if compressed_size > 0 {
        let ratio = (uncompressed_size as f64) / (compressed_size as f64);
        if ratio > MAX_ZIP_EXPANSION_RATIO {
            return Err(ImporterError::ZipRatioExceeded {
                ratio: format!("{ratio:.1}"),
                max: format!("{MAX_ZIP_EXPANSION_RATIO:.1}"),
            });
        }
    }
    Ok(())
}

fn find_eocd(data: &[u8]) -> Option<usize> {
    if data.len() < 22 {
        return None;
    }
    let min_pos = data.len().saturating_sub(65_557);
    let mut pos = data.len() - 22;
    loop {
        if &data[pos..pos + 4] == b"PK\x05\x06" {
            return Some(pos);
        }
        if pos == min_pos {
            break;
        }
        pos -= 1;
    }
    None
}

fn read_u16(data: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([data[off], data[off + 1]])
}

fn read_u32(data: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
}

// --- Bounded RFC 1951 Deflate Inflater (zero external crate bloat, fail-closed on zip bombs) ---
struct BitReader<'a> {
    data: &'a [u8],
    byte_pos: usize,
    bit_buf: u32,
    bit_count: u8,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            byte_pos: 0,
            bit_buf: 0,
            bit_count: 0,
        }
    }

    fn read_bits(&mut self, n: u8) -> Result<u32, ImporterError> {
        while self.bit_count < n {
            if self.byte_pos >= self.data.len() {
                return Err(ImporterError::MalformedPayload {
                    format: "zip".to_string(),
                    reason: "Unexpected EOF in DEFLATE stream".to_string(),
                });
            }
            self.bit_buf |= (self.data[self.byte_pos] as u32) << self.bit_count;
            self.byte_pos += 1;
            self.bit_count += 8;
        }
        let mask = (1u32 << n) - 1;
        let out = self.bit_buf & mask;
        self.bit_buf >>= n;
        self.bit_count -= n;
        Ok(out)
    }

    fn align_byte(&mut self) {
        let drop_bits = self.bit_count % 8;
        self.bit_buf >>= drop_bits;
        self.bit_count -= drop_bits;
    }

    fn read_aligned_u16(&mut self) -> Result<u16, ImporterError> {
        let lo = self.read_bits(8)? as u16;
        let hi = self.read_bits(8)? as u16;
        Ok(lo | (hi << 8))
    }
}

struct HuffmanTree {
    counts: [u16; 16],
    symbols: Vec<u16>,
}

impl HuffmanTree {
    fn from_lengths(lengths: &[u8]) -> Result<Self, ImporterError> {
        let mut counts = [0u16; 16];
        for &len in lengths {
            if len as usize >= 16 {
                return Err(ImporterError::MalformedPayload {
                    format: "zip".to_string(),
                    reason: "Invalid Huffman code length".to_string(),
                });
            }
            if len > 0 {
                counts[len as usize] += 1;
            }
        }
        let mut offsets = [0usize; 16];
        let mut total = 0usize;
        for bits in 1..16 {
            offsets[bits] = total;
            total += counts[bits] as usize;
        }
        let mut symbols = vec![0u16; total];
        for (sym, &len) in lengths.iter().enumerate() {
            if len > 0 {
                let idx = offsets[len as usize];
                symbols[idx] = sym as u16;
                offsets[len as usize] += 1;
            }
        }
        Ok(Self { counts, symbols })
    }

    fn decode(&self, reader: &mut BitReader<'_>) -> Result<u16, ImporterError> {
        let mut code: i32 = 0;
        let mut first: i32 = 0;
        let mut index: usize = 0;
        for len in 1..16 {
            code |= reader.read_bits(1)? as i32;
            let count = self.counts[len] as i32;
            if code - count < first {
                return Ok(self.symbols[index + (code - first) as usize]);
            }
            index += count as usize;
            first = (first + count) << 1;
            code <<= 1;
        }
        Err(ImporterError::MalformedPayload {
            format: "zip".to_string(),
            reason: "Invalid Huffman symbol in DEFLATE stream".to_string(),
        })
    }
}

fn fixed_huffman_trees() -> Result<(HuffmanTree, HuffmanTree), ImporterError> {
    let mut lit = [0u8; 288];
    lit[0..=143].fill(8);
    lit[144..=255].fill(9);
    lit[256..=279].fill(7);
    lit[280..=287].fill(8);
    let dist = [5u8; 32];
    Ok((
        HuffmanTree::from_lengths(&lit)?,
        HuffmanTree::from_lengths(&dist)?,
    ))
}

fn dynamic_huffman_trees(
    reader: &mut BitReader<'_>,
) -> Result<(HuffmanTree, HuffmanTree), ImporterError> {
    const ORDER: [usize; 19] = [
        16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
    ];
    let hlit = reader.read_bits(5)? as usize + 257;
    let hdist = reader.read_bits(5)? as usize + 1;
    let hclen = reader.read_bits(4)? as usize + 4;
    let mut cl_lens = [0u8; 19];
    for i in 0..hclen {
        cl_lens[ORDER[i]] = reader.read_bits(3)? as u8;
    }
    let cl_tree = HuffmanTree::from_lengths(&cl_lens)?;
    let mut lengths = Vec::with_capacity(hlit + hdist);
    while lengths.len() < hlit + hdist {
        let sym = cl_tree.decode(reader)?;
        match sym {
            0..=15 => lengths.push(sym as u8),
            16 => {
                let prev = *lengths
                    .last()
                    .ok_or_else(|| ImporterError::MalformedPayload {
                        format: "zip".to_string(),
                        reason: "Repeat code without previous length".to_string(),
                    })?;
                let repeat = reader.read_bits(2)? as usize + 3;
                lengths.resize(lengths.len() + repeat, prev);
            }
            17 => {
                let repeat = reader.read_bits(3)? as usize + 3;
                lengths.resize(lengths.len() + repeat, 0);
            }
            18 => {
                let repeat = reader.read_bits(7)? as usize + 11;
                lengths.resize(lengths.len() + repeat, 0);
            }
            _ => {
                return Err(ImporterError::MalformedPayload {
                    format: "zip".to_string(),
                    reason: "Invalid dynamic Huffman code length symbol".to_string(),
                });
            }
        }
    }
    Ok((
        HuffmanTree::from_lengths(&lengths[..hlit])?,
        HuffmanTree::from_lengths(&lengths[hlit..hlit + hdist])?,
    ))
}

fn inflate_raw_bounded(
    input: &[u8],
    max_output: usize,
    compressed_size: u64,
    running_total: u64,
) -> Result<Vec<u8>, ImporterError> {
    const LEN_BASE: [usize; 29] = [
        3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115,
        131, 163, 195, 227, 258,
    ];
    const LEN_EXTRA: [u8; 29] = [
        0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
    ];
    const DIST_BASE: [usize; 30] = [
        1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
        2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
    ];
    const DIST_EXTRA: [u8; 30] = [
        0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12,
        13, 13,
    ];

    let mut reader = BitReader::new(input);
    let mut out = Vec::new();
    let check_budget = |next_len: usize| -> Result<(), ImporterError> {
        if next_len > max_output {
            let projected_total = running_total.saturating_add(next_len as u64);
            if projected_total > MAX_ZIP_UNCOMPRESSED_BYTES {
                return Err(ImporterError::ZipSizeLimitExceeded {
                    bytes: projected_total,
                    max: MAX_ZIP_UNCOMPRESSED_BYTES,
                });
            }
            let ratio = if compressed_size == 0 {
                f64::INFINITY
            } else {
                (next_len as f64) / (compressed_size as f64)
            };
            return Err(ImporterError::ZipRatioExceeded {
                ratio: format!("{ratio:.1}"),
                max: format!("{MAX_ZIP_EXPANSION_RATIO:.1}"),
            });
        }
        Ok(())
    };

    loop {
        let bfinal = reader.read_bits(1)?;
        let btype = reader.read_bits(2)?;
        match btype {
            0 => {
                reader.align_byte();
                let len = reader.read_aligned_u16()? as usize;
                let nlen = reader.read_aligned_u16()?;
                if (len as u16) != !nlen {
                    return Err(ImporterError::MalformedPayload {
                        format: "zip".to_string(),
                        reason: "Corrupt stored DEFLATE block length".to_string(),
                    });
                }
                check_budget(out.len().saturating_add(len))?;
                for _ in 0..len {
                    out.push(reader.read_bits(8)? as u8);
                }
            }
            1 | 2 => {
                let (lit_tree, dist_tree) = if btype == 1 {
                    fixed_huffman_trees()?
                } else {
                    dynamic_huffman_trees(&mut reader)?
                };
                loop {
                    let sym = lit_tree.decode(&mut reader)?;
                    if sym < 256 {
                        check_budget(out.len().saturating_add(1))?;
                        out.push(sym as u8);
                    } else if sym == 256 {
                        break;
                    } else {
                        let len_idx = (sym as usize).saturating_sub(257);
                        if len_idx >= LEN_BASE.len() {
                            return Err(ImporterError::MalformedPayload {
                                format: "zip".to_string(),
                                reason: "Invalid DEFLATE length code".to_string(),
                            });
                        }
                        let length =
                            LEN_BASE[len_idx] + reader.read_bits(LEN_EXTRA[len_idx])? as usize;
                        let dist_sym = dist_tree.decode(&mut reader)? as usize;
                        if dist_sym >= DIST_BASE.len() {
                            return Err(ImporterError::MalformedPayload {
                                format: "zip".to_string(),
                                reason: "Invalid DEFLATE distance code".to_string(),
                            });
                        }
                        let distance =
                            DIST_BASE[dist_sym] + reader.read_bits(DIST_EXTRA[dist_sym])? as usize;
                        if distance == 0 || distance > out.len() {
                            return Err(ImporterError::MalformedPayload {
                                format: "zip".to_string(),
                                reason: "Invalid DEFLATE back-reference distance".to_string(),
                            });
                        }
                        check_budget(out.len().saturating_add(length))?;
                        for _ in 0..length {
                            let b = out[out.len() - distance];
                            out.push(b);
                        }
                    }
                }
            }
            _ => {
                return Err(ImporterError::MalformedPayload {
                    format: "zip".to_string(),
                    reason: "Invalid DEFLATE block type 3".to_string(),
                });
            }
        }
        if bfinal == 1 {
            break;
        }
    }

    Ok(out)
}

/// Extracts L1-L6 web-clone Design DNA layers from HTML/CSS/JSON/Markdown payloads.
pub fn extract_web_clone_l1_l6(content: &str) -> WebCloneLayers {
    static COLOR_RE: OnceLock<Regex> = OnceLock::new();
    static FONT_RE: OnceLock<Regex> = OnceLock::new();
    static SPACING_RE: OnceLock<Regex> = OnceLock::new();
    static VAR_RE: OnceLock<Regex> = OnceLock::new();

    let color_re = COLOR_RE.get_or_init(|| {
        Regex::new(r"(?:#[0-9a-fA-F]{3,8}\b|var\(--[a-z0-9-]+\)|(?:rgb|hsl)a?\([^)]{1,40}\))")
            .expect("valid color regex")
    });
    let font_re = FONT_RE.get_or_init(|| {
        Regex::new(r"(?i)(?:font-family\s*:\s*([^;\n}]+)|font-size\s*:\s*([0-9.]+(?:px|rem|em)))")
            .expect("valid font regex")
    });
    let spacing_re = SPACING_RE.get_or_init(|| {
        Regex::new(r"(?i)(?:padding|margin|gap|border-radius)\s*:\s*([0-9a-zA-Z().\- ]{1,32})")
            .expect("valid spacing regex")
    });
    let var_re = VAR_RE.get_or_init(|| {
        Regex::new(r"--(?:color|bg|text|action|border|surface|radius|space|font)-[a-z0-9-]+")
            .expect("valid css var regex")
    });

    let mut l1 = BTreeSet::new();
    for m in color_re.find_iter(content).take(24) {
        l1.insert(m.as_str().trim().to_string());
    }
    for m in var_re.find_iter(content).take(24) {
        l1.insert(m.as_str().to_string());
    }

    let mut l2 = BTreeSet::new();
    for cap in font_re.captures_iter(content).take(20) {
        if let Some(m) = cap.get(1).or_else(|| cap.get(2)) {
            l2.insert(m.as_str().trim().trim_matches(['"', '\'']).to_string());
        }
    }
    for heading in ["h1", "h2", "h3", "display", "body", "mono"] {
        if content
            .to_ascii_lowercase()
            .contains(&format!("<{heading}"))
        {
            l2.insert(format!("tag:{heading}"));
        }
    }

    let mut l3 = BTreeSet::new();
    for cap in spacing_re.captures_iter(content).take(20) {
        if let Some(m) = cap.get(1) {
            l3.insert(m.as_str().trim().to_string());
        }
    }

    let lower = content.to_ascii_lowercase();
    let mut l4 = BTreeSet::new();
    for landmark in [
        "header", "nav", "main", "aside", "section", "footer", "grid", "flex", "sidebar", "hero",
    ] {
        if lower.contains(landmark) {
            l4.insert(landmark.to_string());
        }
    }

    let mut l5 = BTreeSet::new();
    for comp in [
        "button", "input", "table", "card", "modal", "drawer", "tabs", "badge", "chip", "toast",
        "chart", "form",
    ] {
        if lower.contains(comp) {
            l5.insert(comp.to_string());
        }
    }

    let mut l6 = BTreeSet::new();
    for cue in [
        "transition",
        "hover",
        "focus",
        "animation",
        "loading",
        "empty",
        "error",
        "skeleton",
        "aria-",
        "data-theme",
    ] {
        if lower.contains(cue) {
            l6.insert(cue.to_string());
        }
    }

    WebCloneLayers {
        l1_palette: l1.into_iter().collect(),
        l2_typography: l2.into_iter().collect(),
        l3_spacing_radius: l3.into_iter().collect(),
        l4_layout_sections: l4.into_iter().collect(),
        l5_components: l5.into_iter().collect(),
        l6_interactions: l6.into_iter().collect(),
    }
}

/// Dispatches lossless import across `.fig`, Figma Clipboard JSON, `.zip`, `.pen`, `.wire`, and `url`/`github`.
pub fn import_design_payload(
    format: SourceFormat,
    payload_text: Option<&str>,
    payload_base64: Option<&str>,
) -> Result<ImportedDesignArtifact, ImporterError> {
    match format {
        SourceFormat::Fig => import_fig(payload_text, payload_base64),
        SourceFormat::Clipboard => import_clipboard(payload_text),
        SourceFormat::Zip => import_zip(payload_text, payload_base64),
        SourceFormat::Pen => import_structured_design(SourceFormat::Pen, payload_text),
        SourceFormat::Wire => import_structured_design(SourceFormat::Wire, payload_text),
        SourceFormat::Url | SourceFormat::Github => import_web_clone(format, payload_text),
    }
}

fn decode_binary_or_text(
    format: &str,
    payload_text: Option<&str>,
    payload_base64: Option<&str>,
) -> Result<Vec<u8>, ImporterError> {
    if let Some(b64) = payload_base64.map(str::trim).filter(|s| !s.is_empty()) {
        return B64_STD
            .decode(b64)
            .map_err(|e| ImporterError::MalformedPayload {
                format: format.to_string(),
                reason: format!("Invalid base64 payload: {e}"),
            });
    }
    if let Some(txt) = payload_text {
        return Ok(txt.as_bytes().to_vec());
    }
    Err(ImporterError::MalformedPayload {
        format: format.to_string(),
        reason: "Payload is required".to_string(),
    })
}

fn import_fig(
    payload_text: Option<&str>,
    payload_base64: Option<&str>,
) -> Result<ImportedDesignArtifact, ImporterError> {
    let raw_bytes = decode_binary_or_text("fig", payload_text, payload_base64)?;
    // Support ZIP-packaged .fig archives (real Figma files start with PK\x03\x04)
    if raw_bytes.starts_with(b"PK\x03\x04") {
        let entries = read_zip_safely(&raw_bytes)?;
        let mut combined_text = String::new();
        let mut raw_map = serde_json::Map::new();
        let mut file_names = Vec::new();
        for entry in entries {
            file_names.push(entry.name.clone());
            let as_str = String::from_utf8_lossy(&entry.bytes).to_string();
            combined_text.push_str(&as_str);
            combined_text.push('\n');
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&as_str) {
                raw_map.insert(entry.name, parsed);
            } else {
                raw_map.insert(
                    entry.name,
                    serde_json::json!({ "byteLength": entry.bytes.len() }),
                );
            }
        }
        let layers = extract_web_clone_l1_l6(&combined_text);
        let dna = serde_json::json!({
            "kind": "fig-archive",
            "files": file_names,
            "layers": layers,
        });
        return Ok(ImportedDesignArtifact {
            format: SourceFormat::Fig,
            design_dna: dna,
            raw_fields: Some(serde_json::Value::Object(raw_map)),
            auto_summary: format!(
                "Lossless .fig archive ({} entries) with preserved raw_fields and L1-L6 tokens.",
                file_names.len()
            ),
            extracted_files: file_names,
            extracted_text: combined_text,
        });
    }

    let text = String::from_utf8_lossy(&raw_bytes).to_string();
    let parsed: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| ImporterError::MalformedPayload {
            format: "fig".to_string(),
            reason: format!("Expected JSON fig-kiwi or ZIP .fig payload: {e}"),
        })?;
    let layers = extract_web_clone_l1_l6(&text);
    let node_name = parsed
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("Figma Document");
    let raw_fields = extract_preserved_raw_fields(&parsed);
    let dna = serde_json::json!({
        "kind": "fig-kiwi",
        "name": node_name,
        "layers": layers,
        "nodes": parsed.get("document").or_else(|| parsed.get("nodes")).cloned().unwrap_or(serde_json::Value::Null),
    });
    Ok(ImportedDesignArtifact {
        format: SourceFormat::Fig,
        design_dna: dna,
        raw_fields: Some(raw_fields),
        auto_summary: format!(
            "Lossless .fig document '{node_name}' preserving raw_fields and design tokens."
        ),
        extracted_files: vec!["document.fig".to_string()],
        extracted_text: text,
    })
}

fn extract_preserved_raw_fields(parsed: &serde_json::Value) -> serde_json::Value {
    match parsed {
        serde_json::Value::Object(map) => {
            if let Some(explicit) = map.get("raw_fields").or_else(|| map.get("rawFields")) {
                return explicit.clone();
            }
            let mut preserved = serde_json::Map::new();
            for (k, v) in map {
                if !matches!(k.as_str(), "name" | "document" | "nodes") {
                    preserved.insert(k.clone(), v.clone());
                }
            }
            if preserved.is_empty() {
                parsed.clone()
            } else {
                serde_json::Value::Object(preserved)
            }
        }
        other => other.clone(),
    }
}

fn import_clipboard(payload_text: Option<&str>) -> Result<ImportedDesignArtifact, ImporterError> {
    let text = payload_text
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ImporterError::MalformedPayload {
            format: "clipboard".to_string(),
            reason: "Clipboard payload cannot be empty".to_string(),
        })?;

    let parsed_opt = serde_json::from_str::<serde_json::Value>(text).ok();
    let layers = extract_web_clone_l1_l6(text);
    let dna = serde_json::json!({
        "kind": "figma-clipboard",
        "structured": parsed_opt.is_some(),
        "layers": layers,
        "clipboardPayload": parsed_opt.clone().unwrap_or_else(|| serde_json::Value::String(text.chars().take(4096).collect())),
    });
    Ok(ImportedDesignArtifact {
        format: SourceFormat::Clipboard,
        design_dna: dna,
        raw_fields: parsed_opt,
        auto_summary: format!(
            "Figma Clipboard import: {} palette tokens, {} components, {} layout landmarks.",
            layers.l1_palette.len(),
            layers.l5_components.len(),
            layers.l4_layout_sections.len()
        ),
        extracted_files: vec!["clipboard.json".to_string()],
        extracted_text: text.to_string(),
    })
}

fn import_zip(
    payload_text: Option<&str>,
    payload_base64: Option<&str>,
) -> Result<ImportedDesignArtifact, ImporterError> {
    let bytes = decode_binary_or_text("zip", payload_text, payload_base64)?;
    let entries = read_zip_safely(&bytes)?;
    let mut combined = String::new();
    let mut files = Vec::new();
    for entry in &entries {
        files.push(entry.name.clone());
        combined.push_str(&String::from_utf8_lossy(&entry.bytes));
        combined.push('\n');
    }
    let layers = extract_web_clone_l1_l6(&combined);
    let dna = serde_json::json!({
        "kind": "design-zip-bundle",
        "fileCount": files.len(),
        "files": files,
        "layers": layers,
    });
    Ok(ImportedDesignArtifact {
        format: SourceFormat::Zip,
        design_dna: dna,
        raw_fields: Some(serde_json::json!({ "entryNames": files })),
        auto_summary: format!(
            "Design .zip bundle ({} verified entries): {} tokens, {} components.",
            files.len(),
            layers.l1_palette.len(),
            layers.l5_components.len()
        ),
        extracted_files: files,
        extracted_text: combined,
    })
}

fn import_structured_design(
    format: SourceFormat,
    payload_text: Option<&str>,
) -> Result<ImportedDesignArtifact, ImporterError> {
    let text = payload_text
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ImporterError::MalformedPayload {
            format: format.as_str().to_string(),
            reason: "Design payload cannot be empty".to_string(),
        })?;
    let parsed = serde_json::from_str::<serde_json::Value>(text).ok();
    let layers = extract_web_clone_l1_l6(text);
    let dna = serde_json::json!({
        "kind": format.as_str(),
        "layers": layers,
        "document": parsed.clone().unwrap_or_else(|| serde_json::Value::String(text.chars().take(4096).collect())),
    });
    Ok(ImportedDesignArtifact {
        format,
        design_dna: dna,
        raw_fields: parsed,
        auto_summary: format!(
            "Lossless .{} design import: {} palette tokens, {} layout sections, {} components.",
            format.as_str(),
            layers.l1_palette.len(),
            layers.l4_layout_sections.len(),
            layers.l5_components.len()
        ),
        extracted_files: vec![format!("design.{}", format.as_str())],
        extracted_text: text.to_string(),
    })
}

fn import_web_clone(
    format: SourceFormat,
    payload_text: Option<&str>,
) -> Result<ImportedDesignArtifact, ImporterError> {
    let text = payload_text.unwrap_or("").trim();
    let layers = extract_web_clone_l1_l6(text);
    let dna = serde_json::json!({
        "kind": "web-clone-l1-l6",
        "sourceFormat": format.as_str(),
        "layers": layers,
    });
    Ok(ImportedDesignArtifact {
        format,
        design_dna: dna,
        raw_fields: None,
        auto_summary: format!(
            "Web-clone L1-L6 DNA: {} palette tokens, {} typography cues, {} layout landmarks, {} components.",
            layers.l1_palette.len(),
            layers.l2_typography.len(),
            layers.l4_layout_sections.len(),
            layers.l5_components.len()
        ),
        extracted_files: vec!["web-clone-dna.json".to_string()],
        extracted_text: text.to_string(),
    })
}

/// Computes canonical SHA-256 checksum for Honest Figma Proof Export.
pub fn compute_proof_payload_and_sha256(record: &ReaReferenceRecord) -> (String, String) {
    let payload = serde_json::json!({
        "schema": "cowok.figma-proof.v1",
        "referenceId": record.id,
        "slug": record.slug,
        "sourceFormat": record.source_format,
        "licenseSpdx": record.license_spdx,
        "firewallVerdict": record.firewall_verdict,
        "designDnaJson": record.design_dna_json,
        "rawFieldsJson": record.raw_fields_json,
    });
    let canonical = serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string());
    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    let digest = format!("{:x}", hasher.finalize());
    (canonical, digest)
}

/// Constant-time equality for hex SHA-256 digests.
fn constant_time_hex_eq(a: &str, b: &str) -> bool {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    if a_bytes.len() != b_bytes.len() {
        return false;
    }
    let mut diff = 0u8;
    for (&x, &y) in a_bytes.iter().zip(b_bytes.iter()) {
        diff |= x.to_ascii_lowercase() ^ y.to_ascii_lowercase();
    }
    diff == 0
}

/// Honest Figma Proof Exporter:
/// - Without a matching `roundtrip_checksum`, emits `awaiting-verification` (`verified: false`).
/// - Upgrades to `proof-verified` ONLY when `roundtrip_checksum` matches the computed SHA-256.
pub fn evaluate_figma_proof_export(
    record: &ReaReferenceRecord,
    roundtrip_checksum: Option<&str>,
) -> FigmaProofExport {
    let (proof_payload_json, computed_sha256) = compute_proof_payload_and_sha256(record);
    match roundtrip_checksum.map(str::trim).filter(|s| !s.is_empty()) {
        Some(supplied) if constant_time_hex_eq(supplied, &computed_sha256) => FigmaProofExport {
            reference_id: record.id.clone(),
            slug: record.slug.clone(),
            proof_status: ProofStatus::ProofVerified,
            verified: true,
            checksum_matched: true,
            computed_sha256,
            proof_payload_json,
            verification_note:
                "Roundtrip SHA-256 checksum verified; status upgraded to proof-verified"
                    .to_string(),
        },
        Some(_) => FigmaProofExport {
            reference_id: record.id.clone(),
            slug: record.slug.clone(),
            proof_status: ProofStatus::AwaitingVerification,
            verified: false,
            checksum_matched: false,
            computed_sha256,
            proof_payload_json,
            verification_note:
                "Supplied roundtrip checksum did not match computed SHA-256; remaining awaiting-verification"
                    .to_string(),
        },
        None => FigmaProofExport {
            reference_id: record.id.clone(),
            slug: record.slug.clone(),
            proof_status: ProofStatus::AwaitingVerification,
            verified: false,
            checksum_matched: false,
            computed_sha256,
            proof_payload_json,
            verification_note:
                "Proof artifact exported in awaiting-verification state; submit matching roundtripChecksum to verify"
                    .to_string(),
        },
    }
}
