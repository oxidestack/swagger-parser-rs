import com.fasterxml.jackson.databind.*;
import java.nio.file.*;
import java.util.*;
// Reads cases (one JSON-escaped input per line) and prints: strict-mapper result, then pretty-printed tree.
public class JsonCases {
  public static void main(String[] a) throws Exception {
    ObjectMapper strict = io.swagger.v3.parser.ObjectMapperFactory.createJson();
    ObjectMapper plain = new ObjectMapper();
    for (String line : Files.readAllLines(Paths.get(a[0]))) {
      String s = plain.readValue(line, String.class);
      String r;
      try { JsonNode n = strict.readTree(s); r = "OK " + n.getClass().getSimpleName() + " " + n.toString() + " PRETTY " + io.swagger.v3.core.util.Json.pretty(n); }
      catch (Exception e) { r = "ERR " + e.getClass().getName() + ": " + e.getMessage(); }
      System.out.println(plain.writeValueAsString(r));
    }
  }
}
