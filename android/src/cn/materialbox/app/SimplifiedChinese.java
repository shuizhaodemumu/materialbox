package cn.materialbox.app;

import android.content.Context;
import java.io.*;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.util.*;

/** Offline longest-match converter using unmodified Apache-2.0 OpenCC dictionaries. */
public final class SimplifiedChinese {
 private static final class Node {final Map<Character,Node> children=new HashMap<>();String value;}
 private static Node dictionary;
 private static synchronized Node dictionary(Context c)throws IOException {
  if(dictionary!=null)return dictionary;
  Node root=new Node();
  // Phrase exceptions are loaded after individual characters, and win on equal keys.
  for(String name:new String[]{"TSCharacters.txt","TSPhrases.txt"}){
   try(BufferedReader reader=new BufferedReader(new InputStreamReader(c.getAssets().open("opencc/"+name),StandardCharsets.UTF_8))){
    String line;while((line=reader.readLine())!=null){if(line.isEmpty()||line.startsWith("#"))continue;String[] pair=line.split("\t",2);if(pair.length!=2)throw new IOException("简体词典格式错误");String value=pair[1].split(" ",2)[0];Node node=root;for(int i=0;i<pair[0].length();i++){char ch=pair[0].charAt(i);Node next=node.children.get(ch);if(next==null){next=new Node();node.children.put(ch,next);}node=next;}node.value=value;}
   }
  }
  dictionary=root;return root;
 }
 public static String convert(Context c,String input)throws IOException {
  Node root=dictionary(c);StringBuilder out=new StringBuilder(input.length());
  for(int i=0;i<input.length();){Node node=root;String match=null;int end=i;
   for(int j=i;j<input.length();j++){node=node.children.get(input.charAt(j));if(node==null)break;if(node.value!=null){match=node.value;end=j+1;}}
   if(match==null){out.append(input.charAt(i));i++;}else{out.append(match);i=end;}
  }return out.toString();
 }
 public static synchronized void file(Context c,File file)throws IOException {
  if(file.length()>2*1024*1024)throw new IOException("识别文字过长，请分段处理");
  byte[] bytes=Files.readAllBytes(file.toPath());String input=new String(bytes,StandardCharsets.UTF_8),result=convert(c,input);if(input.equals(result))return;
  File temp=new File(file.getParentFile(),file.getName()+".simplified-"+UUID.randomUUID());
  try{Files.write(temp.toPath(),result.getBytes(StandardCharsets.UTF_8));Files.move(temp.toPath(),file.toPath(),StandardCopyOption.ATOMIC_MOVE,StandardCopyOption.REPLACE_EXISTING);}finally{temp.delete();}
 }
 private SimplifiedChinese(){}
}
