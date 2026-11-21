use serde::Serialize;
use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::AppHandle;

const DOC_EXTENSIONS: &[&str] = &[
    "pdf", "txt", "md", "markdown", "csv", "json", "toml", "yaml", "yml", "py", "js", "ts", "rs",
    "html", "css", "log", "xml", "ini", "cfg", "sh", "bat", "c", "cpp", "h", "java", "go", "sql",
];

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

/// Extrai texto de um arquivo PDF usando `pdftotext` (se instalado) ou fallback nativo em Rust (FlateDecode).
pub fn extract_pdf_text_from_path(path: &Path) -> Option<String> {
    if let Ok(out) = Command::new("pdftotext")
        .args(["-q", "-nopgbrk"])
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

fn read_document_text(path: &Path) -> Option<String> {
    let meta = fs::metadata(path).ok()?;
    if is_pdf_file(path) {
        if meta.len() > 25_000_000 {
            return None;
        }
        extract_pdf_text_from_path(path)
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

pub fn search_knowledge_folder(folder_raw: &str, query: &str) -> Option<(String, Vec<String>)> {
    let root = expand_home(folder_raw);
    if !root.is_dir() {
        return None;
    }
    let files = collect_doc_files(&root, 3, 250);
    if files.is_empty() {
        return None;
    }

    let keywords: Vec<String> = query
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 3)
        .map(String::from)
        .collect();

    let mut scored_chunks: Vec<(usize, String, String)> = Vec::new();

    for file_path in &files {
        let Some(content) = read_document_text(file_path) else {
            continue;
        };
        let rel_name = file_path
            .strip_prefix(&root)
            .unwrap_or(file_path)
            .to_string_lossy()
            .to_string();
        let rel_lower = rel_name.to_lowercase();

        for para in content.split("\n\n") {
            let trimmed = para.trim();
            if trimmed.is_empty() {
                continue;
            }
            let lower = trimmed.to_lowercase();
            let mut score = 0usize;
            for kw in &keywords {
                if rel_lower.contains(kw) {
                    score += 4;
                }
                if lower.contains(kw) {
                    score += 3;
                }
            }
            if score > 0 || (keywords.is_empty() && scored_chunks.len() < 2) {
                let snippet: String = trimmed.chars().take(700).collect();
                scored_chunks.push((score.max(1), rel_name.clone(), snippet));
            }
        }
    }

    scored_chunks.sort_by(|a, b| b.0.cmp(&a.0));
    if scored_chunks.is_empty() {
        return None;
    }

    let mut out = String::from(
        "Trechos relevantes encontrados nos documentos e PDFs locais do usuário:\n",
    );
    let mut sources_set = BTreeSet::new();
    for (_, file_rel, snippet) in scored_chunks.into_iter().take(4) {
        sources_set.insert(file_rel.clone());
        out.push_str(&format!("\n--- [Fonte: {file_rel}] ---\n{snippet}\n"));
    }
    let sources: Vec<String> = sources_set.into_iter().collect();
    Some((out, sources))
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
) -> (Option<String>, Vec<String>) {
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
}
