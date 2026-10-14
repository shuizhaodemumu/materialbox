package cn.materialbox.app;
import android.content.Context;
import org.json.*;
import java.io.*;
import java.util.*;

public final class Store {
 public static final List<JSONObject> tasks=new ArrayList<>();
 private static boolean loaded=false;
 public static synchronized void load(Context c) {
  if(loaded)return;loaded=true;
  try {JSONArray a=new JSONArray(c.getSharedPreferences("tasks",0).getString("list","[]"));for(int i=0;i<a.length();i++){JSONObject t=a.getJSONObject(i);if("running".equals(t.optString("status"))||"queued".equals(t.optString("status"))){t.put("status","interrupted");t.put("message","任务中断，可点击重试");}tasks.add(t);}}catch(Exception ignored){}
  save(c);
 }
 public static synchronized void save(Context c){c.getSharedPreferences("tasks",0).edit().putString("list",new JSONArray(tasks).toString()).commit();}
 public static synchronized JSONObject find(String id){for(JSONObject t:tasks)if(id.equals(t.optString("id")))return t;return null;}
 public static synchronized String snapshot(Context c){load(c);return new JSONArray(tasks).toString();}
 public static synchronized void update(Context c,JSONObject t,String status,String message){try{t.put("status",status);t.put("message",message);save(c);}catch(Exception ignored){}}
 public static synchronized void progress(JSONObject t,long ms){try{t.put("elapsed",ms);}catch(Exception ignored){}}
 public static synchronized String enqueue(Context c,JSONObject request)throws Exception{
  load(c);if(tasks.size()>=200)throw new IOException("任务记录已满，请清理已完成任务");
  if(request.optString("input").isEmpty())throw new IOException("请选择文件");
  String mode=request.optString("mode");String id=UUID.randomUUID().toString();String format=request.optString("format");if(!format.matches("[a-z0-9]{1,8}"))throw new IOException("输出格式无效");
  request.put("output",new File(new File(c.getFilesDir(),"outputs"),id+"."+format).getAbsolutePath());
  // Only imported, app-private inputs can enter a job, even from the local WebView bridge.
  String root=new File(c.getFilesDir(),"inputs").getCanonicalPath()+File.separator;
  if(!new File(request.getString("input")).getCanonicalPath().startsWith(root))throw new IOException("输入路径不在导入目录");
  JSONArray in=request.optJSONArray("inputs");if(in!=null)for(int i=0;i<in.length();i++)if(!new File(in.getString(i)).getCanonicalPath().startsWith(root))throw new IOException("输入路径无效");
  JSONObject t=new JSONObject();t.put("id",id);t.put("request",request);t.put("name",request.optString("name","文件"));t.put("label",request.optString("label",""));t.put("mode",mode);t.put("format",format);t.put("status","queued");t.put("message","等待转换");t.put("created",System.currentTimeMillis());t.put("progress",0);tasks.add(0,t);save(c);
  return id;
 }
 public static synchronized void clearCompleted(Context c){Set<String> oldInputs=new HashSet<>();Iterator<JSONObject> it=tasks.iterator();while(it.hasNext()){JSONObject t=it.next();if(!Arrays.asList("running","queued").contains(t.optString("status"))){JSONObject j=t.optJSONObject("request");String out=j.optString("output");if(!out.isEmpty())new File(out).delete();oldInputs.add(j.optString("input"));JSONArray a=j.optJSONArray("inputs");if(a!=null)for(int i=0;i<a.length();i++)oldInputs.add(a.optString(i));it.remove();}}for(JSONObject t:tasks){JSONObject j=t.optJSONObject("request");oldInputs.remove(j.optString("input"));JSONArray a=j.optJSONArray("inputs");if(a!=null)for(int i=0;i<a.length();i++)oldInputs.remove(a.optString(i));}for(String path:oldInputs)try{File f=new File(path);String base=new File(c.getFilesDir(),"inputs").getCanonicalPath()+File.separator;if(f.getCanonicalPath().startsWith(base))f.delete();}catch(Exception ignored){}save(c);}
}
