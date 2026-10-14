//! Rebuild editable Office objects from PDF geometry; never substitute page screenshots.
use super::{error, Job, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    io::{Cursor, Read, Write},
};
fn escape(s: &str) -> String {
    let clean:String=s.chars().filter(|c| matches!(*c,'\t'|'\n'|'\r') || (*c >= '\u{20}' && *c != '\u{fffe}' && *c != '\u{ffff}')).collect();
    super::escape(&clean)
}
fn numeric(s: &str) -> bool {
    let v=s.trim();let digits=v.trim_start_matches(['+','-']);
    let leading_zero=digits.starts_with('0') && digits.as_bytes().get(1).is_some_and(u8::is_ascii_digit);
    !leading_zero && v.parse::<f64>().is_ok_and(f64::is_finite)
}
fn intern(values: &mut Vec<String>, value:String) -> usize {
    if let Some(index)=values.iter().position(|v|v==&value){index}else{let index=values.len();values.push(value);index}
}
#[derive(Clone, Default, Deserialize)]
#[serde(default)]
struct Glyph {
    text: String,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    size: f64,
    font: String,
    color: String,
    bold: bool,
    italic: bool,
}
#[derive(Clone, Default, Deserialize)]
#[serde(default)]
struct Rect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    color: String,
    data: String,
}
#[derive(Clone, Default, Deserialize)]
#[serde(default)]
struct Line {
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    width: f64,
    color: String,
}
#[derive(Default, Deserialize)]
#[serde(default)]
struct Page {
    width: f64,
    height: f64,
    glyphs: Vec<Glyph>,
    vertical: Vec<f64>,
    horizontal: Vec<f64>,
    fills: Vec<Rect>,
    lines: Vec<Line>,
    images: Vec<Rect>,
}
struct Table {
    xs: Vec<f64>,
    ys: Vec<f64>,
    cells: Vec<Vec<Glyph>>,
}
fn num(v: f64) -> f64 {
    if v.is_finite() {
        v.clamp(-100000., 100000.)
    } else {
        0.
    }
}
fn emu(v: f64) -> i64 {
    (num(v) * 12700.).round() as i64
}
fn twip(v: f64) -> i64 {
    (num(v) * 20.).round() as i64
}
fn color(s: &str) -> String {
    if s.len() == 6 && s.chars().all(|c| c.is_ascii_hexdigit()) {
        s.to_uppercase()
    } else {
        "000000".into()
    }
}
fn font(g: &Glyph) -> String {
    let s = g.font.rsplit('+').next().unwrap_or("Carlito");
    if s.is_empty() {
        "Carlito".into()
    } else {
        s.to_owned()
    }
}
fn sorted(mut v: Vec<f64>) -> Vec<f64> {
    v.retain(|n| n.is_finite());
    v.sort_by(f64::total_cmp);
    v.dedup_by(|a, b| (*a - *b).abs() < 0.8);
    v
}
fn coverage(mut intervals:Vec<(f64,f64)>,begin:f64,end:f64)->bool {
 intervals.sort_by(|a,b|a.0.total_cmp(&b.0));let mut cursor=begin;
 for (left,right) in intervals {if left>cursor+1.5{return false;}cursor=cursor.max(right);}
 cursor>=end-1.5
}
fn table(p: &Page) -> Option<Table> {
    let xs = sorted(p.vertical.clone());
    let ys = sorted(p.horizontal.clone());
    if xs.len() < 2 || ys.len() < 2 || xs.len() > 65 || ys.len() > 301 {
        return None;
    }
    for x in &xs {if !coverage(p.lines.iter().filter(|l|(l.x1-x).abs()<0.8&&(l.x2-x).abs()<0.8).map(|l|(l.y1.min(l.y2),l.y1.max(l.y2))).collect(),ys[0],*ys.last().unwrap()){return None;}}
    for y in &ys {if !coverage(p.lines.iter().filter(|l|(l.y1-y).abs()<0.8&&(l.y2-y).abs()<0.8).map(|l|(l.x1.min(l.x2),l.x1.max(l.x2))).collect(),xs[0],*xs.last().unwrap()){return None;}}
    let mut cells = vec![Vec::new(); (xs.len() - 1) * (ys.len() - 1)];
    for g in &p.glyphs {
        let x = g.x + g.w / 2.;
        if let (Some(c), Some(r)) = (
            xs.windows(2).position(|w| x >= w[0] && x <= w[1]),
            ys.windows(2).position(|w| g.y >= w[0] && g.y <= w[1]),
        ) {
            cells[r * (xs.len() - 1) + c].push(g.clone());
        }
    }
    if cells.iter().all(Vec::is_empty) {
        None
    } else {
        Some(Table { xs, ys, cells })
    }
}
fn ordered(mut glyphs: Vec<Glyph>) -> Vec<Glyph> {
    glyphs.sort_by(|a,b|a.y.total_cmp(&b.y).then_with(||a.x.total_cmp(&b.x)));
    let mut start=0;
    while start<glyphs.len(){let baseline=glyphs[start].y;let mut end=start+1;while end<glyphs.len()&&(glyphs[end].y-baseline).abs()<2.{end+=1;}glyphs[start..end].sort_by(|a,b|a.x.total_cmp(&b.x));start=end;}
    glyphs
}
fn text(glyphs: &[Glyph]) -> String {
    let mut out = String::new();
    let mut prev: Option<&Glyph> = None;
    for g in glyphs {
        if let Some(p) = prev {
            if (g.y - p.y).abs() > 2. {
                out.push('\n')
            } else if g.x - p.x - p.w > g.size * 0.25 && !out.ends_with(char::is_whitespace) {
                out.push(' ')
            }
        }
        out.push_str(&g.text);
        prev = Some(g);
    }
    out.chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .collect()
}
fn grouped(p: &Page, skip: Option<&Table>) -> Vec<Vec<Glyph>> {
    let glyphs = ordered(
        p.glyphs
            .iter()
            .filter(|g| {
                !skip.is_some_and(|t| {
                    g.x >= t.xs[0]
                        && g.x <= *t.xs.last().unwrap()
                        && g.y >= t.ys[0]
                        && g.y <= *t.ys.last().unwrap()
                })
            })
            .cloned()
            .collect(),
    );
    let mut out: Vec<Vec<Glyph>> = Vec::new();
    for g in glyphs {
        if let Some(last) = out.last_mut() {
            if (last[0].y - g.y).abs() < 2. && last.last().is_some_and(|previous| g.x - previous.x - previous.w < g.size * 1.5) {
                last.push(g);
                continue;
            }
        }
        out.push(vec![g]);
    }
    out
}
fn entry(files: &mut BTreeMap<String, Vec<u8>>, name: &str, s: String) {
    files.insert(name.into(), s.into_bytes());
}
fn template(bytes: &[u8]) -> Result<BTreeMap<String, Vec<u8>>> {
    let mut z = zip::ZipArchive::new(Cursor::new(bytes)).map_err(error)?;
    let mut out = BTreeMap::new();
    for i in 0..z.len() {
        let mut f = z.by_index(i).map_err(error)?;
        let mut b = Vec::new();
        f.read_to_end(&mut b).map_err(error)?;
        out.insert(f.name().into(), b);
    }
    Ok(out)
}
fn save(files: BTreeMap<String, Vec<u8>>, j: &Job) -> Result<()> {
    super::ensure_output(j)?;
    let partial = format!("{}.partial", j.output);
    let result = (|| {
        let f = fs::File::create(&partial).map_err(error)?;
        let mut z = zip::ZipWriter::new(f);
        let opt = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for (n, b) in files {
            z.start_file(n, opt).map_err(error)?;
            z.write_all(&b).map_err(error)?;
        }
        z.finish().map_err(error)?.sync_all().map_err(error)?;
        fs::rename(&partial, &j.output).map_err(error)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(partial);
    }
    result
}
fn word_run(g: &Glyph, s: &str) -> String {
    let family = escape(&font(g));
    let sz = (g.size.clamp(3., 144.) * 2.).round() as i64;
    format!("<w:r><w:rPr><w:rFonts w:ascii=\"{family}\" w:hAnsi=\"{family}\" w:eastAsia=\"{family}\"/><w:sz w:val=\"{sz}\"/><w:szCs w:val=\"{sz}\"/><w:color w:val=\"{}\"/>{}{}</w:rPr><w:t xml:space=\"preserve\">{}</w:t></w:r>",color(&g.color),if g.bold{"<w:b/>"}else{""},if g.italic{"<w:i/>"}else{""},escape(s))
}
fn word_runs(gs: &[Glyph]) -> String {
    let mut out = String::new();
    let mut pending = String::new();
    let mut style: Option<&Glyph> = None;
    let mut prev: Option<&Glyph> = None;
    for g in gs {
        if let Some(s) = style {
            if s.font != g.font
                || s.color != g.color
                || s.size != g.size
                || s.bold != g.bold
                || s.italic != g.italic
                || (prev.unwrap().y - g.y).abs() > 2.
            {
                out.push_str(&word_run(s, &pending));
                pending.clear();
                if (prev.unwrap().y - g.y).abs() > 2. {
                    out.push_str("<w:r><w:br/></w:r>");
                }
                style = None;
            }
        }
        if let Some(p) = prev {
            if (g.y - p.y).abs() < 2.
                && g.x - p.x - p.w > g.size * 0.25
                && !pending.ends_with(char::is_whitespace)
            {
                pending.push(' ')
            }
        }
        if style.is_none() {
            style = Some(g)
        }
        pending.push_str(&g.text);
        prev = Some(g);
    }
    if let Some(s) = style {
        out.push_str(&word_run(s, &pending));
    }
    out
}
fn anchor(id: usize, x: f64, y: f64, w: f64, h: f64, shape: String, behind: bool) -> String {
    let x = emu(x);
    let y = emu(y);
    let w = emu(w.max(1.));
    let h = emu(h.max(1.));
    format!("<w:r><w:drawing><wp:anchor distT=\"0\" distB=\"0\" distL=\"0\" distR=\"0\" simplePos=\"0\" relativeHeight=\"{id}\" behindDoc=\"{}\" locked=\"0\" layoutInCell=\"1\" allowOverlap=\"1\"><wp:simplePos x=\"0\" y=\"0\"/><wp:positionH relativeFrom=\"page\"><wp:posOffset>{x}</wp:posOffset></wp:positionH><wp:positionV relativeFrom=\"page\"><wp:posOffset>{y}</wp:posOffset></wp:positionV><wp:extent cx=\"{w}\" cy=\"{h}\"/><wp:wrapNone/><wp:docPr id=\"{id}\" name=\"PDF object {id}\"/><wp:cNvGraphicFramePr/><a:graphic>{shape}</a:graphic></wp:anchor></w:drawing></w:r>",if behind{1}else{0})
}
fn word_table(t: &Table, p: &Page) -> String {
    let cols = t.xs.len() - 1;
    let mut out=format!("<w:tbl><w:tblPr><w:tblpPr w:horzAnchor=\"page\" w:vertAnchor=\"page\" w:tblpX=\"{}\" w:tblpY=\"{}\" w:leftFromText=\"0\" w:rightFromText=\"0\" w:topFromText=\"0\" w:bottomFromText=\"0\"/><w:tblW w:w=\"{}\" w:type=\"dxa\"/><w:tblLayout w:type=\"fixed\"/><w:tblBorders>",twip(t.xs[0]),twip(t.ys[0]),twip(t.xs.last().unwrap()-t.xs[0]));
    for edge in ["top", "left", "bottom", "right", "insideH", "insideV"] {
        out.push_str(&format!(
            "<w:{edge} w:val=\"single\" w:sz=\"6\" w:color=\"{}\"/>",
            p.lines
                .first()
                .map(|l| color(&l.color))
                .unwrap_or("000000".into())
        ))
    }
    out.push_str("</w:tblBorders><w:tblCellMar><w:top w:w=\"0\" w:type=\"dxa\"/><w:left w:w=\"0\" w:type=\"dxa\"/><w:bottom w:w=\"0\" w:type=\"dxa\"/><w:right w:w=\"0\" w:type=\"dxa\"/></w:tblCellMar></w:tblPr><w:tblGrid>");
    for xs in t.xs.windows(2) {
        out.push_str(&format!("<w:gridCol w:w=\"{}\"/>", twip(xs[1] - xs[0])))
    }
    out.push_str("</w:tblGrid>");
    for (r, ys) in t.ys.windows(2).enumerate() {
        out.push_str(&format!(
            "<w:tr><w:trPr><w:trHeight w:val=\"{}\" w:hRule=\"exact\"/></w:trPr>",
            twip(ys[1] - ys[0])
        ));
        for (c, xs) in t.xs.windows(2).enumerate() {
            let gs = ordered(t.cells[r * cols + c].clone());
            let fill = p
                .fills
                .iter()
                .rev()
                .find(|f| {
                    f.x <= xs[0] + 1.
                        && f.x + f.w >= xs[1] - 1.
                        && f.y <= ys[0] + 1.
                        && f.y + f.h >= ys[1] - 1.
                })
                .map(|f| color(&f.color))
                .unwrap_or("FFFFFF".into());
            let indent = gs.first().map(|g| twip(g.x - xs[0]).max(0)).unwrap_or(0);
            out.push_str(&format!("<w:tc><w:tcPr><w:tcW w:w=\"{}\" w:type=\"dxa\"/><w:shd w:fill=\"{fill}\"/><w:vAlign w:val=\"center\"/></w:tcPr><w:p><w:pPr><w:spacing w:before=\"0\" w:after=\"0\"/><w:ind w:left=\"{indent}\"/></w:pPr>{}</w:p></w:tc>",twip(xs[1]-xs[0]),word_runs(&gs)));
        }
        out.push_str("</w:tr>");
    }
    out.push_str("</w:tbl>");
    out
}
fn word(pages: &[Page], j: &Job) -> Result<()> {
    let mut files = template(include_bytes!("../../templates/blank.docx"))?;
    let mut body = String::new();
    let mut id = 1;
    let mut image_rels = String::new();
    for (index, p) in pages.iter().enumerate() {
        let t = table(p);
        body.push_str("<w:p><w:pPr><w:spacing w:before=\"0\" w:after=\"0\" w:line=\"20\" w:lineRule=\"exact\"/></w:pPr>");
        for gs in grouped(p, t.as_ref()) {
            let first = &gs[0];
            let left = gs.iter().map(|g| g.x).fold(f64::INFINITY, f64::min);
            let right = gs.iter().map(|g| g.x + g.w).fold(0., f64::max);
            let shape=format!("<a:graphicData uri=\"http://schemas.microsoft.com/office/word/2010/wordprocessingShape\"><wps:wsp><wps:cNvSpPr txBox=\"1\"/><wps:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{}\" cy=\"{}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom><a:noFill/><a:ln><a:noFill/></a:ln></wps:spPr><wps:txbx><w:txbxContent><w:p><w:pPr><w:spacing w:before=\"0\" w:after=\"0\"/></w:pPr>{}</w:p></w:txbxContent></wps:txbx><wps:bodyPr lIns=\"0\" tIns=\"0\" rIns=\"0\" bIns=\"0\" wrap=\"none\"><a:noAutofit/></wps:bodyPr></wps:wsp></a:graphicData>",emu(right-left+8.),emu(first.size*1.8),word_runs(&gs));
            body.push_str(&anchor(
                id,
                left,
                first.y - first.size * 0.8,
                right - left + 8.,
                first.size * 1.8,
                shape,
                false,
            ));
            id += 1;
        }
        for image in &p.images {
            let bytes = STANDARD.decode(&image.data).map_err(error)?;
            if bytes.len() > 16 * 1024 * 1024 {
                return Err("PDF 图片过大".into());
            }
            let name = format!("image{id}.png");
            files.insert(format!("word/media/{name}"), bytes);
            image_rels.push_str(&format!("<Relationship Id=\"img{id}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" Target=\"media/{name}\"/>"));
            let shape=format!("<a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/picture\"><pic:pic><pic:nvPicPr><pic:cNvPr id=\"{id}\" name=\"PDF image\"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed=\"img{id}\"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{}\" cy=\"{}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData>",emu(image.w),emu(image.h));
            body.push_str(&anchor(id, image.x, image.y, image.w, image.h, shape, true));
            id += 1;
        }
        body.push_str("</w:p>");
        if let Some(t) = t {
            body.push_str(&word_table(&t, p));
        }
        let section=format!("<w:sectPr><w:type w:val=\"nextPage\"/><w:pgSz w:w=\"{}\" w:h=\"{}\"/><w:pgMar w:top=\"0\" w:left=\"0\" w:bottom=\"0\" w:right=\"0\" w:header=\"0\" w:footer=\"0\"/></w:sectPr>",twip(p.width),twip(p.height));
        if index + 1 == pages.len() {
            body.push_str(&section)
        } else {
            body.push_str(&format!("<w:p><w:pPr>{section}</w:pPr></w:p>"))
        }
    }
    entry(&mut files,"word/document.xml",format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?><w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:wp=\"http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing\" xmlns:wps=\"http://schemas.microsoft.com/office/word/2010/wordprocessingShape\" xmlns:pic=\"http://schemas.openxmlformats.org/drawingml/2006/picture\"><w:body>{body}</w:body></w:document>"));
    let rel = String::from_utf8(files["word/_rels/document.xml.rels"].clone())
        .map_err(error)?
        .replace("</Relationships>", &(image_rels + "</Relationships>"));
    entry(&mut files, "word/_rels/document.xml.rels", rel);
    let types = String::from_utf8(files["[Content_Types].xml"].clone()).map_err(error)?;
    if !types.contains("Extension=\"png\"") {
        entry(
            &mut files,
            "[Content_Types].xml",
            types.replace(
                "</Types>",
                "<Default Extension=\"png\" ContentType=\"image/png\"/></Types>",
            ),
        );
    }
    save(files, j)
}
fn ppt_runs(gs: &[Glyph]) -> String {
    let mut runs = String::new();
    for g in gs {
        runs.push_str(&format!("<a:r><a:rPr lang=\"zh-CN\" sz=\"{}\" b=\"{}\" i=\"{}\"><a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill><a:latin typeface=\"{}\"/><a:ea typeface=\"{}\"/></a:rPr><a:t>{}</a:t></a:r>",(g.size.clamp(3.,144.)*100.).round() as i64,if g.bold{1}else{0},if g.italic{1}else{0},color(&g.color),escape(&font(g)),escape(&font(g)),escape(&g.text)));
    }
    runs
}
fn ppt_table(t: &Table, p: &Page, id: usize, dx: f64, dy: f64) -> String {
    let mut body=String::from("<a:tbl><a:tblPr/><a:tblGrid>");
    for xs in t.xs.windows(2) {body.push_str(&format!("<a:gridCol w=\"{}\"/>",emu(xs[1]-xs[0])));}
    body.push_str("</a:tblGrid>");
    let cols=t.xs.len()-1;
    for (r,ys) in t.ys.windows(2).enumerate(){
        body.push_str(&format!("<a:tr h=\"{}\">",emu(ys[1]-ys[0])));
        for (c,xs) in t.xs.windows(2).enumerate(){
            let gs=ordered(t.cells[r*cols+c].clone());
            let fill=p.fills.iter().rev().find(|f|f.x<=xs[0]+1.&&f.x+f.w>=xs[1]-1.&&f.y<=ys[0]+1.&&f.y+f.h>=ys[1]-1.).map(|f|color(&f.color)).unwrap_or("FFFFFF".into());
            let inset=gs.first().map(|g|emu((g.x-xs[0]).max(0.))).unwrap_or(0);
            let top=gs.first().map(|g|emu((g.y-g.size*0.8-ys[0]).max(0.))).unwrap_or(0);
            let border_color=p.lines.first().map(|l|color(&l.color)).unwrap_or("000000".into());
            let borders=["lnL","lnR","lnT","lnB"].iter().map(|edge|format!("<a:{edge} w=\"{}\"><a:solidFill><a:srgbClr val=\"{border_color}\"/></a:solidFill><a:prstDash val=\"solid\"/></a:{edge}>",emu(p.lines.first().map(|l|l.width).unwrap_or(0.5)))).collect::<String>();
            body.push_str(&format!("<a:tc><a:txBody><a:bodyPr/><a:lstStyle/><a:p>{}<a:endParaRPr/></a:p></a:txBody><a:tcPr marL=\"{inset}\" marR=\"0\" marT=\"{top}\" marB=\"0\" anchor=\"t\">{borders}<a:solidFill><a:srgbClr val=\"{fill}\"/></a:solidFill></a:tcPr></a:tc>",ppt_runs(&gs)));
        }
        body.push_str("</a:tr>");
    }
    body.push_str("</a:tbl>");
    format!("<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id=\"{id}\" name=\"Editable PDF table\"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr><p:xfrm><a:off x=\"{}\" y=\"{}\"/><a:ext cx=\"{}\" cy=\"{}\"/></p:xfrm><a:graphic><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/table\">{body}</a:graphicData></a:graphic></p:graphicFrame>",emu(t.xs[0]+dx),emu(t.ys[0]+dy),emu(t.xs.last().unwrap()-t.xs[0]),emu(t.ys.last().unwrap()-t.ys[0]))
}
fn ppt(pages: &[Page], j: &Job) -> Result<()> {
    let mut files = template(include_bytes!("../../templates/blank.pptx"))?;
    let width = pages.iter().map(|p| p.width).fold(0., f64::max);
    let height = pages.iter().map(|p| p.height).fold(0., f64::max);
    let mut ids = String::new();
    let mut rels = String::new();
    let mut types = String::from_utf8(files["[Content_Types].xml"].clone()).map_err(error)?;
    for (index, p) in pages.iter().enumerate() {
        let n = index + 1;
        ids.push_str(&format!("<p:sldId id=\"{}\" r:id=\"sld{n}\"/>", 255 + n));
        rels.push_str(&format!("<Relationship Id=\"sld{n}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide\" Target=\"slides/slide{n}.xml\"/>"));
        types=types.replace("</Types>",&format!("<Override PartName=\"/ppt/slides/slide{n}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slide+xml\"/></Types>"));
        let mut shapes = String::new();
        let mut relations=String::from("<Relationship Id=\"layout\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout\" Target=\"../slideLayouts/slideLayout7.xml\"/>");
        let mut id = 2;
        let dx = (width - p.width) / 2.;
        let dy = (height - p.height) / 2.;
        for fill in &p.fills {
            shapes.push_str(&format!("<p:sp><p:nvSpPr><p:cNvPr id=\"{id}\" name=\"PDF background\"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x=\"{}\" y=\"{}\"/><a:ext cx=\"{}\" cy=\"{}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill><a:ln><a:noFill/></a:ln></p:spPr></p:sp>",emu(fill.x+dx),emu(fill.y+dy),emu(fill.w),emu(fill.h),color(&fill.color)));
            id += 1;
        }
        for image in &p.images {
            let name = format!("page{n}-image{id}.png");
            files.insert(
                format!("ppt/media/{name}"),
                STANDARD.decode(&image.data).map_err(error)?,
            );
            relations.push_str(&format!("<Relationship Id=\"img{id}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" Target=\"../media/{name}\"/>"));
            shapes.push_str(&format!("<p:pic><p:nvPicPr><p:cNvPr id=\"{id}\" name=\"PDF image\"/><p:cNvPicPr/><p:nvPr/></p:nvPicPr><p:blipFill><a:blip r:embed=\"img{id}\"/><a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr><a:xfrm><a:off x=\"{}\" y=\"{}\"/><a:ext cx=\"{}\" cy=\"{}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr></p:pic>",emu(image.x+dx),emu(image.y+dy),emu(image.w),emu(image.h)));
            id += 1;
        }
        for l in &p.lines {
            shapes.push_str(&format!("<p:sp><p:nvSpPr><p:cNvPr id=\"{id}\" name=\"PDF line\"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm flipV=\"{}\"><a:off x=\"{}\" y=\"{}\"/><a:ext cx=\"{}\" cy=\"{}\"/></a:xfrm><a:prstGeom prst=\"line\"><a:avLst/></a:prstGeom><a:ln w=\"{}\"><a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill></a:ln></p:spPr></p:sp>",if(l.x2-l.x1)*(l.y2-l.y1)<0.{"1"}else{"0"},emu(l.x1.min(l.x2)+dx),emu(l.y1.min(l.y2)+dy),emu((l.x2-l.x1).abs().max(0.01)),emu((l.y2-l.y1).abs().max(0.01)),emu(l.width.max(0.2)),color(&l.color)));
            id += 1;
        }
        let editable_table = table(p);
        if let Some(t) = &editable_table {shapes.push_str(&ppt_table(t,p,id,dx,dy));id+=1;}
        for gs in grouped(p, editable_table.as_ref()) {
            let g = &gs[0];
            let left = gs.iter().map(|g| g.x).fold(f64::INFINITY, f64::min);
            let right = gs.iter().map(|g| g.x + g.w).fold(0., f64::max);
            let mut runs = String::new();
            for s in &gs {
                runs.push_str(&format!("<a:r><a:rPr lang=\"zh-CN\" sz=\"{}\" b=\"{}\" i=\"{}\"><a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill><a:latin typeface=\"{}\"/><a:ea typeface=\"{}\"/></a:rPr><a:t>{}</a:t></a:r>",(s.size.clamp(3.,144.)*100.).round() as i64,if s.bold{1}else{0},if s.italic{1}else{0},color(&s.color),escape(&font(s)),escape(&font(s)),escape(&s.text)));
            }
            shapes.push_str(&format!("<p:sp><p:nvSpPr><p:cNvPr id=\"{id}\" name=\"Editable PDF text\"/><p:cNvSpPr txBox=\"1\"/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x=\"{}\" y=\"{}\"/><a:ext cx=\"{}\" cy=\"{}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom><a:noFill/><a:ln><a:noFill/></a:ln></p:spPr><p:txBody><a:bodyPr wrap=\"none\" lIns=\"0\" rIns=\"0\" tIns=\"0\" bIns=\"0\"><a:noAutofit/></a:bodyPr><a:lstStyle/><a:p><a:pPr><a:lnSpc><a:spcPct val=\"100000\"/></a:lnSpc></a:pPr>{runs}<a:endParaRPr/></a:p></p:txBody></p:sp>",emu(left+dx),emu(g.y-g.size*0.8+dy),emu(right-left+8.),emu(g.size*1.8)));
            id += 1;
        }
        let xml=format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?><p:sld xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/><a:chOff x=\"0\" y=\"0\"/><a:chExt cx=\"0\" cy=\"0\"/></a:xfrm></p:grpSpPr>{shapes}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>");
        entry(&mut files, &format!("ppt/slides/slide{n}.xml"), xml);
        entry(&mut files,&format!("ppt/slides/_rels/slide{n}.xml.rels"),format!("<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">{relations}</Relationships>"));
    }
    let mut presentation =
        String::from_utf8(files["ppt/presentation.xml"].clone()).map_err(error)?;
    presentation =
        presentation.replace("<p:sldIdLst/>", &format!("<p:sldIdLst>{ids}</p:sldIdLst>"));
    if !presentation.contains("<p:sldIdLst>") {
        presentation = presentation.replace(
            "<p:sldSz",
            &format!("<p:sldIdLst>{ids}</p:sldIdLst><p:sldSz"),
        );
    }
    let re = regex::Regex::new(r#"<p:sldSz[^>]*/>"#).map_err(error)?;
    presentation = re
        .replace(
            &presentation,
            format!(
                "<p:sldSz cx=\"{}\" cy=\"{}\" type=\"custom\"/>",
                emu(width),
                emu(height)
            ),
        )
        .into_owned();
    entry(&mut files, "ppt/presentation.xml", presentation);
    let rel = String::from_utf8(files["ppt/_rels/presentation.xml.rels"].clone())
        .map_err(error)?
        .replace("</Relationships>", &(rels + "</Relationships>"));
    entry(&mut files, "ppt/_rels/presentation.xml.rels", rel);
    if !types.contains("Extension=\"png\"") {
        types = types.replace(
            "</Types>",
            "<Default Extension=\"png\" ContentType=\"image/png\"/></Types>",
        )
    }
    entry(&mut files, "[Content_Types].xml", types);
    save(files, j)
}
fn column(mut i: usize) -> String {
    let mut out = String::new();
    loop {
        out.push((b'A' + (i % 26) as u8) as char);
        if i < 26 {
            break;
        }
        i = i / 26 - 1;
    }
    out.chars().rev().collect()
}
fn spreadsheet(pages: &[Page], j: &Job) -> Result<()> {
    let mut files = BTreeMap::new();
    let mut types = String::new();
    let mut sheets = String::new();
    let mut rels = String::new();
    let mut fonts = vec!["<font><sz val=\"11\"/><name val=\"Carlito\"/></font>".into()];
    let mut fills = vec![
        "<fill><patternFill patternType=\"none\"/></fill>".into(),
        "<fill><patternFill patternType=\"gray125\"/></fill>".into(),
    ];
    let mut styles =
        vec!["<xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\" xfId=\"0\"/>".into()];
    for (index, p) in pages.iter().enumerate() {
        let t = table(p).ok_or_else(|| {
            format!(
                "第 {} 页没有可识别的有边框表格，无法保留单元格结构",
                index + 1
            )
        })?;
        let n = index + 1;
        let mut xml = String::from("<cols>");
        for (c, w) in t.xs.windows(2).enumerate() {
            xml.push_str(&format!(
                "<col min=\"{}\" max=\"{}\" width=\"{:0.3}\" customWidth=\"1\"/>",
                c + 1,
                c + 1,
                (w[1] - w[0]) * 96. / 72. / 7.
            ))
        }
        xml.push_str("</cols><sheetData>");
        let cols = t.xs.len() - 1;
        for (r, y) in t.ys.windows(2).enumerate() {
            xml.push_str(&format!(
                "<row r=\"{}\" ht=\"{}\" customHeight=\"1\">",
                r + 1,
                y[1] - y[0]
            ));
            for (c, x) in t.xs.windows(2).enumerate() {
                let gs = ordered(t.cells[r * cols + c].clone());
                let g = gs.first().cloned().unwrap_or(Glyph {
                    size: 11.,
                    font: "Carlito".into(),
                    color: "000000".into(),
                    ..Default::default()
                });
                let bg = p
                    .fills
                    .iter()
                    .rev()
                    .find(|f| {
                        f.x <= x[0] + 1.
                            && f.x + f.w >= x[1] - 1.
                            && f.y <= y[0] + 1.
                            && f.y + f.h >= y[1] - 1.
                    })
                    .map(|f| color(&f.color))
                    .unwrap_or("FFFFFF".into());
                let font_id = intern(&mut fonts,format!(
                    "<font><sz val=\"{}\"/><name val=\"{}\"/><color rgb=\"FF{}\"/>{}{}</font>",
                    g.size.clamp(3., 144.),
                    escape(&font(&g)),
                    color(&g.color),
                    if g.bold { "<b/>" } else { "" },
                    if g.italic { "<i/>" } else { "" }
                ));
                let fill_id = intern(&mut fills,format!("<fill><patternFill patternType=\"solid\"><fgColor rgb=\"FF{bg}\"/><bgColor indexed=\"64\"/></patternFill></fill>"));
                let style_id = intern(&mut styles,format!("<xf numFmtId=\"0\" fontId=\"{font_id}\" fillId=\"{fill_id}\" borderId=\"1\" xfId=\"0\" applyFont=\"1\" applyFill=\"1\" applyBorder=\"1\" applyAlignment=\"1\"><alignment horizontal=\"center\" vertical=\"center\" wrapText=\"1\"/></xf>"));
                if styles.len()>4096{return Err("表格样式过多，请拆分 PDF 后转换".into());}
                let value = text(&gs);
                let cell = format!("{}{}", column(c), r + 1);
                if numeric(&value) {
                    xml.push_str(&format!(
                        "<c r=\"{cell}\" s=\"{style_id}\"><v>{}</v></c>",
                        escape(&value)
                    ))
                } else {
                    xml.push_str(&format!("<c r=\"{cell}\" s=\"{style_id}\" t=\"inlineStr\"><is><t xml:space=\"preserve\">{}</t></is></c>",escape(&value)))
                }
            }
            xml.push_str("</row>");
        }
        xml.push_str("</sheetData><pageMargins left=\"0.25\" right=\"0.25\" top=\"0.25\" bottom=\"0.25\" header=\"0\" footer=\"0\"/>");
        entry(&mut files,&format!("xl/worksheets/sheet{n}.xml"),format!("<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\">{xml}</worksheet>"));
        types.push_str(&format!("<Override PartName=\"/xl/worksheets/sheet{n}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/>"));
        sheets.push_str(&format!(
            "<sheet name=\"第{n}页\" sheetId=\"{n}\" r:id=\"sheet{n}\"/>"
        ));
        rels.push_str(&format!("<Relationship Id=\"sheet{n}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet{n}.xml\"/>"));
    }
    entry(&mut files,"[Content_Types].xml",format!("<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/><Override PartName=\"/xl/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml\"/>{types}</Types>"));
    entry(&mut files,"_rels/.rels","<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"office\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"xl/workbook.xml\"/></Relationships>".into());
    entry(&mut files,"xl/workbook.xml",format!("<workbook xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><sheets>{sheets}</sheets></workbook>"));
    entry(&mut files,"xl/_rels/workbook.xml.rels",format!("<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">{rels}<Relationship Id=\"styles\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles\" Target=\"styles.xml\"/></Relationships>"));
    entry(&mut files,"xl/styles.xml",format!("<styleSheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><fonts count=\"{}\">{}</fonts><fills count=\"{}\">{}</fills><borders count=\"2\"><border><left/><right/><top/><bottom/><diagonal/></border><border><left style=\"thin\"><color rgb=\"FF000000\"/></left><right style=\"thin\"><color rgb=\"FF000000\"/></right><top style=\"thin\"><color rgb=\"FF000000\"/></top><bottom style=\"thin\"><color rgb=\"FF000000\"/></bottom><diagonal/></border></borders><cellStyleXfs count=\"1\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\"/></cellStyleXfs><cellXfs count=\"{}\">{}</cellXfs><cellStyles count=\"1\"><cellStyle name=\"Normal\" xfId=\"0\" builtinId=\"0\"/></cellStyles></styleSheet>",fonts.len(),fonts.join(""),fills.len(),fills.join(""),styles.len(),styles.join("")));
    save(files, j)
}
pub(super) fn convert(payload: &Value) -> Result<Value> {
    let j: Job = serde_json::from_value(payload.clone()).map_err(error)?;
    let pages: Vec<Page> = serde_json::from_value(payload["pdfPages"].clone()).map_err(error)?;
    if pages.is_empty() || pages.len() > 100 {
        return Err("PDF 页数应在 1 到 100 之间".into());
    }
    if pages.iter().all(|p| p.glyphs.is_empty()) {
        return Err("PDF 没有可识别文字；扫描 PDF 需要 OCR，不能作为可编辑转换".into());
    }
    match j.format.as_str() {
        "docx" => word(&pages, &j)?,
        "pptx" => ppt(&pages, &j)?,
        "xlsx" => spreadsheet(&pages, &j)?,
        _ => return Err("可编辑输出格式无效".into()),
    }
    Ok(
        json!({"output":j.output,"pages":pages.len(),"note":"重建可编辑文字、边框表格和图片；复杂曲线、字体或合并单元格可能需要调整"}),
    )
}

#[cfg(test)]
mod regression_tests {
 use super::*;
 #[test] fn numeric_types_preserve_zero_and_identifiers(){for value in ["0","0.5","0.01","-0.5","12"]{assert!(numeric(value),"{value}");}for value in ["0012","012345","NaN","inf","-inf","text"]{assert!(!numeric(value),"{value}");}}
 #[test] fn xml_controls_and_valid_unicode(){assert_eq!(escape("a\u{0c}\u{08}<\u{00ad}\u{200b}中文🐉"),"a&lt;\u{00ad}\u{200b}中文🐉");}
 #[test] fn repeated_styles_are_reused(){let mut v=vec![];for _ in 0..100000{assert_eq!(intern(&mut v,"same-style".into()),0);}assert_eq!(v.len(),1);}
 #[test] fn clustered_order_is_transitive(){let g=|x,y|Glyph{x,y,..Default::default()};let ordered=ordered(vec![g(3.,0.),g(2.,1.5),g(1.,3.)]);assert_eq!(ordered.iter().map(|g|g.x).collect::<Vec<_>>(),vec![2.,3.,1.]);}
 #[test] fn real_workbook_preserves_types_and_reuses_styles(){
 let output=std::env::temp_dir().join(format!("materialbox-regression-{}.xlsx",uuid::Uuid::new_v4()));
 let mut lines=Vec::new();for x in [0.,100.,200.,300.]{lines.push(json!({"x1":x,"x2":x,"y1":0.,"y2":600.}));}let ys:Vec<_>=(0..=30).map(|r|r as f64*20.).collect();for y in &ys{lines.push(json!({"x1":0.,"x2":300.,"y1":y,"y2":y}));}
 let mut glyphs=Vec::new();for r in 0..30 {for (c,value) in ["0","0.5","0012"].iter().enumerate(){glyphs.push(json!({"text":value,"x":c as f64*100.+10.,"y":r as f64*20.+10.,"w":25.,"h":10.,"size":11.,"font":"Carlito","color":"000000"}));}}
 let payload=json!({"mode":"pdf-excel","input":concat!(env!("CARGO_MANIFEST_DIR"),"/Cargo.toml"),"format":"xlsx","output":output,"pdfPages":[{"width":300.,"height":600.,"vertical":[0.,100.,200.,300.],"horizontal":ys,"lines":lines,"glyphs":glyphs}]});convert(&payload).unwrap();
 let mut zip=zip::ZipArchive::new(std::fs::File::open(&output).unwrap()).unwrap();let mut sheet=String::new();zip.by_name("xl/worksheets/sheet1.xml").unwrap().read_to_string(&mut sheet).unwrap();assert!(sheet.contains("<v>0</v>"));assert!(sheet.contains("<v>0.5</v>"));assert!(sheet.contains("<t xml:space=\"preserve\">0012</t>"));let mut styles=String::new();zip.by_name("xl/styles.xml").unwrap().read_to_string(&mut styles).unwrap();assert!(styles.contains("<cellXfs count=\"2\">"));drop(zip);std::fs::remove_file(output).unwrap();
 }

}
