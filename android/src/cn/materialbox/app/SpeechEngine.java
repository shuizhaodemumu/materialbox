package cn.materialbox.app;

import android.content.Context;
import com.arthenica.ffmpegkit.*;
import java.io.*;
import java.security.MessageDigest;
import java.util.UUID;
import java.util.concurrent.CancellationException;
import java.util.function.BooleanSupplier;
import org.json.JSONObject;

public final class SpeechEngine {
 static { System.loadLibrary("materialbox_speech"); }
 public interface Progress { void onProgress(int percent); }
 public static native void reset();
 public static native void cancel();
 private static native String transcribe(String model,String pcm,String output,String format,String language,int threads,Progress progress);
 private static File model(Context c,BooleanSupplier cancelled)throws Exception {
  File dir=new File(c.getFilesDir(),"speech-models");dir.mkdirs();
  ensure(c,dir,"model.int8.onnx",237115547L,"12ca1a2ae7ecf3e0019ef2822307ee0b5cadc9196569e379b4c4026f8205276d",cancelled);
  ensure(c,dir,"silero_vad.onnx",643854L,"9e2449e1087496d8d4caba907f23e0bd3f78d91fa552479bb9c23ac09cbb1fd6",cancelled);
  ensure(c,dir,"tokens.txt",315894L,"f449eb28dc567533d7fa59be34e2abca8784f771850c78a47fb731a31429a1dc",cancelled);
  return new File(dir,"model.int8.onnx");
 }
 private static void ensure(Context c,File dir,String name,long size,String hash,BooleanSupplier cancelled)throws Exception {
  File dst=new File(dir,name);if(dst.isFile()&&dst.length()==size)return;
  File temp=new File(dir,UUID.randomUUID()+".partial");
  try{MessageDigest sha=MessageDigest.getInstance("SHA-256");
   try(InputStream in=c.getAssets().open("speech/"+name);OutputStream out=new FileOutputStream(temp)){byte[] buf=new byte[1024*1024];int n;while((n=in.read(buf))!=-1){if(cancelled.getAsBoolean())throw new CancellationException();sha.update(buf,0,n);out.write(buf,0,n);}}
   StringBuilder hex=new StringBuilder();for(byte b:sha.digest())hex.append(String.format("%02x",b&255));if(!hash.contentEquals(hex))throw new IOException("离线模型校验失败");
   if(!temp.renameTo(dst))throw new IOException("无法保存离线模型");
  }finally{temp.delete();}
 }
 public static void convert(Context c,JSONObject req,Progress progress,BooleanSupplier cancelled)throws Exception {
  File pcm=new File(c.getCacheDir(),"speech-"+UUID.randomUUID()+".pcm"),partial=new File(req.getString("output")+".partial");
  new File(req.getString("output")).getParentFile().mkdirs();
  try{
   File model=model(c,cancelled);if(cancelled.getAsBoolean())throw new CancellationException();
   FFmpegSession session=FFmpegKit.executeWithArguments(new String[]{"-y","-nostdin","-i",req.getString("input"),"-vn","-map","0:a:0","-ar","16000","-ac","1","-c:a","pcm_s16le","-f","s16le","-fs","460800002",pcm.getAbsolutePath()});
   if(cancelled.getAsBoolean()||ReturnCode.isCancel(session.getReturnCode()))throw new CancellationException();
   if(!ReturnCode.isSuccess(session.getReturnCode()))throw new IOException("无法读取音轨，请选择有效的音频或视频文件");
   String error=transcribe(model.getAbsolutePath(),pcm.getAbsolutePath(),partial.getAbsolutePath(),req.getString("format"),req.optString("language","auto"),Math.min(4,Runtime.getRuntime().availableProcessors()),progress);
   if(cancelled.getAsBoolean())throw new CancellationException();
   if(error!=null){if(error.contains("No recognizable"))throw new IOException("未识别到有效语音，请检查音量与录音内容");throw new IOException("离线识别失败："+error);}
   SimplifiedChinese.file(c,partial);
   if(cancelled.getAsBoolean())throw new CancellationException();
   if(!partial.renameTo(new File(req.getString("output"))))throw new IOException("无法保存识别结果");
  }finally{pcm.delete();partial.delete();}
 }
 private SpeechEngine(){}
}
