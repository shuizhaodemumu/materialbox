"""Build Java shell + Rust core into an aligned, signed arm64 APK."""
from pathlib import Path
import subprocess,zipfile,os,shutil,sys,xml.etree.ElementTree as ET
root=Path(__file__).resolve().parent
workspace=root
tools=Path(os.environ.get('MATERIALBOX_ANDROID_TOOLS','local/android-tools'))
deps=workspace/'work/deps/ffmpeg'
build=root/'build';build.mkdir(exist_ok=True);(root/'outputs').mkdir(exist_ok=True)
jdk=Path(os.environ.get('MATERIALBOX_JDK',str(tools/'jdk/jdk-17.0.20.1+1')));bt=Path(os.environ.get('MATERIALBOX_BUILD_TOOLS',str(tools/'build/android-15')));platform=Path(os.environ.get('MATERIALBOX_ANDROID_JAR',str(tools/'platform/android-35/android.jar')))
def run(args):
 print('RUN',Path(str(args[0])).name,flush=True);subprocess.run([str(a) for a in args],check=True)
compiled=[]
for i,p in enumerate([deps/'res',root/'android/res']):
 archive=build/f'res-{i}.zip';run([bt/'aapt2.exe','compile','--dir',p,'-o',archive]);compiled.append(archive)
generated=build/'generated';generated.mkdir(exist_ok=True)
resources=build/'resources.apk'
debug='--debug' in sys.argv
manifest=root/'android/AndroidManifest.xml'
if debug:
 tree=ET.parse(manifest);tree.getroot().find('application').set('{http://schemas.android.com/apk/res/android}debuggable','true');ET.SubElement(tree.getroot(),'instrumentation',{'{http://schemas.android.com/apk/res/android}name':'cn.materialbox.app.DeviceChecks','{http://schemas.android.com/apk/res/android}targetPackage':'cn.materialbox.app'});manifest=build/'debug-manifest.xml';tree.write(manifest,encoding='utf-8')
args=[bt/'aapt2.exe','link','-o',resources,'--manifest',manifest,'-I',platform,'--java',generated,'--auto-add-overlay','--extra-packages','com.arthenica.ffmpegkit','-A',root/'android/assets','--min-sdk-version','26','--target-sdk-version','35']
for p in compiled:args+=['-R',p]
if debug:args+=['-A',root/'tests/fixtures']
run(args)
classes=build/'classes'
if classes.exists():
 if classes.resolve().parent!=build.resolve():raise RuntimeError('Invalid classes directory')
 shutil.rmtree(classes)
classes.mkdir(exist_ok=True)
sources=list((root/'android/src').rglob('*.java'))+list(generated.rglob('*.java'))
if debug:sources+=list((root/'tests/android').rglob('*.java'))
argfile=build/'javac.args';argfile.write_text('\n'.join('"'+str(p).replace('\\','/')+'"' for p in sources),encoding='utf8')
dependency_jars=[deps/'classes.jar',workspace/'work/deps/smart-exception.jar',workspace/'work/deps/smart-exception-common.jar',workspace/'work/deps/pdfbox/classes.jar',*list((workspace/'work/deps').glob('bc*-jdk15to18.jar'))]
run([jdk/'bin/javac.exe','-J-Dfile.encoding=UTF-8','-encoding','UTF-8','-source','8','-target','8','-classpath',os.pathsep.join(map(str,[platform,*dependency_jars])),'-d',classes,'@'+str(argfile)])
jar=build/'classes.jar'
with zipfile.ZipFile(jar,'w',zipfile.ZIP_DEFLATED) as z:
 for p in classes.rglob('*.class'):z.write(p,p.relative_to(classes).as_posix())
dex=build/'dex';dex.mkdir(exist_ok=True)
run([jdk/'bin/java.exe','-Xmx2g','-cp',tools/'r8-8.13.19.jar','com.android.tools.r8.D8','--release','--min-api','26','--lib',platform,'--output',dex,jar,*dependency_jars])
unsigned=build/'unsigned.apk'
core=root/'core/target/aarch64-linux-android/release/libmaterialbox.so'
if not core.is_file():raise RuntimeError('Android Rust core has not been built')
with zipfile.ZipFile(unsigned,'w',zipfile.ZIP_DEFLATED) as z:
 with zipfile.ZipFile(resources) as source:
  for info in source.infolist():z.writestr(info.filename,source.read(info.filename),compress_type=info.compress_type)
 for p in dex.glob('*.dex'):z.write(p,p.name)
 z.write(core,'lib/arm64-v8a/libmaterialbox.so')
 for p in (deps/'jni/arm64-v8a').glob('*.so'):z.write(p,'lib/arm64-v8a/'+p.name)
 for p in (workspace/'work/deps/libreoffice/lib/arm64-v8a').glob('*.so'):
  if p.name!='libc++_shared.so':z.write(p,'lib/arm64-v8a/'+p.name)
aligned=build/'aligned.apk';run([bt/'zipalign.exe','-P','16','-f','4',unsigned,aligned])
sign=workspace/'work/signing';sign.mkdir(exist_ok=True);key=sign/'materialbox.p12'
passwordfile=sign/'password.txt'
if not passwordfile.exists():
 import secrets
 passwordfile.write_text(secrets.token_urlsafe(32),encoding='ascii')
password=passwordfile.read_text(encoding='ascii').strip()
if not key.exists():run([jdk/'bin/keytool.exe','-genkeypair','-keystore',key,'-storetype','PKCS12','-alias','materialbox','-keyalg','RSA','-keysize','3072','-validity','10000','-storepass',password,'-keypass',password,'-dname','CN=MaterialBox Local Release, O=MaterialBox, C=CN'])
output=build/'device-debug.apk' if debug else workspace/'outputs/材料魔盒_0.3.0_Android_arm64.apk'
run([jdk/'bin/java.exe','-jar',bt/'lib/apksigner.jar','sign','--ks',key,'--ks-key-alias','materialbox','--ks-pass','file:'+str(passwordfile),'--out',output,aligned])
run([jdk/'bin/java.exe','-jar',bt/'lib/apksigner.jar','verify','--verbose',output])
run([bt/'zipalign.exe','-c','-P','16','4',output])
print('APK:',output,'bytes:',output.stat().st_size)
