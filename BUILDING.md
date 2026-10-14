# Build requirements

Rust stable; Android `aarch64-linux-android` target; Android NDK linker/sysroot configured via Cargo environment or `.cargo/config.toml`; JDK 17; Android platform35, build tools, and D8/R8 8.13.19. `MATERIALBOX_WINDRES` selects the Windows resource compiler.

`cargo test --manifest-path core/Cargo.toml` runs 11 tests. `build-core.ps1 -Mode android` builds the JNI library. `build-apk.py` builds/signs the Android shell; `--debug` includes the app-owned instrumentation suite. Its generated key/password stay under ignored `work/signing` and are never published. A new local key cannot update the author's signed release APK.

Set `MATERIALBOX_ANDROID_TOOLS` to the Android tool root, optionally `MATERIALBOX_JDK`, `MATERIALBOX_BUILD_TOOLS` and `MATERIALBOX_ANDROID_JAR`. The tool root must contain r8-8.13.19.jar. Extract the following distributions into `work/deps`, matching build-apk.py's explicit paths:

- FFmpegKit maintained full-gpl7.1.6 AAR: `https://repo.maven.apache.org/maven2/dev/ffmpegkit-maintained/ffmpeg-kit-full-gpl/7.1.6/` (see dependency checksums; upstream namespace must match the exact AAR in the manifest).
- LibreOffice Android26.2.6.3: `https://f-droid.org/repo/org.documentfoundation.libreoffice_131.apk`; extract native libraries under work/deps/libreoffice/lib/arm64-v8a. Matching program/share/unpack assets are already included under android/assets; org.libreoffice.kit wrappers retain their MPL headers.
- PDFBox Android2.0.27.0: `https://repo.maven.apache.org/maven2/com/tom-roush/pdfbox-android/2.0.27.0/`; classes.jar under work/deps/pdfbox.
- BouncyCastle1.72 bcprov/bcpkix/bcutil-jdk15to18 JARs and SmartException0.2.1 Java/common JARs, named as expected by the builder.
- FFmpeg native libc++_shared.so is replaced by official NDKr27c arm64 libc++ with16KB alignment; all shipped ELF load segments were checked. Native sources and notices: https://github.com/ffmpegkit-maintained/ffmpeg , https://github.com/LibreOffice/core/tree/libreoffice-26.2.6.3 , https://github.com/TomRoush/PdfBox-Android , https://github.com/bcgit/bc-java .

The released APK was compiled, signed and tested on the author's Windows toolchain. A clean-machine rebuild with these adapted environment paths has not been verified. This repository ships application source and notices, not the entire NDK, downloaded native engines or signing material.

UI tests: `npm install`, `npx playwright-core install chromium`, `npm test`. Set PLAYWRIGHT_CHROME to an existing Chrome executable if preferred. Tests use a mock Native bridge; real conversion checks are Rust tests and tests/android/DeviceChecks.java in a debug APK.
