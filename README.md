# 材料魔盒 · MaterialBox

Android 手机上独立离线运行的中文格式转换与文字工具箱。基于 [FormatMaster-EN v1.4.1](https://github.com/Gu-0312/FormatMaster-EN/releases/tag/v1.4.1) 的功能设计二次开发，Rust 核心 + Java/JNI + 本地 WebView，使用 FFmpeg、LibreOfficeKit 与 PDFBox。

[下载 APK](https://github.com/shuizhaodemumu/materialbox/releases/latest) · [个人主页](https://shuizhaodemumu.github.io/) · [构建说明](BUILDING.md)

## 功能

- 视频/音频转换、压缩、剪辑、响度与音量调整、GIF及截图。
- 图片转换、缩放、旋转、灰度；17项文字/数据工具。
- 离线语音转文字：SenseVoice INT8 + Silero VAD，中英文混合录音，中文统一简体；TXT/SRT、结果预览、复制、保存和分享。模型随 APK 内置，无需账号或联网。
- PDF独立栏目：图片、Word、PPT、Excel、HTML转PDF；PDF转图片、可编辑Word/PPT/Excel、PDF/A；合并与提取页面。
- 后台任务、系统文件导入、另存为、分享、取消和重试。
- 四张淡紫龙娘背景、紧凑首页、开源与联系方式页面；捐赠完全自愿，软件免费。

## 已验证范围

0.4.0 APK支持Android8.0+ / arm64-v8a，约276 MB，无互联网权限。保持原包名与签名，可覆盖升级0.3.1；旧发行包仍保留。GitHub链接由系统浏览器打开。

保留原有12项Rust测试与26项Android JNI/文件/FFmpeg/Office/任务验证；SenseVoice候选版另有9项语音检查通过，用户已实测认可。真机任务队列、字幕预览复制、系统保存字节一致性及分享选择器已验证。0.4.0保留同一识别代码和模型，仅切回正式应用标识并更新版本；正式升级安装另见发行验证记录。PDF/A-2b样本的历史验证为veraPDF1.30.3、144规则/963检查零失败。界面通过320/390/768/1440宽度检查。Gemini独立审阅代码和执行日志，设备操作由Codex完成。

语音识别并非完全准确，可能误识别人名、数字和专业词；合成样本曾将“PDF”识别为“PF”，不会通过硬替换掩盖误识别。中文使用OpenCC词典转简体，英文保留识别原文（可能全大写）。SRT是语音段时间，非逐字对齐；不区分说话人。单文件最长4小时、输出上限2MB；接近上限的长音频内存压测未完成。取消会等待当前最多20秒音频段的同步推理结束。首次使用解压约238MB模型，建议预留至少1GB空间。

PDF反向转换是几何重建：Word文字框与Word/PPT规则表格可编辑，Excel单元格保留文字和数值。**不能保证所有文档完全保留原版**：无扫描OCR，复杂曲线、旋转文字、合并/无边框/多表格、缺失字体有限制；旋转或偏移裁剪页面拒绝可编辑重建，Excel不支持的网格明确报错。Word定位文本框不具备原始段落自动重排体验。原版其他插件/聊天/下载/代码编辑等尚未全部移植。

## 结构与许可

`core/` Rust核心；`android/` 原生桥接与界面；`native/` SenseVoice C API JNI桥；`templates/` OOXML空白模板；`tests/` 应用自身测试样本；`third-party-licenses/` 组件版权与许可；`DEPENDENCIES.json` 与 `SPEECH-MODELS.json` 记录构建依赖及模型摘要。模型权重不提交Git，构建前运行 `prepare-speech-deps.py`。

应用源码以GPL-3.0-or-later发布，保留上游MIT及第三方许可；各组件依原许可，详见LICENSE与third-party-licenses。没有上传签名私钥、下载缓存或个人简历。

## 美术与协作

玲珑（Codex）实现与打包；山田樱子（Gemini）独立代码/测试证据审阅；咲夜梦（MiniMax H3）按作者参考画风的文字描述生成图标/背景参考。背景采用512×288、22帧中间帧，使用稳定权重节点与分阶段条件准备；不是原生高分辨率图片条件生成。界面通过CSS浅色遮罩保证可读性。关于页二维码以CSS视窗显示完整有效区域。

## 0.3.1 PPTX 兼容性修复

部分第三方导出的PPTX虽能通过ZIP校验和XML解析，仍会被LibreOffice载入器拒绝。新增载入失败后的OOXML容器规范化重试：Rust逐字节保留所有文档/媒体部件，重写ZIP包装并删除目录记录，在手机内再次载入。原始导入文件保持不动，中间副本自动清理。

一个实际5页演示稿已由“loadComponentFromURL returned an empty reference”恢复为正常PDF转换；检查了5页渲染结果。未确定到底是哪项ZIP兼容细节触发旧引擎拒绝，不能泛化为所有Tencent office文件均有问题。修复仅处理PPTX/DOCX/XLSX，兼容整理限原始128MB、解压256MB、最多20000部件，不会修改内部XML或重建幻灯片。
