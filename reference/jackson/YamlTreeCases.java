import com.fasterxml.jackson.databind.*;
import java.nio.file.*;
// For each JSON-string-encoded YAML input: Yaml.mapper().readTree and swagger-parser's strict createYaml().readTree.
public class YamlTreeCases {
  static String cls(JsonNode n) { return n == null ? "null" : n.getClass().getSimpleName(); }
  static String run(ObjectMapper m, String s) {
    try { JsonNode n = m.readTree(s); return "OK " + cls(n) + " " + n + (n != null && n.isObject() ? types(n) : ""); }
    catch (Throwable e) { return "ERR " + e.getClass().getName() + ": " + e.getMessage(); }
  }
  static String types(JsonNode n) { StringBuilder sb = new StringBuilder(" T:"); n.fields().forEachRemaining(e -> sb.append(e.getKey()).append("=").append(cls(e.getValue())).append(",")); return sb.toString(); }
  public static void main(String[] a) throws Exception {
    ObjectMapper plain = new ObjectMapper();
    ObjectMapper y = io.swagger.v3.core.util.Yaml.mapper();
    java.lang.reflect.Method cy = io.swagger.v3.parser.ObjectMapperFactory.class.getDeclaredMethod("createYaml");
    ObjectMapper strict = (ObjectMapper) cy.invoke(null);
    for (String line : Files.readAllLines(Paths.get(a[0]))) {
      String s = plain.readValue(line, String.class);
      System.out.println(plain.writeValueAsString(run(y, s) + "\n" + run(strict, s)));
    }
  }
}
