mod office_ffi;
mod pdf_layout;
use base64::{engine::general_purpose::STANDARD, Engine};
use image::{DynamicImage, GenericImageView, ImageFormat};
use jni::{
    objects::{JClass, JString},
    sys::jstring,
    JNIEnv,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::Digest;
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

type Result<T> = std::result::Result<T, String>;
#[derive(Debug, Default, Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct Job {
    pub mode: String,
    pub input: String,
    pub inputs: Vec<String>,
    pub output: String,
    pub format: String,
    pub quality: u8,
    pub width: u32,
    pub height: u32,
    pub rotate: u16,
    pub grayscale: bool,
    pub start: f64,
    pub duration: f64,
    pub speed: f64,
    pub pages: String,
    pub text: String,
    pub action: String,
    pub bitrate: u32,
    pub fps: u32,
    pub volume: f64,
    pub copy: bool,
}
fn error<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}
fn ensure_output(job: &Job) -> Result<()> {
    if job.input.is_empty() || job.output.is_empty() {
        return Err("请选择输入文件和输出位置".into());
    }
    if !Path::new(&job.input).is_file() {
        return Err("输入文件不存在".into());
    }
    if job.input == job.output || job.inputs.iter().any(|p| p == &job.output) {
        return Err("不能覆盖原始文件".into());
    }
    if Path::new(&job.output).exists() {
        return Err("输出文件已存在，请使用新的文件名".into());
    }
    if let Some(parent) = Path::new(&job.output).parent() {
        fs::create_dir_all(parent).map_err(error)?;
    }
    Ok(())
}
pub fn run_job(job: &Job) -> Result<Value> {
    ensure_output(job)?;
    let tmp = PathBuf::from(format!("{}.partial", job.output));
    let mut working = job.clone();
    working.output = tmp.to_string_lossy().into_owned();
    let result = match job.mode.as_str() {
        "image" | "image-compress" => convert_image(&working),
        "document" => convert_document(&working),
        "pdf-split" => split_pdf(&working),
        "pdf-merge" => merge_pdf(&working),
        "image-pdf" => images_pdf(&working),
        "detect" => detect(&job.input),
        _ => Err("此任务应由媒体引擎执行".into()),
    };
    if result.is_ok() && tmp.exists() {
        fs::rename(&tmp, &job.output).map_err(error)?;
    } else if tmp.exists() {
        let _ = fs::remove_file(tmp);
    }
    result.map(|r| json!({"output":job.output,"detail":r}))
}
pub fn detect(path: &str) -> Result<Value> {
    let mut f = fs::File::open(path).map_err(error)?;
    let mut header = [0; 32];
    let n = f.read(&mut header).map_err(error)?;
    let b = &header[..n];
    let kind = if b.starts_with(b"%PDF-") {
        "PDF"
    } else if b.starts_with(b"\x89PNG") {
        "PNG"
    } else if b.starts_with(b"\xff\xd8\xff") {
        "JPEG"
    } else if b.starts_with(b"GIF8") {
        "GIF"
    } else if b.starts_with(b"BM") {
        "BMP"
    } else if b.starts_with(b"fLaC") {
        "FLAC"
    } else if b.starts_with(b"OggS") {
        "OGG"
    } else if b.starts_with(b"ID3") {
        "MP3"
    } else if b.starts_with(b"PK\x03\x04") {
        "ZIP / Office"
    } else if b.len() > 12 && &b[4..8] == b"ftyp" {
        "MP4 / MOV"
    } else if b.starts_with(b"RIFF") && b.len() > 12 {
        match &b[8..12] {
            b"WAVE" => "WAV",
            b"WEBP" => "WEBP",
            b"AVI " => "AVI",
            _ => "RIFF",
        }
    } else {
        "未知 / 文本"
    };
    Ok(json!({"format":kind,"bytes":f.metadata().map_err(error)?.len()}))
}
fn convert_image(j: &Job) -> Result<Value> {
    let reader = image::ImageReader::open(&j.input)
        .map_err(error)?
        .with_guessed_format()
        .map_err(error)?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(256 * 1024 * 1024);
    let mut reader = reader;
    reader.limits(limits);
    let mut im = reader.decode().map_err(error)?;
    let original = im.dimensions();
    if j.width > 8192 || j.height > 8192 {
        return Err("输出边长最多 8192 像素".into());
    }
    if j.width > 0 || j.height > 0 {
        let w = if j.width > 0 {
            j.width
        } else {
            (im.width() as f64 * j.height as f64 / im.height() as f64)
                .round()
                .max(1.0) as u32
        };
        let h = if j.height > 0 {
            j.height
        } else {
            (im.height() as f64 * w as f64 / im.width() as f64)
                .round()
                .max(1.0) as u32
        };
        if w > 8192 || h > 8192 {
            return Err("保持比例后的边长超过 8192 像素".into());
        }
        im = im.resize_exact(w, h, image::imageops::FilterType::Lanczos3);
    }
    im = match j.rotate {
        0 => im,
        90 => im.rotate90(),
        180 => im.rotate180(),
        270 => im.rotate270(),
        _ => return Err("旋转角度无效".into()),
    };
    if j.grayscale {
        im = im.grayscale()
    }
    let fmt = match j.format.as_str() {
        "jpg" | "jpeg" => ImageFormat::Jpeg,
        "png" => ImageFormat::Png,
        "webp" => ImageFormat::WebP,
        "bmp" => ImageFormat::Bmp,
        "gif" => ImageFormat::Gif,
        "tiff" => ImageFormat::Tiff,
        "ico" => ImageFormat::Ico,
        "tga" => ImageFormat::Tga,
        _ => return Err("不支持的图片格式".into()),
    };
    if fmt == ImageFormat::Ico && (im.width() > 256 || im.height() > 256) {
        im = im.thumbnail(256, 256);
    }
    let mut out = fs::File::create(&j.output).map_err(error)?;
    if fmt == ImageFormat::Jpeg {
        // Composite transparency on white instead of turning transparent pixels black.
        let rgba = im.to_rgba8();
        let mut rgb = image::RgbImage::new(im.width(), im.height());
        for (x, y, p) in rgba.enumerate_pixels() {
            let a = p[3] as u32;
            rgb.put_pixel(
                x,
                y,
                image::Rgb([
                    ((p[0] as u32 * a + 255 * (255 - a)) / 255) as u8,
                    ((p[1] as u32 * a + 255 * (255 - a)) / 255) as u8,
                    ((p[2] as u32 * a + 255 * (255 - a)) / 255) as u8,
                ]),
            );
        }
        image::codecs::jpeg::JpegEncoder::new_with_quality(
            &mut out,
            if j.quality == 0 {
                85
            } else {
                j.quality.clamp(1, 100)
            },
        )
        .encode_image(&DynamicImage::ImageRgb8(rgb))
        .map_err(error)?;
    } else {
        im.write_to(&mut out, fmt).map_err(error)?;
    }
    out.sync_all().map_err(error)?;
    Ok(
        json!({"original":original,"size":im.dimensions(),"note":"动图转静态格式使用第一帧；WebP 使用无损编码"}),
    )
}
fn ext(p: &str) -> String {
    Path::new(p)
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase()
}
fn xml_text(s: &str) -> Result<String> {
    let mut reader = quick_xml::Reader::from_str(s);
    reader.config_mut().trim_text(false);
    let mut out = String::new();
    loop {
        match reader.read_event() {
            Ok(quick_xml::events::Event::Text(t)) => out.push_str(&t.unescape().map_err(error)?),
            Ok(quick_xml::events::Event::End(e)) => {
                let n = e.name();
                if [b"w:p".as_slice(), b"a:p", b"p", b"div", b"tr", b"row"].contains(&n.as_ref()) {
                    out.push('\n')
                }
            }
            Ok(quick_xml::events::Event::Eof) => break,
            Err(e) => return Err(error(e)),
            _ => {}
        }
    }
    Ok(out)
}
fn read_text(p: &str) -> Result<String> {
    let extension = ext(p);
    if ["docx", "pptx", "odt", "epub"].contains(&extension.as_str()) {
        let f = fs::File::open(p).map_err(error)?;
        let mut zip = zip::ZipArchive::new(f).map_err(error)?;
        let mut names = zip
            .file_names()
            .filter(|s| {
                *s == "word/document.xml"
                    || *s == "content.xml"
                    || s.starts_with("ppt/slides/slide") && s.ends_with(".xml")
                    || extension == "epub" && (s.ends_with(".xhtml") || s.ends_with(".html"))
            })
            .map(str::to_owned)
            .collect::<Vec<_>>();
        names.sort_by_key(|s| {
            s.chars()
                .filter(char::is_ascii_digit)
                .collect::<String>()
                .parse::<u32>()
                .unwrap_or(0)
        });
        let mut out = String::new();
        for name in names {
            let f = zip.by_name(&name).map_err(error)?;
            if f.size() > 32 * 1024 * 1024 {
                return Err("文档内容超过 32 MB 限制".into());
            }
            let mut s = String::new();
            f.take(32 * 1024 * 1024 + 1)
                .read_to_string(&mut s)
                .map_err(error)?;
            out.push_str(&xml_text(&s)?);
            out.push('\n');
        }
        return Ok(out);
    }
    if extension == "pdf" {
        let doc = lopdf::Document::load(p).map_err(error)?;
        let pages = doc.get_pages().keys().copied().collect::<Vec<_>>();
        return doc.extract_text(&pages).map_err(error);
    }
    if ["doc", "xls", "xlsx", "ppt", "wps", "et", "dps", "rtf"].contains(&extension.as_str()) {
        return Err("此格式的完整排版转换尚未移植；请先另存为 DOCX、CSV 或文本".into());
    }
    if fs::metadata(p).map_err(error)?.len() > 32 * 1024 * 1024 {
        return Err("文本文件超过 32 MB 限制".into());
    }
    let s = fs::read_to_string(p).map_err(|_| "文本必须使用 UTF-8 编码".to_string())?;
    if ["html", "htm"].contains(&extension.as_str()) {
        let re = regex::Regex::new(r"(?s)<[^>]*>").map_err(error)?;
        return Ok(re
            .replace_all(&s, "")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&amp;", "&"));
    }
    Ok(s)
}
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
fn convert_document(j: &Job) -> Result<Value> {
    let content = read_text(&j.input)?;
    match j.format.as_str() {
        "txt" | "md" => fs::write(&j.output, &content).map_err(error)?,
        "html" => {
            let body = if ext(&j.input) == "md" {
                let parser =
                    pulldown_cmark::Parser::new_ext(&content, pulldown_cmark::Options::all());
                let mut html = String::new();
                pulldown_cmark::html::push_html(&mut html, parser);
                html
            } else {
                format!("<pre>{}</pre>", escape(&content))
            };
            fs::write(&j.output,format!("<!doctype html><html lang=\"zh-CN\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width\"><title>材料魔盒</title><style>body{{max-width:800px;margin:40px auto;padding:20px;font:16px/1.8 sans-serif}}pre{{white-space:pre-wrap}}</style>{body}</html>")).map_err(error)?;
        }
        "docx" => {
            let f = fs::File::create(&j.output).map_err(error)?;
            let mut z = zip::ZipWriter::new(f);
            let opt = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            for (n,s) in [("[Content_Types].xml","<?xml version=\"1.0\"?><Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Override PartName=\"/word/document.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/></Types>"),("_rels/.rels","<?xml version=\"1.0\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"word/document.xml\"/></Relationships>")]{z.start_file(n,opt).map_err(error)?;z.write_all(s.as_bytes()).map_err(error)?;}
            let paragraphs = content
                .lines()
                .map(|l| {
                    format!(
                        "<w:p><w:r><w:t xml:space=\"preserve\">{}</w:t></w:r></w:p>",
                        escape(l)
                    )
                })
                .collect::<String>();
            let doc=format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?><w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body>{paragraphs}<w:sectPr><w:pgSz w:w=\"11906\" w:h=\"16838\"/><w:pgMar w:top=\"1440\" w:bottom=\"1440\" w:left=\"1440\" w:right=\"1440\"/></w:sectPr></w:body></w:document>");
            z.start_file("word/document.xml", opt).map_err(error)?;
            z.write_all(doc.as_bytes()).map_err(error)?;
            z.finish().map_err(error)?;
        }
        "csv" => {
            let mut w = csv::Writer::from_path(&j.output).map_err(error)?;
            for line in content.lines() {
                w.write_record(line.split('\t')).map_err(error)?;
            }
            w.flush().map_err(error)?;
        }
        _ => return Err("不支持的文档输出格式".into()),
    }
    Ok(
        json!({"characters":content.chars().count(),"note":"文档转换保留可提取的文字，不保留原始图表及复杂排版；扫描 PDF 需要 OCR"}),
    )
}
fn images_pdf(j: &Job) -> Result<Value> {
    use lopdf::{dictionary, Document, Object, Stream};
    let paths = if j.inputs.is_empty() {
        vec![j.input.clone()]
    } else {
        j.inputs.clone()
    };
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let mut kids = Vec::new();
    let mut allocated = 0u64;
    for path in paths {
        let mut reader = image::ImageReader::open(&path)
            .map_err(error)?
            .with_guessed_format()
            .map_err(error)?;
        let mut limits = image::Limits::default();
        limits.max_alloc = Some(256 * 1024 * 1024);
        reader.limits(limits);
        let im = reader.decode().map_err(error)?.to_rgba8();
        let (w, h) = im.dimensions();
        allocated += w as u64 * h as u64 * 4;
        if allocated > 256 * 1024 * 1024 {
            return Err("图片总像素超过内存限制，请分批转换".into());
        }
        let mut rgb = Vec::with_capacity(w as usize * h as usize * 3);
        let mut alpha = Vec::with_capacity(w as usize * h as usize);
        for p in im.pixels() {
            rgb.extend_from_slice(&p.0[..3]);
            alpha.push(p[3]);
        }
        let mask = if alpha.iter().any(|a| *a < 255) {
            Some(doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Image","Width"=>w as i64,"Height"=>h as i64,"ColorSpace"=>"DeviceGray","BitsPerComponent"=>8},alpha)))
        } else {
            None
        };
        let mut dict = dictionary! {"Type"=>"XObject","Subtype"=>"Image","Width"=>w as i64,"Height"=>h as i64,"ColorSpace"=>"DeviceRGB","BitsPerComponent"=>8};
        if let Some(id) = mask {
            dict.set("SMask", id);
        }
        let image_id = doc.add_object(Stream::new(dict, rgb));
        // One source pixel per PDF unit; no crop, margin, resampling or JPEG recompression.
        let content_id = doc.add_object(Stream::new(
            dictionary! {},
            format!("q {w} 0 0 {h} 0 0 cm /Im0 Do Q").into_bytes(),
        ));
        let page=doc.add_object(dictionary!{"Type"=>"Page","Parent"=>pages_id,"MediaBox"=>vec![0.into(),0.into(),(w as i64).into(),(h as i64).into()],"Resources"=>dictionary!{"XObject"=>dictionary!{"Im0"=>image_id}},"Contents"=>content_id});
        kids.push(Object::Reference(page));
    }
    let count = kids.len();
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {"Type"=>"Pages","Kids"=>kids,"Count"=>count as i64}),
    );
    let catalog = doc.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages_id});
    doc.trailer.set("Root", catalog);
    doc.compress();
    doc.save(&j.output).map_err(error)?;
    Ok(json!({"pages":count,"note":"图片像素和比例保留；一图一页，无裁剪、无页边距"}))
}

pub fn parse_pages(s: &str, total: u32) -> Result<Vec<u32>> {
    if s.trim().is_empty() {
        return Err("请输入页码，例如 1-3,5".into());
    }
    let mut out = std::collections::BTreeSet::new();
    for token in s.split(',') {
        let parts = token.trim().split('-').collect::<Vec<_>>();
        let from = parts[0].trim().parse::<u32>().map_err(|_| "页码格式错误")?;
        let to = if parts.len() == 2 {
            parts[1].trim().parse::<u32>().map_err(|_| "页码格式错误")?
        } else if parts.len() == 1 {
            from
        } else {
            return Err("页码格式错误".into());
        };
        if from == 0 || to < from || to > total {
            return Err(format!("页码范围应在 1 到 {total} 之间"));
        }
        for p in from..=to {
            out.insert(p);
        }
    }
    Ok(out.into_iter().collect())
}
fn split_pdf(j: &Job) -> Result<Value> {
    let mut doc = lopdf::Document::load(&j.input).map_err(error)?;
    let total = doc.get_pages().len() as u32;
    let keep = parse_pages(&j.pages, total)?;
    let drop = (1..=total)
        .filter(|p| !keep.contains(p))
        .collect::<Vec<_>>();
    doc.delete_pages(&drop);
    doc.prune_objects();
    doc.save(&j.output).map_err(error)?;
    Ok(json!({"pages":keep}))
}
fn merge_pdf(j: &Job) -> Result<Value> {
    use lopdf::{dictionary, Document, Object};
    use std::collections::BTreeMap;
    let paths = if j.inputs.len() > 1 {
        j.inputs.clone()
    } else {
        return Err("请至少选择两个 PDF".into());
    };
    let mut document = Document::with_version("1.5");
    let mut all = BTreeMap::new();
    let mut pages = Vec::new();
    let mut max_id = 1;
    for path in paths {
        let mut d = Document::load(path).map_err(error)?;
        d.renumber_objects_with(max_id);
        max_id = d.max_id + 1;
        for (_, id) in d.get_pages() {
            // Resolve inherited page properties before removing the old Pages tree.
            let mut page = d
                .get_object(id)
                .map_err(error)?
                .as_dict()
                .map_err(error)?
                .clone();
            let mut parent = page.get(b"Parent").ok().and_then(|p| p.as_reference().ok());
            let mut visited = std::collections::BTreeSet::new();
            while let Some(pid) = parent {
                if !visited.insert(pid) {
                    break;
                }
                let obj = d.get_dictionary(pid).map_err(error)?;
                for key in [b"Resources".as_slice(), b"MediaBox", b"CropBox", b"Rotate"] {
                    if !page.has(key) {
                        if let Ok(v) = obj.get(key) {
                            page.set(key, v.clone());
                        }
                    }
                }
                parent = obj.get(b"Parent").ok().and_then(|p| p.as_reference().ok());
            }
            pages.push((id, page));
        }
        all.extend(d.objects);
    }
    let pages_id = (max_id, 0);
    let catalog_id = (max_id + 1, 0);
    let kids = pages
        .iter()
        .map(|(id, _)| Object::Reference(*id))
        .collect::<Vec<_>>();
    for (id, object) in all {
        let ty = object.type_name().unwrap_or(b"");
        if ty != b"Catalog" && ty != b"Pages" && ty != b"Page" && ty != b"Outlines" {
            document.objects.insert(id, object);
        }
    }
    for (id, mut page) in pages {
        page.set("Parent", pages_id);
        document.objects.insert(id, Object::Dictionary(page));
    }
    let count = kids.len();
    document.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {"Type"=>"Pages","Kids"=>kids,"Count"=>count as i64}),
    );
    document.objects.insert(
        catalog_id,
        Object::Dictionary(dictionary! {"Type"=>"Catalog","Pages"=>pages_id}),
    );
    document.trailer.set("Root", catalog_id);
    document.max_id = max_id + 1;
    document.renumber_objects();
    document.prune_objects();
    document.compress();
    document.save(&j.output).map_err(error)?;
    Ok(json!({"pages":count}))
}
pub fn media_plan(j: &Job) -> Result<Vec<String>> {
    ensure_output(j)?;
    if ![j.start, j.duration, j.speed, j.volume]
        .iter()
        .all(|v| v.is_finite())
        || j.start < 0.0
        || j.duration < 0.0
    {
        return Err("时间和速度参数无效".into());
    }
    if j.width > 8192 || j.height > 8192 || j.fps > 120 || j.bitrate > 100000 {
        return Err("媒体参数超出范围".into());
    }
    let mut a = vec!["-hide_banner".into(), "-nostdin".into(), "-y".into()];
    if j.start > 0.0 {
        a.extend(["-ss".into(), j.start.to_string()]);
    }
    a.extend(["-i".into(), j.input.clone()]);
    if j.duration > 0.0 {
        a.extend(["-t".into(), j.duration.to_string()]);
    }
    let mut filters = Vec::new();
    match j.mode.as_str() {
        "video" | "video-compress" | "video-trim" => {
            if ![
                "mp4", "mkv", "mov", "avi", "webm", "ts", "flv", "mpeg", "3gp",
            ]
            .contains(&j.format.as_str())
            {
                return Err("不支持的视频格式".into());
            }
            if j.copy {
                if j.width > 0 || j.height > 0 || j.speed > 0.0 && j.speed != 1.0 || j.fps > 0 {
                    return Err("无损转封装不能同时缩放、变速或修改帧率".into());
                }
                a.extend(["-c".into(), "copy".into()]);
            } else {
                let (codec, audio) = match j.format.as_str() {
                    "webm" => ("libvpx-vp9", "libopus"),
                    "avi" | "mpeg" => ("mpeg4", "mp3"),
                    "3gp" => ("mpeg4", "aac"),
                    _ => ("libx264", "aac"),
                };
                a.extend(["-c:v".into(), codec.into(), "-c:a".into(), audio.into()]);
                if codec == "libx264" {
                    a.extend([
                        "-preset".into(),
                        "veryfast".into(),
                        "-crf".into(),
                        match j.quality {
                            1 => "18",
                            3 => "30",
                            _ => "23",
                        }
                        .into(),
                        "-pix_fmt".into(),
                        "yuv420p".into(),
                    ]);
                }
                if j.width > 0 || j.height > 0 {
                    let w = if j.width > 0 {
                        (j.width / 2 * 2).max(2).to_string()
                    } else {
                        "-2".into()
                    };
                    let h = if j.height > 0 {
                        (j.height / 2 * 2).max(2).to_string()
                    } else {
                        "-2".into()
                    };
                    filters.push(format!("scale={w}:{h}"));
                }
                if j.speed > 0.0 && j.speed != 1.0 {
                    if !(0.5..=2.0).contains(&j.speed) {
                        return Err("速度范围为 0.5 到 2".into());
                    }
                    filters.push(format!("setpts=PTS/{}", j.speed));
                    a.extend(["-af".into(), format!("atempo={}", j.speed)]);
                }
                if j.fps > 0 {
                    a.extend(["-r".into(), j.fps.to_string()]);
                }
            }
            if ["mp4", "mov"].contains(&j.format.as_str()) {
                a.extend(["-movflags".into(), "+faststart".into()]);
            }
        }
        "audio" | "audio-trim" | "audio-enhance" => {
            let codec = match j.format.as_str() {
                "mp3" => "libmp3lame",
                "wav" => "pcm_s16le",
                "aac" | "m4a" => "aac",
                "flac" => "flac",
                "ogg" => "libvorbis",
                "opus" => "libopus",
                "wma" => "wmav2",
                _ => return Err("不支持的音频格式".into()),
            };
            a.extend(["-vn".into(), "-c:a".into(), codec.into()]);
            if j.bitrate > 0 && !["wav", "flac"].contains(&j.format.as_str()) {
                a.extend(["-b:a".into(), format!("{}k", j.bitrate)]);
            }
            if j.mode == "audio-enhance" {
                filters.push(if j.action == "normalize" {
                    "loudnorm=I=-16:TP=-1.5:LRA=11".into()
                } else {
                    if !(0.1..=3.0).contains(&j.volume) {
                        return Err("音量范围为 0.1 到 3".into());
                    }
                    format!("volume={}", j.volume)
                });
            }
        }
        "gif" => {
            if j.duration <= 0.0 || j.duration > 120.0 {
                return Err("GIF 时长应在 0 到 120 秒之间".into());
            }
            filters.push(format!(
                "fps={},scale={}:-1:flags=lanczos,split[a][b];[a]palettegen[p];[b][p]paletteuse",
                if j.fps == 0 { 12 } else { j.fps },
                if j.width == 0 { 480 } else { j.width }
            ));
            a.extend(["-an".into(), "-loop".into(), "0".into()]);
        }
        "frame" => {
            a.extend(["-frames:v".into(), "1".into()]);
        }
        _ => return Err("未知媒体操作".into()),
    }
    if !filters.is_empty() {
        a.extend([
            if j.mode.starts_with("audio") {
                "-af".into()
            } else {
                "-vf".into()
            },
            filters.join(","),
        ]);
    }
    a.push(j.output.clone());
    Ok(a)
}
pub fn text_tool(action: &str, text: &str, options: &Value) -> Result<String> {
    match action {
        "json" => {
            serde_json::to_string_pretty(&serde_json::from_str::<Value>(text).map_err(error)?)
                .map_err(error)
        }
        "base64-encode" => Ok(STANDARD.encode(text)),
        "base64-decode" => {
            String::from_utf8(STANDARD.decode(text.trim()).map_err(error)?).map_err(error)
        }
        "url-encode" => Ok(urlencoding::encode(text).into_owned()),
        "url-decode" => Ok(urlencoding::decode(text).map_err(error)?.into_owned()),
        "sha256" => Ok(format!("{:x}", sha2::Sha256::digest(text.as_bytes()))),
        "md5" => Ok(format!("{:x}", md5::Md5::digest(text.as_bytes()))),
        "uuid" => Ok(uuid::Uuid::new_v4().to_string()),
        "count" => Ok(format!(
            "字符：{}\n不含空白：{}\n行数：{}\n字节：{}",
            text.chars().count(),
            text.chars().filter(|c| !c.is_whitespace()).count(),
            text.lines().count(),
            text.len()
        )),
        "sort" => {
            let mut v = text.lines().collect::<Vec<_>>();
            v.sort();
            if options["unique"].as_bool().unwrap_or(false) {
                v.dedup();
            }
            Ok(v.join("\n"))
        }
        "dedup" => {
            let mut seen = std::collections::HashSet::new();
            Ok(text
                .lines()
                .filter(|s| seen.insert(*s))
                .collect::<Vec<_>>()
                .join("\n"))
        }
        "unicode-encode" => Ok(text
            .chars()
            .map(|c| format!("\\u{{{:x}}}", c as u32))
            .collect()),
        "regex" => {
            let pattern = options["pattern"].as_str().unwrap_or("");
            let re = regex::RegexBuilder::new(pattern)
                .size_limit(1024 * 1024)
                .build()
                .map_err(error)?;
            Ok(re
                .find_iter(text)
                .take(1000)
                .map(|m| format!("{}..{}  {}", m.start(), m.end(), m.as_str()))
                .collect::<Vec<_>>()
                .join("\n"))
        }
        "replace" => {
            let from = options["pattern"].as_str().unwrap_or("");
            if from.is_empty() {
                return Err("查找内容不能为空".into());
            }
            Ok(text.replace(from, options["replacement"].as_str().unwrap_or("")))
        }
        "base" => {
            let from = options["from"].as_u64().unwrap_or(10) as u32;
            let to = options["to"].as_u64().unwrap_or(16) as u32;
            if !(2..=36).contains(&from) || !(2..=36).contains(&to) {
                return Err("进制范围 2 到 36".into());
            }
            let mut n = u128::from_str_radix(text.trim(), from).map_err(error)?;
            if n == 0 {
                return Ok("0".into());
            }
            let mut s = String::new();
            while n > 0 {
                s.push(char::from_digit((n % to as u128) as u32, to).unwrap());
                n /= to as u128;
            }
            Ok(s.chars().rev().collect())
        }
        "jwt" => {
            let part = text.trim().split('.').nth(1).ok_or("JWT 格式无效")?;
            let b = base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(part.trim_end_matches('='))
                .map_err(error)?;
            let v: Value = serde_json::from_slice(&b).map_err(error)?;
            Ok(format!(
                "仅解码，未验证签名\n{}",
                serde_json::to_string_pretty(&v).map_err(error)?
            ))
        }
        "markdown" => {
            let mut s = String::new();
            pulldown_cmark::html::push_html(
                &mut s,
                pulldown_cmark::Parser::new_ext(text, pulldown_cmark::Options::all()),
            );
            Ok(s)
        }
        _ => Err("未知工具".into()),
    }
}
pub fn call(op: &str, input: &str) -> String {
    let result = std::panic::catch_unwind(|| -> Result<Value> {
        match op {
            "pdf-table" | "pdf-editable" => {
                if input.len() > 64 * 1024 * 1024 {
                    return Err("PDF 结构数据过大，请分批转换".into());
                }
                pdf_layout::convert(&serde_json::from_str(input).map_err(error)?)
            }
            "plan" => {
                let j: Job = serde_json::from_str(input).map_err(error)?;
                Ok(json!({"args":media_plan(&j)?}))
            }
            "convert" => {
                let j: Job = serde_json::from_str(input).map_err(error)?;
                run_job(&j)
            }
            "detect" => detect(input),
            "tool" => {
                let v: Value = serde_json::from_str(input).map_err(error)?;
                let text = v["text"].as_str().unwrap_or("");
                if text.len() > 2 * 1024 * 1024 {
                    return Err("文本超过 2 MB 限制".into());
                }
                Ok(json!({"text":text_tool(v["action"].as_str().unwrap_or(""),text,&v)?}))
            }
            _ => Err("未知操作".into()),
        }
    });
    match result {
        Ok(Ok(v)) => json!({"ok":true,"data":v}).to_string(),
        Ok(Err(e)) => json!({"ok":false,"error":e}).to_string(),
        Err(_) => json!({"ok":false,"error":"文件解析失败，请检查文件"}).to_string(),
    }
}
#[no_mangle]
pub extern "system" fn Java_cn_materialbox_app_Core_call(
    mut env: JNIEnv,
    _class: JClass,
    op: JString,
    input: JString,
) -> jstring {
    let op = env
        .get_string(&op)
        .map(|s| s.to_string_lossy().into_owned());
    let input = env
        .get_string(&input)
        .map(|s| s.to_string_lossy().into_owned());
    let output = match (op, input) {
        (Ok(o), Ok(i)) => call(&o, &i),
        _ => json!({"ok":false,"error":"JNI 输入错误"}).to_string(),
    };
    env.new_string(output)
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn page_validation() {
        assert_eq!(parse_pages("3,1-2,2", 5).unwrap(), vec![1, 2, 3]);
        for s in ["0", "3-2", "6", "1-2-3", ""] {
            assert!(parse_pages(s, 5).is_err())
        }
    }
    #[test]
    fn unicode_roundtrip() {
        let s = "材料魔盒 🐉";
        let encoded = text_tool("base64-encode", s, &json!({})).unwrap();
        assert_eq!(text_tool("base64-decode", &encoded, &json!({})).unwrap(), s);
        assert_eq!(
            text_tool(
                "url-decode",
                &text_tool("url-encode", s, &json!({})).unwrap(),
                &json!({})
            )
            .unwrap(),
            s
        );
    }
    #[test]
    fn validation() {
        assert!(text_tool("json", "{", &json!({})).is_err());
        assert!(text_tool("base", "1", &json!({"from":1})).is_err());
        assert!(text_tool("regex", "xx", &json!({"pattern":"["})).is_err());
        assert_eq!(text_tool("dedup", "b\na\nb", &json!({})).unwrap(), "b\na");
    }
    #[test]
    fn hash_known() {
        assert_eq!(
            text_tool("sha256", "abc", &json!({})).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
    #[test]
    fn real_image_and_document() {
        let root = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        fs::create_dir_all(&root).unwrap();
        let input = root.join("中文 图片.png");
        image::RgbaImage::from_pixel(20, 10, image::Rgba([255, 0, 0, 0]))
            .save(&input)
            .unwrap();
        let j = Job {
            mode: "image".into(),
            input: input.to_string_lossy().into(),
            output: root.join("结果.jpg").to_string_lossy().into(),
            format: "jpg".into(),
            width: 10,
            ..Default::default()
        };
        run_job(&j).unwrap();
        let decoded = image::open(&j.output).unwrap();
        assert_eq!(decoded.dimensions(), (10, 5));
        assert!(decoded.to_rgb8().get_pixel(0, 0)[0] > 250);
        assert!(run_job(&j).is_err());
        let text = root.join("材料.txt");
        fs::write(&text, "材料魔盒\n<&> test").unwrap();
        let doc = Job {
            mode: "document".into(),
            input: text.to_string_lossy().into(),
            output: root.join("材料.docx").to_string_lossy().into(),
            format: "docx".into(),
            ..Default::default()
        };
        run_job(&doc).unwrap();
        assert_eq!(read_text(&doc.output).unwrap().trim(), "材料魔盒\n<&> test");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn media_args_no_shell() {
        let root = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        fs::create_dir_all(&root).unwrap();
        let input = root.join("视频 & '测试.mp4");
        fs::write(&input, []).unwrap();
        let mut j = Job {
            mode: "video".into(),
            input: input.to_string_lossy().into(),
            output: root.join("out.mp4").to_string_lossy().into(),
            format: "mp4".into(),
            width: 721,
            ..Default::default()
        };
        let a = media_plan(&j).unwrap();
        assert!(a.contains(&j.input));
        assert!(a.contains(&"scale=720:-2".into()));
        j.copy = true;
        assert!(media_plan(&j).is_err());
        j.width = 0;
        j.speed = 8.;
        assert!(media_plan(&j).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
