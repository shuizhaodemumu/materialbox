#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
#[cfg(windows)]
mod desktop {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use serde_json::{json, Value};
    use std::{
        fs,
        path::PathBuf,
        process::Command,
        sync::{Arc, Mutex},
    };
    use tiny_http::{Header, Response, Server};
    struct State {
        root: PathBuf,
        tasks: Mutex<Vec<Value>>,
        token: String,
        engine: Mutex<()>,
    }
    fn save(s: &State, t: &[Value]) {
        let tmp = s.root.join("tasks.new");
        if let Ok(f) = fs::File::create(&tmp) {
            if serde_json::to_writer(&f, t).is_ok() {
                let _ = f.sync_all();
                let _ = fs::rename(tmp, s.root.join("tasks.json"));
            }
        }
    }
    fn process(s: Arc<State>, id: String) {
        let _engine = s.engine.lock().unwrap();
        let j = {
            let mut ts = s.tasks.lock().unwrap();
            let Some(t) = ts.iter_mut().find(|t| t["id"] == id) else {
                return;
            };
            if t["status"] != "queued" {
                return;
            }
            t["status"] = json!("running");
            t["message"] = json!("正在转换");
            let j = t["request"].clone();
            save(&s, &ts);
            j
        };
        let result = (|| -> Result<Value, String> {
            let job: materialbox::Job =
                serde_json::from_value(j.clone()).map_err(|e| e.to_string())?;
            if job.mode.starts_with("audio")
                || job.mode.starts_with("video")
                || ["gif", "frame"].contains(&job.mode.as_str())
            {
                let args = materialbox::media_plan(&job)?;
                let ffmpeg = std::env::current_exe()
                    .map_err(|e| e.to_string())?
                    .parent()
                    .unwrap()
                    .join("ffmpeg.exe");
                if !ffmpeg.exists() {
                    return Err("找不到随附的 ffmpeg.exe".into());
                }
                use std::os::windows::process::CommandExt;
                let mut child = Command::new(ffmpeg)
                    .args(args)
                    .creation_flags(0x08000000)
                    .stderr(std::process::Stdio::piped())
                    .spawn()
                    .map_err(|e| e.to_string())?;
                {
                    let mut ts = s.tasks.lock().unwrap();
                    if let Some(t) = ts.iter_mut().find(|t| t["id"] == id) {
                        t["pid"] = json!(child.id());
                    }
                }
                let stderr = child.stderr.take().unwrap();
                let log = std::thread::spawn(move || {
                    use std::io::Read;
                    let mut b = Vec::new();
                    let _ = stderr.take(16 * 1024 * 1024).read_to_end(&mut b);
                    String::from_utf8_lossy(&b).into_owned()
                });
                let status = child.wait().map_err(|e| e.to_string())?;
                let logs = log.join().unwrap_or_default();
                if !status.success() {
                    let tail = logs
                        .chars()
                        .rev()
                        .take(600)
                        .collect::<String>()
                        .chars()
                        .rev()
                        .collect::<String>();
                    return Err(format!("媒体转换失败：{tail}"));
                }
                Ok(json!({"output":job.output}))
            } else {
                materialbox::run_job(&job)
            }
        })();
        let mut ts = s.tasks.lock().unwrap();
        if let Some(t) = ts.iter_mut().find(|t| t["id"] == id) {
            t.as_object_mut().unwrap().remove("pid");
            if t["status"] == "cancelled" {
                let _ = fs::remove_file(j["output"].as_str().unwrap_or(""));
            } else {
                match result {
                    Ok(_) => {
                        let path = j["output"].as_str().unwrap_or("");
                        t["status"] = json!("done");
                        t["message"] = json!("转换完成，点击保存");
                        t["bytes"] = json!(fs::metadata(path).map(|m| m.len()).unwrap_or(0));
                    }
                    Err(e) => {
                        t["status"] = json!("failed");
                        t["message"] = json!(e);
                        let _ = fs::remove_file(j["output"].as_str().unwrap_or(""));
                    }
                }
            }
            save(&s, &ts);
        }
    }
    fn api(s: Arc<State>, method: &str, args: &Value) -> Result<Value, String> {
        match method {
            "tasks" => Ok(json!(
                serde_json::to_string(&*s.tasks.lock().unwrap()).unwrap()
            )),
            "tool" => Ok(json!(materialbox::call(
                "tool",
                args[0].as_str().unwrap_or("")
            ))),
            "inspect" => {
                let path = PathBuf::from(args[0].as_str().unwrap_or(""));
                if !path.canonicalize().map_err(|e| e.to_string())?.starts_with(
                    s.root
                        .join("inputs")
                        .canonicalize()
                        .map_err(|e| e.to_string())?,
                ) {
                    return Err("输入路径无效".into());
                }
                Ok(json!(materialbox::call(
                    "detect",
                    path.to_str().unwrap_or("")
                )))
            }
            "import" => {
                let name = args["name"].as_str().unwrap_or("文件");
                let ext = PathBuf::from(name)
                    .extension()
                    .map(|v| v.to_string_lossy().into_owned())
                    .filter(|v| v.len() < 12 && v.chars().all(|c| c.is_ascii_alphanumeric()))
                    .unwrap_or("bin".into());
                let data = STANDARD
                    .decode(args["data"].as_str().unwrap_or(""))
                    .map_err(|e| e.to_string())?;
                if data.len() > 128 * 1024 * 1024 {
                    return Err("桌面单次导入上限 128 MB".into());
                }
                let path = s
                    .root
                    .join("inputs")
                    .join(format!("{}.{}", uuid::Uuid::new_v4(), ext));
                fs::write(&path, &data).map_err(|e| e.to_string())?;
                Ok(json!({"path":path,"name":name,"size":data.len()}))
            }
            "submit" => {
                let mut j: Value = serde_json::from_str(args[0].as_str().unwrap_or(""))
                    .map_err(|e| e.to_string())?;
                let input = PathBuf::from(j["input"].as_str().unwrap_or(""));
                if !input
                    .canonicalize()
                    .map_err(|e| e.to_string())?
                    .starts_with(
                        s.root
                            .join("inputs")
                            .canonicalize()
                            .map_err(|e| e.to_string())?,
                    )
                {
                    return Err("输入路径无效".into());
                }
                let format = j["format"].as_str().unwrap_or("bin").to_owned();
                if format.len() > 8 || !format.chars().all(|c| c.is_ascii_alphanumeric()) {
                    return Err("输出格式无效".into());
                }
                let id = uuid::Uuid::new_v4().to_string();
                j["output"] = json!(s.root.join("outputs").join(format!("{id}.{format}")));
                let t = json!({"id":id,"name":j["name"],"request":j,"mode":j["mode"],"format":format,"status":"queued","message":"等待转换"});
                {
                    let mut ts = s.tasks.lock().unwrap();
                    if ts.len() >= 200 {
                        return Err("任务记录已满".into());
                    }
                    ts.insert(0, t);
                    save(&s, &ts);
                }
                let sid = id.clone();
                std::thread::spawn(move || process(s, sid));
                Ok(json!(json!({"ok":true,"id":id}).to_string()))
            }
            "retry" => {
                let id = args[0].as_str().unwrap_or("").to_owned();
                {
                    let mut ts = s.tasks.lock().unwrap();
                    let t = ts.iter_mut().find(|t| t["id"] == id).ok_or("任务不存在")?;
                    if !["failed", "cancelled", "interrupted"]
                        .contains(&t["status"].as_str().unwrap_or(""))
                    {
                        return Err("任务无需重试".into());
                    }
                    let _ = fs::remove_file(t["request"]["output"].as_str().unwrap_or(""));
                    t["status"] = json!("queued");
                    save(&s, &ts);
                }
                std::thread::spawn(move || process(s, id));
                Ok(json!("{\"ok\":true}"))
            }
            "cancel" => {
                let mut ts = s.tasks.lock().unwrap();
                let t = ts
                    .iter_mut()
                    .find(|t| t["id"] == args[0])
                    .ok_or("任务不存在")?;
                if let Some(pid) = t["pid"].as_u64() {
                    use std::os::windows::process::CommandExt;
                    let _ = Command::new("taskkill.exe")
                        .args(["/PID", &pid.to_string(), "/T", "/F"])
                        .creation_flags(0x08000000)
                        .output();
                }
                t["status"] = json!("cancelled");
                t["message"] = json!("已取消");
                save(&s, &ts);
                Ok(Value::Null)
            }
            "quit" => {
                let tasks = s.tasks.lock().unwrap();
                if tasks
                    .iter()
                    .any(|t| ["running", "queued"].contains(&t["status"].as_str().unwrap_or("")))
                {
                    return Err("请先取消或完成正在转换的任务".into());
                }
                std::thread::spawn(|| {
                    std::thread::sleep(std::time::Duration::from_millis(150));
                    std::process::exit(0);
                });
                Ok(Value::Null)
            }
            "clear" => {
                let mut ts = s.tasks.lock().unwrap();
                ts.retain(|t| {
                    if ["running", "queued"].contains(&t["status"].as_str().unwrap_or("")) {
                        return true;
                    }
                    let _ = fs::remove_file(t["request"]["output"].as_str().unwrap_or(""));
                    false
                });
                save(&s, &ts);
                Ok(Value::Null)
            }
            "export" | "share" => {
                let ts = s.tasks.lock().unwrap();
                let t = ts.iter().find(|t| t["id"] == args[0]).ok_or("任务不存在")?;
                if t["status"] != "done" {
                    return Err("结果尚未完成".into());
                }
                let path = t["request"]["output"].as_str().unwrap_or("");
                if method == "share" {
                    Command::new("explorer.exe")
                        .args(["/select,", path])
                        .spawn()
                        .map_err(|e| e.to_string())?;
                } else {
                    use std::os::windows::process::CommandExt;
                    let name = format!(
                        "{}_魔盒.{}",
                        PathBuf::from(t["name"].as_str().unwrap_or("材料"))
                            .file_stem()
                            .unwrap_or_default()
                            .to_string_lossy(),
                        t["format"].as_str().unwrap_or("bin")
                    );
                    let output=Command::new("powershell.exe").args(["-NoProfile","-STA","-Command","Add-Type -AssemblyName System.Windows.Forms; $d=New-Object System.Windows.Forms.SaveFileDialog; $d.FileName=$env:MATERIALBOX_NAME; if($d.ShowDialog() -eq 'OK') { [Console]::Write($d.FileName) }"]).env("MATERIALBOX_NAME",name).creation_flags(0x08000000).output().map_err(|e|e.to_string())?;
                    let target = String::from_utf8_lossy(&output.stdout).trim().to_owned();
                    if !target.is_empty() {
                        fs::copy(path, target).map_err(|e| e.to_string())?;
                    }
                }
                Ok(Value::Null)
            }
            _ => Err("未知接口".into()),
        }
    }
    pub fn start() {
        let exe = std::env::current_exe().unwrap();
        let root = std::env::var("MATERIALBOX_DATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| exe.parent().unwrap().join("材料魔盒数据"));
        for dir in ["inputs", "outputs"] {
            fs::create_dir_all(root.join(dir)).unwrap();
        }
        let mut tasks: Vec<Value> = fs::read(root.join("tasks.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        for t in &mut tasks {
            if ["running", "queued"].contains(&t["status"].as_str().unwrap_or("")) {
                t["status"] = json!("interrupted");
                t["message"] = json!("任务中断，可重试");
            }
        }
        let server = Server::http("127.0.0.1:0").unwrap();
        let addr = server.server_addr().to_ip().unwrap();
        let token = uuid::Uuid::new_v4().to_string();
        let s = Arc::new(State {
            root,
            tasks: Mutex::new(tasks),
            token: token.clone(),
            engine: Mutex::new(()),
        });
        save(&s, &s.tasks.lock().unwrap());
        let url = format!("http://{addr}/?token={token}");
        fs::write(
            s.root.join("session.json"),
            json!({"url":url,"pid":std::process::id()}).to_string(),
        )
        .unwrap();
        if std::env::var("MATERIALBOX_NO_BROWSER").is_err() {
            let _ = Command::new("cmd.exe")
                .args(["/c", "start", "", &url])
                .spawn();
        }
        for mut req in server.incoming_requests() {
            let path = req.url().split('?').next().unwrap_or("/").to_owned();
            let state = s.clone();
            std::thread::spawn(move || {
                let (body, mime, status) = if path.starts_with("/api/") {
                    let valid = req.headers().iter().any(|h| {
                        h.field.equiv("X-Materialbox-Token") && h.value.as_str() == state.token
                    });
                    if !valid {
                        (
                            b"{\"error\":\"unauthorized\"}".to_vec(),
                            "application/json",
                            403,
                        )
                    } else {
                        use std::io::Read;
                        let mut buf = String::new();
                        let result = req
                            .as_reader()
                            .take(180 * 1024 * 1024)
                            .read_to_string(&mut buf)
                            .map_err(|e| e.to_string())
                            .and_then(|_| {
                                serde_json::from_str::<Value>(&buf).map_err(|e| e.to_string())
                            })
                            .and_then(|v| api(state, path.trim_start_matches("/api/"), &v));
                        let v = match result {
                            Ok(v) => json!({"result":v}),
                            Err(e) => json!({"error":e}),
                        };
                        (v.to_string().into_bytes(), "application/json", 200)
                    }
                } else {
                    let (bytes, mime) = match path.as_str() {
                        "/" | "/index.html" => (
                            include_bytes!("../../android/assets/index.html").as_slice(),
                            "text/html; charset=utf-8",
                        ),
                        "/style.css" => (
                            include_bytes!("../../android/assets/style.css").as_slice(),
                            "text/css",
                        ),
                        "/app.js" => (
                            include_bytes!("../../android/assets/app.js").as_slice(),
                            "application/javascript",
                        ),
                        "/icon.png" => (
                            include_bytes!("../../android/assets/icon.png").as_slice(),
                            "image/png",
                        ),
                        _ => (b"Not found".as_slice(), "text/plain"),
                    };
                    (
                        bytes.to_vec(),
                        mime,
                        if bytes == b"Not found" { 404 } else { 200 },
                    )
                };
                let response = Response::from_data(body)
                    .with_status_code(status)
                    .with_header(Header::from_bytes("Content-Type", mime).unwrap())
                    .with_header(Header::from_bytes("X-Content-Type-Options", "nosniff").unwrap());
                let _ = req.respond(response);
            });
        }
    }
}
#[cfg(windows)]
fn main() {
    desktop::start();
}
#[cfg(not(windows))]
fn main() {}
