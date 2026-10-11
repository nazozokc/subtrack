// Minimal XLSX generator (port of lib/src/xlsx.ts)
use std::io::Write;

use flate2::write::DeflateEncoder;
use flate2::Compression;

fn crc32(buf: &[u8]) -> u32 {
    let mut c: u32 = 0xffffffff;
    for &b in buf {
        c = CRC_TABLE[((c ^ (b as u32)) & 0xff) as usize] ^ (c >> 8);
    }
    c ^ 0xffffffff
}

const CRC_TABLE: [u32; 256] = crc_table();

const fn crc_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut n = 0;
    while n < 256 {
        let mut c: u32 = n as u32;
        let mut k = 0;
        while k < 8 {
            if c & 1 == 1 {
                c = 0xedb88320 ^ (c >> 1);
            } else {
                c >>= 1;
            }
            k += 1;
        }
        table[n] = c;
        n += 1;
    }
    table
}

fn u16(v: u16) -> [u8; 2] {
    v.to_le_bytes()
}

fn u32(v: u32) -> [u8; 4] {
    v.to_le_bytes()
}

const DOS_TIME: u16 = 0x0000; // 00:00:00
const DOS_DATE: u16 = 0x5821; // 2024-01-01

fn local_header(name: &str, crc: u32, comp: u32, uncomp: u32, out: &mut Vec<u8>) {
    out.extend_from_slice(&u32(0x04034b50));
    out.extend_from_slice(&u16(20)); // version needed
    out.extend_from_slice(&u16(0)); // flags
    out.extend_from_slice(&u16(8)); // method: deflate
    out.extend_from_slice(&u16(DOS_TIME));
    out.extend_from_slice(&u16(DOS_DATE));
    out.extend_from_slice(&u32(crc));
    out.extend_from_slice(&u32(comp));
    out.extend_from_slice(&u32(uncomp));
    let nameb = name.as_bytes();
    out.extend_from_slice(&u16(nameb.len() as u16));
    out.extend_from_slice(&u16(0));
    out.extend_from_slice(nameb);
}

fn central_dir(name: &str, crc: u32, comp: u32, uncomp: u32, offset: u32, out: &mut Vec<u8>) {
    out.extend_from_slice(&u32(0x02014b50));
    out.extend_from_slice(&u16(20)); // version made by (host=unix)
    out.extend_from_slice(&u16(20)); // version needed
    out.extend_from_slice(&u16(0)); // flags
    out.extend_from_slice(&u16(8)); // method: deflate
    out.extend_from_slice(&u16(DOS_TIME));
    out.extend_from_slice(&u16(DOS_DATE));
    out.extend_from_slice(&u32(crc));
    out.extend_from_slice(&u32(comp));
    out.extend_from_slice(&u32(uncomp));
    let nameb = name.as_bytes();
    out.extend_from_slice(&u16(nameb.len() as u16));
    out.extend_from_slice(&u16(0));
    out.extend_from_slice(&u16(0));
    out.extend_from_slice(&u16(0));
    out.extend_from_slice(&u16(0));
    out.extend_from_slice(&u32(0)); // external attrs
    out.extend_from_slice(&u32(offset));
    out.extend_from_slice(nameb);
}

fn eocd(count: u16, cd_size: u32, cd_offset: u32, out: &mut Vec<u8>) {
    out.extend_from_slice(&u32(0x06054b50));
    out.extend_from_slice(&u16(0));
    out.extend_from_slice(&u16(0));
    out.extend_from_slice(&u16(count));
    out.extend_from_slice(&u16(count));
    out.extend_from_slice(&u32(cd_size));
    out.extend_from_slice(&u32(cd_offset));
    out.extend_from_slice(&u16(0));
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Convert a 1-based column index to an Excel column letter: 1→A … 26→Z, 27→AA.
fn col_letter(n: usize) -> String {
    let mut s = String::new();
    let mut n = n as i64;
    while n > 0 {
        let rem = (n - 1) % 26;
        s.insert(0, (b'A' + rem as u8) as char);
        n = (n - 1) / 26;
    }
    s
}

/// A cell value: numbers render as `<v>`, everything else as inline strings.
pub enum Cell {
    Num(String),
    Text(String),
}

pub fn generate_xlsx(
    headers: &[String],
    rows: &[Vec<Cell>],
    column_widths: &[usize],
    sheet_name: &str,
) -> Vec<u8> {
    let mut parts: Vec<(String, String)> = Vec::new();

    parts.push((
        "[Content_Types].xml".to_string(),
        concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n",
            "<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">",
            "<Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>",
            "<Default Extension=\"xml\" ContentType=\"application/xml\"/>",
            "<Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/>",
            "<Override PartName=\"/xl/worksheets/sheet1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/>",
            "<Override PartName=\"/xl/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml\"/>",
            "</Types>"
        )
        .to_string(),
    ));

    parts.push((
        "_rels/.rels".to_string(),
        concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n",
            "<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">",
            "<Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"xl/workbook.xml\"/>",
            "</Relationships>"
        )
        .to_string(),
    ));

    parts.push((
        "xl/workbook.xml".to_string(),
        format!(
            concat!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n",
                "<workbook xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" ",
                "xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\">",
                "<sheets><sheet name=\"{}\" sheetId=\"1\" r:id=\"rId1\"/></sheets></workbook>"
            ),
            esc(sheet_name)
        ),
    ));

    parts.push((
        "xl/_rels/workbook.xml.rels".to_string(),
        concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n",
            "<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">",
            "<Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet1.xml\"/>",
            "<Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles\" Target=\"styles.xml\"/>",
            "</Relationships>"
        )
        .to_string(),
    ));

    parts.push((
        "xl/styles.xml".to_string(),
        concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n",
            "<styleSheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\">",
            "<fonts count=\"2\">",
            "<font><sz val=\"11\"/><name val=\"Calibri\"/></font>",
            "<font><b/><color rgb=\"FFFFFFFF\"/><sz val=\"11\"/><name val=\"Calibri\"/></font>",
            "</fonts>",
            "<fills count=\"3\">",
            "<fill><patternFill patternType=\"none\"/></fill>",
            "<fill><patternFill patternType=\"gray125\"/></fill>",
            "<fill><patternFill patternType=\"solid\"><fgColor rgb=\"FF4472C4\"/><bgColor indexed=\"64\"/></patternFill></fill>",
            "</fills>",
            "<borders count=\"1\"><border><left/><right/><top/><bottom/><diagonal/></border></borders>",
            "<cellStyleXfs count=\"1\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\"/></cellStyleXfs>",
            "<cellXfs count=\"2\">",
            "<xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\" xfId=\"0\"/>",
            "<xf numFmtId=\"0\" fontId=\"1\" fillId=\"2\" borderId=\"0\" xfId=\"0\" applyFont=\"1\" applyFill=\"1\"/>",
            "</cellXfs>",
            "</styleSheet>"
        )
        .to_string(),
    ));

    let cols_def: String = column_widths
        .iter()
        .enumerate()
        .map(|(i, w)| {
            format!(
                "<col min=\"{}\" max=\"{}\" width=\"{}.0\" customWidth=\"1\"/>",
                i + 1,
                i + 1,
                w
            )
        })
        .collect();

    let mut rows_xml = String::new();
    // Header row (style s="1").
    let mut cells = String::new();
    for (c, h) in headers.iter().enumerate() {
        let reference = format!("{}1", col_letter(c + 1));
        cells.push_str(&format!(
            "<c r=\"{}\" t=\"inlineStr\" s=\"1\"><is><t>{}</t></is></c>",
            reference,
            esc(h)
        ));
    }
    rows_xml.push_str(&format!("<row r=\"1\">{}</row>", cells));

    for (r, row) in rows.iter().enumerate() {
        let mut cells = String::new();
        for (c, cell) in row.iter().enumerate() {
            let reference = format!("{}{}", col_letter(c + 1), r + 2);
            match cell {
                Cell::Num(v) => {
                    cells.push_str(&format!("<c r=\"{}\"><v>{}</v></c>", reference, v));
                }
                Cell::Text(v) => {
                    cells.push_str(&format!(
                        "<c r=\"{}\" t=\"inlineStr\"><is><t>{}</t></is></c>",
                        reference,
                        esc(v)
                    ));
                }
            }
        }
        rows_xml.push_str(&format!("<row r=\"{}\">{}</row>", r + 2, cells));
    }

    parts.push((
        "xl/worksheets/sheet1.xml".to_string(),
        format!(
            concat!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n",
                "<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\">",
                "<cols>{}</cols><sheetData>{}</sheetData>",
                "<sheetViews><sheetView workbookViewId=\"0\"/></sheetViews></worksheet>"
            ),
            cols_def, rows_xml
        ),
    ));

    // ── Build ZIP archive ──────────────────────────────────────
    let mut outbuf: Vec<u8> = Vec::new();
    // name, crc, comp_len, uncomp_len, offset
    let mut entries: Vec<(String, u32, u32, u32, u32)> = Vec::new();

    for (name, data) in &parts {
        let raw = data.as_bytes();
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::new(6));
        encoder.write_all(raw).unwrap();
        let comp = encoder.finish().unwrap();
        let crc = crc32(raw);
        let offset = outbuf.len() as u32;
        local_header(
            name,
            crc,
            comp.len() as u32,
            raw.len() as u32,
            &mut outbuf,
        );
        outbuf.extend_from_slice(&comp);
        entries.push((name.clone(), crc, comp.len() as u32, raw.len() as u32, offset));
    }

    let cd_offset = outbuf.len() as u32;
    let mut cd_buf: Vec<u8> = Vec::new();
    for (name, crc, comp, uncomp, offset) in &entries {
        central_dir(name, *crc, *comp, *uncomp, *offset, &mut cd_buf);
    }
    let cd_size = cd_buf.len() as u32;
    outbuf.extend_from_slice(&cd_buf);
    eocd(entries.len() as u16, cd_size, cd_offset, &mut outbuf);
    outbuf
}
