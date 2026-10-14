param([ValidateSet('test','android','host','lint')][string]$Mode='test')
$ErrorActionPreference='Stop'
$manifest=Join-Path $PSScriptRoot 'core/Cargo.toml'
switch($Mode){
 'test' {cargo test --manifest-path $manifest}
 'lint' {cargo clippy --manifest-path $manifest --all-targets -- -D warnings}
 'host' {cargo build --manifest-path $manifest --release}
 'android' {cargo build --manifest-path $manifest --lib --release --target aarch64-linux-android}
}
if($LASTEXITCODE -ne 0){throw "Rust $Mode failed"}
