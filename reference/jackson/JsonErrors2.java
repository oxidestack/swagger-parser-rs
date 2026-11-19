import com.fasterxml.jackson.databind.*;
public class JsonErrors2 {
  public static void main(String[] a) throws Exception {
    ObjectMapper m = io.swagger.v3.parser.ObjectMapperFactory.createJson();
    StringBuilder deep = new StringBuilder(); for (int i=0;i<1001;i++) deep.append('['); 
    String[] ins = {deep.toString(), "{\"a\":[1}", "[1]]", "]", "{\"a\":\"abc", "{\"a\"", "{\"a\":", "\u0001", "{\"a\":1 \"b\":2}", "[1 2]", "{\"a\\u0041\":1}", "{\"a\":+1}", "{\"a\":.5}", "1x", "{\"a\":\"\\uD83D\\uDE00é\"}", "{\"é\":x}", "{\"a\":1}\n]", "{\"a\":[\n1,\n2,\n]\n}", "{\"a\":truex}","{\"a\":nul}", "[\"a\"\t,\"b\"]", "﻿{}" };
    for (String s : ins) {
      try { JsonNode n = m.readTree(s); System.out.println("OK " + (n==null?"null":n.getClass().getSimpleName()+" "+n)); }
      catch (Exception e) { System.out.println(e.getClass().getName() + ": " + e.getMessage().replace("\n","\\n")); }
    }
  }
}
