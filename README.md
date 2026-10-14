# 材料魔盒 · MaterialBox

Android 手机上独立离线运行的中文格式转换与文字工具箱。基于 [FormatMaster-EN v1.4.1](https://github.com/Gu-0312/FormatMaster-EN/releases/tag/v1.4.1) 的功能设计二次开发，Rust 核心 + Java/JNI + 本地 WebView，使用 FFmpeg、LibreOfficeKit 与 PDFBox。

[下载 APK](https://github.com/shuizhaodemumu/materialbox/releases/latest) · [个人主页](https://shuizhaodemumu.github.io/) · [构建说明](BUILDING.md)

## 功能

- 视频/音频转换、压缩、剪辑、响度与音量调整、GIF及截图。
- 图片转换、缩放、旋转、灰度；17项文字/数据工具。
- PDF独立栏目：图片、Word、PPT、Excel、HTML转PDF；PDF转图片、可编辑Word/PPT/Excel、PDF/A；合并与提取页面。
- 后台任务、系统文件导入、另存为、分享、取消和重试。
- 四张淡紫龙娘背景、紧凑首页、开源与联系方式页面；捐赠完全自愿，软件免费。

## 已验证范围

0.3.0 APK支持Android8.0+ / arm64-v8a，约106 MB，无互联网权限，实际在Android16手机安装运行。GitHub链接由系统浏览器打开。

11项Rust测试、25项Android JNI/文件/FFmpeg/Office/任务检查通过；系统导入保存与四栏界面另外实测。PDF/A-2b样本经veraPDF1.30.3校验，144规则/963检查零失败。界面通过320/390/768/1440宽度检查，GitHub路由及联系方式复制检查通过。Gemini独立审阅代码和执行日志，实际设备操作由Codex完成。

PDF反向转换是几何重建：Word文字框与Word/PPT规则表格可编辑，Excel单元格保留文字和数值。**不能保证所有文档完全保留原版**：无扫描OCR，复杂曲线、旋转文字、合并/无边框/多表格、缺失字体有限制；旋转或偏移裁剪页面拒绝可编辑重建，Excel不支持的网格明确报错。Word定位文本框不具备原始段落自动重排体验。原版其他插件/聊天/下载/代码编辑等尚未全部移植。

## 结构与许可

`core/` Rust核心；`android/` 原生桥接与界面；`templates/` OOXML空白模板；`tests/` 应用自身测试样本；`third-party-licenses/` 组件版权与许可；`DEPENDENCIES.json` 发行构建依赖的校验摘要。

应用源码以GPL-3.0-or-later发布，保留上游MIT及第三方许可；各组件依原许可，详见LICENSE与third-party-licenses。没有上传签名私钥、下载缓存或个人简历。

## 美术与协作

玲珑（Codex）实现与打包；山田樱子（Gemini）独立代码/测试证据审阅；咲夜梦（MiniMax H3）按作者参考画风的文字描述生成图标/背景参考。背景采用512×288、22帧中间帧，使用稳定权重节点与分阶段条件准备；不是原生高分辨率图片条件生成。界面通过CSS浅色遮罩保证可读性。关于页二维码以CSS视窗显示完整有效区域。
