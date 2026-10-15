package cn.materialbox.app;
import android.content.*;import android.database.*;import android.net.Uri;import android.os.*;import java.io.*;
public final class UpdateProvider extends ContentProvider {
 public boolean onCreate(){return true;}
 private File file(Uri u)throws FileNotFoundException{try{if(!"/apk".equals(u.getPath())||u.getQuery()!=null)throw new IOException();return AppUpdates.get(getContext()).readyFile();}catch(Exception e){throw new FileNotFoundException("没有已校验的更新安装包");}}
 public String getType(Uri u){return "application/vnd.android.package-archive";}
 public Cursor query(Uri u,String[] projection,String selection,String[] args,String order){try{File f=file(u);MatrixCursor c=new MatrixCursor(new String[]{"_display_name","_size"});c.addRow(new Object[]{"材料魔盒更新.apk",f.length()});return c;}catch(Exception e){return null;}}
 public ParcelFileDescriptor openFile(Uri u,String mode)throws FileNotFoundException{if(!"r".equals(mode))throw new FileNotFoundException("只读安装包");return ParcelFileDescriptor.open(file(u),ParcelFileDescriptor.MODE_READ_ONLY);}
 public Uri insert(Uri u,ContentValues v){throw new UnsupportedOperationException();}public int delete(Uri u,String s,String[] a){throw new UnsupportedOperationException();}public int update(Uri u,ContentValues v,String s,String[] a){throw new UnsupportedOperationException();}
}
