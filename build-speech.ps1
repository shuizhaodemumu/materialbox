param([string]$NdkRoot=$env:ANDROID_NDK_HOME,[string]$CMake='cmake',[string]$Ninja='ninja')
$ErrorActionPreference='Stop'
if(!$NdkRoot){throw 'Set ANDROID_NDK_HOME to Android NDK r27c'}
& $CMake -S "$PSScriptRoot/native" -B "$PSScriptRoot/build-speech" -G Ninja "-DCMAKE_MAKE_PROGRAM=$Ninja" "-DCMAKE_TOOLCHAIN_FILE=$NdkRoot/build/cmake/android.toolchain.cmake" -DANDROID_ABI=arm64-v8a -DANDROID_PLATFORM=android-26 -DANDROID_STL=c++_static -DCMAKE_BUILD_TYPE=Release "-DSHERPA_LIBS=$PSScriptRoot/work/deps/sherpa-android/jniLibs/arm64-v8a" "-DSHERPA_INCLUDE=$PSScriptRoot/third_party"
if($LASTEXITCODE -ne 0){throw 'Speech CMake configuration failed'}
& $CMake --build "$PSScriptRoot/build-speech" --parallel 6
if($LASTEXITCODE -ne 0){throw 'Speech native compilation failed'}
