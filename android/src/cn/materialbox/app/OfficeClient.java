package cn.materialbox.app;
import android.content.*;import android.os.*;import org.json.*;import java.io.*;import java.util.concurrent.*;
public final class OfficeClient {
 public static void convert(Context c,JSONObject job)throws Exception{
  if(job.optString("input").toLowerCase(java.util.Locale.ROOT).endsWith(".pdf")&&java.util.Arrays.asList("docx","pptx","xlsx").contains(job.optString("format"))){PdfTables.convert(c,job);return;}
  CountDownLatch latch=new CountDownLatch(1);Bundle[] result=new Bundle[1];
  Messenger reply=new Messenger(new Handler(Looper.getMainLooper()){public void handleMessage(Message m){result[0]=m.getData();latch.countDown();}});
  ServiceConnection connection=new ServiceConnection(){public void onServiceConnected(ComponentName n,IBinder binder){try{Message m=Message.obtain(null,1);Bundle b=new Bundle();b.putString("job",job.toString());m.setData(b);m.replyTo=reply;new Messenger(binder).send(m);}catch(Exception e){result[0]=new Bundle();result[0].putString("error",e.getMessage());latch.countDown();}}public void onServiceDisconnected(ComponentName n){if(result[0]==null){result[0]=new Bundle();result[0].putString("error","本地文档引擎中断，原文件保留");latch.countDown();}}};
  Class<?> engine=OfficeService.class;
  if(job.optString("input").toLowerCase(java.util.Locale.ROOT).endsWith(".pdf")){if(job.optString("format").equals("docx"))engine=WordOfficeService.class;else if(job.optString("format").equals("pptx"))engine=PresentationOfficeService.class;}
  if(!c.bindService(new Intent(c,engine),connection,Context.BIND_AUTO_CREATE))throw new IOException("无法启动本地文档引擎");
  try{if(!latch.await(120,TimeUnit.SECONDS))throw new IOException("文档排版超过两分钟，请减少文档大小后重试");if(!result[0].getBoolean("ok"))throw new IOException(result[0].getString("error","文档转换失败"));}finally{c.unbindService(connection);}
 }
}
