use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum OfficeFormat {
    Docx,
    Xlsx,
    Pptx,
}

impl OfficeFormat {
    pub(crate) fn from_filename(filename: &str) -> Result<Self, String> {
        let lower = filename.trim().to_ascii_lowercase();
        if lower.ends_with(".docx") {
            Ok(Self::Docx)
        } else if lower.ends_with(".xlsx") {
            Ok(Self::Xlsx)
        } else if lower.ends_with(".pptx") {
            Ok(Self::Pptx)
        } else {
            Err("Office preview supports DOCX, XLSX, and PPTX files only".to_string())
        }
    }

    pub(crate) fn expected_mime(self) -> &'static str {
        match self {
            Self::Docx => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            Self::Xlsx => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            Self::Pptx => {
                "application/vnd.openxmlformats-officedocument.presentationml.presentation"
            }
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OfficePreview {
    pub(crate) version: u16,
    pub(crate) format: OfficeFormat,
    pub(crate) document: OfficeDocument,
    pub(crate) truncated: bool,
    pub(crate) warnings: Vec<String>,
    pub(crate) fidelity: Option<OfficeFidelity>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub(crate) enum OfficeDocument {
    Docx {
        blocks: Vec<DocxBlock>,
        links: Vec<OfficeLink>,
    },
    Xlsx {
        sheets: Vec<WorksheetPreview>,
        charts: Vec<ChartPreview>,
    },
    Pptx {
        slides: Vec<SlidePreview>,
    },
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub(crate) enum DocxBlock {
    Paragraph { text: String },
    Heading { level: u8, text: String },
    List { text: String },
    Table { rows: Vec<Vec<String>> },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OfficeLink {
    pub(crate) text: String,
    pub(crate) target: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorksheetPreview {
    pub(crate) name: String,
    pub(crate) rows: Vec<Vec<WorksheetCell>>,
    pub(crate) truncated: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorksheetCell {
    pub(crate) reference: String,
    pub(crate) value: String,
    pub(crate) formula: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChartPreview {
    pub(crate) title: String,
    pub(crate) chart_type: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SlidePreview {
    pub(crate) number: usize,
    pub(crate) title: String,
    pub(crate) body: Vec<String>,
    pub(crate) notes: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OfficeFidelity {
    pub(crate) renderer: &'static str,
    pub(crate) html: String,
    pub(crate) truncated: bool,
}
