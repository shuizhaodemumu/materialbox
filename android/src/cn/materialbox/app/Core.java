package cn.materialbox.app;
public final class Core {
 static { System.loadLibrary("materialbox"); }
 public static native String call(String operation,String input);
 public static native java.nio.ByteBuffer loadOffice(java.nio.ByteBuffer handle,String url,String options);
 private Core() {}
}
