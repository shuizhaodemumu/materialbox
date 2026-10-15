package cn.materialbox.app;

import android.app.*;
import android.content.*;
import android.content.pm.*;
import android.database.Cursor;
import android.net.Uri;
import android.os.*;
import android.provider.Settings;
import java.io.*;
import java.net.*;
import java.security.MessageDigest;
import java.util.*;
import java.util.concurrent.*;
import org.json.*;

/** Native-only updater. WebView never receives network or file-system access. */
public final class AppUpdates {
 public interface Listener { void changed(String snapshot); }
 private static AppUpdates instance;
 public static synchronized AppUpdates get(Context c){if(instance==null)instance=new AppUpdates(c.getApplicationContext());return instance;}
 private final Context context;
 private final DownloadManager downloads;
 private final ScheduledExecutorService worker=Executors.newSingleThreadScheduledExecutor();
 private final android.content.SharedPreferences prefs;
 private volatile Listener listener;
 private JSONObject info;
 private long downloadId=-1;
 private ScheduledFuture<?> poll;
 private boolean verifying=false;
 private boolean installPending=false;
 private final String current;
 interface Validator { void validate(File file,JSONObject release)throws Exception; }
 private final Validator validator;
 private static String installedVersion(Context c){try{return c.getPackageManager().getPackageInfo(c.getPackageName(),0).versionName;}catch(Exception e){throw new IllegalStateException(e);}}
 private AppUpdates(Context c){this(c,"app-updates",installedVersion(c),(file,data)->verify(file,data,c));}
 // Package-private seams for instrumentation: production always uses the constructor above.
 AppUpdates(Context c,String preferenceName,String installedVersion,Validator validator){context=c;downloads=(DownloadManager)c.getSystemService(Context.DOWNLOAD_SERVICE);prefs=c.getSharedPreferences(preferenceName,0);current=installedVersion;this.validator=validator;
  try{
   info=new JSONObject(prefs.getString("state","{}"));downloadId=prefs.getLong("downloadId",-1);
   String saved=info.optString("state");
   if(Arrays.asList("ready","downloading","verifying").contains(saved)&&validCachedDownload()){
    put("state","downloading");put("message","正在恢复并核验上次下载…");put("current",current);emit();startPolling();
   }else{resetCachedState();}
  }catch(Exception e){resetCachedState();}
 }
 private boolean validCachedDownload()throws Exception{
  if(info.optInt("schema")!=2||!"github:shuizhaodemumu/materialbox".equals(info.optString("source"))||downloadId<0||compare(info.getString("latest"),current)<=0)return false;
  // Reuse the exact production release parser: cache data cannot bypass URL/asset validation.
  JSONObject asset=new JSONObject().put("name",info.getString("name")).put("state","uploaded").put("size",info.getLong("total")).put("digest",info.optString("digest","")).put("browser_download_url",info.getString("url"));
  release(new JSONObject().put("tag_name",info.getString("tag")).put("assets",new JSONArray().put(asset)),current);
  File file=new File(info.getString("file")).getCanonicalFile();if(!ownedUpdateFile(file))return false;
  try(Cursor c=downloads.query(new DownloadManager.Query().setFilterById(downloadId))){
   if(c==null||!c.moveToFirst()||!info.getString("url").equals(c.getString(c.getColumnIndexOrThrow(DownloadManager.COLUMN_URI))))return false;
   int status=c.getInt(c.getColumnIndexOrThrow(DownloadManager.COLUMN_STATUS));
   // DownloadManager has no local URI until a pending transfer creates its file.
   String local=c.getString(c.getColumnIndexOrThrow(DownloadManager.COLUMN_LOCAL_URI));if(local!=null&&!file.equals(new File(Uri.parse(local).getPath()).getCanonicalFile()))return false;
   return status!=DownloadManager.STATUS_FAILED&&(status!=DownloadManager.STATUS_SUCCESSFUL||(local!=null&&file.isFile()));
  }
 }
 private boolean ownedUpdateFile(File file)throws IOException{return new File(context.getExternalFilesDir(null),"updates").getCanonicalFile().equals(file.getCanonicalFile().getParentFile());}
 private void resetCachedState(){
  if(info==null)info=new JSONObject();try{removeDownload();}catch(Exception ignored){}downloadId=-1;info=new JSONObject();put("state","idle");put("current",current);emit();
 }

 private void put(String k,Object v){try{info.put(k,v);}catch(JSONException e){throw new IllegalStateException(e);}}
 public synchronized String snapshot(){return info.toString();}
 public void attach(Listener l){listener=l;emit();}
 public void detach(Listener l){if(listener==l)listener=null;}
 private synchronized void emit(){prefs.edit().putString("state",info.toString()).putLong("downloadId",downloadId).apply();Listener l=listener;if(l!=null)l.changed(info.toString());}
 private synchronized void state(String value,String message){put("state",value);put("message",message);emit();}
 static int[] version(String value){if(value==null||!value.matches("v?\\d{1,9}\\.\\d{1,9}\\.\\d{1,9}"))throw new IllegalArgumentException("版本格式无效");String[] p=value.replaceFirst("^v","").split("\\.");return new int[]{Integer.parseInt(p[0]),Integer.parseInt(p[1]),Integer.parseInt(p[2])};}
 static int compare(String left,String right){int[] a=version(left),b=version(right);for(int i=0;i<3;i++){int n=Integer.compare(a[i],b[i]);if(n!=0)return n;}return 0;}
 static JSONObject release(JSONObject data,String current)throws Exception{
  if(data.optBoolean("draft")||data.optBoolean("prerelease"))throw new IOException("此发行版不是正式版");
  String tag=data.getString("tag_name");version(tag);String v=tag.replaceFirst("^v","");
  JSONObject result=new JSONObject().put("schema",2).put("source","github:shuizhaodemumu/materialbox").put("tag",tag).put("current",current).put("latest",v).put("state",compare(v,current)>0?"available":"current");
  JSONArray assets=data.getJSONArray("assets");JSONObject selected=null;String name="MaterialBox-"+v+"-arm64.apk";
  for(int i=0;i<assets.length();i++){JSONObject asset=assets.getJSONObject(i);if(name.equals(asset.optString("name"))&&"uploaded".equals(asset.optString("state"))){selected=asset;break;}}
  if(selected==null)throw new IOException("发行版尚未提供完整的 arm64 APK，请稍后重试");
  long size=selected.getLong("size");if(size<=0||size>1024L*1024*1024)throw new IOException("安装包大小无效");
  String url=selected.getString("browser_download_url");URI u=new URI(url);String expected="/shuizhaodemumu/materialbox/releases/download/"+tag+"/"+name;
  if(!"https".equals(u.getScheme())||!"github.com".equals(u.getHost())||u.getPort()!=-1||u.getUserInfo()!=null||u.getQuery()!=null||u.getFragment()!=null||!expected.equals(u.getRawPath()))throw new IOException("更新地址不是本项目的 GitHub 发行文件");
  String digest=selected.isNull("digest")?"":selected.optString("digest","");if(!digest.isEmpty()&&!digest.matches("sha256:[a-fA-F0-9]{64}"))throw new IOException("发行文件校验信息无效");
  String notes=data.optString("body","");if(notes.length()>6000)notes=notes.substring(0,6000)+"…";
  return result.put("name",name).put("url",url).put("total",size).put("notes",notes).put("digest",digest).put("downloaded",0).put("percent",0).put("message",compare(v,current)>0?"新版本已准备好，是否下载？":"你使用的已是当前最新版本");
 }
 public synchronized void check(){String s=info.optString("state");if(Arrays.asList("checking","downloading","verifying").contains(s)){emit();return;}if("ready".equals(s)){emit();return;}state("checking","正在连接 GitHub 检查正式发行版…");worker.execute(()->{
   HttpURLConnection connection=null;try{connection=(HttpURLConnection)new URL("https://api.github.com/repos/shuizhaodemumu/materialbox/releases/latest").openConnection();connection.setConnectTimeout(15000);connection.setReadTimeout(20000);connection.setInstanceFollowRedirects(false);connection.setRequestProperty("Accept","application/vnd.github+json");connection.setRequestProperty("User-Agent","MaterialBox/"+current);int code=connection.getResponseCode();if(code==403||code==429)throw new IOException("GitHub 请求暂时受限，请稍后重试");if(code!=200)throw new IOException(code==404?"项目暂时没有可用的正式发行版":"GitHub 返回错误（"+code+"）");ByteArrayOutputStream out=new ByteArrayOutputStream();try(InputStream in=connection.getInputStream()){byte[] bytes=new byte[8192];int n;while((n=in.read(bytes))!=-1){if(out.size()+n>1024*1024)throw new IOException("发行信息过大");out.write(bytes,0,n);}}
    JSONObject parsed=release(new JSONObject(out.toString("UTF-8")),current);synchronized(this){info=parsed;downloadId=-1;emit();}
   }catch(Exception e){state("error","检查更新失败："+readable(e));}finally{if(connection!=null)connection.disconnect();}
  });}
 public synchronized void download(){if(!"available".equals(info.optString("state")))return;try{
  File dir=new File(context.getExternalFilesDir(null),"updates");if(!dir.exists()&&!dir.mkdirs())throw new IOException("无法创建下载目录");
  File file=new File(dir,UUID.randomUUID()+".apk");put("file",file.getAbsolutePath());DownloadManager.Request request=new DownloadManager.Request(Uri.parse(info.getString("url"))).setTitle("材料魔盒 "+info.getString("latest")).setDescription("正在下载更新安装包").setMimeType("application/vnd.android.package-archive").setNotificationVisibility(DownloadManager.Request.VISIBILITY_VISIBLE).setDestinationUri(Uri.fromFile(file));
  downloadId=downloads.enqueue(request);put("downloaded",0);put("percent",0);state("downloading","正在下载，可切换到其他页面");startPolling();
 }catch(Exception e){state("error","无法开始下载："+readable(e));}}
 private synchronized void startPolling(){if(poll!=null&&!poll.isDone())return;poll=worker.scheduleWithFixedDelay(this::pollDownload,0,750,TimeUnit.MILLISECONDS);}
 private synchronized void stopPolling(){if(poll!=null){poll.cancel(false);poll=null;}}
 private synchronized void pollDownload(){if(downloadId<0||verifying)return;try(Cursor c=downloads.query(new DownloadManager.Query().setFilterById(downloadId))){if(c==null||!c.moveToFirst())throw new IOException("下载任务已被移除，请重新检查更新");int status=c.getInt(c.getColumnIndexOrThrow(DownloadManager.COLUMN_STATUS));long bytes=c.getLong(c.getColumnIndexOrThrow(DownloadManager.COLUMN_BYTES_DOWNLOADED_SO_FAR));long expected=info.getLong("total");if(bytes>expected){downloads.remove(downloadId);throw new IOException("下载文件大小与发行信息不符");}put("downloaded",Math.max(0,bytes));put("percent",Math.min(99,Math.max(0,bytes)*100/expected));
  if(status==DownloadManager.STATUS_FAILED){int reason=c.getInt(c.getColumnIndexOrThrow(DownloadManager.COLUMN_REASON));throw new IOException(reason==DownloadManager.ERROR_INSUFFICIENT_SPACE?"手机存储空间不足":"下载失败（"+reason+"），请检查网络后重试");}
  if(status==DownloadManager.STATUS_SUCCESSFUL){stopPolling();verifying=true;state("verifying","下载完成，正在核验文件和签名…");worker.execute(()->{try{validator.validate(new File(info.getString("file")),info);synchronized(this){put("downloaded",new File(info.getString("file")).length());put("percent",100);state("ready","安装包已校验，可以安装新版本");}}catch(Exception e){synchronized(this){removeDownload();state("error","安装包校验失败："+readable(e));}}finally{synchronized(this){verifying=false;}}});return;}
  put("message",status==DownloadManager.STATUS_PAUSED?"下载已暂停，等待网络或系统恢复":"正在下载，可切换到其他页面");emit();
 }catch(Exception e){stopPolling();removeDownload();state("error",readable(e));}}
 private synchronized void removeDownload(){if(downloadId>=0)downloads.remove(downloadId);downloadId=-1;String path=info.optString("file");if(!path.isEmpty())try{File file=new File(path);if(ownedUpdateFile(file))file.delete();}catch(IOException ignored){}info.remove("file");}
 public synchronized void cancel(){if(verifying){state("verifying","正在核验安装包，请稍候…");return;}if(installPending)return;boolean ready="ready".equals(info.optString("state"));if(!ready&&!"downloading".equals(info.optString("state")))return;stopPolling();removeDownload();state("cancelled",ready?"安装包已删除，可以重新检查更新":"下载已取消，可以重新检查更新");}
 static String readable(Exception e){return e.getMessage()==null?"网络不可用，请稍后重试":e.getMessage();}
 static android.content.pm.Signature[] signatures(PackageInfo p){return Build.VERSION.SDK_INT>=28?(p.signingInfo==null?null:p.signingInfo.getApkContentsSigners()):p.signatures;}
 static void verify(File file,JSONObject info,Context context)throws Exception{
  if(!file.isFile()||file.length()!=info.getLong("total"))throw new IOException("下载文件不完整");String digest=info.optString("digest","");if(!digest.isEmpty()){MessageDigest sha=MessageDigest.getInstance("SHA-256");try(InputStream in=new FileInputStream(file)){byte[] bytes=new byte[1024*1024];int n;while((n=in.read(bytes))!=-1)sha.update(bytes,0,n);}StringBuilder h=new StringBuilder();for(byte b:sha.digest())h.append(String.format(Locale.ROOT,"%02x",b&255));if(!digest.substring(7).equalsIgnoreCase(h.toString()))throw new IOException("文件摘要不匹配，请重新下载");}
  PackageManager pm=context.getPackageManager();int flags=Build.VERSION.SDK_INT>=28?PackageManager.GET_SIGNING_CERTIFICATES:PackageManager.GET_SIGNATURES;
  PackageInfo incoming=pm.getPackageArchiveInfo(file.getAbsolutePath(),flags),installed=pm.getPackageInfo(context.getPackageName(),flags);
  verifyPackage(incoming,installed,info);
 }
 static void verifyPackage(PackageInfo incoming,PackageInfo installed,JSONObject info)throws Exception{
  if(incoming==null||!installed.packageName.equals(incoming.packageName))throw new IOException("APK 不是材料魔盒安装包");
  if(!info.getString("latest").equals(incoming.versionName)||compare(incoming.versionName,installed.versionName)<=0||(Build.VERSION.SDK_INT>=28?incoming.getLongVersionCode()<=installed.getLongVersionCode():incoming.versionCode<=installed.versionCode))throw new IOException("APK 版本与新版本信息不符");
  android.content.pm.Signature[] a=signatures(installed),b=signatures(incoming);if(a==null||b==null||a.length!=b.length)throw new IOException("安装包签名不匹配");Set<android.content.pm.Signature> actual=new HashSet<>(Arrays.asList(a));if(!actual.equals(new HashSet<>(Arrays.asList(b))))throw new IOException("安装包签名不匹配，已阻止安装");
 }
 public synchronized void install(Activity activity){if(installPending||!"ready".equals(info.optString("state")))return;installPending=true;worker.execute(()->{try{JSONObject copy; synchronized(this){if(!"ready".equals(info.optString("state")))throw new IOException("请先完成安装包下载");copy=new JSONObject(info.toString());}verify(new File(copy.getString("file")),copy,context);activity.runOnUiThread(()->{try{if(!context.getPackageManager().canRequestPackageInstalls()){activity.startActivity(new Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES,Uri.parse("package:"+context.getPackageName())));state("ready","请在系统设置允许安装应用，然后返回点击“立即安装”");return;}Intent i=new Intent(Intent.ACTION_VIEW).setDataAndType(Uri.parse("content://"+context.getPackageName()+".updates/apk"),"application/vnd.android.package-archive").addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);i.setClipData(ClipData.newRawUri("材料魔盒更新",i.getData()));activity.startActivity(i);}catch(Exception e){state("ready","无法打开安装器："+readable(e));}finally{synchronized(this){installPending=false;}}});}catch(Exception e){synchronized(this){installPending=false;}state("error",readable(e));}});}
 synchronized File readyFile()throws Exception{if(!"ready".equals(info.optString("state")))throw new IOException("安装包尚未就绪");File f=new File(info.getString("file")).getCanonicalFile();File root=new File(context.getExternalFilesDir(null),"updates").getCanonicalFile();if(!root.equals(f.getParentFile())||!f.isFile())throw new IOException("安装包不存在");return f;}
}
