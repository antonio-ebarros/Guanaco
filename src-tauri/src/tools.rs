use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::AppHandle;

const DOC_EXTENSIONS: &[&str] = &[
    "pdf", "docx", "odt", "txt", "md", "markdown", "csv", "json", "toml", "yaml", "yml", "py",
    "js", "ts", "rs", "html", "css", "log", "xml", "ini", "cfg", "sh", "bat", "c", "cpp", "h",
    "java", "go", "sql",
];

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RagSource {
    pub file: String,
    pub section: String,
    pub snippet: String,
    pub score: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeFolderStats {
    pub folder: String,
    pub exists: bool,
    pub file_count: usize,
    pub pdf_count: usize,
    pub total_bytes: u64,
    pub sample_files: Vec<String>,
}

fn expand_home(raw: &str) -> PathBuf {
    let trimmed = raw.trim();
    if let Some(rest) = trimmed.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(trimmed)
}

fn is_supported_doc_file(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    DOC_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str())
}

fn is_pdf_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("pdf"))
        .unwrap_or(false)
}

fn collect_doc_files(root: &Path, max_depth: usize, max_files: usize) -> Vec<PathBuf> {
    let mut out = Vec::new();
    fn walk(dir: &Path, depth: usize, max_depth: usize, max_files: usize, out: &mut Vec<PathBuf>) {
        if depth > max_depth || out.len() >= max_files {
            return;
        }
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            if out.len() >= max_files {
                break;
            }
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.')
                || matches!(
                    name.as_str(),
                    "node_modules" | "target" | "dist" | "build" | "__pycache__" | "venv"
                )
            {
                continue;
            }
            if path.is_dir() {
                walk(&path, depth + 1, max_depth, max_files, out);
            } else if path.is_file() && is_supported_doc_file(&path) {
                out.push(path);
            }
        }
    }
    walk(root, 0, max_depth, max_files, &mut out);
    out
}

/// Extrai texto de um arquivo PDF usando `pdftotext` (preservando `\x0C` por página) ou fallback nativo em Rust.
pub fn extract_pdf_text_from_path(path: &Path) -> Option<String> {
    if let Ok(out) = Command::new("pdftotext")
        .args(["-q"])
        .arg(path)
        .arg("-")
        .output()
    {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !text.is_empty() {
                return Some(text);
            }
        }
    }

    let bytes = fs::read(path).ok()?;
    extract_pdf_text_pure_rust(&bytes)
}

fn extract_pdf_text_pure_rust(bytes: &[u8]) -> Option<String> {
    let mut out = String::new();
    let mut pos = 0;
    while pos < bytes.len() {
        let Some(s_idx) = find_subslice(&bytes[pos..], b"stream") else {
            break;
        };
        let mut stream_start = pos + s_idx + 6;
        if bytes.get(stream_start) == Some(&b'\r') {
            stream_start += 1;
        }
        if bytes.get(stream_start) == Some(&b'\n') {
            stream_start += 1;
        }
        let Some(e_idx) = find_subslice(&bytes[stream_start..], b"endstream") else {
            break;
        };
        let raw_stream = &bytes[stream_start..stream_start + e_idx];
        pos = stream_start + e_idx + 9;

        let mut decoded = Vec::new();
        let mut z = flate2::read::ZlibDecoder::new(raw_stream);
        let content_bytes = if z.read_to_end(&mut decoded).is_ok() && !decoded.is_empty() {
            &decoded[..]
        } else {
            raw_stream
        };

        extract_pdf_parenthesized_strings(content_bytes, &mut out);
    }
    let cleaned = out.trim().to_string();
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn extract_pdf_parenthesized_strings(stream: &[u8], out: &mut String) {
    let mut i = 0;
    while i < stream.len() {
        if stream[i] == b'(' {
            i += 1;
            let mut depth = 1;
            let mut token = String::new();
            while i < stream.len() && depth > 0 {
                let b = stream[i];
                if b == b'\\' && i + 1 < stream.len() {
                    let next = stream[i + 1];
                    match next {
                        b'n' => token.push('\n'),
                        b'r' => token.push(' '),
                        b't' => token.push(' '),
                        b'(' | b')' | b'\\' => token.push(next as char),
                        _ => {}
                    }
                    i += 2;
                    continue;
                }
                if b == b'(' {
                    depth += 1;
                } else if b == b')' {
                    depth -= 1;
                    if depth == 0 {
                        i += 1;
                        break;
                    }
                }
                if b >= 32 && b <= 126 || b >= 160 {
                    token.push(b as char);
                }
                i += 1;
            }
            if !token.trim().is_empty() {
                out.push_str(token.trim());
                out.push(' ');
            }
        } else {
            i += 1;
        }
    }
    if !out.ends_with('\n') {
        out.push('\n');
    }
}

fn is_office_doc_file(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?;
    if ext.eq_ignore_ascii_case("docx") {
        Some("word/document.xml")
    } else if ext.eq_ignore_ascii_case("odt") {
        Some("content.xml")
    } else {
        None
    }
}

fn le_u16(bytes: &[u8], off: usize) -> Option<u16> {
    let slice = bytes.get(off..off + 2)?;
    Some(u16::from_le_bytes([slice[0], slice[1]]))
}

fn le_u32(bytes: &[u8], off: usize) -> Option<u32> {
    let slice = bytes.get(off..off + 4)?;
    Some(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn find_eocd_offset(bytes: &[u8]) -> Option<usize> {
    if bytes.len() < 22 {
        return None;
    }
    let min_pos = bytes.len().saturating_sub(65_557);
    let mut i = bytes.len() - 22;
    loop {
        if &bytes[i..i + 4] == b"PK\x05\x06" {
            return Some(i);
        }
        if i == min_pos {
            break;
        }
        i -= 1;
    }
    None
}

pub fn read_zip_entry_bytes(bytes: &[u8], target_entry: &str) -> Option<Vec<u8>> {
    let mut pos = find_eocd_offset(bytes)
        .and_then(|eocd| le_u32(bytes, eocd + 16).map(|v| v as usize))
        .unwrap_or(0);

    if bytes.get(pos..pos + 4) != Some(b"PK\x01\x02") {
        pos = find_subslice(bytes, b"PK\x01\x02")?;
    }

    while bytes.get(pos..pos + 4) == Some(b"PK\x01\x02") {
        let method = le_u16(bytes, pos + 10)?;
        let comp_size = le_u32(bytes, pos + 20)? as usize;
        let fname_len = le_u16(bytes, pos + 28)? as usize;
        let extra_len = le_u16(bytes, pos + 30)? as usize;
        let comment_len = le_u16(bytes, pos + 32)? as usize;
        let local_off = le_u32(bytes, pos + 42)? as usize;
        let name_end = pos + 46 + fname_len;
        let name_bytes = bytes.get(pos + 46..name_end)?;
        let entry_name = std::str::from_utf8(name_bytes).unwrap_or("");

        if entry_name == target_entry {
            if bytes.get(local_off..local_off + 4) != Some(b"PK\x03\x04") {
                return None;
            }
            let lfh_fname = le_u16(bytes, local_off + 26)? as usize;
            let lfh_extra = le_u16(bytes, local_off + 28)? as usize;
            let data_start = local_off + 30 + lfh_fname + lfh_extra;
            let data_end = data_start.checked_add(comp_size)?;
            let raw_data = bytes.get(data_start..data_end)?;
            if method == 0 {
                return Some(raw_data.to_vec());
            } else if method == 8 {
                let mut out = Vec::new();
                let mut dec = flate2::read::DeflateDecoder::new(raw_data);
                dec.read_to_end(&mut out).ok()?;
                return Some(out);
            }
            return None;
        }

        let step = 46 + fname_len + extra_len + comment_len;
        if step == 0 {
            break;
        }
        pos += step;
    }
    None
}

pub fn strip_office_xml_tags(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len() / 3);
    let mut chars = xml.chars().peekable();
    let mut in_tag = false;
    let mut tag_buf = String::new();

    while let Some(ch) = chars.next() {
        if ch == '<' {
            in_tag = true;
            tag_buf.clear();
            continue;
        }
        if in_tag {
            if ch == '>' {
                in_tag = false;
                let tag = tag_buf.trim();
                if tag == "/w:p"
                    || tag == "/text:p"
                    || tag == "/text:h"
                    || tag == "/w:tr"
                    || tag.starts_with("w:br")
                    || tag.starts_with("text:line-break")
                    || tag.starts_with("w:cr")
                {
                    if !out.ends_with("\n\n") {
                        out.push('\n');
                    }
                    if tag.starts_with('/') && !out.ends_with("\n\n") {
                        out.push('\n');
                    }
                } else if tag.starts_with("w:tab") || tag.starts_with("text:tab") {
                    out.push('\t');
                }
            } else {
                tag_buf.push(ch);
            }
            continue;
        }
        if ch == '&' {
            let mut entity = String::new();
            let mut found_semi = false;
            for _ in 0..8 {
                if let Some(&next) = chars.peek() {
                    chars.next();
                    if next == ';' {
                        found_semi = true;
                        break;
                    }
                    entity.push(next);
                } else {
                    break;
                }
            }
            if found_semi {
                match entity.as_str() {
                    "amp" => out.push('&'),
                    "lt" => out.push('<'),
                    "gt" => out.push('>'),
                    "quot" => out.push('"'),
                    "apos" => out.push('\''),
                    _ if entity.starts_with('#') => {
                        let code_opt = if let Some(hex) = entity.strip_prefix("#x") {
                            u32::from_str_radix(hex, 16).ok()
                        } else {
                            entity[1..].parse::<u32>().ok()
                        };
                        if let Some(c) = code_opt.and_then(char::from_u32) {
                            out.push(c);
                        }
                    }
                    _ => {
                        out.push('&');
                        out.push_str(&entity);
                        out.push(';');
                    }
                }
            } else {
                out.push('&');
                out.push_str(&entity);
            }
        } else {
            out.push(ch);
        }
    }

    let cleaned_lines: Vec<String> = out
        .lines()
        .map(|l| l.trim().to_string())
        .collect();
    cleaned_lines.join("\n").trim().to_string()
}

pub fn extract_office_xml_text(path: &Path, target_xml: &str) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    let xml_bytes = read_zip_entry_bytes(&bytes, target_xml)?;
    let xml = String::from_utf8_lossy(&xml_bytes);
    let text = strip_office_xml_tags(&xml);
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn read_document_text(path: &Path) -> Option<String> {
    let meta = fs::metadata(path).ok()?;
    if is_pdf_file(path) {
        if meta.len() > 25_000_000 {
            return None;
        }
        extract_pdf_text_from_path(path)
    } else if let Some(target_xml) = is_office_doc_file(path) {
        if meta.len() > 25_000_000 {
            return None;
        }
        extract_office_xml_text(path, target_xml)
    } else {
        if meta.len() > 600_000 {
            return None;
        }
        fs::read_to_string(path).ok()
    }
}

#[tauri::command]
pub async fn pick_knowledge_folder() -> Result<Option<String>, String> {
    tokio::task::spawn_blocking(|| -> Result<Option<String>, String> {
        #[cfg(not(target_os = "windows"))]
        {
            if let Ok(out) = Command::new("zenity")
                .args([
                    "--file-selection",
                    "--directory",
                    "--title=Escolher Pasta de Documentos & PDFs Locais",
                ])
                .output()
            {
                if out.status.success() {
                    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    if !s.is_empty() {
                        return Ok(Some(s));
                    }
                }
                return Ok(None);
            }
            if let Ok(out) = Command::new("kdialog")
                .args(["--getexistingdirectory", "."])
                .output()
            {
                if out.status.success() {
                    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    if !s.is_empty() {
                        return Ok(Some(s));
                    }
                }
                return Ok(None);
            }
            Err("Nenhum seletor gráfico (zenity/kdialog) encontrado. Digite o caminho da pasta.".to_string())
        }
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            let script = "Add-Type -AssemblyName System.Windows.Forms; $f = New-Object System.Windows.Forms.FolderBrowserDialog; if ($f.ShowDialog() -eq 'OK') { [Console]::Out.Write($f.SelectedPath) }";
            let out = Command::new("powershell")
                .args(["-NoProfile", "-Sta", "-Command", script])
                .creation_flags(0x08000000)
                .output()
                .map_err(|e| e.to_string())?;
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if s.is_empty() {
                Ok(None)
            } else {
                Ok(Some(s))
            }
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn scan_knowledge_folder(folder: String) -> KnowledgeFolderStats {
    let path = expand_home(&folder);
    if !path.is_dir() {
        return KnowledgeFolderStats {
            folder,
            exists: false,
            file_count: 0,
            pdf_count: 0,
            total_bytes: 0,
            sample_files: Vec::new(),
        };
    }
    let files = collect_doc_files(&path, 3, 250);
    let mut total_bytes = 0u64;
    let mut pdf_count = 0usize;
    let mut sample_files = Vec::new();
    for f in &files {
        if is_pdf_file(f) {
            pdf_count += 1;
        }
        if let Ok(meta) = fs::metadata(f) {
            total_bytes += meta.len();
        }
        if sample_files.len() < 5 {
            let rel = f
                .strip_prefix(&path)
                .unwrap_or(f)
                .to_string_lossy()
                .to_string();
            sample_files.push(rel);
        }
    }
    KnowledgeFolderStats {
        folder: path.to_string_lossy().to_string(),
        exists: true,
        file_count: files.len(),
        pdf_count,
        total_bytes,
        sample_files,
    }
}

#[tauri::command]
pub async fn export_conversation_markdown(
    app: AppHandle,
    title: String,
    markdown: String,
) -> Result<Option<String>, String> {
    if !crate::license::is_pro_active(&app) {
        return Err("Exportação em Markdown é um recurso do Guanaco Pro.".to_string());
    }
    let slug: String = title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    let default_name = if slug.is_empty() {
        "conversa-guanaco.md".to_string()
    } else {
        format!("{slug}.md")
    };

    tokio::task::spawn_blocking(move || -> Result<Option<String>, String> {
        let chosen_path: Option<PathBuf> = {
            #[cfg(not(target_os = "windows"))]
            {
                if let Ok(out) = Command::new("zenity")
                    .args([
                        "--file-selection",
                        "--save",
                        "--confirm-overwrite",
                        "--title=Exportar conversa em Markdown (.md)",
                        &format!("--filename={default_name}"),
                    ])
                    .output()
                {
                    if out.status.success() {
                        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                        if !s.is_empty() {
                            Some(PathBuf::from(s))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            #[cfg(target_os = "windows")]
            {
                use std::os::windows::process::CommandExt;
                let script = format!(
                    "Add-Type -AssemblyName System.Windows.Forms; $f = New-Object System.Windows.Forms.SaveFileDialog; $f.Filter = 'Markdown (*.md)|*.md'; $f.FileName = '{}'; if ($f.ShowDialog() -eq 'OK') {{ [Console]::Out.Write($f.FileName) }}",
                    default_name.replace('\'', "")
                );
                let out = Command::new("powershell")
                    .args(["-NoProfile", "-Sta", "-Command", &script])
                    .creation_flags(0x08000000)
                    .output()
                    .ok();
                out.and_then(|o| {
                    let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                    if s.is_empty() { None } else { Some(PathBuf::from(s)) }
                })
            }
        };

        let Some(dest) = chosen_path else {
            return Ok(None);
        };
        fs::write(&dest, markdown).map_err(|e| e.to_string())?;
        Ok(Some(dest.to_string_lossy().to_string()))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Avaliador matemático determinístico (ativo tanto no Guanaco Gratuito quanto no Pro!).
pub fn try_eval_math_in_text(query: &str) -> Option<(String, f64)> {
    let mut best_expr = String::new();
    let mut cur = String::new();
    let mut has_op = false;
    let mut digit_groups = 0;
    let mut prev_was_digit = false;

    for ch in query.chars() {
        let mapped = if ch == ',' {
            '.'
        } else if ch == '×' {
            '*'
        } else if ch == '÷' {
            '/'
        } else {
            ch
        };
        if mapped.is_ascii_digit() || mapped == '.' {
            if !prev_was_digit {
                digit_groups += 1;
                prev_was_digit = true;
            }
            cur.push(mapped);
        } else if matches!(mapped, '+' | '-' | '*' | '/' | '^' | '(' | ')') {
            prev_was_digit = false;
            if matches!(mapped, '+' | '-' | '*' | '/' | '^') {
                has_op = true;
            }
            cur.push(mapped);
        } else if mapped == ' ' && !cur.is_empty() {
            prev_was_digit = false;
            cur.push(' ');
        } else {
            if has_op && digit_groups >= 2 && cur.trim().len() > best_expr.len() {
                best_expr = cur.trim().to_string();
            }
            cur.clear();
            has_op = false;
            digit_groups = 0;
            prev_was_digit = false;
        }
    }
    if has_op && digit_groups >= 2 && cur.trim().len() > best_expr.len() {
        best_expr = cur.trim().to_string();
    }
    let clean = best_expr
        .trim_matches(|c: char| !c.is_ascii_digit() && c != '(' && c != ')')
        .trim()
        .to_string();
    if clean.len() < 3 || !clean.chars().any(|c| matches!(c, '+' | '-' | '*' | '/' | '^')) {
        return None;
    }
    let val = eval_expr(&clean)?;
    if val.is_finite() {
        Some((clean, val))
    } else {
        None
    }
}

fn eval_expr(s: &str) -> Option<f64> {
    struct Parser<'a> {
        bytes: &'a [u8],
        pos: usize,
    }
    impl<'a> Parser<'a> {
        fn skip_ws(&mut self) {
            while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_whitespace() {
                self.pos += 1;
            }
        }
        fn parse_expr(&mut self) -> Option<f64> {
            let mut v = self.parse_term()?;
            loop {
                self.skip_ws();
                if self.pos >= self.bytes.len() {
                    break;
                }
                let op = self.bytes[self.pos];
                if op == b'+' || op == b'-' {
                    self.pos += 1;
                    let rhs = self.parse_term()?;
                    v = if op == b'+' { v + rhs } else { v - rhs };
                } else {
                    break;
                }
            }
            Some(v)
        }
        fn parse_term(&mut self) -> Option<f64> {
            let mut v = self.parse_power()?;
            loop {
                self.skip_ws();
                if self.pos >= self.bytes.len() {
                    break;
                }
                let op = self.bytes[self.pos];
                if op == b'*' || op == b'/' {
                    self.pos += 1;
                    let rhs = self.parse_power()?;
                    if op == b'/' && rhs == 0.0 {
                        return None;
                    }
                    v = if op == b'*' { v * rhs } else { v / rhs };
                } else {
                    break;
                }
            }
            Some(v)
        }
        fn parse_power(&mut self) -> Option<f64> {
            let base = self.parse_factor()?;
            self.skip_ws();
            if self.pos < self.bytes.len() && self.bytes[self.pos] == b'^' {
                self.pos += 1;
                let exp = self.parse_power()?;
                Some(base.powf(exp))
            } else {
                Some(base)
            }
        }
        fn parse_factor(&mut self) -> Option<f64> {
            self.skip_ws();
            if self.pos >= self.bytes.len() {
                return None;
            }
            if self.bytes[self.pos] == b'+' {
                self.pos += 1;
                return self.parse_factor();
            }
            if self.bytes[self.pos] == b'-' {
                self.pos += 1;
                return Some(-self.parse_factor()?);
            }
            if self.bytes[self.pos] == b'(' {
                self.pos += 1;
                let v = self.parse_expr()?;
                self.skip_ws();
                if self.pos < self.bytes.len() && self.bytes[self.pos] == b')' {
                    self.pos += 1;
                    return Some(v);
                }
                return None;
            }
            let start = self.pos;
            while self.pos < self.bytes.len()
                && (self.bytes[self.pos].is_ascii_digit() || self.bytes[self.pos] == b'.')
            {
                self.pos += 1;
            }
            if self.pos == start {
                return None;
            }
            std::str::from_utf8(&self.bytes[start..self.pos])
                .ok()?
                .parse::<f64>()
                .ok()
        }
    }
    let mut p = Parser {
        bytes: s.as_bytes(),
        pos: 0,
    };
    let res = p.parse_expr()?;
    p.skip_ws();
    if p.pos == p.bytes.len() {
        Some(res)
    } else {
        None
    }
}

pub fn normalize_for_search(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        for lc in ch.to_lowercase() {
            let mapped = match lc {
                'á' | 'à' | 'ã' | 'â' | 'ä' => 'a',
                'é' | 'è' | 'ê' | 'ë' => 'e',
                'í' | 'ì' | 'î' | 'ï' => 'i',
                'ó' | 'ò' | 'õ' | 'ô' | 'ö' => 'o',
                'ú' | 'ù' | 'û' | 'ü' => 'u',
                'ç' => 'c',
                'ñ' => 'n',
                _ => lc,
            };
            out.push(mapped);
        }
    }
    out
}

fn split_text_chunks(text: &str, max_chars: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut buf = String::new();
    for para in text.split("\n\n") {
        let p = para.trim();
        if p.is_empty() {
            continue;
        }
        if !buf.is_empty() && buf.chars().count() + p.chars().count() + 2 > max_chars {
            chunks.push(std::mem::take(&mut buf));
        }
        if p.chars().count() > max_chars {
            if !buf.is_empty() {
                chunks.push(std::mem::take(&mut buf));
            }
            let mut cur = String::new();
            for line in p.lines() {
                if !cur.is_empty() && cur.chars().count() + line.chars().count() + 1 > max_chars {
                    chunks.push(std::mem::take(&mut cur));
                }
                if !cur.is_empty() {
                    cur.push('\n');
                }
                cur.push_str(line);
            }
            if !cur.is_empty() {
                chunks.push(cur);
            }
        } else {
            if !buf.is_empty() {
                buf.push_str("\n\n");
            }
            buf.push_str(p);
        }
    }
    if !buf.is_empty() {
        chunks.push(buf);
    }
    chunks
}

pub fn extract_document_chunks(content: &str, is_pdf: bool) -> Vec<(String, String)> {
    let mut out = Vec::new();
    if is_pdf && content.contains('\x0C') {
        for (idx, page_raw) in content.split('\x0C').enumerate() {
            let trimmed = page_raw.trim();
            if trimmed.is_empty() {
                continue;
            }
            let label = format!("Pág. {}", idx + 1);
            for chunk in split_text_chunks(trimmed, 680) {
                out.push((label.clone(), chunk));
            }
        }
        return out;
    }

    let mut current_heading: Option<String> = None;
    let mut buf = String::new();
    let mut start_line = 1usize;
    let mut last_line = 1usize;

    let flush = |buf: &mut String,
                 start_l: usize,
                 end_l: usize,
                 heading: &Option<String>,
                 out: &mut Vec<(String, String)>| {
        let trimmed = buf.trim();
        if !trimmed.is_empty() {
            let section = heading
                .clone()
                .unwrap_or_else(|| format!("Linhas {start_l}–{end_l}"));
            out.push((section, trimmed.to_string()));
        }
        buf.clear();
    };

    for (idx, raw_line) in content.lines().enumerate() {
        let line_no = idx + 1;
        let trimmed = raw_line.trim();
        let is_heading = (trimmed.starts_with("# ")
            || trimmed.starts_with("## ")
            || trimmed.starts_with("### ")
            || trimmed.starts_with("#### "))
            && trimmed.len() > 2;

        if is_heading {
            flush(&mut buf, start_line, last_line, &current_heading, &mut out);
            let clean_h = trimmed.trim_start_matches('#').trim();
            let short_h: String = clean_h.chars().take(48).collect();
            current_heading = Some(format!("Seção: {short_h}"));
            start_line = line_no;
            buf.push_str(raw_line);
            last_line = line_no;
            continue;
        }

        if trimmed.is_empty() {
            if buf.chars().count() >= 380 {
                flush(&mut buf, start_line, last_line, &current_heading, &mut out);
                start_line = line_no + 1;
            } else if !buf.is_empty() && !buf.ends_with("\n\n") {
                buf.push('\n');
            }
            continue;
        }

        if buf.is_empty() {
            start_line = line_no;
        } else {
            buf.push('\n');
        }
        buf.push_str(raw_line);
        last_line = line_no;

        if buf.chars().count() >= 680 {
            flush(&mut buf, start_line, last_line, &current_heading, &mut out);
            start_line = line_no + 1;
        }
    }
    flush(&mut buf, start_line, last_line, &current_heading, &mut out);
    out
}

pub fn search_knowledge_folder(folder_raw: &str, query: &str) -> Option<(String, Vec<RagSource>)> {
    let root = expand_home(folder_raw);
    if !root.is_dir() {
        return None;
    }
    let files = collect_doc_files(&root, 3, 250);
    if files.is_empty() {
        return None;
    }

    let keywords: Vec<String> = normalize_for_search(query)
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 3)
        .map(String::from)
        .collect();

    let mut scored: Vec<RagSource> = Vec::new();

    for file_path in &files {
        let Some(content) = read_document_text(file_path) else {
            continue;
        };
        let rel_name = file_path
            .strip_prefix(&root)
            .unwrap_or(file_path)
            .to_string_lossy()
            .to_string();
        let rel_norm = normalize_for_search(&rel_name);
        let chunks = extract_document_chunks(&content, is_pdf_file(file_path));

        for (section, chunk_text) in chunks {
            let chunk_norm = normalize_for_search(&chunk_text);
            let sec_norm = normalize_for_search(&section);
            let mut score = 0usize;
            let mut distinct = 0usize;

            for kw in &keywords {
                let mut hit = false;
                if rel_norm.contains(kw) {
                    score += 4;
                    hit = true;
                }
                if sec_norm.contains(kw) {
                    score += 5;
                    hit = true;
                }
                let matches_in_body = chunk_norm.match_indices(kw).take(3).count();
                if matches_in_body > 0 {
                    score += 3 + (matches_in_body.saturating_sub(1)) * 2;
                    hit = true;
                }
                if hit {
                    distinct += 1;
                }
            }
            if distinct >= 2 {
                score += distinct * 5;
            }
            if score > 0 || (keywords.is_empty() && scored.len() < 2) {
                let snippet: String = chunk_text.chars().take(720).collect();
                scored.push(RagSource {
                    file: rel_name.clone(),
                    section,
                    snippet,
                    score: score.max(1),
                });
            }
        }
    }

    scored.sort_by(|a, b| b.score.cmp(&a.score));
    if scored.is_empty() {
        return None;
    }

    let top: Vec<RagSource> = scored.into_iter().take(5).collect();
    let mut out = String::from(
        "Trechos relevantes encontrados nos documentos locais do usuário (RAG 2.0):\n",
    );
    for src in &top {
        out.push_str(&format!(
            "\n--- [Fonte: {} ({})] ---\n{}\n",
            src.file, src.section, src.snippet
        ));
    }
    Some((out, top))
}

fn local_datetime_string() -> String {
    if let Ok(out) = Command::new("date")
        .arg("+%Y-%m-%d %H:%M:%S (%A)")
        .output()
    {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() {
                return s;
            }
        }
    }
    "horário local ativo".to_string()
}

/// Constrói o contexto inteligente:
/// - Data/hora local exata e Calculadora Matemática Exata estão SEMPRE ativas (no Gratuito e no Pro!)
/// - A busca em Pasta de Documentos & PDFs Locais (RAG Offline) é exclusiva do Guanaco Pro e retorna as fontes consultadas.
pub fn build_tools_augmentation(
    last_user_query: &str,
    pro_active: bool,
    knowledge_folder: Option<&str>,
) -> (Option<String>, Vec<RagSource>) {
    let mut blocks = Vec::new();
    let mut matched_sources = Vec::new();

    blocks.push(format!(
        "[Contexto do Sistema em Tempo Real]: Data e hora local: {} | Sistema Operacional: {}.",
        local_datetime_string(),
        std::env::consts::OS
    ));
    if let Some((expr, val)) = try_eval_math_in_text(last_user_query) {
        let formatted = if (val - val.round()).abs() < 1e-9 {
            format!("{:.0}", val)
        } else {
            format!("{:.4}", val)
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_string()
        };
        blocks.push(format!(
            "[Calculadora Exata Nativa]: O resultado matemático exato de `{expr}` é `{formatted}`. Use este valor exato na sua resposta."
        ));
    }

    if pro_active {
        if let Some(folder) = knowledge_folder.map(str::trim).filter(|s| !s.is_empty()) {
            if let Some((rag_ctx, sources)) = search_knowledge_folder(folder, last_user_query) {
                blocks.push(rag_ctx);
                matched_sources = sources;
            }
        }
    }

    if blocks.is_empty() {
        (None, matched_sources)
    } else {
        (Some(blocks.join("\n\n")), matched_sources)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluates_math_expressions_in_natural_text() {
        let (expr, val) =
            try_eval_math_in_text("Quanto é (1250 * 4) + 350 / 2 no total?").expect("deve calcular");
        assert_eq!(expr, "(1250 * 4) + 350 / 2");
        assert!((val - 5175.0).abs() < 1e-6);
    }

    #[test]
    fn normalizes_portuguese_accents() {
        assert_eq!(normalize_for_search("Configuração & Código Ção"), "configuracao & codigo cao");
    }

    #[test]
    fn strips_docx_and_odt_xml_cleanly() {
        let xml = r#"<w:document><w:body><w:p><w:r><w:t>Olá &amp; bem-vindo</w:t></w:r></w:p><w:p><w:r><w:t>Segunda linha</w:t></w:r></w:p></w:body></w:document>"#;
        let text = strip_office_xml_tags(xml);
        assert!(text.contains("Olá & bem-vindo"));
        assert!(text.contains("Segunda linha"));
    }

    #[test]
    fn extracts_chunks_with_markdown_sections_and_pdf_pages() {
        let md = "# Visão Geral\nTexto introdutório\n\n## Código Secreto\nA senha é ALPHA-99";
        let md_chunks = extract_document_chunks(md, false);
        assert_eq!(md_chunks.len(), 2);
        assert_eq!(md_chunks[0].0, "Seção: Visão Geral");
        assert_eq!(md_chunks[1].0, "Seção: Código Secreto");

        let pdf = "Conteúdo página um\x0CConteúdo página dois";
        let pdf_chunks = extract_document_chunks(pdf, true);
        assert_eq!(pdf_chunks.len(), 2);
        assert_eq!(pdf_chunks[0].0, "Pág. 1");
        assert_eq!(pdf_chunks[1].0, "Pág. 2");
    }
}
