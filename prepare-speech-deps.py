"""Fetch pinned speech dependencies. Run on the build PC; Android stays offline."""
from pathlib import Path
import argparse,hashlib,json,shutil,tarfile,urllib.request
root=Path(__file__).resolve().parent
parser=argparse.ArgumentParser();parser.add_argument('--cache-dir',type=Path);args=parser.parse_args()
deps=root/'work/deps';deps.mkdir(parents=True,exist_ok=True)
cache=args.cache_dir or deps;cache.mkdir(parents=True,exist_ok=True)
files=[
 ('sherpa-onnx-v1.13.8-android.tar.bz2','v1.13.8','2ff63469a71cb6009aa2e3ed5f4a670f8abdcbe4bb9ffd23776afc792a6b4f44'),
 ('sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2025-09-09.tar.bz2','asr-models','7305f7905bfcf77fa0b39388a313f3da35c68d971661a65475b56fb2162c8e63'),
 ('silero_vad.onnx','asr-models','9e2449e1087496d8d4caba907f23e0bd3f78d91fa552479bb9c23ac09cbb1fd6')]
for name,tag,sha in files:
 p=cache/name
 if not p.exists():
  tmp=p.with_suffix(p.suffix+'.partial')
  try:urllib.request.urlretrieve('https://github.com/k2-fsa/sherpa-onnx/releases/download/'+tag+'/'+name,tmp);tmp.replace(p)
  finally:tmp.unlink(missing_ok=True)
 with p.open('rb') as f:assert hashlib.file_digest(f,'sha256').hexdigest()==sha,'Checksum mismatch: '+name
 if name.endswith('tar.bz2'):
  dest=deps/('sherpa-android' if tag=='v1.13.8' else 'sensevoice-model');dest.mkdir(exist_ok=True)
  with tarfile.open(p) as t:t.extractall(dest,filter='data')
speech=root/'android/assets/speech';speech.mkdir(parents=True,exist_ok=True)
model=next((deps/'sensevoice-model').glob('sherpa-*'))
for name in ['model.int8.onnx','tokens.txt']:shutil.copy2(model/name,speech/name)
shutil.copy2(cache/'silero_vad.onnx',speech/'silero_vad.onnx')
for name,entry in json.loads((root/'SPEECH-MODELS.json').read_text(encoding='utf8')).items():
 p=speech/name
 with p.open('rb') as f:assert p.stat().st_size==entry['bytes'] and hashlib.file_digest(f,'sha256').hexdigest()==entry['sha256'],name
print('SPEECH_DEPENDENCIES_VERIFIED: pinned Android libraries, SenseVoice model, VAD and tokens')
