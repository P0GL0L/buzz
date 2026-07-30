use std::collections::HashMap;
use std::io::{Cursor, Read};

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use zip::ZipArchive;

use super::model::{
    ChartPreview, DocxBlock, OfficeDocument, OfficeFormat, OfficeLink, SlidePreview, WorksheetCell,
    WorksheetPreview,
};

const MAX_ARCHIVE_ENTRIES: usize = 512;
const MAX_EXPANDED_BYTES: u64 = 64 * 1024 * 1024;
const MAX_ENTRY_BYTES: u64 = 8 * 1024 * 1024;
const MAX_COMPRESSION_RATIO: u64 = 250;
const MAX_DOCX_BLOCKS: usize = 2_000;
const MAX_DOCX_TABLE_ROWS: usize = 500;
const MAX_WORKSHEETS: usize = 32;
const MAX_SHEET_ROWS: usize = 500;
const MAX_SHEET_COLUMNS: usize = 100;
const MAX_SHEET_CELLS: usize = 20_000;
const MAX_SLIDES: usize = 300;
const MAX_OUTPUT_CHARS: usize = 2_000_000;

#[derive(Debug)]
pub(crate) struct ParsedOffice {
    pub(crate) format: OfficeFormat,
    pub(crate) document: OfficeDocument,
    pub(crate) truncated: bool,
    pub(crate) warnings: Vec<String>,
}

struct GuardedArchive<'a> {
    archive: ZipArchive<Cursor<&'a [u8]>>,
    names: Vec<String>,
}

impl<'a> GuardedArchive<'a> {
    fn open(bytes: &'a [u8]) -> Result<Self, String> {
        if bytes.starts_with(&[0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1]) {
            return Err(
                "encrypted or legacy compound Office packages cannot be previewed".to_string(),
            );
        }
        if !bytes.starts_with(b"PK\x03\x04")
            && !bytes.starts_with(b"PK\x05\x06")
            && !bytes.starts_with(b"PK\x07\x08")
        {
            return Err("format mismatch: Office file is not an OOXML ZIP package".to_string());
        }
        let mut archive = ZipArchive::new(Cursor::new(bytes))
            .map_err(|error| format!("malformed OOXML: {error}"))?;
        if archive.len() > MAX_ARCHIVE_ENTRIES {
            return Err(format!(
                "Office package has too many archive entries (max {MAX_ARCHIVE_ENTRIES})"
            ));
        }

        let mut total = 0_u64;
        let mut names = Vec::with_capacity(archive.len());
        for index in 0..archive.len() {
            let file = archive
                .by_index(index)
                .map_err(|error| format!("malformed OOXML entry: {error}"))?;
            if file.encrypted() {
                return Err("encrypted Office packages cannot be previewed".to_string());
            }
            let name = file.name().replace('\\', "/");
            let lower = name.to_ascii_lowercase();
            if lower.contains("../")
                || lower.ends_with("vbaproject.bin")
                || lower.contains("/macros/")
            {
                return Err(if lower.contains("../") {
                    "Office package contains an unsafe archive path".to_string()
                } else {
                    "macro-bearing Office packages are not previewed".to_string()
                });
            }
            if file.size() > MAX_ENTRY_BYTES {
                return Err(format!(
                    "Office package entry is too large: {name} (max {} MiB)",
                    MAX_ENTRY_BYTES / (1024 * 1024)
                ));
            }
            if file.size() > 0
                && (file.compressed_size() == 0
                    || file.size() / file.compressed_size().max(1) > MAX_COMPRESSION_RATIO)
            {
                return Err(format!(
                    "Office package entry has an unsafe compression ratio: {name}"
                ));
            }
            total = total
                .checked_add(file.size())
                .ok_or_else(|| "Office package expanded size overflow".to_string())?;
            if total > MAX_EXPANDED_BYTES {
                return Err(format!(
                    "Office package expands beyond the {} MiB preview limit",
                    MAX_EXPANDED_BYTES / (1024 * 1024)
                ));
            }
            names.push(name);
        }
        Ok(Self { archive, names })
    }

    fn read(&mut self, name: &str) -> Result<Vec<u8>, String> {
        let mut file = self
            .archive
            .by_name(name)
            .map_err(|_| format!("OOXML package is missing {name}"))?;
        let mut bytes = Vec::with_capacity(file.size() as usize);
        file.read_to_end(&mut bytes)
            .map_err(|error| format!("failed to read {name}: {error}"))?;
        Ok(bytes)
    }

    fn read_optional(&mut self, name: &str) -> Result<Option<Vec<u8>>, String> {
        match self.archive.by_name(name) {
            Ok(mut file) => {
                let mut bytes = Vec::with_capacity(file.size() as usize);
                file.read_to_end(&mut bytes)
                    .map_err(|error| format!("failed to read {name}: {error}"))?;
                Ok(Some(bytes))
            }
            Err(zip::result::ZipError::FileNotFound) => Ok(None),
            Err(error) => Err(format!("failed to inspect {name}: {error}")),
        }
    }
}

pub(crate) fn parse_office(bytes: &[u8], format: OfficeFormat) -> Result<ParsedOffice, String> {
    let mut archive = GuardedArchive::open(bytes)?;
    let content_types = archive.read("[Content_Types].xml")?;
    let content_types_text = String::from_utf8_lossy(&content_types).to_ascii_lowercase();
    if content_types_text.contains("macroenabled") || content_types_text.contains("vbaproject") {
        return Err("macro-bearing Office packages are not previewed".to_string());
    }

    let (document, truncated, warnings) = match format {
        OfficeFormat::Docx => parse_docx(&mut archive)?,
        OfficeFormat::Xlsx => parse_xlsx(&mut archive)?,
        OfficeFormat::Pptx => parse_pptx(&mut archive)?,
    };
    Ok(ParsedOffice {
        format,
        document,
        truncated,
        warnings,
    })
}

fn local_name(name: &[u8]) -> &[u8] {
    name.rsplit(|byte| *byte == b':').next().unwrap_or(name)
}

fn attribute(reader: &Reader<&[u8]>, start: &BytesStart<'_>, local: &[u8]) -> Option<String> {
    start.attributes().flatten().find_map(|attr| {
        (local_name(attr.key.as_ref()) == local)
            .then(|| attr.decode_and_unescape_value(reader.decoder()).ok())
            .flatten()
            .map(|value| value.into_owned())
    })
}

fn append_bounded(target: &mut String, value: &str, truncated: &mut bool) {
    let remaining = MAX_OUTPUT_CHARS.saturating_sub(target.len());
    if remaining == 0 {
        *truncated = true;
    } else if value.len() <= remaining {
        target.push_str(value);
    } else {
        let mut boundary = remaining;
        while boundary > 0 && !value.is_char_boundary(boundary) {
            boundary -= 1;
        }
        target.push_str(&value[..boundary]);
        *truncated = true;
    }
}

fn relationship_map(xml: &[u8]) -> Result<HashMap<String, String>, String> {
    let mut reader = Reader::from_reader(xml);
    let mut relationships = HashMap::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(start)) | Ok(Event::Empty(start))
                if local_name(start.name().as_ref()) == b"Relationship" =>
            {
                if let (Some(id), Some(target)) = (
                    attribute(&reader, &start, b"Id"),
                    attribute(&reader, &start, b"Target"),
                ) {
                    relationships.insert(id, target);
                }
            }
            Ok(Event::Eof) => break,
            Err(error) => return Err(format!("malformed OOXML relationships: {error}")),
            _ => {}
        }
    }
    Ok(relationships)
}

fn parse_docx(
    archive: &mut GuardedArchive<'_>,
) -> Result<(OfficeDocument, bool, Vec<String>), String> {
    let document = archive.read("word/document.xml")?;
    let relationships = archive
        .read_optional("word/_rels/document.xml.rels")?
        .map(|xml| relationship_map(&xml))
        .transpose()?
        .unwrap_or_default();
    let mut reader = Reader::from_reader(document.as_slice());
    let mut blocks = Vec::new();
    let mut links = Vec::new();
    let mut paragraph = String::new();
    let mut style = String::new();
    let mut is_list = false;
    let mut in_text = false;
    let mut in_paragraph = false;
    let mut in_table = false;
    let mut table_rows: Vec<Vec<String>> = Vec::new();
    let mut table_row: Vec<String> = Vec::new();
    let mut table_cell = String::new();
    let mut in_cell = false;
    let mut active_link: Option<(String, String)> = None;
    let mut truncated = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(start)) => match local_name(start.name().as_ref()) {
                b"p" => {
                    in_paragraph = true;
                    paragraph.clear();
                    style.clear();
                    is_list = false;
                }
                b"pStyle" => style = attribute(&reader, &start, b"val").unwrap_or_default(),
                b"numPr" => is_list = true,
                b"tbl" => {
                    in_table = true;
                    table_rows.clear();
                }
                b"tr" if in_table => table_row.clear(),
                b"tc" if in_table => {
                    in_cell = true;
                    table_cell.clear();
                }
                b"t" => in_text = true,
                b"tab" => append_bounded(&mut paragraph, "\t", &mut truncated),
                b"br" => append_bounded(&mut paragraph, "\n", &mut truncated),
                b"hyperlink" => {
                    if let Some(id) = attribute(&reader, &start, b"id") {
                        active_link = Some((id, String::new()));
                    }
                }
                _ => {}
            },
            Ok(Event::Empty(start)) => match local_name(start.name().as_ref()) {
                b"pStyle" => style = attribute(&reader, &start, b"val").unwrap_or_default(),
                b"numPr" => is_list = true,
                b"tab" => append_bounded(&mut paragraph, "\t", &mut truncated),
                b"br" => append_bounded(&mut paragraph, "\n", &mut truncated),
                _ => {}
            },
            Ok(Event::Text(text)) if in_text => {
                let value = text
                    .decode()
                    .map_err(|error| format!("malformed DOCX text: {error}"))?;
                append_bounded(&mut paragraph, &value, &mut truncated);
                if in_cell {
                    append_bounded(&mut table_cell, &value, &mut truncated);
                }
                if let Some((_, link_text)) = &mut active_link {
                    append_bounded(link_text, &value, &mut truncated);
                }
            }
            Ok(Event::End(end)) => match local_name(end.name().as_ref()) {
                b"t" => in_text = false,
                b"hyperlink" => {
                    if let Some((id, text)) = active_link.take() {
                        if let Some(target) = relationships.get(&id) {
                            links.push(OfficeLink {
                                text,
                                target: target.clone(),
                            });
                        }
                    }
                }
                b"p" => {
                    in_paragraph = false;
                    let text = paragraph.trim().to_string();
                    if !in_table && !text.is_empty() && blocks.len() < MAX_DOCX_BLOCKS {
                        let lower = style.to_ascii_lowercase();
                        let block = if lower.starts_with("heading") {
                            let level = lower
                                .trim_start_matches("heading")
                                .parse::<u8>()
                                .unwrap_or(1)
                                .clamp(1, 6);
                            DocxBlock::Heading { level, text }
                        } else if is_list {
                            DocxBlock::List { text }
                        } else {
                            DocxBlock::Paragraph { text }
                        };
                        blocks.push(block);
                    } else if blocks.len() >= MAX_DOCX_BLOCKS {
                        truncated = true;
                    }
                }
                b"tc" if in_table => {
                    in_cell = false;
                    table_row.push(table_cell.trim().to_string());
                }
                b"tr" if in_table => {
                    if table_rows.len() < MAX_DOCX_TABLE_ROWS {
                        table_rows.push(std::mem::take(&mut table_row));
                    } else {
                        truncated = true;
                    }
                }
                b"tbl" => {
                    in_table = false;
                    if !table_rows.is_empty() && blocks.len() < MAX_DOCX_BLOCKS {
                        blocks.push(DocxBlock::Table {
                            rows: std::mem::take(&mut table_rows),
                        });
                    }
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(error) => return Err(format!("malformed DOCX XML: {error}")),
            _ => {}
        }
    }
    if in_paragraph || in_table || in_cell {
        return Err("malformed DOCX XML: unclosed document structure".to_string());
    }
    Ok((
        OfficeDocument::Docx { blocks, links },
        truncated,
        Vec::new(),
    ))
}

fn collect_text(xml: &[u8]) -> Result<Vec<String>, String> {
    let mut reader = Reader::from_reader(xml);
    let mut in_text = false;
    let mut values = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(start)) if local_name(start.name().as_ref()) == b"t" => in_text = true,
            Ok(Event::Text(text)) if in_text => {
                let value = text
                    .decode()
                    .map_err(|error| format!("malformed OOXML text: {error}"))?;
                if !value.trim().is_empty() {
                    values.push(value.into_owned());
                }
            }
            Ok(Event::End(end)) if local_name(end.name().as_ref()) == b"t" => in_text = false,
            Ok(Event::Eof) => break,
            Err(error) => return Err(format!("malformed OOXML XML: {error}")),
            _ => {}
        }
    }
    Ok(values)
}

fn normalize_xl_target(target: &str) -> String {
    let trimmed = target.trim_start_matches('/');
    if trimmed.starts_with("xl/") {
        trimmed.to_string()
    } else {
        format!("xl/{}", trimmed.trim_start_matches("../"))
    }
}

fn parse_workbook_sheets(xml: &[u8]) -> Result<Vec<(String, String)>, String> {
    let mut reader = Reader::from_reader(xml);
    let mut sheets = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(start)) | Ok(Event::Empty(start))
                if local_name(start.name().as_ref()) == b"sheet" =>
            {
                if let (Some(name), Some(id)) = (
                    attribute(&reader, &start, b"name"),
                    attribute(&reader, &start, b"id"),
                ) {
                    sheets.push((name, id));
                }
            }
            Ok(Event::Eof) => break,
            Err(error) => return Err(format!("malformed XLSX workbook: {error}")),
            _ => {}
        }
    }
    Ok(sheets)
}

fn parse_shared_strings(xml: &[u8]) -> Result<Vec<String>, String> {
    let mut reader = Reader::from_reader(xml);
    let mut values = Vec::new();
    let mut current = String::new();
    let mut in_item = false;
    let mut in_text = false;
    let mut truncated = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(start)) => match local_name(start.name().as_ref()) {
                b"si" => {
                    in_item = true;
                    current.clear();
                }
                b"t" if in_item => in_text = true,
                _ => {}
            },
            Ok(Event::Text(text)) if in_text => {
                let value = text
                    .decode()
                    .map_err(|error| format!("malformed XLSX shared strings: {error}"))?;
                append_bounded(&mut current, &value, &mut truncated);
            }
            Ok(Event::End(end)) => match local_name(end.name().as_ref()) {
                b"t" => in_text = false,
                b"si" => {
                    in_item = false;
                    values.push(std::mem::take(&mut current));
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(error) => return Err(format!("malformed XLSX shared strings: {error}")),
            _ => {}
        }
    }
    Ok(values)
}

fn column_index(reference: &str) -> usize {
    let letters = reference
        .bytes()
        .take_while(|byte| byte.is_ascii_alphabetic());
    letters.fold(0_usize, |value, byte| {
        value
            .saturating_mul(26)
            .saturating_add((byte.to_ascii_uppercase() - b'A' + 1) as usize)
    })
}

fn parse_worksheet(
    xml: &[u8],
    name: String,
    shared: &[String],
) -> Result<WorksheetPreview, String> {
    let mut reader = Reader::from_reader(xml);
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut cell_reference = String::new();
    let mut cell_type = String::new();
    let mut value = String::new();
    let mut formula = String::new();
    let mut in_value = false;
    let mut in_formula = false;
    let mut in_inline_text = false;
    let mut cells = 0_usize;
    let mut truncated = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(start)) => match local_name(start.name().as_ref()) {
                b"row" => row.clear(),
                b"c" => {
                    cell_reference = attribute(&reader, &start, b"r").unwrap_or_default();
                    cell_type = attribute(&reader, &start, b"t").unwrap_or_default();
                    value.clear();
                    formula.clear();
                }
                b"v" => in_value = true,
                b"f" => in_formula = true,
                b"t" if cell_type == "inlineStr" => in_inline_text = true,
                _ => {}
            },
            Ok(Event::Text(text)) => {
                let decoded = text
                    .decode()
                    .map_err(|error| format!("malformed XLSX cell: {error}"))?;
                if in_value || in_inline_text {
                    value.push_str(&decoded);
                } else if in_formula {
                    formula.push_str(&decoded);
                }
            }
            Ok(Event::End(end)) => match local_name(end.name().as_ref()) {
                b"v" => in_value = false,
                b"f" => in_formula = false,
                b"t" => in_inline_text = false,
                b"c" => {
                    let column = column_index(&cell_reference);
                    if cells < MAX_SHEET_CELLS && column <= MAX_SHEET_COLUMNS {
                        let rendered = match cell_type.as_str() {
                            "s" => value
                                .parse::<usize>()
                                .ok()
                                .and_then(|index| shared.get(index))
                                .cloned()
                                .unwrap_or_default(),
                            "b" => match value.as_str() {
                                "1" => "TRUE".to_string(),
                                "0" => "FALSE".to_string(),
                                _ => value.clone(),
                            },
                            _ => value.clone(),
                        };
                        row.push(WorksheetCell {
                            reference: cell_reference.clone(),
                            value: rendered,
                            formula: (!formula.is_empty()).then(|| formula.clone()),
                        });
                        cells += 1;
                    } else {
                        truncated = true;
                    }
                }
                b"row" => {
                    if rows.len() < MAX_SHEET_ROWS {
                        rows.push(std::mem::take(&mut row));
                    } else {
                        truncated = true;
                    }
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(error) => return Err(format!("malformed XLSX worksheet: {error}")),
            _ => {}
        }
    }
    Ok(WorksheetPreview {
        name,
        rows,
        truncated,
    })
}

fn parse_chart(xml: &[u8], fallback: String) -> Result<ChartPreview, String> {
    let texts = collect_text(xml)?;
    let title = texts.first().cloned().unwrap_or(fallback);
    let mut reader = Reader::from_reader(xml);
    let mut chart_type = "chart".to_string();
    loop {
        match reader.read_event() {
            Ok(Event::Start(start)) | Ok(Event::Empty(start)) => {
                let name = String::from_utf8_lossy(local_name(start.name().as_ref())).to_string();
                if name.ends_with("Chart") && name != "chart" {
                    chart_type = name.trim_end_matches("Chart").to_string();
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(error) => return Err(format!("malformed XLSX chart: {error}")),
            _ => {}
        }
    }
    Ok(ChartPreview { title, chart_type })
}

fn parse_xlsx(
    archive: &mut GuardedArchive<'_>,
) -> Result<(OfficeDocument, bool, Vec<String>), String> {
    let workbook = archive.read("xl/workbook.xml")?;
    let workbook_rels = archive.read("xl/_rels/workbook.xml.rels")?;
    let sheet_defs = parse_workbook_sheets(&workbook)?;
    let relationships = relationship_map(&workbook_rels)?;
    let shared = archive
        .read_optional("xl/sharedStrings.xml")?
        .map(|xml| parse_shared_strings(&xml))
        .transpose()?
        .unwrap_or_default();
    let mut sheets = Vec::new();
    let mut warnings = Vec::new();
    let mut truncated = sheet_defs.len() > MAX_WORKSHEETS;
    for (name, id) in sheet_defs.into_iter().take(MAX_WORKSHEETS) {
        let Some(target) = relationships.get(&id) else {
            warnings.push(format!("Worksheet {name} has no package relationship"));
            continue;
        };
        let path = normalize_xl_target(target);
        match archive.read(&path) {
            Ok(xml) => {
                let sheet = parse_worksheet(&xml, name, &shared)?;
                truncated |= sheet.truncated;
                sheets.push(sheet);
            }
            Err(error) => warnings.push(error),
        }
    }

    let chart_names: Vec<String> = archive
        .names
        .iter()
        .filter(|name| name.starts_with("xl/charts/") && name.ends_with(".xml"))
        .take(64)
        .cloned()
        .collect();
    let mut charts = Vec::new();
    for name in chart_names {
        if let Ok(xml) = archive.read(&name) {
            charts.push(parse_chart(&xml, name)?);
        }
    }
    Ok((OfficeDocument::Xlsx { sheets, charts }, truncated, warnings))
}

fn numbered_part(name: &str, prefix: &str, suffix: &str) -> Option<usize> {
    name.strip_prefix(prefix)?
        .strip_suffix(suffix)?
        .parse::<usize>()
        .ok()
}

fn parse_pptx(
    archive: &mut GuardedArchive<'_>,
) -> Result<(OfficeDocument, bool, Vec<String>), String> {
    archive.read("ppt/presentation.xml")?;
    let mut slide_parts: Vec<(usize, String)> = archive
        .names
        .iter()
        .filter_map(|name| {
            numbered_part(name, "ppt/slides/slide", ".xml").map(|number| (number, name.clone()))
        })
        .collect();
    slide_parts.sort_by_key(|(number, _)| *number);
    let mut truncated = slide_parts.len() > MAX_SLIDES;
    let mut slides = Vec::new();
    for (number, path) in slide_parts.into_iter().take(MAX_SLIDES) {
        let text = collect_text(&archive.read(&path)?)?;
        let title = text
            .first()
            .cloned()
            .unwrap_or_else(|| format!("Slide {number}"));
        let body = text.into_iter().skip(1).collect();
        let notes_path = format!("ppt/notesSlides/notesSlide{number}.xml");
        let notes = archive
            .read_optional(&notes_path)?
            .map(|xml| collect_text(&xml))
            .transpose()?
            .unwrap_or_default();
        if slides.len() >= MAX_SLIDES {
            truncated = true;
            break;
        }
        slides.push(SlidePreview {
            number,
            title,
            body,
            notes,
        });
    }
    Ok((OfficeDocument::Pptx { slides }, truncated, Vec::new()))
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};

    use zip::write::SimpleFileOptions;

    use super::{parse_office, OfficeDocument, OfficeFormat};

    fn package(entries: &[(&str, &str)]) -> Vec<u8> {
        let mut output = Vec::new();
        {
            let mut writer = zip::ZipWriter::new(Cursor::new(&mut output));
            for (name, body) in entries {
                writer
                    .start_file(name, SimpleFileOptions::default())
                    .unwrap();
                writer.write_all(body.as_bytes()).unwrap();
            }
            writer.finish().unwrap();
        }
        output
    }

    #[test]
    fn extracts_docx_headings_lists_tables_and_links() {
        let bytes = package(&[
            ("[Content_Types].xml", "<Types/>"),
            (
                "word/_rels/document.xml.rels",
                r#"<Relationships><Relationship Id="rId7" Target="https://example.com"/></Relationships>"#,
            ),
            (
                "word/document.xml",
                r#"<w:document xmlns:w="w" xmlns:r="r"><w:body>
                <w:p><w:pPr><w:pStyle w:val="Heading2"/></w:pPr><w:r><w:t>Plan</w:t></w:r></w:p>
                <w:p><w:pPr><w:numPr/></w:pPr><w:hyperlink r:id="rId7"><w:r><w:t>Evidence</w:t></w:r></w:hyperlink></w:p>
                <w:tbl><w:tr><w:tc><w:p><w:r><w:t>A1</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
                </w:body></w:document>"#,
            ),
        ]);
        let parsed = parse_office(&bytes, OfficeFormat::Docx).unwrap();
        let OfficeDocument::Docx { blocks, links } = parsed.document else {
            panic!("expected DOCX");
        };
        assert_eq!(blocks.len(), 3);
        assert_eq!(links[0].target, "https://example.com");
    }

    #[test]
    fn extracts_xlsx_shared_strings_sparse_cells_and_formulas() {
        let bytes = package(&[
            ("[Content_Types].xml", "<Types/>"),
            (
                "xl/workbook.xml",
                r#"<workbook xmlns:r="r"><sheets><sheet name="Data" r:id="rId1"/></sheets></workbook>"#,
            ),
            (
                "xl/_rels/workbook.xml.rels",
                r#"<Relationships><Relationship Id="rId1" Target="worksheets/sheet1.xml"/></Relationships>"#,
            ),
            ("xl/sharedStrings.xml", "<sst><si><t>Revenue</t></si></sst>"),
            (
                "xl/worksheets/sheet1.xml",
                r#"<worksheet><sheetData><row r="1"><c r="A1" t="s"><v>0</v></c><c r="C1"><f>SUM(A2:A3)</f><v>42</v></c></row></sheetData></worksheet>"#,
            ),
        ]);
        let parsed = parse_office(&bytes, OfficeFormat::Xlsx).unwrap();
        let OfficeDocument::Xlsx { sheets, .. } = parsed.document else {
            panic!("expected XLSX");
        };
        assert_eq!(sheets[0].rows[0][0].value, "Revenue");
        assert_eq!(sheets[0].rows[0][1].formula.as_deref(), Some("SUM(A2:A3)"));
    }

    #[test]
    fn extracts_pptx_slides_in_numeric_order_with_notes() {
        let bytes = package(&[
            ("[Content_Types].xml", "<Types/>"),
            ("ppt/presentation.xml", "<p:presentation xmlns:p=\"p\"/>"),
            (
                "ppt/slides/slide2.xml",
                "<p:sld xmlns:p=\"p\" xmlns:a=\"a\"><a:t>Second</a:t><a:t>Body</a:t></p:sld>",
            ),
            (
                "ppt/slides/slide1.xml",
                "<p:sld xmlns:p=\"p\" xmlns:a=\"a\"><a:t>First</a:t></p:sld>",
            ),
            (
                "ppt/notesSlides/notesSlide2.xml",
                "<p:notes xmlns:p=\"p\" xmlns:a=\"a\"><a:t>Speaker note</a:t></p:notes>",
            ),
        ]);
        let parsed = parse_office(&bytes, OfficeFormat::Pptx).unwrap();
        let OfficeDocument::Pptx { slides } = parsed.document else {
            panic!("expected PPTX");
        };
        assert_eq!(slides[0].title, "First");
        assert_eq!(slides[1].notes, vec!["Speaker note"]);
    }

    #[test]
    fn rejects_macro_bearing_packages_and_signature_mismatch() {
        let macro_package = package(&[
            (
                "[Content_Types].xml",
                "<Types><Override ContentType=\"application/vnd.ms-word.document.macroEnabled.main+xml\"/></Types>",
            ),
            ("word/document.xml", "<document/>"),
        ]);
        assert!(parse_office(&macro_package, OfficeFormat::Docx)
            .unwrap_err()
            .contains("macro-bearing"));
        assert!(parse_office(b"not a zip", OfficeFormat::Docx)
            .unwrap_err()
            .contains("format mismatch"));
        assert!(parse_office(
            &[0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1],
            OfficeFormat::Docx
        )
        .unwrap_err()
        .contains("encrypted or legacy"));
    }
}
