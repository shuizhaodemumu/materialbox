package cn.materialbox.app;
import android.app.*;
import android.content.*;
import android.os.*;
import org.json.*;
import java.io.*;
import java.util.concurrent.*;
import com.arthenica.ffmpegkit.*;

public class ConversionService extends Service {
 private final ExecutorService worker=Executors.newSingleThreadExecutor();
 private volatile boolean scheduled=false, cancelled=false;
 private volatile JSONObject current;
 private PowerManager.WakeLock wake;
 @Override public void onCreate(){super.onCreate();Store.load(this);NotificationManager n=(NotificationManager)getSystemService(NOTIFICATION_SERVICE);n.createNotificationChannel(new NotificationChannel("convert","转换任务",NotificationManager.IMPORTANCE_LOW));if(Build.VERSION.SDK_INT>=33)registerReceiver(cancelReceiver,new IntentFilter("cn.materialbox.CANCEL"),Context.RECEIVER_NOT_EXPORTED);else registerReceiver(cancelReceiver,new IntentFilter("cn.materialbox.CANCEL"));}
 private Notification notification(String text){PendingIntent open=PendingIntent.getActivity(this,0,new Intent(this,MainActivity.class),PendingIntent.FLAG_IMMUTABLE);return new Notification.Builder(this,"convert").setSmallIcon(android.R.drawable.stat_sys_download).setContentTitle("材料魔盒").setContentText(text).setContentIntent(open).setOngoing(true).build();}
 @Override public synchronized int onStartCommand(Intent intent,int flags,int startId){
  startForeground(42,notification("正在处理本地文件"));
  if(!scheduled){scheduled=true;worker.submit(this::drain);}return START_NOT_STICKY;
 }
 private void drain(){
  wake=((PowerManager)getSystemService(POWER_SERVICE)).newWakeLock(PowerManager.PARTIAL_WAKE_LOCK,"materialbox:convert");wake.acquire(6*60*60*1000L);
  try{while(true){JSONObject next=null;synchronized(Store.class){for(int i=Store.tasks.size()-1;i>=0;i--){JSONObject t=Store.tasks.get(i);if("queued".equals(t.optString("status"))){next=t;current=t;cancelled=false;Store.update(this,next,"running","正在转换");break;}}}
   if(next==null)break;process(next);current=null;
  }}finally{if(wake!=null&&wake.isHeld())wake.release();new Handler(getMainLooper()).post(()->{synchronized(Store.class){for(JSONObject t:Store.tasks)if("queued".equals(t.optString("status"))){worker.submit(this::drain);return;}}synchronized(this){scheduled=false;}stopForeground(STOP_FOREGROUND_REMOVE);stopSelf();});}
 }
 private void process(JSONObject t){JSONObject req=t.optJSONObject("request");String mode=req.optString("mode"),output=req.optString("output");
  try{
   JSONObject r;
   if(mode.startsWith("video")||mode.startsWith("audio")||mode.equals("gif")||mode.equals("frame")){
    r=new JSONObject(Core.call("plan",req.toString()));if(!r.optBoolean("ok"))throw new IOException(r.optString("error"));
    JSONArray a=r.getJSONObject("data").getJSONArray("args");String[] args=new String[a.length()];for(int i=0;i<a.length();i++)args[i]=a.getString(i);
    final JSONObject task=t;FFmpegKitConfig.enableStatisticsCallback(stats->Store.progress(task,(long)stats.getTime()));
    FFmpegSession session=FFmpegKit.executeWithArguments(args);
    FFmpegKitConfig.enableStatisticsCallback(null);
    if(cancelled||ReturnCode.isCancel(session.getReturnCode()))throw new CancellationException();
    if(!ReturnCode.isSuccess(session.getReturnCode())){String logs=session.getAllLogsAsString();throw new IOException("媒体转换失败："+logs.substring(Math.max(0,logs.length()-600)));}
   }else if(mode.equals("pdf-excel")||mode.equals("office-docx")||mode.equals("office-pptx")){
    PdfTables.convert(this,req);if(cancelled)throw new CancellationException();
   }else if(mode.equals("pdf-image")){
    PdfLocal.images(req);if(cancelled)throw new CancellationException();
   }else if(mode.startsWith("office")){
    OfficeClient.convert(this,req);if(cancelled)throw new CancellationException();
   }else{
    r=new JSONObject(Core.call("convert",req.toString()));if(!r.optBoolean("ok"))throw new IOException(r.optString("error"));
    t.put("detail",r.optJSONObject("data"));if(cancelled)throw new CancellationException();
   }
   File out=new File(output);if(!out.isFile()||out.length()==0)throw new IOException("转换未生成有效文件");t.put("bytes",out.length());Store.update(this,t,"done","转换完成，点击保存或分享");
  }catch(CancellationException e){new File(output).delete();Store.update(this,t,"cancelled","已取消，原始文件保留");}
  catch(Throwable e){new File(output).delete();Store.update(this,t,"failed",e.getMessage()==null?"转换失败":e.getMessage());}
 }
 public static void cancel(Context c,String id){JSONObject t=Store.find(id);if(t==null)return;if("queued".equals(t.optString("status"))){Store.update(c,t,"cancelled","已取消");return;}c.sendBroadcast(new Intent("cn.materialbox.CANCEL").setPackage(c.getPackageName()).putExtra("id",id));}
 private final BroadcastReceiver cancelReceiver=new BroadcastReceiver(){public void onReceive(Context c,Intent i){if(current!=null&&current.optString("id").equals(i.getStringExtra("id"))){cancelled=true;FFmpegKit.cancel();}}};
 @Override public void onDestroy(){try{unregisterReceiver(cancelReceiver);}catch(Exception ignored){}FFmpegKitConfig.enableStatisticsCallback(null);worker.shutdown();super.onDestroy();}
 @Override public IBinder onBind(Intent intent){return null;}
 @Override public void onTaskRemoved(Intent intent){super.onTaskRemoved(intent);}
 @Override public void onTimeout(int startId,int fgsType){cancelled=true;FFmpegKit.cancel();stopSelf();}
 @Override public void onStart(Intent intent,int startId){super.onStart(intent,startId);}
}
