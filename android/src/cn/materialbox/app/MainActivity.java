package cn.materialbox.app;
import android.app.*;
import android.content.*;
import android.database.Cursor;
import android.net.Uri;
import android.os.*;
import android.provider.OpenableColumns;
import android.webkit.*;
import android.view.*;
import android.widget.*;
import org.json.*;
import java.io.*;
import java.util.*;
import java.util.concurrent.*;

public class MainActivity extends Activity {
 private WebView web;private String exportId="";
 private final ExecutorService io=Executors.newSingleThreadExecutor();
 @Override public void onCreate(Bundle saved){super.onCreate(saved);
  Store.load(this);web=new WebView(this);web.setBackgroundColor(0xfff8f5fe);
  if((getApplicationInfo().flags&android.content.pm.ApplicationInfo.FLAG_DEBUGGABLE)!=0)WebView.setWebContentsDebuggingEnabled(true);
  web.getSettings().setJavaScriptEnabled(true);web.getSettings().setDomStorageEnabled(true);
  web.getSettings().setAllowFileAccess(true);web.getSettings().setAllowContentAccess(false);
  web.getSettings().setAllowFileAccessFromFileURLs(false);web.getSettings().setAllowUniversalAccessFromFileURLs(false);web.getSettings().setMixedContentMode(WebSettings.MIXED_CONTENT_NEVER_ALLOW);
  web.addJavascriptInterface(new Bridge(),"Native");
  web.setWebViewClient(new WebViewClient(){@Override public boolean shouldOverrideUrlLoading(WebView v,WebResourceRequest r){return !r.getUrl().toString().equals("file:///android_asset/index.html");}@Override public WebResourceResponse shouldInterceptRequest(WebView v,WebResourceRequest r){if(!r.getUrl().toString().startsWith("file:///android_asset/"))return new WebResourceResponse("text/plain","UTF-8",new ByteArrayInputStream(new byte[0]));return null;}});
  web.setWebChromeClient(new WebChromeClient());setContentView(web);
  web.setOnApplyWindowInsetsListener((v,insets)->{v.setPadding(insets.getSystemWindowInsetLeft(),insets.getSystemWindowInsetTop(),insets.getSystemWindowInsetRight(),insets.getSystemWindowInsetBottom());return insets.consumeSystemWindowInsets();});web.loadUrl("file:///android_asset/index.html");
 }
 private void event(String name,String data){runOnUiThread(()->{if(!isFinishing())web.evaluateJavascript("window.nativeEvent("+JSONObject.quote(name)+","+data+")",null);});}
 private void fail(String msg){event("error",JSONObject.quote(msg));}
 public class Bridge {
  @JavascriptInterface public void openExternal(String raw){try{Uri uri=Uri.parse(raw);if(!"https".equals(uri.getScheme())||!"github.com".equals(uri.getHost())||uri.getUserInfo()!=null)throw new IllegalArgumentException("链接无效");runOnUiThread(()->{try{Intent browser=new Intent(Intent.ACTION_VIEW,uri);browser.addCategory(Intent.CATEGORY_BROWSABLE);startActivity(browser);}catch(Exception e){fail("没有可用的浏览器");}});}catch(Exception e){fail("链接无效");}}

  @JavascriptInterface public void pick(String mime,boolean multi){runOnUiThread(()->{Intent i=new Intent(Intent.ACTION_OPEN_DOCUMENT);i.setType(mime);i.addCategory(Intent.CATEGORY_OPENABLE);i.putExtra(Intent.EXTRA_ALLOW_MULTIPLE,multi);startActivityForResult(i,10);});}
  @JavascriptInterface public String tasks(){return Store.snapshot(MainActivity.this);}
  @JavascriptInterface public String tool(String input){return Core.call("tool",input);}
  @JavascriptInterface public String inspect(String path){try{String root=new File(getFilesDir(),"inputs").getCanonicalPath()+File.separator;if(!new File(path).getCanonicalPath().startsWith(root))return "{\"ok\":false,\"error\":\"输入路径无效\"}";return Core.call("detect",path);}catch(Exception e){return "{\"ok\":false,\"error\":\"读取失败\"}";}}
  @JavascriptInterface public String submit(String payload){try{JSONObject j=new JSONObject(payload);String id=Store.enqueue(MainActivity.this,j);runOnUiThread(()->startWorker());return new JSONObject().put("ok",true).put("id",id).toString();}catch(Exception e){return "{\"ok\":false,\"error\":"+JSONObject.quote(e.getMessage())+"}";}}
  @JavascriptInterface public void cancel(String id){ConversionService.cancel(MainActivity.this,id);}
  @JavascriptInterface public String retry(String id){try{synchronized(Store.class){JSONObject t=Store.find(id);if(t==null)throw new IOException("任务不存在");if(!Arrays.asList("failed","cancelled","interrupted").contains(t.optString("status")))throw new IOException("当前任务无需重试");new File(t.getJSONObject("request").getString("output")).delete();t.remove("elapsed");Store.update(MainActivity.this,t,"queued","等待重试");}runOnUiThread(()->startWorker());return "{\"ok\":true}";}catch(Exception e){return "{\"ok\":false,\"error\":"+JSONObject.quote(e.getMessage())+"}";}}
  @JavascriptInterface public void clear(){Store.clearCompleted(MainActivity.this);}
  @JavascriptInterface public void export(String id){runOnUiThread(()->{try{JSONObject t=Store.find(id);if(t==null||!"done".equals(t.optString("status")))throw new IOException("文件尚未完成");exportId=id;Intent i=new Intent(Intent.ACTION_CREATE_DOCUMENT);i.addCategory(Intent.CATEGORY_OPENABLE);i.setType("application/octet-stream");String name=t.optString("name","材料").replaceAll("\\.[^.]+$","");i.putExtra(Intent.EXTRA_TITLE,name+"_魔盒."+t.optString("format"));startActivityForResult(i,11);}catch(Exception e){fail(e.getMessage());}});}
  @JavascriptInterface public void share(String id){runOnUiThread(()->{try{JSONObject t=Store.find(id);if(t==null||!"done".equals(t.optString("status")))throw new IOException("文件尚未完成");File f=new File(t.getJSONObject("request").getString("output"));Uri uri=Uri.parse("content://cn.materialbox.app.outputs/"+f.getName());Intent share=new Intent(Intent.ACTION_SEND);share.setType(OutputProvider.mime(f.getName()));share.putExtra(Intent.EXTRA_STREAM,uri);share.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);share.setClipData(ClipData.newRawUri("材料魔盒",uri));startActivity(Intent.createChooser(share,"分享转换结果"));}catch(Exception e){fail(e.getMessage());}});}
  @JavascriptInterface public void copy(String text){runOnUiThread(()->{android.content.ClipboardManager cm=(android.content.ClipboardManager)getSystemService(CLIPBOARD_SERVICE);cm.setPrimaryClip(ClipData.newPlainText("材料魔盒",text));Toast.makeText(MainActivity.this,"已复制",Toast.LENGTH_SHORT).show();});}
 }
 private void startWorker(){try{if(Build.VERSION.SDK_INT>=33&&checkSelfPermission("android.permission.POST_NOTIFICATIONS")!=android.content.pm.PackageManager.PERMISSION_GRANTED&&!getSharedPreferences("settings",0).getBoolean("notificationAsked",false)){getSharedPreferences("settings",0).edit().putBoolean("notificationAsked",true).commit();requestPermissions(new String[]{"android.permission.POST_NOTIFICATIONS"},20);}startForegroundService(new Intent(this,ConversionService.class));}catch(Exception e){fail("无法启动后台转换："+e.getMessage());}}
 @Override protected void onActivityResult(int req,int result,Intent data){super.onActivityResult(req,result,data);if(result!=RESULT_OK||data==null){if(req==10)event("importDone","[]");return;}
  if(req==10){List<Uri> uris=new ArrayList<>();if(data.getClipData()!=null){for(int i=0;i<Math.min(50,data.getClipData().getItemCount());i++)uris.add(data.getClipData().getItemAt(i).getUri());}else if(data.getData()!=null)uris.add(data.getData());event("importStart","null");io.submit(()->{JSONArray list=new JSONArray();try{for(Uri uri:uris){String name="文件";long length=-1;try(Cursor c=getContentResolver().query(uri,null,null,null,null)){if(c!=null&&c.moveToFirst()){int n=c.getColumnIndex(OpenableColumns.DISPLAY_NAME),s=c.getColumnIndex(OpenableColumns.SIZE);if(n>=0)name=c.getString(n);if(s>=0&&!c.isNull(s))length=c.getLong(s);}}if(length>2L*1024*1024*1024)throw new IOException("单个文件最多 2 GB");String extension=name.contains(".")?name.substring(name.lastIndexOf('.')).toLowerCase(Locale.ROOT):"";if(!extension.matches("\\.[a-z0-9]{1,12}"))extension=".bin";File dir=new File(getFilesDir(),"inputs");dir.mkdirs();File dst=new File(dir,UUID.randomUUID()+extension);try(InputStream in=getContentResolver().openInputStream(uri);OutputStream out=new FileOutputStream(dst)){copy(in,out,2L*1024*1024*1024);}catch(Exception e){dst.delete();throw e;}list.put(new JSONObject().put("path",dst.getAbsolutePath()).put("name",name).put("size",dst.length()));}event("importDone",list.toString());}catch(Exception e){fail("导入失败："+e.getMessage());event("importDone",list.toString());}});
  }else if(req==11){final String id=exportId;final Uri uri=data.getData();io.submit(()->{try{JSONObject t=Store.find(id);if(t==null)throw new IOException("任务记录已移除");File src=new File(t.getJSONObject("request").getString("output"));try(InputStream in=new FileInputStream(src);OutputStream out=getContentResolver().openOutputStream(uri,"wt")){copy(in,out,Long.MAX_VALUE);}event("saved","null");}catch(Exception e){fail("保存失败："+e.getMessage());}});}
 }
 private static void copy(InputStream in,OutputStream out,long limit)throws IOException{if(in==null||out==null)throw new IOException("无法打开文件");byte[] b=new byte[65536];long total=0;int n;while((n=in.read(b))!=-1){total+=n;if(total>limit)throw new IOException("文件超过大小限制");out.write(b,0,n);}out.flush();}
 @Override public void onBackPressed(){web.evaluateJavascript("window.goBack()",handled->{if("false".equals(handled))finish();});}
 @Override protected void onDestroy(){io.shutdown();web.removeJavascriptInterface("Native");web.destroy();super.onDestroy();}
}
