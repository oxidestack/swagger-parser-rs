import com.fasterxml.jackson.databind.*;
import com.fasterxml.jackson.core.*;
public class JsonErrors {
  public static void main(String[] a) throws Exception {
    ObjectMapper m = io.swagger.v3.parser.ObjectMapperFactory.createJson();
    String[] ins = {"{", "{\"a\":1,}", "{\"a\" 1}", "{\"a\":1,\"a\":2}", "{a:1}", "[1,2", "{\"a\":01}", "{\"a\":tru}", "{\"a\":\"x\ny\"}", "{\"a\":1}}", "{\"a\":NaN}", "{\"a\":-}", "{\"a\":1.}", "{\"a\":\"\\q\"}", "", "  ", "{\"a\":1} x", "{\"a\":[1,]}", "{\n  \"a\": {\n    \"b\": 1\n  \n", "{\"a\":1e}", "{\"a\":\"\\u12\"}", "{/*c*/}", "{\"a\":1}[" };
    for (String s : ins) {
      try { JsonNode n = m.readTree(s); System.out.println("OK " + (n==null?"null":n.getClass().getSimpleName()+" "+n)); }
      catch (Exception e) { System.out.println(e.getClass().getName() + ": " + e.getMessage().replace("\n","\\n")); }
    }
    for (String s : new String[]{"1", "2147483648", "9223372036854775808", "1.5", "1e3", "-0", "-0.0", "1.0E400", "123456789012345678901234567890.5", "0.1"}) {
      JsonNode n = m.readTree(s); System.out.println(s + " -> " + n.getClass().getSimpleName() + " " + n + " asText=" + n.asText());
    }
  }
}
