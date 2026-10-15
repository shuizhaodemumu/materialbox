#include <jni.h>
#include <sherpa-onnx/c-api/c-api.h>
#include <algorithm>
#include <atomic>
#include <fstream>
#include <iomanip>
#include <memory>
#include <regex>
#include <sstream>
#include <stdexcept>
#include <string>
#include <vector>
static std::atomic<bool> cancelled{false};
static std::string utf(JNIEnv* e,jstring s){const char* p=e->GetStringUTFChars(s,nullptr);if(!p)throw std::runtime_error("Invalid argument");std::string v(p);e->ReleaseStringUTFChars(s,p);return v;}
static std::string stamp(long long ms){std::ostringstream s;s<<std::setfill('0')<<std::setw(2)<<ms/3600000<<':'<<std::setw(2)<<ms/60000%60<<':'<<std::setw(2)<<ms/1000%60<<','<<std::setw(3)<<ms%1000;return s.str();}
static void check(){if(cancelled)throw std::runtime_error("Cancelled");}
extern "C" JNIEXPORT void JNICALL Java_cn_materialbox_app_SpeechEngine_reset(JNIEnv*,jclass){cancelled=false;}
extern "C" JNIEXPORT void JNICALL Java_cn_materialbox_app_SpeechEngine_cancel(JNIEnv*,jclass){cancelled=true;}
extern "C" JNIEXPORT jstring JNICALL Java_cn_materialbox_app_SpeechEngine_transcribe(JNIEnv* env,jclass,jstring model,jstring pcm,jstring output,jstring format,jstring language,jint threads,jobject receiver){
 try{
  auto path=utf(env,model),input=utf(env,pcm),dest=utf(env,output),fmt=utf(env,format),lang=utf(env,language);
  if(fmt!="txt"&&fmt!="srt")throw std::runtime_error("Unsupported transcript format");
  if(lang!="auto"&&lang!="zh"&&lang!="en")throw std::runtime_error("Unsupported language");
  auto dir=path.substr(0,path.find_last_of('/'));auto tokens=dir+"/tokens.txt",vadPath=dir+"/silero_vad.onnx";
  std::ifstream in(input,std::ios::binary|std::ios::ate);long long bytes=in?static_cast<long long>(in.tellg()):0;
  if(bytes<=0||bytes%2||bytes>16000LL*2*3600*4)throw std::runtime_error("Audio must be nonempty and at most 4 hours");
  in.seekg(0);long long total=bytes/2,offset=0,lastEnd=0;int index=0,lastProgress=-1;
  auto method=env->GetMethodID(env->GetObjectClass(receiver),"onProgress","(I)V");if(!method)throw std::runtime_error("Progress unavailable");
  auto progress=[&](){int p=std::min(99,int(offset*100/total));if(p!=lastProgress){env->CallVoidMethod(receiver,method,p);lastProgress=p;}if(env->ExceptionCheck())throw std::runtime_error("Progress failed");check();};
  progress();
  SherpaOnnxOfflineRecognizerConfig config{};config.feat_config.sample_rate=16000;config.feat_config.feature_dim=80;
  config.model_config.sense_voice.model=path.c_str();config.model_config.sense_voice.language=lang.c_str();config.model_config.sense_voice.use_itn=1;
  config.model_config.tokens=tokens.c_str();config.model_config.num_threads=std::clamp(int(threads),1,4);config.model_config.provider="cpu";config.model_config.model_type="sense_voice";config.decoding_method="greedy_search";
  std::unique_ptr<const SherpaOnnxOfflineRecognizer,decltype(&SherpaOnnxDestroyOfflineRecognizer)> rec(SherpaOnnxCreateOfflineRecognizer(&config),SherpaOnnxDestroyOfflineRecognizer);
  if(!rec)throw std::runtime_error("Cannot load SenseVoice model");check();
  SherpaOnnxVadModelConfig vc{};vc.sample_rate=16000;vc.num_threads=1;vc.provider="cpu";vc.silero_vad.model=vadPath.c_str();vc.silero_vad.threshold=.5f;vc.silero_vad.min_silence_duration=.5f;vc.silero_vad.min_speech_duration=.25f;vc.silero_vad.window_size=512;vc.silero_vad.max_speech_duration=20;
  std::unique_ptr<const SherpaOnnxVoiceActivityDetector,decltype(&SherpaOnnxDestroyVoiceActivityDetector)> vad(SherpaOnnxCreateVoiceActivityDetector(&vc,60),SherpaOnnxDestroyVoiceActivityDetector);
  if(!vad)throw std::runtime_error("Cannot load speech detector");
  std::ofstream out(dest,std::ios::binary|std::ios::trunc);if(!out)throw std::runtime_error("Cannot create transcript");
  auto drain=[&](){while(!SherpaOnnxVoiceActivityDetectorEmpty(vad.get())){
   std::unique_ptr<const SherpaOnnxSpeechSegment,decltype(&SherpaOnnxDestroySpeechSegment)> seg(SherpaOnnxVoiceActivityDetectorFront(vad.get()),SherpaOnnxDestroySpeechSegment);
   if(!seg)throw std::runtime_error("Invalid speech segment");
   for(int pos=0;pos<seg->n;pos+=320000){check();int n=std::min(320000,seg->n-pos);
    std::unique_ptr<const SherpaOnnxOfflineStream,decltype(&SherpaOnnxDestroyOfflineStream)> stream(SherpaOnnxCreateOfflineStream(rec.get()),SherpaOnnxDestroyOfflineStream);
    if(!stream)throw std::runtime_error("Cannot create decoder");SherpaOnnxAcceptWaveformOffline(stream.get(),16000,seg->samples+pos,n);SherpaOnnxDecodeOfflineStream(rec.get(),stream.get());check();
    std::unique_ptr<const SherpaOnnxOfflineRecognizerResult,decltype(&SherpaOnnxDestroyOfflineRecognizerResult)> result(SherpaOnnxGetOfflineStreamResult(stream.get()),SherpaOnnxDestroyOfflineRecognizerResult);
    if(!result)throw std::runtime_error("Cannot read recognition result");
    std::string text=result->text?result->text:"";text=std::regex_replace(text,std::regex("<\\|[^|]*\\|>"),"");auto first=text.find_first_not_of(" \r\n\t");if(first==std::string::npos)continue;text=text.substr(first,text.find_last_not_of(" \r\n\t")-first+1);++index;
    long long start=std::max(lastEnd,(static_cast<long long>(seg->start)+pos)*1000/16000),end=std::min(total*1000/16000,(static_cast<long long>(seg->start)+pos+n)*1000/16000);end=std::max(start,end);lastEnd=end;
    if(fmt=="srt")out<<index<<'\n'<<stamp(start)<<" --> "<<stamp(end)<<'\n'<<text<<"\n\n";else out<<text<<'\n';
    if(out.tellp()>2*1024*1024)throw std::runtime_error("Transcript exceeds 2 MB");progress();
   }SherpaOnnxVoiceActivityDetectorPop(vad.get());
  }};
  while(offset<total){check();int n=int(std::min(512LL,total-offset));std::vector<int16_t> raw(n);in.read(reinterpret_cast<char*>(raw.data()),n*2);if(in.gcount()!=n*2)throw std::runtime_error("Truncated audio");std::vector<float> samples(n);for(int i=0;i<n;++i)samples[i]=raw[i]/32768.f;SherpaOnnxVoiceActivityDetectorAcceptWaveform(vad.get(),samples.data(),n);offset+=n;drain();progress();}
  SherpaOnnxVoiceActivityDetectorFlush(vad.get());drain();check();if(!index)throw std::runtime_error("No recognizable speech found");out.flush();if(!out)throw std::runtime_error("Transcript write failed");return nullptr;
 }catch(const std::exception& e){if(env->ExceptionCheck())env->ExceptionClear();return env->NewStringUTF(e.what());}
}
