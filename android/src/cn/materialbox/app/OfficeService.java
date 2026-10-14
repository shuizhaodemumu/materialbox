package cn.materialbox.app;
import android.app.Service;import android.content.*;import android.os.*;import android.net.Uri;import android.content.res.AssetManager;import java.io.*;import java.util.concurrent.*;import org.json.*;import org.libreoffice.kit.*;

/** Serial reusable Office engine in its own process, separate from media tasks. */
public class OfficeService extends Service {
 private static final ExecutorService worker=Executors.newSingleThreadExecutor();
 private static Office office;
 protected String engineFlavor(){return "general";}
 private final Messenger messenger=new Messenger(new Handler(Looper.getMainLooper()){public void handleMessage(Message m){final Messenger reply=m.replyTo;final String raw=m.getData().getString("job");worker.submit(()->convert(raw,reply));}});
 private void copyAssets(String source,File destination)throws IOException{
  String[] children=getAssets().list(source);if(children!=null&&children.length>0){destination.mkdirs();for(String child:children)copyAssets(source+"/"+child,new File(destination,child));}
  else{destination.getParentFile().mkdirs();try(InputStream in=getAssets().open(source);OutputStream out=new FileOutputStream(destination)){byte[] b=new byte[65536];int n;while((n=in.read(b))!=-1)out.write(b,0,n);}}
 }
 private void convert(String raw,Messenger reply){Message response=Message.obtain(null,1);Bundle b=new Bundle();File normalized=null;
  try{
   JSONObject j=new JSONObject(raw);String input=j.getString("input"),output=j.getString("output"),format=j.getString("format");
   if((input.toLowerCase(java.util.Locale.ROOT).endsWith(".html")||input.toLowerCase(java.util.Locale.ROOT).endsWith(".htm"))&&format.equals("pdf")){PdfLocal.html(this,j);b.putBoolean("ok",true);b.putLong("bytes",new File(output).length());response.setData(b);reply.send(response);stopSelf();return;}
   if(!java.util.Arrays.asList("pdf","docx","pptx","xlsx").contains(format))throw new IOException("不支持的文档输出格式");
   File data=new File(getApplicationInfo().dataDir,"office-home/"+engineFlavor());
   if(office==null){copyAssets("unpack",data);
   File registry=new File(data,"office-registry");copyAssets("share/registry",registry);
   File pdfImport=new File(registry,"pdfimport.xcd");
   if(input.toLowerCase(java.util.Locale.ROOT).endsWith(".pdf")&&(format.equals("docx")||format.equals("pptx"))){
    String preferred=format.equals("docx")?"writer_pdf_import":"impress_pdf_import";
    byte[] bytes=new byte[(int)pdfImport.length()];try(InputStream in=new FileInputStream(pdfImport)){int n=0,r;while(n<bytes.length&&(r=in.read(bytes,n,bytes.length-n))>0)n+=r;}
    String xml=new String(bytes,"UTF-8").replace("<prop oor:name=\"PreferredFilter\"><value>draw_pdf_import</value></prop>","<prop oor:name=\"PreferredFilter\"><value>"+preferred+"</value></prop>");
    xml=xml.replace("3RDPARTYFILTER ALIEN IMPORT PREFERRED SUPPORTSSIGNING","3RDPARTYFILTER ALIEN IMPORT SUPPORTSSIGNING");
    java.util.regex.Matcher matcher=java.util.regex.Pattern.compile("(<node oor:name=\""+preferred+"\"[^>]*>.*?<prop oor:name=\"Flags\"[^>]*><value>)([^<]*)(</value>)",java.util.regex.Pattern.DOTALL).matcher(xml);
    if(matcher.find())xml=matcher.replaceFirst(java.util.regex.Matcher.quoteReplacement(matcher.group(1)+matcher.group(2)+" PREFERRED"+matcher.group(3)));
    // PDFDetector in this upstream build hard-codes Draw and overrides PreferredFilter.
    // For a known PDF conversion input, use the selected registered import filter directly.
    xml=xml.replaceAll("(<prop oor:name=\"DetectService\"[^>]*>)<value>[^<]*</value>","$1<value/>");
    try(OutputStream out=new FileOutputStream(pdfImport)){out.write(xml.getBytes("UTF-8"));}
   }
   LibreOfficeKit.init(this,data.getAbsolutePath());office=new Office(LibreOfficeKit.getLibreOfficeKitHandle());}
   java.nio.ByteBuffer documentHandle=Core.loadOffice(LibreOfficeKit.getLibreOfficeKitHandle(),Uri.fromFile(new File(input)).toString(),"Batch=true,MacroSecurityLevel=3,EnableMacrosExecution=false");
   if(documentHandle==null){String ext=input.substring(input.lastIndexOf('.')+1).toLowerCase(java.util.Locale.ROOT);if(java.util.Arrays.asList("pptx","docx","xlsx").contains(ext)){
    normalized=new File(getCacheDir(),"office-compat-"+java.util.UUID.randomUUID()+"."+ext);
    JSONObject fixed=new JSONObject(Core.call("convert",new JSONObject().put("mode","office-package").put("input",input).put("output",normalized.getAbsolutePath()).put("format",ext).toString()));
    if(!fixed.optBoolean("ok"))throw new IOException("文档载入失败，兼容性整理未完成："+fixed.optString("error"));
    documentHandle=Core.loadOffice(LibreOfficeKit.getLibreOfficeKitHandle(),Uri.fromFile(normalized).toString(),"Batch=true,MacroSecurityLevel=3,EnableMacrosExecution=false");
   }}
   Document doc=documentHandle==null?null:new Document(documentHandle);if(doc==null)throw new IOException("文档无法载入："+office.getError());
   try{
    int type=doc.getDocumentType();if(format.equals("docx")&&type!=Document.DOCTYPE_TEXT)throw new IOException("PDF 未被正确载入为可编辑文字文档");
    if(format.equals("pptx")&&type!=Document.DOCTYPE_PRESENTATION)throw new IOException("PDF 未被正确载入为可编辑演示文稿");
    if(format.equals("xlsx")&&type!=Document.DOCTYPE_SPREADSHEET)throw new IOException("PDF 表格重建尚未得到验证，不能用截图代替单元格");
    File dst=new File(output);dst.getParentFile().mkdirs();if(dst.exists())throw new IOException("输出文件已存在");
    doc.saveAs(Uri.fromFile(dst).toString(),format,j.optString("options",""));
    if(!dst.isFile()||dst.length()==0)throw new IOException("排版引擎未生成结果："+office.getError());
    b.putBoolean("ok",true);b.putLong("bytes",dst.length());
   }finally{doc.destroy();}
  }catch(Throwable e){b.putBoolean("ok",false);b.putString("error",e.getMessage()==null?e.toString():e.getMessage());}
  if(normalized!=null)normalized.delete();
  response.setData(b);try{reply.send(response);}catch(RemoteException ignored){}
  stopSelf();
 }
 public IBinder onBind(Intent i){return messenger.getBinder();}
}
