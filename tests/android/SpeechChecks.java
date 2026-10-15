package cn.materialbox.app;
import android.app.Instrumentation;
import android.content.Context;
import android.os.Bundle;
import java.io.*;
import java.util.concurrent.CancellationException;
import org.json.*;
import com.arthenica.ffmpegkit.*;

public class SpeechChecks extends Instrumentation {
 private void check(boolean value,String message){if(!value)throw new AssertionError(message);}
 private String read(File f)throws Exception{try(InputStream in=new FileInputStream(f);ByteArrayOutputStream out=new ByteArrayOutputStream()){byte[] b=new byte[8192];int n;while((n=in.read(b))!=-1)out.write(b,0,n);return out.toString("UTF-8");}}
 public void onCreate(Bundle b){super.onCreate(b);start();}
 public void onStart(){Bundle report=new Bundle();int checks=0;Context c=getTargetContext();
  startActivitySync(new android.content.Intent(c,MainActivity.class).addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK));
  try{
   String sample="我們測試離線語音識別，頭髮與發展，乾隆和乾坤。English PDF 123 🙂";
   String simple=SimplifiedChinese.convert(c,sample);
   check(simple.equals("我们测试离线语音识别，头发与发展，乾隆和乾坤。English PDF 123 🙂"),"phrase-aware simplified conversion: "+simple);checks++;
   String cue="1\n00:00:01,000 --> 00:00:02,500\n軟體與後臺 English PDF\n\n";
   check(SimplifiedChinese.convert(c,cue).equals("1\n00:00:01,000 --> 00:00:02,500\n软体与后台 English PDF\n\n"),"SRT structure and English unchanged");checks++;
   File legacy=new File(c.getCacheDir(),"legacy-speech-check.srt");try(OutputStream out=new FileOutputStream(legacy)){out.write(cue.getBytes("UTF-8"));}SimplifiedChinese.file(c,legacy);String normalized=read(legacy);SimplifiedChinese.file(c,legacy);check(read(legacy).equals(normalized)&&normalized.contains("后台"),"existing transcript normalization is idempotent");legacy.delete();checks++;
   File dir=new File(c.getFilesDir(),"inputs");dir.mkdirs();File audio=new File(dir,"speech-mixed.wav");
   try(InputStream in=c.getAssets().open("speech-mixed.wav");OutputStream out=new FileOutputStream(audio)){byte[] b=new byte[65536];int n;while((n=in.read(b))!=-1)out.write(b,0,n);}
   File result=new File(c.getFilesDir(),"speech-check.txt");JSONObject req=new JSONObject().put("mode","speech").put("input",audio.getAbsolutePath()).put("output",result.getAbsolutePath()).put("format","txt").put("language","auto");
   SpeechEngine.reset();long began=System.currentTimeMillis();SpeechEngine.convert(c,req,p->{},()->false);String text=read(result);
   check(text.matches("(?s).*[\\u4e00-\\u9fff].*")&&text.toLowerCase().contains("offline")&&text.toLowerCase().contains("speech recognition"),"Chinese-English original transcript: "+text);checks++;
   check(text.equals(SimplifiedChinese.convert(c,text))&&!text.contains("我們")&&!text.contains("識別"),"real speech output is simplified");
   report.putString("transcript",text);report.putLong("recognitionMillis",System.currentTimeMillis()-began);
   File longer=new File(dir,"speech-mixed-long.wav");FFmpegSession loop=FFmpegKit.executeWithArguments(new String[]{"-y","-stream_loop","1","-i",audio.getAbsolutePath(),"-c:a","pcm_s16le",longer.getAbsolutePath()});check(ReturnCode.isSuccess(loop.getReturnCode()),"long audio fixture");
   File srt=new File(c.getFilesDir(),"speech-check.srt");req.put("input",longer.getAbsolutePath()).put("format","srt").put("output",srt.getAbsolutePath());SpeechEngine.reset();SpeechEngine.convert(c,req,p->{},()->false);String subtitles=read(srt);check(subtitles.contains(" --> ")&&subtitles.contains("00:00:"),"SRT timestamps");
   java.util.regex.Matcher stamps=java.util.regex.Pattern.compile("(\\d{2}):(\\d{2}):(\\d{2}),(\\d{3})").matcher(subtitles);long last=0;int stampCount=0;while(stamps.find()){long ms=Long.parseLong(stamps.group(1))*3600000+Long.parseLong(stamps.group(2))*60000+Long.parseLong(stamps.group(3))*1000+Long.parseLong(stamps.group(4));check(ms>=last&&ms<=41100,"monotonic timestamps across chunks");last=ms;stampCount++;}check(stampCount>=4&&last>30000,"long recording crosses 30-second boundary");checks++;
   req.put("input",audio.getAbsolutePath()).put("format","txt").put("output",result.getAbsolutePath()+".cancelled");SpeechEngine.reset();final boolean[] aborted={false};boolean cancelled=false;
   try{SpeechEngine.convert(c,req,p->{aborted[0]=true;SpeechEngine.cancel();},()->aborted[0]);}catch(CancellationException e){cancelled=true;}
   check(cancelled&&!new File(req.getString("output")).exists()&&!new File(req.getString("output")+".partial").exists(),"Cancellation and partial cleanup");checks++;
   File silence=new File(dir,"speech-silence.wav");FFmpegSession ff=FFmpegKit.executeWithArguments(new String[]{"-y","-f","lavfi","-i","anullsrc=r=16000:cl=mono","-t","3",silence.getAbsolutePath()});check(ReturnCode.isSuccess(ff.getReturnCode()),"silence fixture");
   req.put("input",silence.getAbsolutePath()).put("output",result.getAbsolutePath()+".silence");SpeechEngine.reset();boolean rejected=false;try{SpeechEngine.convert(c,req,p->{},()->false);}catch(IOException e){rejected=e.getMessage().contains("未识别到有效语音");}check(rejected&&!new File(req.getString("output")).exists(),"Silence produces no hallucinated transcript");checks++;
   req.put("input",audio.getAbsolutePath()).put("output",result.getAbsolutePath()+".bad").put("format","html");SpeechEngine.reset();rejected=false;try{SpeechEngine.convert(c,req,p->{},()->false);}catch(IOException e){rejected=true;}check(rejected,"Invalid output rejected");checks++;
   File[] leftovers=c.getCacheDir().listFiles((d,n)->n.startsWith("speech-")&&n.endsWith(".pcm"));check(leftovers!=null&&leftovers.length==0,"Decoded PCM cleaned after success/failure/cancel");checks++;
   report.putString("result","SPEECH_CHECKS_PASS: "+checks);finish(-1,report);
  }catch(Throwable e){report.putString("result","SPEECH_CHECKS_FAIL after "+checks+": "+e);finish(0,report);}
 }
}

