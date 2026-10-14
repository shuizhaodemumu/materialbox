package cn.materialbox.app;
import android.content.Context;import android.graphics.*;import android.graphics.pdf.PdfRenderer;import android.os.*;import android.net.Uri;import android.webkit.*;import android.print.*;import org.json.*;import java.io.*;import java.util.concurrent.*;import java.util.zip.*;
public final class PdfLocal {
 public static void html(Context c,JSONObject j)throws Exception{
  File source=new File(j.getString("input")),target=new File(j.getString("output"));target.getParentFile().mkdirs();if(target.exists())throw new IOException("输出文件已存在");
  if(source.length()>16*1024*1024)throw new IOException("HTML 文件超过 16 MB 限制");
  CountDownLatch latch=new CountDownLatch(1);Throwable[] error=new Throwable[1];
  new Handler(Looper.getMainLooper()).post(()->{
   WebView web=new WebView(c);web.getSettings().setJavaScriptEnabled(false);web.getSettings().setAllowFileAccess(true);web.getSettings().setAllowUniversalAccessFromFileURLs(false);web.getSettings().setAllowFileAccessFromFileURLs(false);
   web.setWebViewClient(new WebViewClient(){boolean started=false;public WebResourceResponse shouldInterceptRequest(WebView v,WebResourceRequest r){String uri=r.getUrl().toString();if(uri.startsWith("http:")||uri.startsWith("https:"))return new WebResourceResponse("text/plain","UTF-8",new ByteArrayInputStream(new byte[0]));return null;}
    public void onPageFinished(WebView v,String url){if(started)return;started=true;try{
     PrintDocumentAdapter adapter=web.createPrintDocumentAdapter("材料魔盒");
     PrintAttributes attrs=new PrintAttributes.Builder().setMediaSize(PrintAttributes.MediaSize.ISO_A4).setResolution(new PrintAttributes.Resolution("pdf","PDF",300,300)).setMinMargins(PrintAttributes.Margins.NO_MARGINS).build();
     adapter.onStart();adapter.onLayout(null,attrs,new CancellationSignal(),android.print.MaterialBoxCallbacks.layout(new android.print.MaterialBoxCallbacks.LayoutHandler(){
      public void failed(CharSequence msg){error[0]=new IOException("HTML 排版失败："+msg);web.destroy();latch.countDown();}
      public void finished(PrintDocumentInfo info,boolean changed){try{
       ParcelFileDescriptor fd=ParcelFileDescriptor.open(target,ParcelFileDescriptor.MODE_CREATE|ParcelFileDescriptor.MODE_TRUNCATE|ParcelFileDescriptor.MODE_READ_WRITE);
       adapter.onWrite(new PageRange[]{PageRange.ALL_PAGES},fd,new CancellationSignal(),android.print.MaterialBoxCallbacks.write(new android.print.MaterialBoxCallbacks.WriteHandler(){
        private void finish(){try{fd.close();}catch(Exception ignored){}adapter.onFinish();web.destroy();latch.countDown();}
        public void finished(PageRange[] pages){finish();}
        public void failed(CharSequence msg){error[0]=new IOException("HTML PDF 写入失败："+msg);finish();}
        public void cancelled(){error[0]=new IOException("HTML PDF 写入取消");finish();}
       }));
      }catch(Throwable e){error[0]=e;web.destroy();latch.countDown();}}
     }),new Bundle());
    }catch(Throwable e){error[0]=e;web.destroy();latch.countDown();}}
   });web.loadUrl(Uri.fromFile(source).toString());
  });
  if(!latch.await(90,TimeUnit.SECONDS))throw new IOException("HTML 排版超时");if(error[0]!=null)throw new IOException(error[0]);if(target.length()==0)throw new IOException("HTML 未生成 PDF");
 }
 public static void images(JSONObject j)throws Exception{
  String imageFormat=j.optString("action","jpg");if(!imageFormat.equals("jpg")&&!imageFormat.equals("png"))throw new IOException("页面图片格式无效");
  int dpi=j.optInt("width",144);if(dpi<72||dpi>300)throw new IOException("分辨率范围为 72 到 300 DPI");
  File output=new File(j.getString("output"));output.getParentFile().mkdirs();if(output.exists())throw new IOException("输出文件已存在");
  try(PdfRenderer pdf=new PdfRenderer(ParcelFileDescriptor.open(new File(j.getString("input")),ParcelFileDescriptor.MODE_READ_ONLY));ZipOutputStream zip=new ZipOutputStream(new FileOutputStream(output))){
   if(pdf.getPageCount()>200)throw new IOException("一次最多渲染 200 页");
   for(int i=0;i<pdf.getPageCount();i++){try(PdfRenderer.Page page=pdf.openPage(i)){
    int width=(int)Math.ceil(page.getWidth()*dpi/72.0),height=(int)Math.ceil(page.getHeight()*dpi/72.0);if((long)width*height>24_000_000)throw new IOException("页面过大，请降低 DPI");
    Bitmap bitmap=Bitmap.createBitmap(width,height,Bitmap.Config.ARGB_8888);bitmap.eraseColor(Color.WHITE);page.render(bitmap,null,null,PdfRenderer.Page.RENDER_MODE_FOR_PRINT);
    zip.putNextEntry(new ZipEntry(String.format(java.util.Locale.ROOT,"page-%04d.%s",i+1,imageFormat)));boolean ok=bitmap.compress(imageFormat.equals("png")?Bitmap.CompressFormat.PNG:Bitmap.CompressFormat.JPEG,95,zip);bitmap.recycle();zip.closeEntry();if(!ok)throw new IOException("页面图片编码失败");
   }}
  }
 }
}
