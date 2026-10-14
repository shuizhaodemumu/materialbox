package cn.materialbox.app;
import android.content.*;import android.database.*;import android.net.Uri;import android.os.ParcelFileDescriptor;import android.webkit.MimeTypeMap;import java.io.*;
public class OutputProvider extends ContentProvider {
 public boolean onCreate(){return true;}
 private File file(Uri u)throws FileNotFoundException{try{String name=u.getLastPathSegment();if(name==null||!name.matches("[a-f0-9-]+\\.[a-z0-9]+"))throw new IOException();File root=new File(getContext().getFilesDir(),"outputs").getCanonicalFile();File f=new File(root,name).getCanonicalFile();if(!f.getParentFile().equals(root)||!f.isFile())throw new IOException();return f;}catch(Exception e){throw new FileNotFoundException("结果文件不存在");}}
 public static String mime(String name){String ext=name.substring(name.lastIndexOf('.')+1);String m=MimeTypeMap.getSingleton().getMimeTypeFromExtension(ext);return m==null?"application/octet-stream":m;}
 public String getType(Uri u){return mime(u.getLastPathSegment());}
 public Cursor query(Uri u,String[] projection,String selection,String[] args,String sort){try{File f=file(u);MatrixCursor c=new MatrixCursor(new String[]{"_display_name","_size"});c.addRow(new Object[]{"材料魔盒."+f.getName().substring(f.getName().lastIndexOf('.')+1),f.length()});return c;}catch(Exception e){return null;}}
 public ParcelFileDescriptor openFile(Uri u,String mode)throws FileNotFoundException{if(!"r".equals(mode))throw new FileNotFoundException("只读结果");return ParcelFileDescriptor.open(file(u),ParcelFileDescriptor.MODE_READ_ONLY);}
 public Uri insert(Uri u,ContentValues v){throw new UnsupportedOperationException();}public int delete(Uri u,String s,String[] a){throw new UnsupportedOperationException();}public int update(Uri u,ContentValues v,String s,String[] a){throw new UnsupportedOperationException();}
}
