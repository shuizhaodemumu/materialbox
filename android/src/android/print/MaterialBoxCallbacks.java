package android.print;
/** Callback construction adapter for the Android print framework's package-scoped constructors. */
public final class MaterialBoxCallbacks {
 public interface LayoutHandler{void finished(PrintDocumentInfo info,boolean changed);void failed(CharSequence error);}
 public interface WriteHandler{void finished(PageRange[] pages);void failed(CharSequence error);void cancelled();}
 public static PrintDocumentAdapter.LayoutResultCallback layout(final LayoutHandler h){return new PrintDocumentAdapter.LayoutResultCallback(){public void onLayoutFinished(PrintDocumentInfo i,boolean c){h.finished(i,c);}public void onLayoutFailed(CharSequence e){h.failed(e);}};}
 public static PrintDocumentAdapter.WriteResultCallback write(final WriteHandler h){return new PrintDocumentAdapter.WriteResultCallback(){public void onWriteFinished(PageRange[] p){h.finished(p);}public void onWriteFailed(CharSequence e){h.failed(e);}public void onWriteCancelled(){h.cancelled();}};}
}
